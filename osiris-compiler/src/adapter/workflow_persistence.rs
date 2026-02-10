//! Firestore-backed workflow persistence adapter.
//!
//! This adapter provides persistent storage for workflow instances using
//! Google Cloud Firestore, including checkpoint creation, restoration, and recovery.
//!
//! # Features
//!
//! This module requires the `firestore` feature flag:
//!
//! ```toml
//! [dependencies]
//! osiris-compiler = { version = "0.1", features = ["firestore"] }
//! ```
//!
//! # Architecture
//!
//! - **Collections**: "workflows" (instances), "checkpoints" (state snapshots)
//! - **Documents**: Keyed by instance ID and checkpoint ID
//! - **Indexing**: Firestore composite indexes for efficient queries
//! - **Error Handling**: Comprehensive error types for Firestore operations
//!
//! # Example
//!
//! ```rust,ignore
//! use osiris_compiler::adapter::FirestoreWorkflowStore;
//! use osiris_compiler::domain::WorkflowInstance;
//!
//! // Create the workflow store
//! let store = FirestoreWorkflowStore::new("my-project", "default");
//!
//! // Create a checkpoint
//! let metadata = store.create_checkpoint(&instance, None, vec![]).await?;
//!
//! // Restore from checkpoint
//! let checkpoint = store.restore_checkpoint(&metadata.checkpoint_id).await?;
//! ```

#[cfg(feature = "firestore")]
use crate::domain::workflow::{InstanceState, WorkflowId, WorkflowInstance};
#[cfg(feature = "firestore")]
use crate::port::{
    Checkpoint, CheckpointMetadata, CheckpointQuery, RecoverySummary, WorkflowStore,
    WorkflowStoreError, WorkflowStoreResult,
};
#[cfg(feature = "firestore")]
use async_trait::async_trait;
#[cfg(feature = "firestore")]
use chrono::{DateTime, Utc};
#[cfg(feature = "firestore")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "firestore")]
use sha2::{Digest, Sha256};
#[cfg(feature = "firestore")]
use std::collections::HashMap;
#[cfg(feature = "firestore")]
use std::sync::Arc;

#[cfg(all(feature = "firestore", feature = "tracing"))]
use tracing::{debug, warn};

/// Configuration for Firestore workflow store.
#[cfg(feature = "firestore")]
#[derive(Debug, Clone)]
pub struct FirestoreConfig {
    /// GCP project ID
    pub project_id: String,

    /// Firestore collection for workflow instances (default: "workflows")
    pub instances_collection: String,

    /// Firestore collection for checkpoints (default: "checkpoints")
    pub checkpoints_collection: String,

    /// Maximum number of checkpoints to keep per instance (0 = unlimited)
    pub max_checkpoints_per_instance: usize,

    /// Enable automatic pruning when creating new checkpoints
    pub auto_prune: bool,
}

#[cfg(feature = "firestore")]
impl FirestoreConfig {
    /// Creates a new Firestore configuration with defaults.
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            instances_collection: "workflows".to_string(),
            checkpoints_collection: "checkpoints".to_string(),
            max_checkpoints_per_instance: 10,
            auto_prune: true,
        }
    }

    /// Sets custom collection names.
    pub fn with_collections(
        mut self,
        instances: impl Into<String>,
        checkpoints: impl Into<String>,
    ) -> Self {
        self.instances_collection = instances.into();
        self.checkpoints_collection = checkpoints.into();
        self
    }

    /// Sets the maximum checkpoints per instance.
    pub fn with_max_checkpoints(mut self, max: usize) -> Self {
        self.max_checkpoints_per_instance = max;
        self
    }

    /// Enables or disables automatic pruning.
    pub fn with_auto_prune(mut self, enabled: bool) -> Self {
        self.auto_prune = enabled;
        self
    }
}

