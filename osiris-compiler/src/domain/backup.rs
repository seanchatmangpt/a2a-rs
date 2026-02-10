//! Backup domain types for point-in-time recovery and state snapshots.
//!
//! Defines the data structures for backing up compiler state, managing
//! incremental backups, and supporting point-in-time recovery.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A snapshot of compiler state at a specific point in time.
///
/// Contains serialized state of all major components for recovery purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerStateSnapshot {
    /// Unique identifier for this snapshot
    pub id: Uuid,

    /// Timestamp when this snapshot was created
    pub timestamp: DateTime<Utc>,

    /// The actual state data (JSON)
    pub state_data: serde_json::Value,

    /// Hash of the state data for integrity verification
    pub state_hash: String,

    /// Size in bytes
    pub size_bytes: u64,

    /// Whether this is a full or incremental backup
    pub backup_type: BackupType,

    /// Optional reference to parent snapshot for incremental backups
    pub parent_id: Option<Uuid>,

    /// Metadata about the backup
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl CompilerStateSnapshot {
    /// Creates a new full backup snapshot.
    pub fn new_full(state_data: serde_json::Value, state_hash: String, size_bytes: u64) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            state_data,
            state_hash,
            size_bytes,
            backup_type: BackupType::Full,
            parent_id: None,
            metadata: HashMap::new(),
        }
    }

    /// Creates a new incremental backup snapshot.
    pub fn new_incremental(
        state_data: serde_json::Value,
        state_hash: String,
        size_bytes: u64,
        parent_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            state_data,
            state_hash,
            size_bytes,
            backup_type: BackupType::Incremental,
            parent_id: Some(parent_id),
            metadata: HashMap::new(),
        }
    }

    /// Adds metadata to this snapshot.
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Type of backup strategy.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BackupType {
    /// Full backup containing complete state
    Full,

    /// Incremental backup with only changes since parent
    Incremental,
}

/// A backup chain tracking full and incremental backups.
///
/// Supports point-in-time recovery by maintaining a linked list
/// of incremental backups based on full backups.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupChain {
    /// ID of the full backup at the root of this chain
    pub full_backup_id: Uuid,

    /// Timestamp of the full backup
    pub full_backup_timestamp: DateTime<Utc>,

    /// Ordered list of incremental backup IDs (in order of creation)
    pub incremental_backups: Vec<Uuid>,

    /// Current size of all backups in this chain
    pub total_size_bytes: u64,

    /// Number of backups in the chain (1 full + N incrementals)
    pub backup_count: usize,

    /// Optional metadata
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl BackupChain {
    /// Creates a new backup chain from a full backup.
    pub fn new(
        full_backup_id: Uuid,
        full_backup_timestamp: DateTime<Utc>,
        size_bytes: u64,
    ) -> Self {
        Self {
            full_backup_id,
            full_backup_timestamp,
            incremental_backups: Vec::new(),
            total_size_bytes: size_bytes,
            backup_count: 1,
            metadata: HashMap::new(),
        }
    }

    /// Adds an incremental backup to this chain.
    pub fn add_incremental(&mut self, incremental_id: Uuid, size_bytes: u64) {
        self.incremental_backups.push(incremental_id);
        self.total_size_bytes += size_bytes;
        self.backup_count += 1;
    }

    /// Returns the latest backup ID in the chain.
    pub fn latest_backup_id(&self) -> Uuid {
        self.incremental_backups
            .last()
            .copied()
            .unwrap_or(self.full_backup_id)
    }
}

/// A point-in-time recovery request.
///
/// Specifies which snapshot in the backup chain to restore to.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryRequest {
    /// Target snapshot ID to restore to
    pub target_snapshot_id: Uuid,

    /// Optional target timestamp for recovery
    /// If specified, the closest snapshot before this time will be used
    pub target_timestamp: Option<DateTime<Utc>>,

    /// Whether to verify restored state after recovery
    #[serde(default = "bool::default")]
    pub verify: bool,

    /// Optional metadata for recovery tracking
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Result of a recovery operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryResult {
    /// Unique ID for this recovery operation
    pub recovery_id: Uuid,

    /// ID of the snapshot that was restored
    pub source_snapshot_id: Uuid,

    /// Timestamp of the recovered state
    pub state_timestamp: DateTime<Utc>,

    /// Timestamp when recovery was completed
    pub recovery_timestamp: DateTime<Utc>,

    /// Whether verification passed
    pub verified: bool,

    /// Optional verification report
    pub verification_report: Option<String>,

    /// Size of recovered state
    pub state_size_bytes: u64,

    /// Optional metadata
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Backup statistics and metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupStats {
    /// Total number of backups
    pub total_backups: usize,

    /// Number of full backups
    pub full_backup_count: usize,

    /// Number of incremental backups
    pub incremental_backup_count: usize,

    /// Total storage size of all backups
    pub total_storage_bytes: u64,

    /// Timestamp of oldest backup
    pub oldest_backup_timestamp: Option<DateTime<Utc>>,

    /// Timestamp of newest backup
    pub newest_backup_timestamp: Option<DateTime<Utc>>,

    /// Average backup size
    pub average_backup_size: u64,

    /// Timestamp of last successful recovery
    pub last_recovery_timestamp: Option<DateTime<Utc>>,
}

/// Backup verification result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationResult {
    /// Unique verification ID
    pub verification_id: Uuid,

    /// ID of the snapshot being verified
    pub snapshot_id: Uuid,

    /// Whether the backup is valid
    pub is_valid: bool,

    /// Timestamp of verification
    pub verified_at: DateTime<Utc>,

    /// Hash of the snapshot data for integrity check
    pub computed_hash: String,

    /// Original hash from the snapshot metadata
    pub stored_hash: String,

    /// Hash verification result
    pub hash_matches: bool,

    /// Optional error details if verification failed
    pub error: Option<String>,

    /// Optional verification notes
    pub notes: Option<String>,
}

/// Configuration for backup rotation policies.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRotationPolicy {
    /// Maximum age in seconds for a backup before it can be deleted
    pub max_backup_age_secs: u64,

    /// Maximum total backup storage in bytes
    pub max_total_storage_bytes: u64,

    /// Minimum number of full backups to retain
    pub min_full_backups: usize,

    /// Minimum number of backup chains to maintain
    pub min_backup_chains: usize,

    /// Whether to automatically create new full backups periodically
    pub auto_full_backup_interval_secs: Option<u64>,
}

impl Default for BackupRotationPolicy {
    fn default() -> Self {
        Self {
            max_backup_age_secs: 30 * 24 * 60 * 60,            // 30 days
            max_total_storage_bytes: 100 * 1024 * 1024 * 1024, // 100 GB
            min_full_backups: 3,
            min_backup_chains: 2,
            auto_full_backup_interval_secs: Some(7 * 24 * 60 * 60), // 7 days
        }
    }
}

/// Error types for backup operations.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum BackupError {
    #[error("Backup failed: {0}")]
    BackupFailed(String),

    #[error("Snapshot not found: {0}")]
    SnapshotNotFound(String),

    #[error("Recovery failed: {0}")]
    RecoveryFailed(String),

    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    #[error("Insufficient storage: required {required}, available {available}")]
    InsufficientStorage { required: u64, available: u64 },

    #[error("Invalid backup state: {0}")]
    InvalidBackupState(String),

    #[error("GCS error: {0}")]
    GcsError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Parent snapshot not found: {0}")]
    ParentSnapshotNotFound(String),
}
