//! Zero-copy transport port definitions

#[cfg(feature = "zerocopy")]
use async_trait::async_trait;

#[cfg(feature = "zerocopy")]
use bytes::Bytes;

use crate::domain::A2AError;

/// A trait for zero-copy message transport
///
/// This trait defines operations for sending and receiving messages
/// using zero-copy techniques to avoid unnecessary memory copies.
#[cfg(feature = "zerocopy")]
#[async_trait]
pub trait ZeroCopyTransport: Send + Sync {
    /// Send a message using zero-copy semantics
    ///
    /// The message payload is passed as `Bytes` which uses reference counting
    /// to avoid copying the underlying buffer.
    async fn send_zerocopy(&self, payload: Bytes) -> Result<(), A2AError>;

    /// Receive a message using zero-copy semantics
    ///
    /// Returns a `Bytes` object that references the received buffer without copying.
    async fn receive_zerocopy(&self) -> Result<Bytes, A2AError>;

    /// Send a large message using sendfile-style transfer
    ///
    /// For large payloads, this uses io_uring's zero-copy capabilities
    /// to transfer data directly from kernel space.
    async fn send_large_zerocopy(&self, payload: Bytes, threshold: usize) -> Result<(), A2AError> {
        if payload.len() > threshold {
            self.send_large_internal(payload).await
        } else {
            self.send_zerocopy(payload).await
        }
    }

    /// Internal method for large message transfer
    async fn send_large_internal(&self, payload: Bytes) -> Result<(), A2AError>;

    /// Get the underlying buffer pool statistics
    fn get_buffer_stats(&self) -> BufferStats;
}

/// Statistics about buffer usage
#[cfg(feature = "zerocopy")]
#[derive(Debug, Clone)]
pub struct BufferStats {
    /// Total bytes allocated
    pub total_allocated: usize,
    /// Total bytes in use
    pub total_in_use: usize,
    /// Number of active buffers
    pub active_buffers: usize,
    /// Number of buffer reuses (zero-copy successes)
    pub buffer_reuses: usize,
    /// Number of buffer copies (zero-copy failures)
    pub buffer_copies: usize,
}