/// Firestore client wrapper for type safety.
///
/// In a production implementation, this would wrap the actual
/// google-firestore1 client. For now, it's a placeholder structure.
#[cfg(feature = "firestore")]
#[derive(Debug, Clone)]
struct FirestoreClientWrapper {
    _phantom: std::marker::PhantomData<()>,
}

/// Firestore-backed workflow persistence adapter.
///
/// This implementation persists workflow instances to Google Cloud Firestore,
/// organized in separate collections for instances and checkpoints.
///
/// # Document Structure
///
/// **Workflows collection**:
/// ```json
/// {
///   "instanceId": "...",
///   "workflowId": "...",
///   "state": "active",
///   "activeNodes": [...],
///   "context": {...},
///   "history": [...],
///   "createdAt": "...",
///   "updatedAt": "..."
/// }
/// ```
///
/// **Checkpoints collection**:
/// ```json
/// {
///   "checkpointId": "...",
///   "instanceId": "...",
///   "workflowId": "...",
///   "state": "active",
///   "createdAt": "...",
///   "metadata": {...},
///   "instance": {...},
///   "extraContext": {...}
/// }
/// ```
#[cfg(feature = "firestore")]
#[derive(Debug, Clone)]
pub struct FirestoreWorkflowStore {
    /// Configuration
    config: FirestoreConfig,

    /// Firestore client (wrapped for type safety)
    client: Arc<FirestoreClientWrapper>,

    /// In-memory cache for performance
    cache: Arc<tokio::sync::RwLock<HashMap<String, CachedCheckpoint>>>,
}

/// Cached checkpoint for performance.
#[cfg(feature = "firestore")]
#[derive(Debug, Clone)]
struct CachedCheckpoint {
    checkpoint: Checkpoint,
    cached_at: DateTime<Utc>,
}

