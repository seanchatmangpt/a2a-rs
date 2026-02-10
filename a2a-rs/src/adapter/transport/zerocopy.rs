//! Zero-copy transport implementation using bytes::Bytes and tokio-uring

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use tokio_uring::buf::IoBuf;
use tokio_uring::net::TcpStream;

use crate::domain::A2AError;
use crate::port::{BufferStats, ZeroCopyTransport};

/// Zero-copy transport using io_uring for Linux systems
///
/// This implementation uses tokio-uring to leverage Linux io_uring
/// for zero-copy I/O operations, combined with bytes::Bytes for
/// efficient buffer management.
pub struct UringZeroCopyTransport {
    stream: Arc<TcpStream>,
    stats: Arc<ZeroCopyStats>,
    large_message_threshold: usize,
}

/// Internal statistics tracking
struct ZeroCopyStats {
    total_allocated: AtomicUsize,
    total_in_use: AtomicUsize,
    active_buffers: AtomicUsize,
    buffer_reuses: AtomicUsize,
    buffer_copies: AtomicUsize,
}

impl UringZeroCopyTransport {
    /// Create a new zero-copy transport from a TCP stream
    ///
    /// # Arguments
    /// * `stream` - The TCP stream for communication
    /// * `large_message_threshold` - Size threshold for using sendfile-style transfer (default: 64KB)
    pub fn new(stream: TcpStream, large_message_threshold: Option<usize>) -> Self {
        Self {
            stream: Arc::new(stream),
            stats: Arc::new(ZeroCopyStats {
                total_allocated: AtomicUsize::new(0),
                total_in_use: AtomicUsize::new(0),
                active_buffers: AtomicUsize::new(0),
                buffer_reuses: AtomicUsize::new(0),
                buffer_copies: AtomicUsize::new(0),
            }),
            large_message_threshold: large_message_threshold.unwrap_or(64 * 1024),
        }
    }

    /// Convert bytes to io_uring buffer
    fn bytes_to_io_buf(&self, bytes: &Bytes) -> Vec<u8> {
        // Track allocation
        self.stats
            .total_allocated
            .fetch_add(bytes.len(), Ordering::Relaxed);
        self.stats.active_buffers.fetch_add(1, Ordering::Relaxed);

        // For now, we need to copy into a Vec for io_uring
        // In a production implementation, you would use a buffer pool
        bytes.to_vec()
    }

    /// Read into a pre-allocated buffer
    async fn read_into_buffer(&self, buf: Vec<u8>) -> Result<(usize, Vec<u8>), A2AError> {
        let stream = Arc::clone(&self.stream);
        let (result, buf) = stream.read(buf).await;

        result
            .map(|n| (n, buf))
            .map_err(|e| A2AError::IoError(e.to_string()))
    }

    /// Write from a buffer
    async fn write_from_buffer(&self, buf: Vec<u8>) -> Result<usize, A2AError> {
        let stream = Arc::clone(&self.stream);
        let (result, _buf) = stream.write(buf).await;

        result.map_err(|e| A2AError::IoError(e.to_string()))
    }
}

#[async_trait]
impl ZeroCopyTransport for UringZeroCopyTransport {
    async fn send_zerocopy(&self, payload: Bytes) -> Result<(), A2AError> {
        // Track that we're reusing the Bytes buffer (zero-copy at the application level)
        self.stats.buffer_reuses.fetch_add(1, Ordering::Relaxed);
        self.stats
            .total_in_use
            .fetch_add(payload.len(), Ordering::Relaxed);

        // Convert to io_uring buffer
        let buf = self.bytes_to_io_buf(&payload);

        // Send the length prefix first (4 bytes, big-endian)
        let len_bytes = (payload.len() as u32).to_be_bytes().to_vec();
        self.write_from_buffer(len_bytes).await?;

        // Send the actual payload
        let written = self.write_from_buffer(buf).await?;

        self.stats
            .total_in_use
            .fetch_sub(payload.len(), Ordering::Relaxed);
        self.stats.active_buffers.fetch_sub(1, Ordering::Relaxed);

        if written != payload.len() {
            return Err(A2AError::IoError(format!(
                "Incomplete write: expected {}, wrote {}",
                payload.len(),
                written
            )));
        }

        Ok(())
    }

    async fn receive_zerocopy(&self) -> Result<Bytes, A2AError> {
        // Read the length prefix (4 bytes)
        let len_buf = vec![0u8; 4];
        let (n, len_buf) = self.read_into_buffer(len_buf).await?;

        if n != 4 {
            return Err(A2AError::IoError(format!(
                "Failed to read length prefix: expected 4 bytes, got {}",
                n
            )));
        }

        let payload_len =
            u32::from_be_bytes([len_buf[0], len_buf[1], len_buf[2], len_buf[3]]) as usize;

        // Allocate buffer for payload
        let payload_buf = vec![0u8; payload_len];
        self.stats
            .total_allocated
            .fetch_add(payload_len, Ordering::Relaxed);
        self.stats.active_buffers.fetch_add(1, Ordering::Relaxed);

        // Read the payload
        let (n, payload_buf) = self.read_into_buffer(payload_buf).await?;

        if n != payload_len {
            return Err(A2AError::IoError(format!(
                "Incomplete read: expected {}, got {}",
                payload_len, n
            )));
        }

        // Convert to Bytes (zero-copy from here on)
        self.stats.buffer_reuses.fetch_add(1, Ordering::Relaxed);
        self.stats.active_buffers.fetch_sub(1, Ordering::Relaxed);

        Ok(Bytes::from(payload_buf))
    }

