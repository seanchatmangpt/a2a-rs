//! Google Cloud Storage receipt storage implementation.
//!
//! Stores receipts as JSON objects in Google Cloud Storage with SHA-256 hash-based naming.
//! Each receipt is stored as: gs://{bucket}/{prefix}/receipts/{receipt_hash}.json
//!
//! # Feature Gate
//! This module requires the "gcs" feature to be enabled.
//!
//! # Note on Querying
//! GCS doesn't support complex queries like listing by operation_id or time range.
//! For production use, consider:
//! 1. Storing metadata in Firestore with references to GCS objects
//! 2. Using BigQuery for querying with automatic export from GCS
//! 3. Implementing a separate index service using DynamoDB, Redis, or similar
//!
//! # Example
//!
//! ```rust,ignore
//! use osiris_compiler::adapter::{GcsConfig, GcsReceiptStorage};
//!
//! let config = GcsConfig::new("my-bucket".to_string())
//!     .with_prefix("prod/receipts".to_string());
//!
//! // Initialize GCS client and create storage
//! let client = google_cloud_storage::client::Client::new(Default::default());
//! let storage = GcsReceiptStorage::new(config, client);
//!
//! // Store a receipt
//! let url = storage.store_receipt(&receipt).await?;
//! println!("Stored at: {}", url);
//! ```

use crate::domain::{Receipt, ReceiptError};
use crate::port::ReceiptStorage;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "gcs")]
use google_cloud_storage::client::Client as GcsClient;

/// GCS configuration for receipt storage.
///
/// Supports both authenticated and unauthenticated GCS access.
/// Authentication credentials are loaded from the environment using
/// GOOGLE_APPLICATION_CREDENTIALS or other standard GCP auth methods.
#[derive(Debug, Clone)]
pub struct GcsConfig {
    /// GCS bucket name where receipts will be stored
    pub bucket: String,

    /// Optional prefix for receipt objects
    /// Example: "prod/receipts" → gs://{bucket}/prod/receipts/receipts/{hash}.json
    pub prefix: Option<String>,

    /// Optional project ID for authenticated requests
    /// If not specified, inferred from credentials
    pub project_id: Option<String>,

    /// Path to service account key file (optional)
    /// If not specified, uses default application credentials
    pub service_account_key: Option<String>,
}

impl GcsConfig {
    /// Creates a new GCS configuration.
    pub fn new(bucket: String) -> Self {
        Self {
            bucket,
            prefix: None,
            project_id: None,
            service_account_key: None,
        }
    }

    /// Sets the object prefix.
    pub fn with_prefix(mut self, prefix: String) -> Self {
        self.prefix = Some(prefix);
        self
    }

    /// Sets the project ID.
    pub fn with_project_id(mut self, project_id: String) -> Self {
        self.project_id = Some(project_id);
        self
    }

    /// Sets the service account key path.
    pub fn with_service_account_key(mut self, path: String) -> Self {
        self.service_account_key = Some(path);
        self
    }

    /// Creates a GCS configuration from environment variables.
    ///
    /// Expected environment variables:
    /// - `GCS_BUCKET`: GCS bucket name (required)
    /// - `GCS_PREFIX`: Object prefix (optional)
    /// - `GCS_PROJECT_ID`: GCP project ID (optional)
    /// - `GOOGLE_APPLICATION_CREDENTIALS`: Service account key path (optional)
    pub fn from_env() -> Result<Self, ReceiptError> {
        let bucket = std::env::var("GCS_BUCKET").map_err(|_| {
            ReceiptError::InvalidFormat("Missing GCS_BUCKET environment variable".to_string())
        })?;

        Ok(Self {
            bucket,
            prefix: std::env::var("GCS_PREFIX").ok(),
            project_id: std::env::var("GCS_PROJECT_ID").ok(),
            service_account_key: std::env::var("GOOGLE_APPLICATION_CREDENTIALS").ok(),
        })
    }
}

/// Google Cloud Storage receipt storage implementation.
///
/// Stores receipts as JSON objects in GCS with SHA256 hash-based object names.
/// The object path format is: {prefix}/receipts/{sha256_hash}.json
///
/// # Storage Details
/// - Receipt ID is hashed using SHA-256 to create deterministic, unique object names
/// - Receipts are stored as pretty-printed JSON with Content-Type: application/json
/// - All operations are async and use the google-cloud-storage client
///
/// # Limitations
/// Query operations (get_receipts_for_operation, list_receipts) are not supported
/// in this implementation as GCS doesn't support complex filtering. Use Firestore
/// or similar for metadata indexing.
#[cfg(feature = "gcs")]
pub struct GcsReceiptStorage {
    config: Arc<GcsConfig>,
    client: Arc<GcsClient>,
}

