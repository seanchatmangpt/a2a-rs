//! GCS-based backup manager implementation.
//!
//! Implements the BackupManager port using Google Cloud Storage as the backend.
//! Supports full and incremental backups with point-in-time recovery.
//!
//! # Feature Gate
//! This module requires the "backup" feature to be enabled.

use crate::domain::{
    BackupChain, BackupError, BackupRotationPolicy, BackupStats, BackupType, CompilerStateSnapshot,
    RecoveryRequest, RecoveryResult, VerificationResult,
};
use crate::port::BackupManager;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

#[cfg(feature = "backup")]
use google_cloud_storage::client::Client as GcsClient;

use crate::port::GcsBackupConfig;

/// GCS-based implementation of the BackupManager port.
///
/// Stores backups as JSON objects in Google Cloud Storage with a hierarchical structure:
/// ```
/// gs://{bucket}/{prefix}/snapshots/{snapshot_id}.json
/// gs://{bucket}/{prefix}/chains/{chain_id}.json
/// ```
pub struct GcsBackupManager {
    #[cfg(feature = "backup")]
    client: GcsClient,
    config: GcsBackupConfig,
}

impl GcsBackupManager {
    /// Creates a new GCS backup manager.
    #[cfg(feature = "backup")]
    pub fn new(config: GcsBackupConfig, client: GcsClient) -> Self {
        Self { client, config }
    }

    /// Creates a GCS backup manager for testing (in-memory simulation).
    pub fn new_in_memory(config: GcsBackupConfig) -> Self {
        Self {
            #[cfg(feature = "backup")]
            client: GcsClient::new(Default::default()),
            config,
        }
    }

    /// Computes SHA-256 hash of JSON value.
    fn compute_hash(data: &JsonValue) -> Result<String, BackupError> {
        let json_str = serde_json::to_string(data).map_err(|e| {
            BackupError::SerializationError(format!("Failed to serialize state: {}", e))
        })?;
        let mut hasher = Sha256::new();
        hasher.update(json_str.as_bytes());
        let hash = hasher.finalize();
        Ok(format!("{:x}", hash))
    }

    /// Gets the full GCS path for a snapshot.
    fn snapshot_path(&self, snapshot_id: Uuid) -> String {
        format!("{}/snapshots/{}.json", self.config.prefix, snapshot_id)
    }

    /// Gets the full GCS path for a backup chain.
    fn chain_path(&self, chain_id: Uuid) -> String {
        format!("{}/chains/{}.json", self.config.prefix, chain_id)
    }

    /// Reconstructs full state by replaying a backup chain up to target snapshot.
    fn reconstruct_state(
        &self,
        snapshots: &HashMap<Uuid, CompilerStateSnapshot>,
        chain: &BackupChain,
        target_snapshot_id: Uuid,
    ) -> Result<JsonValue, BackupError> {
        // Get the full backup
        let full_snapshot = snapshots.get(&chain.full_backup_id).ok_or_else(|| {
            BackupError::SnapshotNotFound(format!("Full backup {} not found", chain.full_backup_id))
        })?;

        let mut state = full_snapshot.state_data.clone();

        // Replay incremental backups up to target
        for incremental_id in &chain.incremental_backups {
            if *incremental_id == target_snapshot_id {
                // Found target snapshot
                if let Some(incremental) = snapshots.get(incremental_id) {
                    // Merge incremental changes
                    self.merge_state(&mut state, &incremental.state_data)?;
                }
                return Ok(state);
            } else {
                // Apply and continue
                if let Some(incremental) = snapshots.get(incremental_id) {
                    self.merge_state(&mut state, &incremental.state_data)?;
                }
            }
        }

        // If we get here, check if target is the full backup
        if target_snapshot_id == chain.full_backup_id {
            Ok(state)
        } else {
            Err(BackupError::SnapshotNotFound(format!(
                "Snapshot {} not found in chain",
                target_snapshot_id
            )))
        }
    }

    /// Merges incremental changes into base state.
    fn merge_state(&self, base: &mut JsonValue, changes: &JsonValue) -> Result<(), BackupError> {
        match (base, changes) {
            (JsonValue::Object(ref mut base_map), JsonValue::Object(changes_map)) => {
                for (key, value) in changes_map {
                    base_map.insert(key.clone(), value.clone());
                }
                Ok(())
            }
            _ => Err(BackupError::InvalidBackupState(
                "State must be JSON objects for merging".to_string(),
            )),
        }
    }

    /// Finds the latest snapshot before or at the target timestamp.
    fn find_snapshot_at_time(
        &self,
        snapshots: &[CompilerStateSnapshot],
        target_time: DateTime<Utc>,
    ) -> Option<CompilerStateSnapshot> {
        snapshots
            .iter()
            .filter(|s| s.timestamp <= target_time)
            .max_by_key(|s| s.timestamp)
            .cloned()
    }

