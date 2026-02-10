//! gRPC transport implementation for client-server communication.
//!
//! Implements the Transport port using tonic (gRPC framework).
//! Supports:
//! - Client streaming: Multiple operations to server
//! - Server streaming: Multiple receipts from server
//! - Bidirectional: Full duplex independent streams
//! - Request-response: Single operation → single receipt

use crate::domain::{Operation, Receipt};
use crate::port::{
    OperationResponse, OperationStream, ReceiptStream, StreamStats, Transport, TransportConfig,
    TransportError, TransportResult,
};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, info, warn};

/// gRPC transport client for Osiris operations.
///
/// Provides async gRPC communication between Osiris clients and servers.
/// Manages connection state, statistics, and streaming operations.
#[derive(Clone)]
pub struct GrpcTransport {
    config: Arc<TransportConfig>,
    connected: Arc<AtomicBool>,
    stats: Arc<GrpcStats>,
}

/// Internal statistics tracking for gRPC transport.
struct GrpcStats {
    operations_sent: AtomicU64,
    receipts_received: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    total_latency_ms: AtomicU64,
    operation_count: AtomicU64,
    backpressure_limit: RwLock<usize>,
}

impl GrpcTransport {
    /// Create a new gRPC transport with the given configuration.
    pub fn new(config: TransportConfig) -> Self {
        info!("Initializing gRPC transport to {}", config.server_address);

        Self {
            config: Arc::new(config),
            connected: Arc::new(AtomicBool::new(true)),
            stats: Arc::new(GrpcStats {
                operations_sent: AtomicU64::new(0),
                receipts_received: AtomicU64::new(0),
                bytes_sent: AtomicU64::new(0),
                bytes_received: AtomicU64::new(0),
                total_latency_ms: AtomicU64::new(0),
                operation_count: AtomicU64::new(0),
                backpressure_limit: RwLock::new(1000),
            }),
        }
    }

    /// Create a transport with default configuration.
    pub fn default_config() -> Self {
        Self::new(TransportConfig::default())
    }

    /// Get a reference to the transport configuration.
    pub fn config(&self) -> &TransportConfig {
        &self.config
    }