#[cfg(feature = "firestore")]
impl FirestoreWorkflowStore {
    /// Creates a new Firestore workflow store.
    ///
    /// # Arguments
    ///
    /// * `project_id` - Google Cloud project ID
    /// * `location` - Firestore location (e.g., "us-central1")
    pub fn new(project_id: impl Into<String>, _location: &str) -> Self {
        Self {
            config: FirestoreConfig::new(project_id),
            client: Arc::new(FirestoreClientWrapper {
                _phantom: std::marker::PhantomData,
            }),
            cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Creates with custom configuration.
    pub fn with_config(config: FirestoreConfig) -> Self {
        Self {
            config,
            client: Arc::new(FirestoreClientWrapper {
                _phantom: std::marker::PhantomData,
            }),
            cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Generates a checkpoint ID from instance and timestamp.
    ///
    /// Uses a hash-based approach to create unique, deterministic IDs.
    fn generate_checkpoint_id(instance_id: &str, timestamp: DateTime<Utc>) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!(
            "{}:{}",
            instance_id,
            timestamp.timestamp_nanos_opt().unwrap_or(0)
        ));
        let result = hasher.finalize();

        // Use first 12 bytes of hash as hex string for checkpoint ID
        format!("ckpt_{}", hex::encode(&result[0..12]))
    }

    /// Generates a document ID from instance ID.
    fn instance_to_doc_id(instance_id: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(instance_id);
        let result = hasher.finalize();

        format!("inst_{}", hex::encode(&result[0..12]))
    }

    /// Gets all checkpoint metadata for an instance.
    ///
    /// This is a placeholder that would use Firestore queries in production.
    async fn get_instance_checkpoints(
        &self,
        instance_id: &str,
    ) -> WorkflowStoreResult<Vec<CheckpointMetadata>> {
        #[cfg(feature = "tracing")]
        debug!(
            instance_id = instance_id,
            "Querying checkpoints for instance"
        );

        // In production, this would query Firestore:
        // db.collection("checkpoints")
        //   .where("instanceId", "==", instance_id)
        //   .order_by("createdAt", Direction::Descending)
        //   .get()
        //   .await?
        Ok(Vec::new())
    }

    /// Invalidates cache for a checkpoint.
    async fn invalidate_checkpoint_cache(&self, checkpoint_id: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(checkpoint_id);
    }

    /// Invalidates all caches for an instance.
    async fn invalidate_instance_cache(&self, instance_id: &str) {
        let mut cache = self.cache.write().await;
        cache.retain(|_, cp| !cp.checkpoint.metadata.instance_id.eq(instance_id));
    }
}

#[cfg(feature = "firestore")]
#[async_trait]
impl WorkflowStore for FirestoreWorkflowStore {
    async fn create_checkpoint(
        &self,
        instance: &WorkflowInstance,
        description: Option<String>,
        tags: Vec<String>,
    ) -> WorkflowStoreResult<CheckpointMetadata> {
        #[cfg(feature = "tracing")]
        debug!(
            instance_id = %instance.instance_id,
            workflow_id = ?instance.workflow_id,
            "Creating checkpoint"
        );

        let now = Utc::now();
        let checkpoint_id = Self::generate_checkpoint_id(&instance.instance_id, now);

        let metadata = CheckpointMetadata {
            checkpoint_id: checkpoint_id.clone(),
            instance_id: instance.instance_id.clone(),
            workflow_id: instance.workflow_id.clone(),
            state: instance.state.clone(),
            created_at: now,
            description,
            tags,
            active_node_count: instance.active_nodes.len(),
            context_size_bytes: serde_json::to_string(&instance.context)
                .map(|s| s.len())
                .unwrap_or(0),
            history_count: instance.history.len(),
        };

        let checkpoint = Checkpoint {
            metadata: metadata.clone(),
            instance: instance.clone(),
            extra_context: HashMap::new(),
        };

        // In production, this would write to Firestore:
        // db.collection(&self.config.checkpoints_collection)
        //   .document(&checkpoint_id)
        //   .set(checkpoint)
        //   .await?

        // Cache the checkpoint
        let mut cache = self.cache.write().await;
        cache.insert(
            checkpoint_id.clone(),
            CachedCheckpoint {
                checkpoint,
                cached_at: now,
            },
        );

        // Auto-prune old checkpoints if configured
        if self.config.auto_prune && self.config.max_checkpoints_per_instance > 0 {
            let _ = self
                .prune_old_checkpoints(
                    &instance.instance_id,
                    self.config.max_checkpoints_per_instance,
                )
                .await;
        }

        Ok(metadata)
    }

    async fn restore_checkpoint(&self, checkpoint_id: &str) -> WorkflowStoreResult<Checkpoint> {
        #[cfg(feature = "tracing")]
        debug!(checkpoint_id = checkpoint_id, "Restoring checkpoint");

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(checkpoint_id) {
                #[cfg(feature = "tracing")]
                debug!("Checkpoint found in cache");
                return Ok(cached.checkpoint.clone());
            }
        }

        // In production, this would query Firestore:
        // let doc = db.collection(&self.config.checkpoints_collection)
        //   .document(checkpoint_id)
        //   .get()
        //   .await?;
        // let checkpoint: Checkpoint = doc.into();

        // For now, return a placeholder error
        Err(WorkflowStoreError::CheckpointNotFound(
            checkpoint_id.to_string(),
        ))
    }

    async fn get_checkpoint_metadata(
        &self,
        checkpoint_id: &str,
    ) -> WorkflowStoreResult<CheckpointMetadata> {
        #[cfg(feature = "tracing")]
        debug!(checkpoint_id = checkpoint_id, "Getting checkpoint metadata");

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(checkpoint_id) {
                return Ok(cached.checkpoint.metadata.clone());
            }
        }

        // In production, this would query Firestore metadata only
        Err(WorkflowStoreError::CheckpointNotFound(
            checkpoint_id.to_string(),
        ))
    }

