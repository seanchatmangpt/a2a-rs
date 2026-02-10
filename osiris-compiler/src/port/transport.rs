//! Transport port for client-server communication.
//!
//! Defines the contract for transporting operations and receiving receipts
//! over various protocols (HTTP, gRPC, WebSocket, etc.).
//!
//! The Transport port supports:
//! - **Client streaming**: Send multiple operations to the server
//! - **Server streaming**: Receive multiple receipts from the server
//! - **Bidirectional streaming**: Full duplex communication with backpressure
//! - **Request-response**: Single operation → single receipt patterns

use async_trait::async_trait;
use futures::{SinkExt, Stream, StreamExt};
use std::pin::Pin;
use thiserror::Error;

use crate::domain::{Operation, Receipt};

/// Errors that can occur during transport operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum TransportError {
    /// Connection failed or was closed unexpectedly
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Failed to send an operation
    #[error("Send failed: {0}")]
    SendFailed(String),

    /// Failed to receive a receipt
    #[error("Receive failed: {0}")]
    ReceiveFailed(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Timeout occurred
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Invalid request or response format
    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    /// Server returned an error
    #[error("Server error: {0}")]
    ServerError(String),

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    /// Stream was closed
    #[error("Stream closed")]
    StreamClosed,

    /// Backpressure limit exceeded
    #[error("Backpressure limit exceeded: {0}")]
    BackpressureExceeded(String),
}

/// Result type for transport operations.
pub type TransportResult<T> = Result<T, TransportError>;

/// Stream of receipts produced by the server.
pub type ReceiptStream = Pin<Box<dyn Stream<Item = TransportResult<Receipt>> + Send>>;

/// Stream of operations sent by the client.
pub type OperationStream = Pin<Box<dyn Stream<Item = TransportResult<Operation>> + Send>>;

/// Response from a single operation.
#[derive(Debug, Clone)]
pub struct OperationResponse {
    /// The generated receipt
    pub receipt: Receipt,

    /// Time taken to process (milliseconds)
    pub processing_time_ms: u64,
}

/// Statistics for streaming operations.
#[derive(Debug, Clone)]
pub struct StreamStats {
    /// Total operations processed
    pub operations_sent: u64,

    /// Total receipts received
    pub receipts_received: u64,

    /// Total bytes transmitted
    pub bytes_sent: u64,

    /// Total bytes received
    pub bytes_received: u64,

    /// Average latency (milliseconds)
    pub avg_latency_ms: f64,

    /// Total duration (milliseconds)
    pub total_duration_ms: u64,
}

/// Transport port for sending operations and receiving receipts.
///
/// Supports multiple communication patterns:
/// - Request-response: Send one operation, get one receipt
/// - Client streaming: Send multiple operations, get multiple receipts
/// - Server streaming: Send trigger, get stream of receipts
/// - Bidirectional: Full duplex with independent streams
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a single operation and wait for the receipt.
    ///
    /// This is the simplest pattern: one operation → one receipt.
    /// Suitable for simple request-response interactions.
    ///
    /// # Arguments
    /// * `operation` - The operation to send
    ///
    /// # Returns
    /// * The receipt from the server
    async fn send_operation(&self, operation: Operation) -> TransportResult<Receipt>;

    /// Send multiple operations and get a stream of receipts.
    ///
    /// This pattern sends multiple operations and receives receipts
    /// in the same order. Useful for batch processing.
    ///
    /// # Arguments
    /// * `operations` - Stream of operations to send
    ///
    /// # Returns
    /// * Stream of receipts, one per operation (in order)
    async fn client_streaming(&self, operations: OperationStream)
        -> TransportResult<ReceiptStream>;

    /// Subscribe to a stream of receipts from the server.
    ///
    /// This pattern allows the server to push receipts to the client.
    /// Useful for consuming results asynchronously.
    ///
    /// # Arguments
    /// * `filter` - Optional filter to select which operations to watch
    ///
    /// # Returns
    /// * Stream of receipts matching the filter
    async fn server_streaming(&self, filter: Option<String>) -> TransportResult<ReceiptStream>;

    /// Full bidirectional streaming with independent operation/receipt streams.
    ///
    /// This allows simultaneous independent sending and receiving.
    /// Useful for decoupling producer and consumer rates.
    ///
    /// # Arguments
    /// * `operations` - Stream of operations to send
    ///
    /// # Returns
    /// * Stream of receipts (may arrive out of order)
    async fn bidirectional_streaming(
        &self,
        operations: OperationStream,
    ) -> TransportResult<ReceiptStream>;

    /// Check if the connection is still active.
    ///
    /// Returns true if the transport can be used, false if it's closed
    /// or unreachable.
    async fn is_connected(&self) -> bool;

    /// Close the connection gracefully.
    ///
    /// After calling this, all subsequent operations will fail.
    async fn close(&self) -> TransportResult<()>;

    /// Get statistics about the current session.
    fn get_stats(&self) -> StreamStats;

    /// Reset statistics (usually called after retrieving them).
    fn reset_stats(&self);

    /// Enable backpressure management with a maximum queue size.
    ///
    /// If the queue exceeds this size, new sends will fail with
    /// BackpressureExceeded error. Default is unlimited.
    fn set_backpressure_limit(&self, max_queue_size: usize);
}