#[cfg(feature = "gcs")]
impl GcsReceiptStorage {
    /// Creates a new GCS receipt storage instance.
    ///
    /// # Arguments
    /// * `config` - GCS configuration with bucket and optional prefix
    /// * `client` - Initialized GCS client (must have proper credentials)
    pub fn new(config: GcsConfig, client: GcsClient) -> Self {
        Self {
            config: Arc::new(config),
            client: Arc::new(client),
        }
    }

    /// Creates a GCS receipt storage from environment variables.
    ///
    /// Loads configuration from GCS_* environment variables and initializes
    /// a GCS client using default application credentials.
    ///
    /// # Environment Variables
    /// - `GCS_BUCKET` (required): GCS bucket name
    /// - `GCS_PREFIX` (optional): Object prefix
    /// - `GCS_PROJECT_ID` (optional): GCP project ID
    /// - `GOOGLE_APPLICATION_CREDENTIALS` (optional): Path to service account key
    ///
    /// # Returns
    /// A new GcsReceiptStorage instance
    pub async fn from_env() -> Result<Self, ReceiptError> {
        let config = GcsConfig::from_env()?;
        let client = Self::create_client(&config).await?;
        Ok(Self::new(config, client))
    }

    /// Creates a GCS client with the given configuration.
    async fn create_client(_config: &GcsConfig) -> Result<GcsClient, ReceiptError> {
        // Initialize with default credentials (GOOGLE_APPLICATION_CREDENTIALS env var)
        let client = GcsClient::new(Default::default());
        Ok(client)
    }