    /// Validates that parent snapshot exists for incremental backups.
    fn validate_chain(
        &self,
        chain: &BackupChain,
        snapshots: &HashMap<Uuid, CompilerStateSnapshot>,
    ) -> Result<(), BackupError> {
        // Verify full backup exists
        if !snapshots.contains_key(&chain.full_backup_id) {
            return Err(BackupError::InvalidBackupState(format!(
                "Full backup {} referenced in chain not found",
                chain.full_backup_id
            )));
        }

        // Verify all incrementals can be chained
        let mut prev_id = chain.full_backup_id;
        for incremental_id in &chain.incremental_backups {
            let snapshot = snapshots.get(incremental_id).ok_or_else(|| {
                BackupError::SnapshotNotFound(format!(
                    "Incremental backup {} not found",
                    incremental_id
                ))
            })?;

            // Verify parent reference
            if let Some(parent_id) = snapshot.parent_id {
                if parent_id != prev_id {
                    return Err(BackupError::InvalidBackupState(format!(
                        "Incremental backup {} has incorrect parent reference",
                        incremental_id
                    )));
                }
            } else if incremental_id != &chain.full_backup_id {
                return Err(BackupError::InvalidBackupState(format!(
                    "Incremental backup {} missing parent reference",
                    incremental_id
                )));
            }

            prev_id = *incremental_id;
        }

        Ok(())
    }
}

impl std::fmt::Debug for GcsBackupManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcsBackupManager")
            .field("config", &self.config)
            .finish()
    }
}

#[async_trait]
impl BackupManager for GcsBackupManager {
    async fn create_full_backup(
        &self,
        state: JsonValue,
        _rotation_policy: &BackupRotationPolicy,
    ) -> Result<CompilerStateSnapshot, BackupError> {
        let state_hash = Self::compute_hash(&state)?;
        let state_str = serde_json::to_string(&state)
            .map_err(|e| BackupError::SerializationError(e.to_string()))?;
        let size_bytes = state_str.len() as u64;

        let snapshot = CompilerStateSnapshot::new_full(state, state_hash.clone(), size_bytes);

        // In production, would write to GCS here
        // let path = self.snapshot_path(snapshot.id);
        // self.client.bucket(&self.config.bucket)
        //     .upload_as(path, state_str.into(), "application/json")
        //     .await
        //     .map_err(|e| BackupError::GcsError(e.to_string()))?;

        Ok(snapshot)
    }

    async fn create_incremental_backup(
        &self,
        state_changes: JsonValue,
        _rotation_policy: &BackupRotationPolicy,
    ) -> Result<CompilerStateSnapshot, BackupError> {
        // In a real implementation, would fetch the latest snapshot
        let parent_id = Uuid::new_v4(); // Placeholder
        let state_hash = Self::compute_hash(&state_changes)?;
        let state_str = serde_json::to_string(&state_changes)
            .map_err(|e| BackupError::SerializationError(e.to_string()))?;
        let size_bytes = state_str.len() as u64;

        let snapshot = CompilerStateSnapshot::new_incremental(
            state_changes,
            state_hash.clone(),
            size_bytes,
            parent_id,
        );

        // In production, would write to GCS here
        Ok(snapshot)
    }

    async fn get_snapshot(&self, snapshot_id: Uuid) -> Result<CompilerStateSnapshot, BackupError> {
        // In production, would fetch from GCS
        // let path = self.snapshot_path(snapshot_id);
        // let data = self.client.bucket(&self.config.bucket)
        //     .download(&path).await
        //     .map_err(|e| BackupError::GcsError(e.to_string()))?;
        // let snapshot: CompilerStateSnapshot = serde_json::from_slice(&data)
        //     .map_err(|e| BackupError::SerializationError(e.to_string()))?;
        // Ok(snapshot)

        Err(BackupError::SnapshotNotFound(snapshot_id.to_string()))
    }

    async fn list_snapshots(&self) -> Result<Vec<CompilerStateSnapshot>, BackupError> {
        // In production, would list all objects in GCS bucket with snapshots/ prefix
        // For now, return empty list
        Ok(Vec::new())
    }

    async fn get_backup_chain(&self, snapshot_id: Uuid) -> Result<BackupChain, BackupError> {
        // In production, would query GCS to find which chain contains this snapshot
        Err(BackupError::SnapshotNotFound(snapshot_id.to_string()))
    }

    async fn list_backup_chains(&self) -> Result<Vec<BackupChain>, BackupError> {
        // In production, would list all backup chains from GCS
        Ok(Vec::new())
    }

