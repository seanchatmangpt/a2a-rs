//! Receipt storage implementations.
//!
//! Provides adapters for storing receipts in various backends
//! (in-memory, Cloud Storage, etc.).

use crate::domain::{Receipt, ReceiptError};
use crate::port::ReceiptStorage;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// In-memory receipt storage for testing and development.
///
/// Stores receipts in memory. Not suitable for production use.
pub struct InMemoryReceiptStorage {
    receipts: Arc<RwLock<HashMap<Uuid, Receipt>>>,
}

impl InMemoryReceiptStorage {
    /// Creates a new in-memory storage.
    pub fn new() -> Self {
        Self {
            receipts: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryReceiptStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ReceiptStorage for InMemoryReceiptStorage {
    async fn store_receipt(&self, receipt: &Receipt) -> Result<String, ReceiptError> {
        let mut storage = self.receipts.write().await;
        let key = receipt.id.to_string();
        storage.insert(receipt.id, receipt.clone());
        Ok(key)
    }

    async fn get_receipt(&self, receipt_id: Uuid) -> Result<Receipt, ReceiptError> {
        let storage = self.receipts.read().await;
        storage
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| ReceiptError::InvalidFormat(format!("Receipt {} not found", receipt_id)))
    }

    async fn get_receipts_for_operation(
        &self,
        operation_id: Uuid,
    ) -> Result<Vec<Receipt>, ReceiptError> {
        let storage = self.receipts.read().await;
        let receipts: Vec<Receipt> = storage
            .values()
            .filter(|r| r.operation_id == operation_id)
            .cloned()
            .collect();
        Ok(receipts)
    }

    async fn list_receipts(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Receipt>, ReceiptError> {
        let storage = self.receipts.read().await;
        let receipts: Vec<Receipt> = storage
            .values()
            .filter(|r| r.timestamp >= start && r.timestamp <= end)
            .cloned()
            .collect();
        Ok(receipts)
    }
}

/// Cloud Storage configuration for receipt storage.
#[cfg(feature = "storage")]
#[derive(Debug, Clone)]
pub struct CloudStorageConfig {
    /// GCS bucket name
    pub bucket: String,

    /// Optional prefix for receipt objects
    pub prefix: Option<String>,

    /// Path to service account key file
    pub service_account_key: Option<String>,
}

/// Cloud Storage receipt storage implementation.
///
/// Stores receipts as JSON objects in Google Cloud Storage.
/// Each receipt is stored as: gs://{bucket}/{prefix}/receipts/{receipt_id}.json
#[cfg(feature = "storage")]
pub struct CloudStorageReceiptStorage {
    config: CloudStorageConfig,
    // In a real implementation, this would be a GCS client
    // For now, we'll keep it as a placeholder
}

#[cfg(feature = "storage")]
impl CloudStorageReceiptStorage {
    /// Creates a new Cloud Storage receipt storage.
    pub fn new(config: CloudStorageConfig) -> Self {
        Self { config }
    }

    /// Creates a Cloud Storage receipt storage from environment variables.
    ///
    /// Expected environment variables:
    /// - `RECEIPT_STORAGE_BUCKET`: GCS bucket name
    /// - `RECEIPT_STORAGE_PREFIX` (optional): Object prefix
    /// - `GOOGLE_APPLICATION_CREDENTIALS` (optional): Service account key path
    pub fn from_env() -> Result<Self, ReceiptError> {
        let config = CloudStorageConfig {
            bucket: std::env::var("RECEIPT_STORAGE_BUCKET").map_err(|_| {
                ReceiptError::InvalidFormat("Missing RECEIPT_STORAGE_BUCKET".to_string())
            })?,
            prefix: std::env::var("RECEIPT_STORAGE_PREFIX").ok(),
            service_account_key: std::env::var("GOOGLE_APPLICATION_CREDENTIALS").ok(),
        };

        Ok(Self::new(config))
    }

    /// Returns the GCS object path for a receipt.
    fn object_path(&self, receipt_id: Uuid) -> String {
        let base = format!("receipts/{}.json", receipt_id);
        if let Some(prefix) = &self.config.prefix {
            format!("{}/{}", prefix, base)
        } else {
            base
        }
    }
}

#[cfg(feature = "storage")]
#[async_trait]
impl ReceiptStorage for CloudStorageReceiptStorage {
    async fn store_receipt(&self, receipt: &Receipt) -> Result<String, ReceiptError> {
        // In a real implementation, this would:
        // 1. Serialize receipt to JSON
        // 2. Upload to GCS using the GCS client
        // 3. Return the gs:// URL

        let object_path = self.object_path(receipt.id);
        let _json = serde_json::to_string_pretty(receipt)
            .map_err(|e| ReceiptError::SerializationError(e.to_string()))?;

        // TODO: Implement actual GCS upload
        // For now, just return the path
        Ok(format!("gs://{}/{}", self.config.bucket, object_path))
    }

    async fn get_receipt(&self, receipt_id: Uuid) -> Result<Receipt, ReceiptError> {
        // In a real implementation, this would:
        // 1. Download object from GCS
        // 2. Deserialize JSON to Receipt
        // 3. Return the receipt

        let _object_path = self.object_path(receipt_id);

        // TODO: Implement actual GCS download
        Err(ReceiptError::InvalidFormat(
            "Cloud Storage not yet implemented".to_string(),
        ))
    }

    async fn get_receipts_for_operation(
        &self,
        _operation_id: Uuid,
    ) -> Result<Vec<Receipt>, ReceiptError> {
        // In a real implementation, this would:
        // 1. List objects with matching metadata (requires indexing)
        // 2. Download and deserialize each receipt
        // 3. Return the list

        // TODO: Implement with proper indexing (e.g., Firestore for metadata)
        Err(ReceiptError::InvalidFormat(
            "Cloud Storage querying not yet implemented".to_string(),
        ))
    }

    async fn list_receipts(
        &self,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<Receipt>, ReceiptError> {
        // In a real implementation, this would:
        // 1. List objects in time range (requires indexing)
        // 2. Download and deserialize each receipt
        // 3. Return the list

        // TODO: Implement with proper indexing
        Err(ReceiptError::InvalidFormat(
            "Cloud Storage querying not yet implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Operation, OperationKind, OperationResult};

    fn create_test_receipt() -> Receipt {
        let operation = Operation::new(
            OperationKind::Parse {
                input: "test".into(),
            },
            1,
        );

        Receipt {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation_id: operation.id,
            operation_hash: "abc123".to_string(),
            attestation_hash: "abc123".to_string(),
            signature: Some("sig".to_string()),
            replay_pointers: vec![],
            result: OperationResult::Success {
                output_hash: "def456".to_string(),
                output: None,
            },
            refusal: None,
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_in_memory_storage() {
        let storage = InMemoryReceiptStorage::new();
        let receipt = create_test_receipt();

        // Store receipt
        let key = storage.store_receipt(&receipt).await.unwrap();
        assert_eq!(key, receipt.id.to_string());

        // Retrieve receipt
        let retrieved = storage.get_receipt(receipt.id).await.unwrap();
        assert_eq!(retrieved.id, receipt.id);
        assert_eq!(retrieved.operation_id, receipt.operation_id);

        // Get receipts for operation
        let op_receipts = storage
            .get_receipts_for_operation(receipt.operation_id)
            .await
            .unwrap();
        assert_eq!(op_receipts.len(), 1);
        assert_eq!(op_receipts[0].id, receipt.id);
    }

    #[tokio::test]
    async fn test_in_memory_storage_time_range() {
        let storage = InMemoryReceiptStorage::new();
        let receipt = create_test_receipt();

        storage.store_receipt(&receipt).await.unwrap();

        // Query with time range that includes the receipt
        let start = receipt.timestamp - chrono::Duration::hours(1);
        let end = receipt.timestamp + chrono::Duration::hours(1);
        let receipts = storage.list_receipts(start, end).await.unwrap();
        assert_eq!(receipts.len(), 1);

        // Query with time range that excludes the receipt
        let start = receipt.timestamp + chrono::Duration::hours(1);
        let end = receipt.timestamp + chrono::Duration::hours(2);
        let receipts = storage.list_receipts(start, end).await.unwrap();
        assert_eq!(receipts.len(), 0);
    }

    #[cfg(feature = "storage")]
    #[test]
    fn test_cloud_storage_object_path() {
        let config = CloudStorageConfig {
            bucket: "my-bucket".to_string(),
            prefix: Some("prod".to_string()),
            service_account_key: None,
        };

        let storage = CloudStorageReceiptStorage::new(config);
        let receipt_id = Uuid::new_v4();
        let path = storage.object_path(receipt_id);

        assert_eq!(path, format!("prod/receipts/{}.json", receipt_id));
    }

    #[cfg(feature = "storage")]
    #[test]
    fn test_cloud_storage_object_path_no_prefix() {
        let config = CloudStorageConfig {
            bucket: "my-bucket".to_string(),
            prefix: None,
            service_account_key: None,
        };

        let storage = CloudStorageReceiptStorage::new(config);
        let receipt_id = Uuid::new_v4();
        let path = storage.object_path(receipt_id);

        assert_eq!(path, format!("receipts/{}.json", receipt_id));
    }
}
