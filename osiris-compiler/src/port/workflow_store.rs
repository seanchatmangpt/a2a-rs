//! Workflow persistence port trait.
//!
//! Defines the contract for persisting workflow instances and managing
//! checkpoints for recovery and replay.

use crate::domain::workflow::{InstanceState, WorkflowId, WorkflowInstance};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Workflow persistence errors.
#[derive(Error, Debug, Clone)]
pub enum WorkflowStoreError {
    /// Workflow instance not found
    #[error("Workflow instance not found: {0}")]
    InstanceNotFound(String),

    /// Checkpoint not found
    #[error("Checkpoint not found: {0}")]
    CheckpointNotFound(String),

    /// Failed to save checkpoint
    #[error("Failed to save checkpoint: {0}")]
    SaveFailed(String),

    /// Failed to restore checkpoint
    #[error("Failed to restore checkpoint: {0}")]
    RestoreFailed(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Replay failure
    #[error("Replay failed: {0}")]
    ReplayFailed(String),

    /// Query error
    #[error("Query error: {0}")]
    QueryError(String),

    /// Invalid checkpoint state
    #[error("Invalid checkpoint state: {0}")]
    InvalidState(String),
}

/// Result type for workflow store operations.
pub type WorkflowStoreResult<T> = Result<T, WorkflowStoreError>;

/// Checkpoint metadata containing workflow state snapshot information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointMetadata {
    /// Checkpoint identifier (usually UUID or timestamp-based)
    pub checkpoint_id: String,

    /// Workflow instance ID
    pub instance_id: String,

    /// Workflow pattern ID
    pub workflow_id: WorkflowId,

    /// State at checkpoint time
    pub state: InstanceState,

    /// Timestamp when checkpoint was created
    pub created_at: DateTime<Utc>,

    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Tags for categorizing checkpoints
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,

    /// Number of active nodes at checkpoint
    pub active_node_count: usize,

    /// Size of context variables in bytes
    pub context_size_bytes: usize,

    /// Number of history events persisted
    pub history_count: usize,
}

/// Checkpoint containing a complete workflow instance state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    /// Checkpoint metadata
    pub metadata: CheckpointMetadata,

    /// Complete workflow instance snapshot
    pub instance: WorkflowInstance,

    /// Additional context variables specific to this checkpoint
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub extra_context: HashMap<String, serde_json::Value>,
}

/// Query filter for listing checkpoints.
#[derive(Debug, Clone, Default)]
pub struct CheckpointQuery {
    /// Filter by instance ID (optional)
    pub instance_id: Option<String>,

    /// Filter by workflow ID (optional)
    pub workflow_id: Option<WorkflowId>,

    /// Filter by state (optional)
    pub state: Option<InstanceState>,

    /// Filter by tags (optional, matches any tag)
    pub tags: Vec<String>,

    /// Minimum checkpoint age (from most recent)
    pub created_after: Option<DateTime<Utc>>,

    /// Maximum checkpoint age
    pub created_before: Option<DateTime<Utc>>,

    /// Maximum number of results (default: 100)
    pub limit: Option<usize>,

    /// Skip first N results
    pub offset: Option<usize>,
}

/// Summary of recovery information for restart.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverySummary {
    /// Latest checkpoint used for recovery
    pub checkpoint_id: String,

    /// Instance recovered
    pub instance_id: String,

    /// Events replayed since checkpoint
    pub events_replayed: usize,

    /// Time to recover (milliseconds)
    pub recovery_time_ms: u64,

    /// Whether recovery was successful
    pub success: bool,

    /// Optional error message if recovery failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Workflow store port trait.
///
/// Defines persistence and recovery operations for workflow instances.
/// Implementations support checkpoint-based recovery and replay.
#[async_trait]
pub trait WorkflowStore: Send + Sync {
    /// Saves a workflow instance as a checkpoint.
    ///
    /// Creates a new checkpoint with the current instance state.
    /// Returns the checkpoint metadata.
    async fn create_checkpoint(
        &self,
        instance: &WorkflowInstance,
        description: Option<String>,
        tags: Vec<String>,
    ) -> WorkflowStoreResult<CheckpointMetadata>;

    /// Restores a workflow instance from a checkpoint.
    ///
    /// Reconstructs the complete workflow instance state from persisted data.
    /// Does not replay events; returns the instance as it was at checkpoint time.
    async fn restore_checkpoint(&self, checkpoint_id: &str) -> WorkflowStoreResult<Checkpoint>;

    /// Gets checkpoint metadata by ID.
    ///
    /// Returns only metadata without the full instance snapshot.
    async fn get_checkpoint_metadata(
        &self,
        checkpoint_id: &str,
    ) -> WorkflowStoreResult<CheckpointMetadata>;

    /// Finds the latest checkpoint for a given instance.
    ///
    /// Returns the most recent checkpoint created for the instance,
    /// or an error if none exists.
    async fn find_latest_checkpoint(
        &self,
        instance_id: &str,
    ) -> WorkflowStoreResult<CheckpointMetadata>;

    /// Queries checkpoints by filter criteria.
    ///
    /// Allows complex queries to find checkpoints matching multiple criteria.
    async fn query_checkpoints(
        &self,
        query: &CheckpointQuery,
    ) -> WorkflowStoreResult<Vec<CheckpointMetadata>>;

    /// Deletes a checkpoint by ID.
    ///
    /// Removes the checkpoint and associated data from storage.
    async fn delete_checkpoint(&self, checkpoint_id: &str) -> WorkflowStoreResult<()>;

    /// Deletes all checkpoints for an instance.
    ///
    /// Cleans up all checkpoint history for a specific workflow instance.
    async fn delete_instance_checkpoints(&self, instance_id: &str) -> WorkflowStoreResult<()>;

    /// Recovers a workflow instance to the latest checkpoint.
    ///
    /// Performs recovery by:
    /// 1. Finding the latest checkpoint
    /// 2. Restoring the instance from that checkpoint
    /// 3. Replaying recent events (if implementation supports it)
    /// 4. Returning recovery summary
    async fn recover_to_latest(&self, instance_id: &str) -> WorkflowStoreResult<RecoverySummary>;

    /// Checks if a checkpoint exists.
    async fn checkpoint_exists(&self, checkpoint_id: &str) -> WorkflowStoreResult<bool>;

    /// Gets the total size of stored checkpoints (bytes).
    ///
    /// Returns the approximate total size of all checkpoint data.
    async fn get_total_size(&self) -> WorkflowStoreResult<u64>;

    /// Gets the count of checkpoints for an instance.
    async fn get_checkpoint_count(&self, instance_id: &str) -> WorkflowStoreResult<usize>;

    /// Prunes old checkpoints for an instance.
    ///
    /// Keeps only the N most recent checkpoints, deleting older ones.
    /// Returns the number of deleted checkpoints.
    async fn prune_old_checkpoints(
        &self,
        instance_id: &str,
        keep_count: usize,
    ) -> WorkflowStoreResult<usize>;

    /// Exports a checkpoint to JSON string format.
    ///
    /// Useful for backup and migration.
    async fn export_checkpoint(&self, checkpoint_id: &str) -> WorkflowStoreResult<String>;

    /// Imports a checkpoint from JSON string format.
    ///
    /// Returns the metadata of the imported checkpoint.
    async fn import_checkpoint(&self, json: &str) -> WorkflowStoreResult<CheckpointMetadata>;
}