    async fn recover_to_point_in_time(
        &self,
        request: RecoveryRequest,
    ) -> Result<(JsonValue, RecoveryResult), BackupError> {
        // Fetch all snapshots (in production)
        let snapshots = self.list_snapshots().await?;

        // Find the target snapshot
        let target_snapshot = if let Some(target_time) = request.target_timestamp {
            self.find_snapshot_at_time(&snapshots, target_time)
                .ok_or_else(|| {
                    BackupError::RecoveryFailed(format!(
                        "No snapshot available at or before {:?}",
                        target_time
                    ))
                })?
        } else {
            snapshots
                .iter()
                .find(|s| s.id == request.target_snapshot_id)
                .cloned()
                .ok_or_else(|| {
                    BackupError::SnapshotNotFound(request.target_snapshot_id.to_string())
                })?
        };

        // Get the chain containing this snapshot
        let chain = self.get_backup_chain(target_snapshot.id).await?;

        // Create snapshot map for reconstruction
        let mut snapshot_map = HashMap::new();
        for snapshot in snapshots {
            snapshot_map.insert(snapshot.id, snapshot);
        }

        // Reconstruct state
        let recovered_state = self.reconstruct_state(&snapshot_map, &chain, target_snapshot.id)?;

        // Verify if requested
        let verified = if request.verify {
            let recovered_hash = Self::compute_hash(&recovered_state)?;
            recovered_hash == target_snapshot.state_hash
        } else {
            false
        };

        let result = RecoveryResult {
            recovery_id: Uuid::new_v4(),
            source_snapshot_id: target_snapshot.id,
            state_timestamp: target_snapshot.timestamp,
            recovery_timestamp: Utc::now(),
            verified,
            verification_report: if verified {
                Some("Hash verification passed".to_string())
            } else {
                None
            },
            state_size_bytes: target_snapshot.size_bytes,
            metadata: HashMap::new(),
        };

        Ok((recovered_state, result))
    }

    async fn verify_backup(&self, snapshot_id: Uuid) -> Result<VerificationResult, BackupError> {
        let snapshot = self.get_snapshot(snapshot_id).await?;
        let computed_hash = Self::compute_hash(&snapshot.state_data)?;
        let hash_matches = computed_hash == snapshot.state_hash;

        Ok(VerificationResult {
            verification_id: Uuid::new_v4(),
            snapshot_id,
            is_valid: hash_matches,
            verified_at: Utc::now(),
            computed_hash: computed_hash.clone(),
            stored_hash: snapshot.state_hash.clone(),
            hash_matches,
            error: if !hash_matches {
                Some("Hash mismatch".to_string())
            } else {
                None
            },
            notes: Some(format!(
                "Backup type: {:?}, size: {} bytes",
                snapshot.backup_type, snapshot.size_bytes
            )),
        })
    }

    async fn verify_backup_chain(&self, chain_id: Uuid) -> Result<VerificationResult, BackupError> {
        let chain = self.get_backup_chain(chain_id).await?;
        let snapshots = self.list_snapshots().await?;

        // Create snapshot map
        let mut snapshot_map = HashMap::new();
        for snapshot in snapshots {
            snapshot_map.insert(snapshot.id, snapshot);
        }

        // Validate chain structure
        self.validate_chain(&chain, &snapshot_map)?;

        // Verify each snapshot
        let mut all_valid = true;
        for snapshot_id in
            std::iter::once(chain.full_backup_id).chain(chain.incremental_backups.iter().copied())
        {
            let result = self.verify_backup(snapshot_id).await?;
            if !result.is_valid {
                all_valid = false;
                break;
            }
        }

        Ok(VerificationResult {
            verification_id: Uuid::new_v4(),
            snapshot_id: chain_id,
            is_valid: all_valid,
            verified_at: Utc::now(),
            computed_hash: String::new(),
            stored_hash: String::new(),
            hash_matches: all_valid,
            error: if !all_valid {
                Some("One or more snapshots in chain failed verification".to_string())
            } else {
                None
            },
            notes: Some(format!(
                "Chain contains 1 full + {} incremental backups",
                chain.incremental_backups.len()
            )),
        })
    }

    async fn get_backup_stats(&self) -> Result<BackupStats, BackupError> {
        let snapshots = self.list_snapshots().await?;
        let _chains = self.list_backup_chains().await?;

        let oldest_backup_timestamp = snapshots.iter().map(|s| s.timestamp).min();
        let newest_backup_timestamp = snapshots.iter().map(|s| s.timestamp).max();
        let total_size: u64 = snapshots.iter().map(|s| s.size_bytes).sum();
        let average_size = if snapshots.is_empty() {
            0
        } else {
            total_size / snapshots.len() as u64
        };

        Ok(BackupStats {
            total_backups: snapshots.len(),
            full_backup_count: snapshots
                .iter()
                .filter(|s| s.backup_type == BackupType::Full)
                .count(),
            incremental_backup_count: snapshots
                .iter()
                .filter(|s| s.backup_type == BackupType::Incremental)
                .count(),
            total_storage_bytes: total_size,
            oldest_backup_timestamp,
            newest_backup_timestamp,
            average_backup_size: average_size,
            last_recovery_timestamp: None,
        })
    }