    /// Computes a SHA-256 hash for a receipt ID.
    ///
    /// Produces deterministic, 64-character hex strings suitable for GCS object names.
    fn hash_receipt_id(receipt_id: Uuid) -> String {
        let mut hasher = Sha256::new();
        hasher.update(receipt_id.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Returns the GCS object path for a receipt.
    ///
    /// Format: {prefix}/receipts/{hash}.json or receipts/{hash}.json
    fn object_path(&self, receipt_id: Uuid) -> String {
        let receipt_hash = Self::hash_receipt_id(receipt_id);
        let base = format!("receipts/{}.json", receipt_hash);

        if let Some(prefix) = &self.config.prefix {
            format!("{}/{}", prefix, base)
        } else {
            base
        }
    }

    /// Uploads a receipt to GCS as a JSON object.
    async fn upload_receipt_bytes(
        &self,
        object_path: &str,
        bytes: &[u8],
    ) -> Result<(), ReceiptError> {
        let bucket = &self.config.bucket;

        // Upload using the GCS client
        // The client handles authentication and returns metadata on success
        self.client
            .upload_object(bucket, object_path, bytes, "application/json")
            .await
            .map_err(|e| {
                ReceiptError::InvalidFormat(format!("GCS upload failed for {}: {}", object_path, e))
            })?;

        Ok(())
    }

    /// Downloads a receipt from GCS as bytes.
    async fn download_receipt_bytes(&self, object_path: &str) -> Result<Vec<u8>, ReceiptError> {
        let bucket = &self.config.bucket;

        self.client
            .download_object(bucket, object_path)
            .await
            .map_err(|e| {
                ReceiptError::InvalidFormat(format!(
                    "GCS download failed for {}: {}",
                    object_path, e
                ))
            })
    }
}

#[cfg(feature = "gcs")]
#[async_trait]
impl ReceiptStorage for GcsReceiptStorage {
    async fn store_receipt(&self, receipt: &Receipt) -> Result<String, ReceiptError> {
        // Serialize receipt to pretty-printed JSON
        let json = serde_json::to_vec_pretty(receipt)
            .map_err(|e| ReceiptError::SerializationError(e.to_string()))?;

        // Compute hash-based object path
        let object_path = self.object_path(receipt.id);

        // Upload to GCS
        self.upload_receipt_bytes(&object_path, &json).await?;

        // Return the GCS URL for reference
        Ok(format!("gs://{}/{}", self.config.bucket, object_path))
    }

    async fn get_receipt(&self, receipt_id: Uuid) -> Result<Receipt, ReceiptError> {
        // Compute object path from receipt ID
        let object_path = self.object_path(receipt_id);

        // Download from GCS
        let bytes = self.download_receipt_bytes(&object_path).await?;

        // Deserialize JSON to Receipt
        let receipt = serde_json::from_slice::<Receipt>(&bytes)
            .map_err(|e| ReceiptError::SerializationError(e.to_string()))?;

        Ok(receipt)
    }

    async fn get_receipts_for_operation(
        &self,
        _operation_id: Uuid,
    ) -> Result<Vec<Receipt>, ReceiptError> {
        // GCS doesn't support filtering by metadata queries
        // This requires a separate metadata index (Firestore, BigQuery, etc.)
        Err(ReceiptError::InvalidFormat(
            "get_receipts_for_operation not supported by GCS backend. \
             Use Firestore for metadata indexing with GCS object references."
                .to_string(),
        ))
    }

    async fn list_receipts(
        &self,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<Receipt>, ReceiptError> {
        // GCS doesn't support filtering by timestamp queries
        // This requires a separate metadata index (Firestore, BigQuery, etc.)
        Err(ReceiptError::InvalidFormat(
            "list_receipts not supported by GCS backend. \
             Use Firestore for metadata indexing with GCS object references."
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcs_config_new() {
        let config = GcsConfig::new("my-bucket".to_string());
        assert_eq!(config.bucket, "my-bucket");
        assert!(config.prefix.is_none());
        assert!(config.project_id.is_none());
    }

    #[test]
    fn test_gcs_config_builder_chain() {
        let config = GcsConfig::new("my-bucket".to_string())
            .with_prefix("prod/receipts".to_string())
            .with_project_id("my-project".to_string())
            .with_service_account_key("/path/to/key.json".to_string());

        assert_eq!(config.bucket, "my-bucket");
        assert_eq!(config.prefix, Some("prod/receipts".to_string()));
        assert_eq!(config.project_id, Some("my-project".to_string()));
        assert_eq!(
            config.service_account_key,
            Some("/path/to/key.json".to_string())
        );
    }

    #[test]
    fn test_hash_receipt_id_is_deterministic() {
        let receipt_id = Uuid::new_v4();

        // Same receipt ID should produce same hash
        let h1 = GcsConfig::new("test".to_string());
        let h2 = GcsConfig::new("test".to_string());

        // Hash should be deterministic
        let hash1 = GcsReceiptStorage::hash_receipt_id(receipt_id);
        let hash2 = GcsReceiptStorage::hash_receipt_id(receipt_id);
        assert_eq!(hash1, hash2);

        // SHA-256 hex output is 64 characters
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_hash_receipt_id_different_for_different_ids() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let hash1 = GcsReceiptStorage::hash_receipt_id(id1);
        let hash2 = GcsReceiptStorage::hash_receipt_id(id2);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_gcs_config_from_env_missing_bucket() {
        std::env::remove_var("GCS_BUCKET");
        let result = GcsConfig::from_env();
        assert!(result.is_err());
    }

    #[test]
    fn test_gcs_object_path_with_prefix() {
        let config = GcsConfig::new("my-bucket".to_string()).with_prefix("prod/data".to_string());

        #[cfg(feature = "gcs")]
        {
            let storage = GcsReceiptStorage {
                config: Arc::new(config),
                client: Arc::new(GcsClient::new(Default::default())),
            };

            let receipt_id = Uuid::new_v4();
            let path = storage.object_path(receipt_id);

            assert!(path.starts_with("prod/data/"));
            assert!(path.contains("receipts/"));
            assert!(path.ends_with(".json"));
        }
    }

    #[test]
    fn test_gcs_object_path_without_prefix() {
        let config = GcsConfig::new("my-bucket".to_string());

        #[cfg(feature = "gcs")]
        {
            let storage = GcsReceiptStorage {
                config: Arc::new(config),
                client: Arc::new(GcsClient::new(Default::default())),
            };

            let receipt_id = Uuid::new_v4();
            let path = storage.object_path(receipt_id);

            assert!(path.starts_with("receipts/"));
            assert!(path.ends_with(".json"));
        }
    }

    #[test]
    fn test_receipt_json_serialization() {
        use crate::domain::{Operation, OperationKind, OperationResult};
        use std::collections::HashMap;

        let operation = Operation::new(
            OperationKind::Parse {
                input: "test".into(),
            },
            1,
        );

        let receipt = Receipt {
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
        };

        // Serialize
        let json_bytes = serde_json::to_vec_pretty(&receipt).expect("Serialization failed");
        assert!(!json_bytes.is_empty());

        // Deserialize
        let deserialized =
            serde_json::from_slice::<Receipt>(&json_bytes).expect("Deserialization failed");

        assert_eq!(deserialized.id, receipt.id);
        assert_eq!(deserialized.operation_id, receipt.operation_id);
    }
}