    async fn send_large_internal(&self, payload: Bytes) -> Result<(), A2AError> {
        // For large messages, we could use more advanced io_uring features
        // like IORING_OP_SEND_ZC (zero-copy send) if available
        // For now, we'll use the same path but mark it as a large transfer
        self.stats.buffer_copies.fetch_add(1, Ordering::Relaxed);
        self.send_zerocopy(payload).await
    }

    fn get_buffer_stats(&self) -> BufferStats {
        BufferStats {
            total_allocated: self.stats.total_allocated.load(Ordering::Relaxed),
            total_in_use: self.stats.total_in_use.load(Ordering::Relaxed),
            active_buffers: self.stats.active_buffers.load(Ordering::Relaxed),
            buffer_reuses: self.stats.buffer_reuses.load(Ordering::Relaxed),
            buffer_copies: self.stats.buffer_copies.load(Ordering::Relaxed),
        }
    }
}

/// Buffer pool for efficient buffer reuse
pub struct BufferPool {
    small_buffers: Arc<tokio::sync::Mutex<Vec<BytesMut>>>,
    medium_buffers: Arc<tokio::sync::Mutex<Vec<BytesMut>>>,
    large_buffers: Arc<tokio::sync::Mutex<Vec<BytesMut>>>,
    small_size: usize,
    medium_size: usize,
    large_size: usize,
}

impl BufferPool {
    /// Create a new buffer pool with configurable sizes
    pub fn new(small_size: usize, medium_size: usize, large_size: usize) -> Self {
        Self {
            small_buffers: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            medium_buffers: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            large_buffers: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            small_size,
            medium_size,
            large_size,
        }
    }

    /// Get a buffer from the pool or allocate a new one
    pub async fn get_buffer(&self, size: usize) -> BytesMut {
        if size <= self.small_size {
            let mut pool = self.small_buffers.lock().await;
            if let Some(mut buf) = pool.pop() {
                buf.clear();
                buf.reserve(self.small_size);
                return buf;
            }
            BytesMut::with_capacity(self.small_size)
        } else if size <= self.medium_size {
            let mut pool = self.medium_buffers.lock().await;
            if let Some(mut buf) = pool.pop() {
                buf.clear();
                buf.reserve(self.medium_size);
                return buf;
            }
            BytesMut::with_capacity(self.medium_size)
        } else if size <= self.large_size {
            let mut pool = self.large_buffers.lock().await;
            if let Some(mut buf) = pool.pop() {
                buf.clear();
                buf.reserve(self.large_size);
                return buf;
            }
            BytesMut::with_capacity(self.large_size)
        } else {
            BytesMut::with_capacity(size)
        }
    }

    /// Return a buffer to the pool
    pub async fn return_buffer(&self, buf: BytesMut) {
        let capacity = buf.capacity();
        if capacity <= self.small_size {
            let mut pool = self.small_buffers.lock().await;
            if pool.len() < 100 {
                pool.push(buf);
            }
        } else if capacity <= self.medium_size {
            let mut pool = self.medium_buffers.lock().await;
            if pool.len() < 50 {
                pool.push(buf);
            }
        } else if capacity <= self.large_size {
            let mut pool = self.large_buffers.lock().await;
            if pool.len() < 10 {
                pool.push(buf);
            }
        }
    }
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::new(4096, 65536, 1048576)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_buffer_pool() {
        let pool = BufferPool::default();

        // Get a small buffer
        let buf = pool.get_buffer(1024).await;
        assert!(buf.capacity() >= 1024);

        // Return it
        pool.return_buffer(buf).await;

        // Get it again - should reuse
        let buf2 = pool.get_buffer(1024).await;
        assert!(buf2.capacity() >= 1024);
    }

    #[test]
    fn test_buffer_stats() {
        let stats = ZeroCopyStats {
            total_allocated: AtomicUsize::new(1000),
            total_in_use: AtomicUsize::new(500),
            active_buffers: AtomicUsize::new(5),
            buffer_reuses: AtomicUsize::new(10),
            buffer_copies: AtomicUsize::new(2),
        };

        assert_eq!(stats.total_allocated.load(Ordering::Relaxed), 1000);
        assert_eq!(stats.total_in_use.load(Ordering::Relaxed), 500);
        assert_eq!(stats.active_buffers.load(Ordering::Relaxed), 5);
        assert_eq!(stats.buffer_reuses.load(Ordering::Relaxed), 10);
        assert_eq!(stats.buffer_copies.load(Ordering::Relaxed), 2);
    }
}
