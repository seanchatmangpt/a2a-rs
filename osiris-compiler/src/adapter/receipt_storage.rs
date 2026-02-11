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
///
/// Query operations (get_receipts_for_operation, list_receipts) use an
/// in-memory index that must be kept in sync with the storage layer.
/// For production use, consider using Firestore for metadata indexing.
///
/// # Note
/// The `storage` feature provides a simulated GCS implementation for testing.
/// For actual GCS integration, use the `gcs` feature with `GcsReceiptStorage`
/// from the `gcs_receipt_storage` module.
#[cfg(feature = "storage")]
pub struct CloudStorageReceiptStorage {
    config: CloudStorageConfig,
    /// In-memory storage simulating GCS
    receipts: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    /// In-memory index for querying receipts by operation_id and timestamp
    /// Maps (operation_id) -> Vec<receipt_id>
    operation_index: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>,
    /// Maps (receipt_id) -> (timestamp, operation_id)
    /// for time-range queries
    timestamp_index: Arc<RwLock<HashMap<Uuid, (DateTime<Utc>, Uuid)>>>,
}

#[cfg(feature = "storage")]
impl CloudStorageReceiptStorage {
    /// Creates a new Cloud Storage receipt storage.
    ///
    /// # Arguments
    /// * `config` - Cloud Storage configuration
    ///
    /// # Returns
    /// A new CloudStorageReceiptStorage instance
    ///
    /// # Example
    /// ```no_run
    /// use osiris_compiler::adapter::receipt_storage::{CloudStorageReceiptStorage, CloudStorageConfig};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = CloudStorageConfig {
    ///     bucket: "my-bucket".to_string(),
    ///     prefix: Some("receipts".to_string()),
    ///     service_account_key: Some("/path/to/key.json".to_string()),
    /// };
    /// let storage = CloudStorageReceiptStorage::new(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(config: CloudStorageConfig) -> Result<Self, ReceiptError> {
        Ok(Self {
            config,
            receipts: Arc::new(RwLock::new(HashMap::new())),
            operation_index: Arc::new(RwLock::new(HashMap::new())),
            timestamp_index: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Creates a Cloud Storage receipt storage from environment variables.
    ///
    /// Expected environment variables:
    /// - `RECEIPT_STORAGE_BUCKET`: GCS bucket name
    /// - `RECEIPT_STORAGE_PREFIX` (optional): Object prefix
    /// - `GOOGLE_APPLICATION_CREDENTIALS` (optional): Service account key path
    ///
    /// # Example
    /// ```no_run
    /// use osiris_compiler::adapter::receipt_storage::CloudStorageReceiptStorage;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// std::env::set_var("RECEIPT_STORAGE_BUCKET", "my-bucket");
    /// std::env::set_var("RECEIPT_STORAGE_PREFIX", "receipts");
    /// let storage = CloudStorageReceiptStorage::from_env().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn from_env() -> Result<Self, ReceiptError> {
        let config = CloudStorageConfig {
            bucket: std::env::var("RECEIPT_STORAGE_BUCKET").map_err(|_| {
                ReceiptError::InvalidFormat("Missing RECEIPT_STORAGE_BUCKET".to_string())
            })?,
            prefix: std::env::var("RECEIPT_STORAGE_PREFIX").ok(),
            service_account_key: std::env::var("GOOGLE_APPLICATION_CREDENTIALS").ok(),
        };

        Self::new(config).await
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

    /// Uploads data to GCS.
    ///
    /// # Arguments
    /// * `object_path` - The object path within the bucket
    /// * `data` - The data to upload (JSON bytes)
    ///
    /// # Returns
    /// Ok(()) if successful
    ///
    /// # Note
    /// This is a simulated implementation for testing. For actual GCS integration,
    /// use the `gcs` feature with `GcsReceiptStorage` from `gcs_receipt_storage`.
    async fn upload_to_gcs(&self, object_path: &str, data: &[u8]) -> Result<(), ReceiptError> {
        // Simulate GCS upload by storing in memory
        let mut storage = self.receipts.write().await;
        storage.insert(object_path.to_string(), data.to_vec());

        tracing::debug!(
            "Simulated GCS upload: gs://{}/{} ({} bytes)",
            self.config.bucket,
            object_path,
            data.len()
        );

        Ok(())
    }

    /// Downloads data from GCS.
    ///
    /// # Arguments
    /// * `object_path` - The object path within the bucket
    ///
    /// # Returns
    /// The downloaded data (JSON bytes)
    ///
    /// # Note
    /// This is a simulated implementation for testing. For actual GCS integration,
    /// use the `gcs` feature with `GcsReceiptStorage` from `gcs_receipt_storage`.
    async fn download_from_gcs(&self, object_path: &str) -> Result<Vec<u8>, ReceiptError> {
        // Simulate GCS download by reading from memory
        let storage = self.receipts.read().await;
        storage.get(object_path).cloned().ok_or_else(|| {
            ReceiptError::InvalidFormat(format!(
                "Receipt not found at gs://{}/{}",
                self.config.bucket, object_path
            ))
        })
    }
}

#[cfg(feature = "storage")]
#[async_trait]
#[cfg(feature = "storage")]
#[async_trait]
impl ReceiptStorage for CloudStorageReceiptStorage {
    async fn store_receipt(&self, receipt: &Receipt) -> Result<String, ReceiptError> {
        let object_path = self.object_path(receipt.id);

        // Serialize receipt to JSON
        let json_bytes = serde_json::to_vec_pretty(receipt)
            .map_err(|e| ReceiptError::SerializationError(e.to_string()))?;

        // Upload to GCS
        self.upload_to_gcs(&object_path, &json_bytes).await?;

        // Update indexes for querying
        {
            let mut op_index = self.operation_index.write().await;
            op_index
                .entry(receipt.operation_id)
                .or_insert_with(Vec::new)
                .push(receipt.id);
        }

        {
            let mut ts_index = self.timestamp_index.write().await;
            ts_index.insert(receipt.id, (receipt.timestamp, receipt.operation_id));
        }

        // Return the gs:// URL
        Ok(format!("gs://{}/{}", self.config.bucket, object_path))
    }

    async fn get_receipt(&self, receipt_id: Uuid) -> Result<Receipt, ReceiptError> {
        let object_path = self.object_path(receipt_id);

        // Download from GCS
        let json_bytes = self.download_from_gcs(&object_path).await?;

        // Deserialize JSON to Receipt
        let receipt: Receipt = serde_json::from_slice(&json_bytes).map_err(|e| {
            ReceiptError::SerializationError(format!("Failed to deserialize receipt: {}", e))
        })?;

        Ok(receipt)
    }

    async fn get_receipts_for_operation(
        &self,
        operation_id: Uuid,
    ) -> Result<Vec<Receipt>, ReceiptError> {
        // Query the in-memory index for receipt IDs
        let receipt_ids = {
            let op_index = self.operation_index.read().await;
            op_index.get(&operation_id).cloned().unwrap_or_default()
        };

        // Download each receipt
        let mut receipts = Vec::new();
        for receipt_id in receipt_ids {
            match self.get_receipt(receipt_id).await {
                Ok(receipt) => receipts.push(receipt),
                Err(e) => {
                    tracing::warn!("Failed to fetch receipt {}: {}", receipt_id, e);
                    // Continue fetching other receipts even if one fails
                }
            }
        }

        Ok(receipts)
    }

    async fn list_receipts(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Receipt>, ReceiptError> {
        // Query the in-memory timestamp index for receipt IDs in range
        let receipt_ids = {
            let ts_index = self.timestamp_index.read().await;
            ts_index
                .iter()
                .filter(|(_, (timestamp, _))| *timestamp >= start && *timestamp <= end)
                .map(|(receipt_id, _)| *receipt_id)
                .collect::<Vec<_>>()
        };

        // Download each receipt
        let mut receipts = Vec::new();
        for receipt_id in receipt_ids {
            match self.get_receipt(receipt_id).await {
                Ok(receipt) => receipts.push(receipt),
                Err(e) => {
                    tracing::warn!("Failed to fetch receipt {}: {}", receipt_id, e);
                    // Continue fetching other receipts even if one fails
                }
            }
        }

        // Sort by timestamp
        receipts.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(receipts)
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

        // Create a minimal storage instance just for testing object_path
        let storage = CloudStorageReceiptStorage {
            config,
            receipts: Arc::new(RwLock::new(HashMap::new())),
            operation_index: Arc::new(RwLock::new(HashMap::new())),
            timestamp_index: Arc::new(RwLock::new(HashMap::new())),
        };

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

        // Create a minimal storage instance just for testing object_path
        let storage = CloudStorageReceiptStorage {
            config,
            receipts: Arc::new(RwLock::new(HashMap::new())),
            operation_index: Arc::new(RwLock::new(HashMap::new())),
            timestamp_index: Arc::new(RwLock::new(HashMap::new())),
        };

        let receipt_id = Uuid::new_v4();
        let path = storage.object_path(receipt_id);

        assert_eq!(path, format!("receipts/{}.json", receipt_id));
    }
}