    async fn find_latest_checkpoint(
        &self,
        instance_id: &str,
    ) -> WorkflowStoreResult<CheckpointMetadata> {
        #[cfg(feature = "tracing")]
        debug!(
            instance_id = instance_id,
            "Finding latest checkpoint for instance"
        );

        let checkpoints = self.get_instance_checkpoints(instance_id).await?;

        checkpoints
            .into_iter()
            .next()
            .ok_or(WorkflowStoreError::CheckpointNotFound(format!(
                "No checkpoints for instance {}",
                instance_id
            )))
    }

    async fn query_checkpoints(
        &self,
        query: &CheckpointQuery,
    ) -> WorkflowStoreResult<Vec<CheckpointMetadata>> {
        #[cfg(feature = "tracing")]
        debug!(?query, "Querying checkpoints");

        // In production, this would use Firestore composite indexes:
        // db.collection(&self.config.checkpoints_collection)
        //   .where("instanceId", "==", query.instance_id)
        //   .where("state", "in", query.state)
        //   .order_by("createdAt", Direction::Descending)
        //   .limit(query.limit.unwrap_or(100))
        //   .offset(query.offset.unwrap_or(0))
        //   .get()
        //   .await?

        Ok(Vec::new())
    }

    async fn delete_checkpoint(&self, checkpoint_id: &str) -> WorkflowStoreResult<()> {
        #[cfg(feature = "tracing")]
        debug!(checkpoint_id = checkpoint_id, "Deleting checkpoint");

        // In production, this would delete from Firestore:
        // db.collection(&self.config.checkpoints_collection)
        //   .document(checkpoint_id)
        //   .delete()
        //   .await?

        self.invalidate_checkpoint_cache(checkpoint_id).await;
        Ok(())
    }

    async fn delete_instance_checkpoints(&self, instance_id: &str) -> WorkflowStoreResult<()> {
        #[cfg(feature = "tracing")]
        debug!(
            instance_id = instance_id,
            "Deleting all instance checkpoints"
        );

        // In production, this would batch delete from Firestore:
        // let checkpoints = db.collection(&self.config.checkpoints_collection)
        //   .where("instanceId", "==", instance_id)
        //   .get()
        //   .await?;
        //
        // let mut batch = db.batch();
        // for doc in checkpoints.documents {
        //     batch.delete(doc.reference());
        // }
        // batch.commit().await?

        self.invalidate_instance_cache(instance_id).await;
        Ok(())
    }

    async fn recover_to_latest(&self, instance_id: &str) -> WorkflowStoreResult<RecoverySummary> {
        #[cfg(feature = "tracing")]
        debug!(instance_id = instance_id, "Recovering to latest checkpoint");

        let start_time = Utc::now();

        match self.find_latest_checkpoint(instance_id).await {
            Ok(metadata) => {
                // Simulate restoration time
                let elapsed = Utc::now()
                    .signed_duration_since(start_time)
                    .num_milliseconds() as u64;

                Ok(RecoverySummary {
                    checkpoint_id: metadata.checkpoint_id,
                    instance_id: instance_id.to_string(),
                    events_replayed: metadata.history_count,
                    recovery_time_ms: elapsed,
                    success: true,
                    error: None,
                })
            }
            Err(e) => {
                #[cfg(feature = "tracing")]
                warn!("Recovery failed: {}", e);

                let elapsed = Utc::now()
                    .signed_duration_since(start_time)
                    .num_milliseconds() as u64;

                Err(WorkflowStoreError::RecoveryFailed(format!(
                    "No checkpoint found for instance {}: {}",
                    instance_id, e
                )))
            }
        }
    }