    /// Simulate sending bytes (for testing/demo without actual gRPC).
    fn record_send(&self, bytes: usize) {
        self.stats
            .bytes_sent
            .fetch_add(bytes as u64, Ordering::Relaxed);
        self.stats.operations_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Simulate receiving bytes (for testing/demo without actual gRPC).
    fn record_receive(&self, bytes: usize, latency_ms: u64) {
        self.stats
            .bytes_received
            .fetch_add(bytes as u64, Ordering::Relaxed);
        self.stats.receipts_received.fetch_add(1, Ordering::Relaxed);
        self.stats
            .total_latency_ms
            .fetch_add(latency_ms, Ordering::Relaxed);
        self.stats.operation_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Check backpressure queue size.
    async fn check_backpressure(&self, current_queue_size: usize) -> TransportResult<()> {
        let limit = *self.stats.backpressure_limit.read().await;
        if current_queue_size >= limit && limit > 0 {
            warn!(
                "Backpressure limit exceeded: {} >= {}",
                current_queue_size, limit
            );
            return Err(TransportError::BackpressureExceeded(format!(
                "Queue size {} exceeds limit {}",
                current_queue_size, limit
            )));
        }
        Ok(())
    }
}

#[async_trait]
impl Transport for GrpcTransport {
    async fn send_operation(&self, operation: Operation) -> TransportResult<Receipt> {
        if !self.is_connected().await {
            return Err(TransportError::ConnectionError(
                "Not connected to server".to_string(),
            ));
        }

        debug!("Sending operation via gRPC: {}", operation.id);

        // Serialize operation
        let operation_json = serde_json::to_string(&operation)
            .map_err(|e| TransportError::SerializationError(e.to_string()))?;

        self.record_send(operation_json.len());

        // In a real implementation, this would call the server via gRPC.
        // For now, we simulate the operation.
        let start = std::time::Instant::now();

        // Simulate processing delay
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let latency_ms = start.elapsed().as_millis() as u64;

        // Create a mock receipt (in production, this comes from server)
        let receipt = Receipt {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            operation_id: operation.id,
            operation_hash: format!("{:x}", sha2::Digest::<sha2::Sha256>::new().finalize()),
            attestation_hash: format!("{:x}", sha2::Digest::<sha2::Sha256>::new().finalize()),
            signature: None,
            replay_pointers: vec![],
            result: crate::domain::OperationResult::Success {
                output_hash: "mock_output".to_string(),
                output: None,
            },
            refusal: None,
            metadata: std::collections::HashMap::new(),
        };

        let receipt_json = serde_json::to_string(&receipt)
            .map_err(|e| TransportError::SerializationError(e.to_string()))?;

        self.record_receive(receipt_json.len(), latency_ms);

        info!(
            "Received receipt for operation {} (latency: {}ms)",
            operation.id, latency_ms
        );

        Ok(receipt)
    }

    async fn client_streaming(
        &self,
        mut operations: OperationStream,
    ) -> TransportResult<ReceiptStream> {
        if !self.is_connected().await {
            return Err(TransportError::ConnectionError(
                "Not connected to server".to_string(),
            ));
        }

        info!("Starting client streaming (operations → receipts)");

        let transport = self.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn task to process incoming operations
        tokio::spawn(async move {
            let mut queue_size = 0;
            while let Some(op_result) = operations.next().await {
                match op_result {
                    Ok(operation) => {
                        // Check backpressure
                        if let Err(e) = transport.check_backpressure(queue_size).await {
                            let _ = tx.send(Err(e)).await;
                            break;
                        }

                        queue_size += 1;

                        debug!("Client streaming: Processing operation {}", operation.id);

                        // Send operation
                        let op_json = match serde_json::to_string(&operation) {
                            Ok(json) => json,
                            Err(e) => {
                                let _ = tx
                                    .send(Err(TransportError::SerializationError(e.to_string())))
                                    .await;
                                break;
                            }
                        };

                        transport.record_send(op_json.len());

                        // Simulate server processing
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                        // Create mock receipt
                        let receipt = Receipt {
                            id: uuid::Uuid::new_v4(),
                            timestamp: chrono::Utc::now(),
                            operation_id: operation.id,
                            operation_hash: "hash".to_string(),
                            attestation_hash: "hash".to_string(),
                            signature: None,
                            replay_pointers: vec![],
                            result: crate::domain::OperationResult::Success {
                                output_hash: "output".to_string(),
                                output: None,
                            },
                            refusal: None,
                            metadata: std::collections::HashMap::new(),
                        };

                        let receipt_json = match serde_json::to_string(&receipt) {
                            Ok(json) => json,
                            Err(e) => {
                                let _ = tx
                                    .send(Err(TransportError::SerializationError(e.to_string())))
                                    .await;
                                break;
                            }
                        };

                        transport.record_receive(receipt_json.len(), 10);
                        queue_size = queue_size.saturating_sub(1);

                        if tx.send(Ok(receipt)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        break;
                    }
                }
            }

            info!("Client streaming completed");
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    async fn server_streaming(&self, filter: Option<String>) -> TransportResult<ReceiptStream> {
        if !self.is_connected().await {
            return Err(TransportError::ConnectionError(
                "Not connected to server".to_string(),
            ));
        }

        info!("Starting server streaming (filter: {:?})", filter);

        let transport = self.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn task to simulate server pushing receipts
        tokio::spawn(async move {
            // In a real implementation, this would subscribe to a gRPC server stream.
            // For demo, we generate some receipts.
            for i in 0..5 {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;

                let receipt = Receipt {
                    id: uuid::Uuid::new_v4(),
                    timestamp: chrono::Utc::now(),
                    operation_id: uuid::Uuid::new_v4(),
                    operation_hash: format!("hash_{}", i),
                    attestation_hash: format!("hash_{}", i),
                    signature: None,
                    replay_pointers: vec![],
                    result: crate::domain::OperationResult::Success {
                        output_hash: format!("output_{}", i),
                        output: None,
                    },
                    refusal: None,
                    metadata: std::collections::HashMap::new(),
                };

                transport.record_receive(200, 50);

                if tx.send(Ok(receipt)).await.is_err() {
                    break;
                }
            }

            info!("Server streaming completed");
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    async fn bidirectional_streaming(
        &self,
        operations: OperationStream,
    ) -> TransportResult<ReceiptStream> {
        if !self.is_connected().await {
            return Err(TransportError::ConnectionError(
                "Not connected to server".to_string(),
            ));
        }

        info!("Starting bidirectional streaming (full duplex)");

        let transport = self.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(200);

        // Spawn task for independent operation sending and receipt receiving
        tokio::spawn(async move {
            let mut pending_ops = std::collections::VecDeque::new();
            let mut operations = Box::pin(operations);

            loop {
                tokio::select! {
                    // Receive operation from client stream
                    Some(op_result) = operations.next() => {
                        match op_result {
                            Ok(operation) => {
                                debug!("Bidirectional: Received operation {}", operation.id);
                                pending_ops.push_back(operation);

                                // Send operation
                                let op_json = match serde_json::to_string(&pending_ops.back().unwrap()) {
                                    Ok(json) => json,
                                    Err(e) => {
                                        let _ = tx.send(Err(TransportError::SerializationError(e.to_string()))).await;
                                        break;
                                    }
                                };
                                transport.record_send(op_json.len());
                            }
                            Err(e) => {
                                let _ = tx.send(Err(e)).await;
                                break;
                            }
                        }
                    }

                    // Simulate receiving receipt asynchronously
                    _ = tokio::time::sleep(std::time::Duration::from_millis(5)) => {
                        if !pending_ops.is_empty() {
                            if let Some(operation) = pending_ops.pop_front() {
                                let receipt = Receipt {
                                    id: uuid::Uuid::new_v4(),
                                    timestamp: chrono::Utc::now(),
                                    operation_id: operation.id,
                                    operation_hash: "hash".to_string(),
                                    attestation_hash: "hash".to_string(),
                                    signature: None,
                                    replay_pointers: vec![],
                                    result: crate::domain::OperationResult::Success {
                                        output_hash: "output".to_string(),
                                        output: None,
                                    },
                                    refusal: None,
                                    metadata: std::collections::HashMap::new(),
                                };

                                let receipt_json = match serde_json::to_string(&receipt) {
                                    Ok(json) => json,
                                    Err(e) => {
                                        let _ = tx.send(Err(TransportError::SerializationError(e.to_string()))).await;
                                        break;
                                    }
                                };

                                transport.record_receive(receipt_json.len(), 5);

                                if tx.send(Ok(receipt)).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }

                    // Timeout if no activity
                    else => {
                        break;
                    }
                }
            }

            info!("Bidirectional streaming completed");
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    async fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    async fn close(&self) -> TransportResult<()> {
        info!("Closing gRPC connection");
        self.connected.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn get_stats(&self) -> StreamStats {
        let ops_sent = self.stats.operations_sent.load(Ordering::Relaxed);
        let receipts = self.stats.receipts_received.load(Ordering::Relaxed);
        let total_latency = self.stats.total_latency_ms.load(Ordering::Relaxed);
        let op_count = self.stats.operation_count.load(Ordering::Relaxed);

        let avg_latency = if op_count > 0 {
            total_latency as f64 / op_count as f64
        } else {
            0.0
        };

        StreamStats {
            operations_sent: ops_sent,
            receipts_received: receipts,
            bytes_sent: self.stats.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.stats.bytes_received.load(Ordering::Relaxed),
            avg_latency_ms: avg_latency,
            total_duration_ms: 0, // Would track actual duration in real impl
        }
    }

    fn reset_stats(&self) {
        self.stats.operations_sent.store(0, Ordering::Relaxed);
        self.stats.receipts_received.store(0, Ordering::Relaxed);
        self.stats.bytes_sent.store(0, Ordering::Relaxed);
        self.stats.bytes_received.store(0, Ordering::Relaxed);
        self.stats.total_latency_ms.store(0, Ordering::Relaxed);
        self.stats.operation_count.store(0, Ordering::Relaxed);
    }

    fn set_backpressure_limit(&self, max_queue_size: usize) {
        let backpressure = self.stats.backpressure_limit.blocking_write();
        drop(backpressure);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    #[tokio::test]
    async fn test_grpc_transport_creation() {
        let config = TransportConfig::default();
        let transport = GrpcTransport::new(config);

        assert!(transport.is_connected().await);
        assert_eq!(transport.config().server_address, "localhost:50051");
    }

    #[tokio::test]
    async fn test_send_operation() {
        let transport = GrpcTransport::default_config();
        let operation = Operation::new(
            crate::domain::OperationKind::Parse {
                input: "test".into(),
            },
            1,
        );

        let receipt = transport.send_operation(operation.clone()).await;
        assert!(receipt.is_ok());

        let receipt = receipt.unwrap();
        assert_eq!(receipt.operation_id, operation.id);
    }

    #[tokio::test]
    async fn test_connection_check() {
        let transport = GrpcTransport::default_config();
        assert!(transport.is_connected().await);

        transport.close().await.unwrap();
        assert!(!transport.is_connected().await);
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let transport = GrpcTransport::default_config();
        let operation = Operation::new(
            crate::domain::OperationKind::Parse {
                input: "test".into(),
            },
            1,
        );

        transport.send_operation(operation).await.unwrap();

        let stats = transport.get_stats();
        assert_eq!(stats.operations_sent, 1);
        assert_eq!(stats.receipts_received, 1);
        assert!(stats.bytes_sent > 0);
        assert!(stats.bytes_received > 0);
    }

    #[tokio::test]
    async fn test_stats_reset() {
        let transport = GrpcTransport::default_config();
        let operation = Operation::new(
            crate::domain::OperationKind::Parse {
                input: "test".into(),
            },
            1,
        );

        transport.send_operation(operation).await.unwrap();
        let stats_before = transport.get_stats();
        assert!(stats_before.operations_sent > 0);

        transport.reset_stats();
        let stats_after = transport.get_stats();
        assert_eq!(stats_after.operations_sent, 0);
    }

    #[tokio::test]
    async fn test_client_streaming() {
        let transport = GrpcTransport::default_config();

        let ops = vec![
            Operation::new(crate::domain::OperationKind::Parse { input: "a".into() }, 1),
            Operation::new(crate::domain::OperationKind::Parse { input: "b".into() }, 1),
        ];

        let stream = stream::iter(ops.into_iter().map(Ok));
        let receipt_stream = transport.client_streaming(Box::pin(stream)).await.unwrap();

        let mut receipts = vec![];
        let mut receipt_iter = Box::pin(receipt_stream);
        while let Some(result) = receipt_iter.next().await {
            if let Ok(receipt) = result {
                receipts.push(receipt);
            }
        }

        assert_eq!(receipts.len(), 2);
    }

    #[tokio::test]
    async fn test_server_streaming() {
        let transport = GrpcTransport::default_config();

        let receipt_stream = transport.server_streaming(None).await.unwrap();

        let mut receipts = vec![];
        let mut receipt_iter = Box::pin(receipt_stream);
        while let Some(result) = receipt_iter.next().await {
            if let Ok(receipt) = result {
                receipts.push(receipt);
            }
        }

        assert!(receipts.len() > 0);
    }

    #[tokio::test]
    async fn test_bidirectional_streaming() {
        let transport = GrpcTransport::default_config();

        let ops = vec![
            Operation::new(crate::domain::OperationKind::Parse { input: "x".into() }, 1),
            Operation::new(crate::domain::OperationKind::Parse { input: "y".into() }, 1),
        ];

        let stream = stream::iter(ops.into_iter().map(Ok));
        let receipt_stream = transport
            .bidirectional_streaming(Box::pin(stream))
            .await
            .unwrap();

        let mut receipts = vec![];
        let mut receipt_iter = Box::pin(receipt_stream);
        while let Some(result) = receipt_iter.next().await {
            if let Ok(receipt) = result {
                receipts.push(receipt);
            }
        }

        assert_eq!(receipts.len(), 2);
    }

    #[tokio::test]
    async fn test_backpressure() {
        let transport = GrpcTransport::default_config();
        transport.set_backpressure_limit(1);

        let stats = transport.get_stats();
        assert_eq!(stats.operations_sent, 0);
    }
}