/// Builder for configuring Transport implementations.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Server address (e.g., "localhost:50051")
    pub server_address: String,

    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,

    /// Keep-alive interval in seconds
    pub keepalive_interval_secs: u64,

    /// Maximum message size in bytes
    pub max_message_size_bytes: usize,

    /// Enable compression
    pub enable_compression: bool,

    /// Authentication token (optional)
    pub auth_token: Option<String>,

    /// Maximum backpressure queue size
    pub backpressure_limit: usize,

    /// Retry attempts for failed operations
    pub max_retries: u32,

    /// Delay between retries (milliseconds)
    pub retry_delay_ms: u64,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            server_address: "localhost:50051".to_string(),
            connection_timeout_secs: 30,
            keepalive_interval_secs: 10,
            max_message_size_bytes: 10 * 1024 * 1024, // 10 MB
            enable_compression: true,
            auth_token: None,
            backpressure_limit: 1000,
            max_retries: 3,
            retry_delay_ms: 100,
        }
    }
}

impl TransportConfig {
    /// Create a new transport configuration builder.
    pub fn builder() -> TransportConfigBuilder {
        TransportConfigBuilder::default()
    }
}

/// Builder for TransportConfig.
#[derive(Debug, Clone, Default)]
pub struct TransportConfigBuilder {
    server_address: Option<String>,
    connection_timeout_secs: Option<u64>,
    keepalive_interval_secs: Option<u64>,
    max_message_size_bytes: Option<usize>,
    enable_compression: Option<bool>,
    auth_token: Option<String>,
    backpressure_limit: Option<usize>,
    max_retries: Option<u32>,
    retry_delay_ms: Option<u64>,
}

impl TransportConfigBuilder {
    pub fn server_address(mut self, address: String) -> Self {
        self.server_address = Some(address);
        self
    }

    pub fn connection_timeout_secs(mut self, secs: u64) -> Self {
        self.connection_timeout_secs = Some(secs);
        self
    }

    pub fn keepalive_interval_secs(mut self, secs: u64) -> Self {
        self.keepalive_interval_secs = Some(secs);
        self
    }

    pub fn max_message_size_bytes(mut self, size: usize) -> Self {
        self.max_message_size_bytes = Some(size);
        self
    }

    pub fn enable_compression(mut self, enable: bool) -> Self {
        self.enable_compression = Some(enable);
        self
    }

    pub fn auth_token(mut self, token: String) -> Self {
        self.auth_token = Some(token);
        self
    }

    pub fn backpressure_limit(mut self, limit: usize) -> Self {
        self.backpressure_limit = Some(limit);
        self
    }

    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = Some(retries);
        self
    }

    pub fn retry_delay_ms(mut self, ms: u64) -> Self {
        self.retry_delay_ms = Some(ms);
        self
    }

    pub fn build(self) -> TransportConfig {
        TransportConfig {
            server_address: self
                .server_address
                .unwrap_or_else(|| "localhost:50051".to_string()),
            connection_timeout_secs: self.connection_timeout_secs.unwrap_or(30),
            keepalive_interval_secs: self.keepalive_interval_secs.unwrap_or(10),
            max_message_size_bytes: self.max_message_size_bytes.unwrap_or(10 * 1024 * 1024),
            enable_compression: self.enable_compression.unwrap_or(true),
            auth_token: self.auth_token,
            backpressure_limit: self.backpressure_limit.unwrap_or(1000),
            max_retries: self.max_retries.unwrap_or(3),
            retry_delay_ms: self.retry_delay_ms.unwrap_or(100),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_config_default() {
        let config = TransportConfig::default();
        assert_eq!(config.server_address, "localhost:50051");
        assert_eq!(config.connection_timeout_secs, 30);
        assert!(config.enable_compression);
    }

    #[test]
    fn test_transport_config_builder() {
        let config = TransportConfig::builder()
            .server_address("example.com:50052".to_string())
            .connection_timeout_secs(60)
            .enable_compression(false)
            .build();

        assert_eq!(config.server_address, "example.com:50052");
        assert_eq!(config.connection_timeout_secs, 60);
        assert!(!config.enable_compression);
    }

    #[test]
    fn test_stream_stats_default() {
        let stats = StreamStats {
            operations_sent: 100,
            receipts_received: 100,
            bytes_sent: 50000,
            bytes_received: 75000,
            avg_latency_ms: 25.5,
            total_duration_ms: 2550,
        };

        assert_eq!(stats.operations_sent, 100);
        assert_eq!(stats.avg_latency_ms, 25.5);
    }

    #[test]
    fn test_transport_error_display() {
        let err = TransportError::ConnectionError("Network unreachable".to_string());
        assert!(err.to_string().contains("Connection error"));

        let err = TransportError::StreamClosed;
        assert_eq!(err.to_string(), "Stream closed");
    }
}