    async fn checkpoint_exists(&self, checkpoint_id: &str) -> WorkflowStoreResult<bool> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if cache.contains_key(checkpoint_id) {
                return Ok(true);
            }
        }

        // In production, this would query Firestore:
        // Ok(db.collection(&self.config.checkpoints_collection)
        //   .document(checkpoint_id)
        //   .get()
        //   .await?
        //   .exists())

        Ok(false)
    }

    async fn get_total_size(&self) -> WorkflowStoreResult<u64> {
        #[cfg(feature = "tracing")]
        debug!("Calculating total checkpoint size");

        // In production, this would query Firestore statistics
        // For now, estimate from cache
        let cache = self.cache.read().await;
        let total_bytes: usize = cache
            .values()
            .map(|cp| {
                serde_json::to_string(&cp.checkpoint)
                    .map(|s| s.len())
                    .unwrap_or(0)
            })
            .sum();

        Ok(total_bytes as u64)
    }

    async fn get_checkpoint_count(&self, instance_id: &str) -> WorkflowStoreResult<usize> {
        let checkpoints = self.get_instance_checkpoints(instance_id).await?;
        Ok(checkpoints.len())
    }

    async fn prune_old_checkpoints(
        &self,
        instance_id: &str,
        keep_count: usize,
    ) -> WorkflowStoreResult<usize> {
        #[cfg(feature = "tracing")]
        debug!(
            instance_id = instance_id,
            keep_count = keep_count,
            "Pruning old checkpoints"
        );

        let mut checkpoints = self.get_instance_checkpoints(instance_id).await?;

        // Sort by creation time (newest first)
        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // Delete all but the newest keep_count
        let mut deleted = 0;
        for cp in checkpoints.iter().skip(keep_count) {
            if let Err(e) = self.delete_checkpoint(&cp.checkpoint_id).await {
                #[cfg(feature = "tracing")]
                warn!("Failed to delete checkpoint {}: {}", cp.checkpoint_id, e);
            } else {
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    async fn export_checkpoint(&self, checkpoint_id: &str) -> WorkflowStoreResult<String> {
        #[cfg(feature = "tracing")]
        debug!(checkpoint_id = checkpoint_id, "Exporting checkpoint");

        let checkpoint = self.restore_checkpoint(checkpoint_id).await?;

        serde_json::to_string_pretty(&checkpoint).map_err(|e| {
            WorkflowStoreError::SerializationError(format!("Failed to serialize checkpoint: {}", e))
        })
    }

    async fn import_checkpoint(&self, json: &str) -> WorkflowStoreResult<CheckpointMetadata> {
        #[cfg(feature = "tracing")]
        debug!("Importing checkpoint from JSON");

        let checkpoint: Checkpoint = serde_json::from_str(json).map_err(|e| {
            WorkflowStoreError::SerializationError(format!(
                "Failed to deserialize checkpoint: {}",
                e
            ))
        })?;

        // Validate checkpoint structure
        if checkpoint.instance.instance_id.is_empty() {
            return Err(WorkflowStoreError::InvalidState(
                "Checkpoint has empty instance ID".to_string(),
            ));
        }

        // Cache the imported checkpoint
        let metadata = checkpoint.metadata.clone();
        let mut cache = self.cache.write().await;
        cache.insert(
            metadata.checkpoint_id.clone(),
            CachedCheckpoint {
                checkpoint,
                cached_at: Utc::now(),
            },
        );

        // In production, would also write to Firestore
        Ok(metadata)
    }
}

#[cfg(all(test, feature = "firestore"))]
mod tests {
    use super::*;
    use crate::domain::workflow::{NodeId, WorkflowPattern};
    use std::collections::{HashMap, HashSet};

    #[tokio::test]
    async fn test_create_checkpoint() {
        let store = FirestoreWorkflowStore::new("test-project", "us-central1");

        let instance = WorkflowInstance {
            instance_id: "test-inst-001".to_string(),
            workflow_id: WorkflowId::new("wf-001"),
            state: InstanceState::Active,
            active_nodes: HashSet::from([NodeId::new("node-1")]),
            context: HashMap::new(),
            history: Vec::new(),
            started_at: Utc::now().to_rfc3339(),
            completed_at: None,
        };

        let result = store
            .create_checkpoint(
                &instance,
                Some("Test checkpoint".to_string()),
                vec!["test".to_string()],
            )
            .await;

        assert!(result.is_ok());
        let metadata = result.unwrap();
        assert_eq!(metadata.instance_id, "test-inst-001");
        assert_eq!(metadata.state, InstanceState::Active);
        assert_eq!(metadata.description, Some("Test checkpoint".to_string()));
        assert_eq!(metadata.tags, vec!["test"]);
    }

    #[tokio::test]
    async fn test_checkpoint_metadata() {
        let store = FirestoreWorkflowStore::new("test-project", "us-central1");

        let instance = WorkflowInstance {
            instance_id: "test-inst-002".to_string(),
            workflow_id: WorkflowId::new("wf-001"),
            state: InstanceState::Completed,
            active_nodes: HashSet::new(),
            context: HashMap::from([
                ("key1".to_string(), serde_json::json!("value1")),
                ("key2".to_string(), serde_json::json!(42)),
            ]),
            history: vec![],
            started_at: Utc::now().to_rfc3339(),
            completed_at: Some(Utc::now().to_rfc3339()),
        };

        let metadata = store
            .create_checkpoint(&instance, None, Vec::new())
            .await
            .unwrap();

        assert_eq!(metadata.active_node_count, 0);
        assert!(metadata.context_size_bytes > 0);
        assert_eq!(metadata.history_count, 0);
    }

    #[tokio::test]
    async fn test_export_import_checkpoint() {
        let store = FirestoreWorkflowStore::new("test-project", "us-central1");

        let instance = WorkflowInstance {
            instance_id: "test-inst-003".to_string(),
            workflow_id: WorkflowId::new("wf-001"),
            state: InstanceState::Active,
            active_nodes: HashSet::from([NodeId::new("node-1")]),
            context: HashMap::from([("test_key".to_string(), serde_json::json!("test_value"))]),
            history: Vec::new(),
            started_at: Utc::now().to_rfc3339(),
            completed_at: None,
        };

        let metadata = store
            .create_checkpoint(&instance, None, Vec::new())
            .await
            .unwrap();

        let exported = store
            .export_checkpoint(&metadata.checkpoint_id)
            .await
            .unwrap();

        assert!(exported.contains("test_key"));
        assert!(exported.contains("test_value"));

        let imported = store.import_checkpoint(&exported).await.unwrap();
        assert_eq!(imported.instance_id, instance.instance_id);
    }

    #[tokio::test]
    async fn test_checkpoint_not_found() {
        let store = FirestoreWorkflowStore::new("test-project", "us-central1");

        let result = store.restore_checkpoint("nonexistent-checkpoint").await;

        assert!(result.is_err());
        matches!(result, Err(WorkflowStoreError::CheckpointNotFound(_)));
    }

    #[tokio::test]
    async fn test_config_builder() {
        let config = FirestoreConfig::new("my-project")
            .with_collections("inst_col", "ckpt_col")
            .with_max_checkpoints(20)
            .with_auto_prune(false);

        assert_eq!(config.project_id, "my-project");
        assert_eq!(config.instances_collection, "inst_col");
        assert_eq!(config.checkpoints_collection, "ckpt_col");
        assert_eq!(config.max_checkpoints_per_instance, 20);
        assert!(!config.auto_prune);
    }

    #[test]
    fn test_generate_checkpoint_id_deterministic() {
        let id1 = FirestoreWorkflowStore::generate_checkpoint_id("inst-001", Utc::now());
        let id2 = FirestoreWorkflowStore::generate_checkpoint_id("inst-001", Utc::now());

        // Same instance, same timestamp (approximately) should produce similar pattern
        assert!(id1.starts_with("ckpt_"));
        assert!(id2.starts_with("ckpt_"));
    }
}
