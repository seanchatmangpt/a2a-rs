//! Transport port trait for bidirectional streaming communication.
//!
//! The `Transport` port defines the interface for streaming typed packets
//! bidirectionally with support for ping/pong heartbeats and reconnection logic.

use crate::domain::TypedPacket;
use async_trait::async_trait;
use std::time::Duration;
use thiserror::Error;

/// Transport errors
#[derive(Debug, Clone, Error)]
pub enum TransportError {
    /// Connection failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Send failed
    #[error("Send failed: {0}")]
    SendFailed(String),

    /// Receive failed
    #[error("Receive failed: {0}")]
    ReceiveFailed(String),

    /// Connection closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// Timeout waiting for response
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Invalid message format
    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    /// Connection already established
    #[error("Already connected")]
    AlreadyConnected,

    /// Not connected
    #[error("Not connected")]
    NotConnected,

    /// Reconnection exhausted max retries
    #[error("Max reconnection retries exhausted")]
    MaxRetriesExhausted,
}

/// Transport configuration for connection and heartbeat settings
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Connection URL (e.g., ws://localhost:8080/ws)
    pub url: String,

    /// Ping interval - how often to send ping frames
    pub ping_interval: Duration,

    /// Pong timeout - how long to wait for pong response
    pub pong_timeout: Duration,

    /// Initial reconnection delay
    pub initial_reconnect_delay: Duration,

    /// Maximum reconnection delay
    pub max_reconnect_delay: Duration,

    /// Backoff multiplier (exponential backoff)
    pub reconnect_backoff: f64,

    /// Maximum number of reconnection attempts (None = unlimited)
    pub max_reconnect_attempts: Option<u32>,

    /// Message buffer size
    pub buffer_size: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            url: "ws://localhost:8080/ws".to_string(),
            ping_interval: Duration::from_secs(30),
            pong_timeout: Duration::from_secs(10),
            initial_reconnect_delay: Duration::from_millis(100),
            max_reconnect_delay: Duration::from_secs(30),
            reconnect_backoff: 2.0,
            max_reconnect_attempts: Some(10),
            buffer_size: 256,
        }
    }
}

impl TransportConfig {
    /// Create a new transport config with URL
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Default::default()
        }
    }

    /// Set ping interval
    #[must_use]
    pub fn with_ping_interval(mut self, interval: Duration) -> Self {
        self.ping_interval = interval;
        self
    }

    /// Set pong timeout
    #[must_use]
    pub fn with_pong_timeout(mut self, timeout: Duration) -> Self {
        self.pong_timeout = timeout;
        self
    }

    /// Set reconnection delays
    #[must_use]
    pub fn with_reconnect_config(mut self, initial: Duration, max: Duration, backoff: f64) -> Self {
        self.initial_reconnect_delay = initial;
        self.max_reconnect_delay = max;
        self.reconnect_backoff = backoff;
        self
    }

    /// Set max reconnection attempts
    #[must_use]
    pub fn with_max_reconnect_attempts(mut self, attempts: Option<u32>) -> Self {
        self.max_reconnect_attempts = attempts;
        self
    }

    /// Set buffer size
    #[must_use]
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }
}

/// Transport status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportStatus {
    /// Disconnected
    Disconnected,

    /// Connecting
    Connecting,

    /// Connected and healthy
    Connected,

    /// Degraded (reconnecting)
    Degraded,

    /// Permanently failed
    Failed,
}

/// Bidirectional transport for streaming typed packets
#[async_trait]
pub trait Transport: Send + Sync {
    /// Connect to the remote endpoint
    ///
    /// # Errors
    /// Returns error if connection fails
    async fn connect(&mut self) -> Result<(), TransportError>;

    /// Send a typed packet
    ///
    /// # Errors
    /// Returns error if send fails
    async fn send(&mut self, packet: TypedPacket) -> Result<(), TransportError>;

    /// Receive the next typed packet
    ///
    /// Returns `None` if connection is closed
    ///
    /// # Errors
    /// Returns error if receive fails
    async fn receive(&mut self) -> Result<Option<TypedPacket>, TransportError>;

    /// Get current connection status
    fn status(&self) -> TransportStatus;

    /// Check if connected
    fn is_connected(&self) -> bool {
        self.status() == TransportStatus::Connected
    }

    /// Disconnect gracefully
    ///
    /// # Errors
    /// Returns error if disconnect fails
    async fn disconnect(&mut self) -> Result<(), TransportError>;

    /// Reconnect with backoff logic
    ///
    /// Automatically handles reconnection with exponential backoff.
    ///
    /// # Errors
    /// Returns error if max retries exceeded
    async fn reconnect(&mut self) -> Result<(), TransportError>;

    /// Send a batch of packets
    ///
    /// # Errors
    /// Returns error if any send fails
    async fn send_batch(&mut self, packets: Vec<TypedPacket>) -> Result<(), TransportError> {
        for packet in packets {
            self.send(packet).await?;
        }
        Ok(())
    }

    /// Receive multiple packets with timeout
    ///
    /// Returns packets received within timeout period
    ///
    /// # Errors
    /// Returns error if receive fails (but not on timeout)
    async fn receive_batch(
        &mut self,
        timeout: Duration,
    ) -> Result<Vec<TypedPacket>, TransportError> {
        let start = std::time::Instant::now();
        let mut packets = Vec::new();

        while start.elapsed() < timeout {
            match tokio::time::timeout(timeout - start.elapsed(), self.receive()).await {
                Ok(Ok(Some(packet))) => packets.push(packet),
                Ok(Ok(None)) => break,
                Ok(Err(e)) => return Err(e),
                Err(_) => break, // Timeout
            }
        }

        Ok(packets)
    }
}