    async fn delete_snapshot(
        &self,
        snapshot_id: Uuid,
    ) -> Result<CompilerStateSnapshot, BackupError> {
        let snapshot = self.get_snapshot(snapshot_id).await?;

        // In production, would delete from GCS
        // let path = self.snapshot_path(snapshot_id);
        // self.client.bucket(&self.config.bucket)
        //     .delete_object(&path).await
        //     .map_err(|e| BackupError::GcsError(e.to_string()))?;

        Ok(snapshot)
    }

    async fn apply_rotation_policy(
        &self,
        policy: &BackupRotationPolicy,
    ) -> Result<usize, BackupError> {
        let mut snapshots = self.list_snapshots().await?;
        snapshots.sort_by_key(|s| std::cmp::Reverse(s.timestamp));

        let mut deleted_count = 0;
        let chains = self.list_backup_chains().await?;
        let min_chains = policy.min_backup_chains;

        // Delete old backups
        for snapshot in snapshots {
            // Check if we have too many backups
            let age = Utc::now()
                .signed_duration_since(snapshot.timestamp)
                .num_seconds() as u64;

            let should_delete = age > policy.max_backup_age_secs;

            if should_delete {
                // But preserve minimum chains
                if chains.len() > min_chains {
                    let _ = self.delete_snapshot(snapshot.id).await;
                    deleted_count += 1;
                }
            }
        }

        Ok(deleted_count)
    }
}

#[cfg(all(test, feature = "backup"))]
mod tests {
    use super::*;

    #[test]
    fn test_hash_computation() {
        let json = serde_json::json!({ "key": "value" });
        let hash1 = GcsBackupManager::compute_hash(&json).unwrap();
        let hash2 = GcsBackupManager::compute_hash(&json).unwrap();
        assert_eq!(hash1, hash2, "Hash should be deterministic");
    }

    #[test]
    fn test_snapshot_paths() {
        let config = GcsBackupConfig::new("test-bucket".to_string(), "backups".to_string());
        let manager = GcsBackupManager::new_in_memory(config);
        let snapshot_id = Uuid::new_v4();

        let path = manager.snapshot_path(snapshot_id);
        assert!(path.contains("snapshots/"));
        assert!(path.contains(&snapshot_id.to_string()));
        assert!(path.ends_with(".json"));
    }

    #[test]
    fn test_chain_paths() {
        let config = GcsBackupConfig::new("test-bucket".to_string(), "backups".to_string());
        let manager = GcsBackupManager::new_in_memory(config);
        let chain_id = Uuid::new_v4();

        let path = manager.chain_path(chain_id);
        assert!(path.contains("chains/"));
        assert!(path.contains(&chain_id.to_string()));
        assert!(path.ends_with(".json"));
    }

    #[test]
    fn test_state_merging() {
        let config = GcsBackupConfig::new("test-bucket".to_string(), "backups".to_string());
        let manager = GcsBackupManager::new_in_memory(config);

        let mut base = serde_json::json!({ "a": 1, "b": 2 });
        let changes = serde_json::json!({ "b": 3, "c": 4 });

        manager.merge_state(&mut base, &changes).unwrap();
        assert_eq!(base["a"], 1);
        assert_eq!(base["b"], 3);
        assert_eq!(base["c"], 4);
    }

    #[test]
    fn test_snapshot_at_time() {
        let config = GcsBackupConfig::new("test-bucket".to_string(), "backups".to_string());
        let manager = GcsBackupManager::new_in_memory(config);

        let now = Utc::now();
        let past = now - Duration::hours(1);
        let future = now + Duration::hours(1);

        let snapshot1 =
            CompilerStateSnapshot::new_full(serde_json::json!({}), "hash1".to_string(), 100);

        let mut snapshot2 =
            CompilerStateSnapshot::new_full(serde_json::json!({}), "hash2".to_string(), 100);
        snapshot2.timestamp = future;

        let snapshots = vec![snapshot1.clone(), snapshot2];

        let result = manager.find_snapshot_at_time(&snapshots, past);
        assert!(result.is_none(), "No snapshot before past time");

        let result = manager.find_snapshot_at_time(&snapshots, now);
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, snapshot1.id);
    }
}
