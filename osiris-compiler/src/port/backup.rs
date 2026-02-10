//! Port trait for backup management and recovery.
//!
//! This port defines the interface for backing up compiler state to cloud storage,
//! managing incremental backups, and supporting point-in-time recovery.

use crate::domain::{
    BackupChain, BackupError, BackupRotationPolicy, BackupStats, BackupType, CompilerStateSnapshot,
    RecoveryRequest, RecoveryResult, VerificationResult,
};
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// Port trait for backup and recovery operations.
///
/// Implementations can use different storage backends (GCS, S3, etc.) while
/// maintaining a consistent interface for backup management.
#[async_trait]
pub trait BackupManager: Send + Sync {
    /// Creates a new full backup of the compiler state.
    ///
    /// # Arguments
    /// * `state` - The current compiler state to backup (as JSON)
    /// * `rotation_policy` - Policy for managing old backups
    ///
    /// # Returns
    /// A CompilerStateSnapshot representing the backup
    async fn create_full_backup(
        &self,
        state: JsonValue,
        rotation_policy: &BackupRotationPolicy,
    ) -> Result<CompilerStateSnapshot, BackupError>;

    /// Creates an incremental backup based on the latest snapshot.
    ///
    /// Incremental backups only store changes since the last snapshot,
    /// reducing storage requirements.
    ///
    /// # Arguments
    /// * `state_changes` - The changes since the last backup (as JSON)
    /// * `rotation_policy` - Policy for managing old backups
    ///
    /// # Returns
    /// A CompilerStateSnapshot representing the incremental backup
    async fn create_incremental_backup(
        &self,
        state_changes: JsonValue,
        rotation_policy: &BackupRotationPolicy,
    ) -> Result<CompilerStateSnapshot, BackupError>;

    /// Retrieves a snapshot by ID.
    ///
    /// # Arguments
    /// * `snapshot_id` - The ID of the snapshot to retrieve
    ///
    /// # Returns
    /// The requested snapshot, or an error if not found
    async fn get_snapshot(&self, snapshot_id: Uuid) -> Result<CompilerStateSnapshot, BackupError>;

    /// Lists all available snapshots.
    ///
    /// # Returns
    /// A vector of all snapshots, ordered by timestamp (newest first)
    async fn list_snapshots(&self) -> Result<Vec<CompilerStateSnapshot>, BackupError>;

    /// Retrieves the backup chain containing the given snapshot.
    ///
    /// A backup chain tracks a full backup and all its incremental backups.
    ///
    /// # Arguments
    /// * `snapshot_id` - ID of a snapshot in the chain
    ///
    /// # Returns
    /// The complete backup chain, or an error if not found
    async fn get_backup_chain(&self, snapshot_id: Uuid) -> Result<BackupChain, BackupError>;

    /// Lists all backup chains.
    ///
    /// # Returns
    /// A vector of all backup chains
    async fn list_backup_chains(&self) -> Result<Vec<BackupChain>, BackupError>;

    /// Performs point-in-time recovery.
    ///
    /// Reconstructs the full compiler state as it was at the specified point in time
    /// by replaying the full backup and all incremental backups up to the target point.
    ///
    /// # Arguments
    /// * `request` - The recovery request specifying the target point in time
    ///
    /// # Returns
    /// The recovered state and verification results
    async fn recover_to_point_in_time(
        &self,
        request: RecoveryRequest,
    ) -> Result<(JsonValue, RecoveryResult), BackupError>;

    /// Verifies the integrity of a backup snapshot.
    ///
    /// Checks that:
    /// 1. The stored hash matches the computed hash
    /// 2. The snapshot can be successfully deserialized
    /// 3. All parent snapshots are available (for incremental backups)
    ///
    /// # Arguments
    /// * `snapshot_id` - The snapshot to verify
    ///
    /// # Returns
    /// Verification results
    async fn verify_backup(&self, snapshot_id: Uuid) -> Result<VerificationResult, BackupError>;

    /// Verifies an entire backup chain for consistency.
    ///
    /// Ensures all snapshots in the chain are valid and can be replayed
    /// to reconstruct the complete state.
    ///
    /// # Arguments
    /// * `chain_id` - The full backup ID of the chain to verify
    ///
    /// # Returns
    /// Verification results for the chain
    async fn verify_backup_chain(&self, chain_id: Uuid) -> Result<VerificationResult, BackupError>;

    /// Gets backup statistics.
    ///
    /// # Returns
    /// Current backup statistics
    async fn get_backup_stats(&self) -> Result<BackupStats, BackupError>;

    /// Deletes a snapshot.
    ///
    /// Snapshots can only be deleted if they are not part of an active backup chain
    /// with fewer than min_backup_chains chains.
    ///
    /// # Arguments
    /// * `snapshot_id` - The snapshot to delete
    ///
    /// # Returns
    /// The deleted snapshot
    async fn delete_snapshot(
        &self,
        snapshot_id: Uuid,
    ) -> Result<CompilerStateSnapshot, BackupError>;

    /// Applies a backup rotation policy.
    ///
    /// Automatically deletes old backups according to the policy rules:
    /// - Deletes backups older than max_backup_age_secs
    /// - Deletes if total storage exceeds max_total_storage_bytes
    /// - Preserves at least min_full_backups full backups
    /// - Preserves at least min_backup_chains backup chains
    ///
    /// # Arguments
    /// * `policy` - The rotation policy to apply
    ///
    /// # Returns
    /// Number of snapshots deleted
    async fn apply_rotation_policy(
        &self,
        policy: &BackupRotationPolicy,
    ) -> Result<usize, BackupError>;
}

/// Configuration for the GCS-based backup manager.
#[derive(Debug, Clone)]
pub struct GcsBackupConfig {
    /// GCS bucket name where backups will be stored
    pub bucket: String,

    /// Prefix for backup objects in the bucket
    /// Example: "prod/backups" → gs://{bucket}/prod/backups/{snapshot_id}/
    pub prefix: String,

    /// Optional project ID for authenticated requests
    pub project_id: Option<String>,

    /// Path to service account key file (optional)
    pub service_account_key: Option<String>,
}

impl GcsBackupConfig {
    /// Creates a new GCS backup configuration.
    pub fn new(bucket: String, prefix: String) -> Self {
        Self {
            bucket,
            prefix,
            project_id: None,
            service_account_key: None,
        }
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
}
