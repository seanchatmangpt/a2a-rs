//! Artifact management system for immutable, content-addressed storage.
//!
//! This module provides a "parts" system where artifacts are treated as
//! immutable produced parts that can be appended but never modified.
//! Storage is content-addressed using SHA256 hashing for deduplication
//! and integrity verification.

use crate::domain::{Artifact, Part};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[cfg(feature = "receipts")]
use sha2::{Digest, Sha256};

/// Errors that can occur during artifact storage operations.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactStoreError {
    #[error("Artifact not found: {artifact_id}")]
    ArtifactNotFound { artifact_id: String },

    #[error("Task not found: {task_id}")]
    TaskNotFound { task_id: String },

    #[error("Content hash mismatch for artifact {artifact_id}")]
    ContentHashMismatch { artifact_id: String },

    #[error("Artifact {artifact_id} is already committed and cannot be modified")]
    ArtifactAlreadyCommitted { artifact_id: String },

    #[error("Task {task_id} is already finalized")]
    TaskAlreadyFinalized { task_id: String },

    #[error("Storage error: {message}")]
    StorageError { message: String },
}

/// Content hash for content-addressed storage.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentHash(String);

impl ContentHash {
    /// Create a new content hash from a string.
    pub fn new(hash: String) -> Self {
        Self(hash)
    }

    /// Get the hash value as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Compute the content hash of an artifact.
    #[cfg(feature = "receipts")]
    pub fn from_artifact(artifact: &Artifact) -> Result<Self, ArtifactStoreError> {
        let json =
            serde_json::to_string(artifact).map_err(|e| ArtifactStoreError::StorageError {
                message: format!("Failed to serialize artifact: {}", e),
            })?;

        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        let result = hasher.finalize();
        Ok(Self(hex::encode(result)))
    }

    /// Compute the content hash of an artifact (fallback without receipts feature).
    #[cfg(not(feature = "receipts"))]
    pub fn from_artifact(artifact: &Artifact) -> Result<Self, ArtifactStoreError> {
        // Fallback: use artifact_id as hash
        Ok(Self(artifact.artifact_id.clone()))
    }

    /// Compute the content hash of a part.
    #[cfg(feature = "receipts")]
    pub fn from_part(part: &Part) -> Result<Self, ArtifactStoreError> {
        let json = serde_json::to_string(part).map_err(|e| ArtifactStoreError::StorageError {
            message: format!("Failed to serialize part: {}", e),
        })?;

        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        let result = hasher.finalize();
        Ok(Self(hex::encode(result)))
    }

    /// Compute the content hash of a part (fallback without receipts feature).
    #[cfg(not(feature = "receipts"))]
    pub fn from_part(part: &Part) -> Result<Self, ArtifactStoreError> {
        // Fallback: use a simple string representation
        let json = serde_json::to_string(part).map_err(|e| ArtifactStoreError::StorageError {
            message: format!("Failed to serialize part: {}", e),
        })?;
        Ok(Self(json))
    }
}

/// Stored artifact with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredArtifact {
    /// The artifact itself.
    pub artifact: Artifact,

    /// Content hash for content-addressed retrieval.
    pub content_hash: ContentHash,

    /// Whether this artifact has been committed (finalized).
    pub committed: bool,

    /// Task ID this artifact belongs to.
    pub task_id: String,

    /// Timestamp when the artifact was stored.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub stored_at: chrono::DateTime<chrono::Utc>,
}

/// Task artifact collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifacts {
    /// Task ID.
    pub task_id: String,

    /// All artifacts for this task.
    pub artifacts: Vec<StoredArtifact>,

    /// Whether the task artifacts have been finalized.
    pub finalized: bool,

    /// Timestamp when finalized (if finalized).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finalized_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Trait for artifact storage operations.
///
/// Implementations must ensure immutability: once an artifact is committed,
/// it cannot be modified. All operations are append-only.
pub trait ArtifactStore: Send + Sync {
    /// Append an artifact to storage (not yet committed).
    ///
    /// The artifact is stored in a pending state and can be committed later.
    /// Returns the content hash of the stored artifact.
    fn append(&self, task_id: &str, artifact: Artifact) -> Result<ContentHash, ArtifactStoreError>;

    /// Commit an artifact by its ID, making it immutable.
    ///
    /// Once committed, the artifact cannot be modified or removed.
    fn commit(&self, task_id: &str, artifact_id: &str) -> Result<(), ArtifactStoreError>;

    /// Commit all artifacts for a task, finalizing the task's artifact collection.
    ///
    /// After this operation, no more artifacts can be added to the task.
    fn commit_task(&self, task_id: &str) -> Result<(), ArtifactStoreError>;

    /// Retrieve an artifact by its content hash.
    fn get_by_hash(&self, hash: &ContentHash) -> Result<StoredArtifact, ArtifactStoreError>;

    /// Retrieve an artifact by task ID and artifact ID.
    fn get_by_id(
        &self,
        task_id: &str,
        artifact_id: &str,
    ) -> Result<StoredArtifact, ArtifactStoreError>;

    /// List all artifacts for a task.
    fn list_by_task(&self, task_id: &str) -> Result<Vec<StoredArtifact>, ArtifactStoreError>;

    /// Get the task artifacts collection.
    fn get_task_artifacts(&self, task_id: &str) -> Result<TaskArtifacts, ArtifactStoreError>;
}

/// In-memory implementation of ArtifactStore.
///
/// Suitable for testing and development. For production use,
/// consider a persistent storage backend.
#[derive(Debug, Clone)]
pub struct InMemoryArtifactStore {
    /// Storage by content hash.
    by_hash: Arc<RwLock<HashMap<ContentHash, StoredArtifact>>>,

    /// Storage by task ID.
    by_task: Arc<RwLock<HashMap<String, TaskArtifacts>>>,

    /// Index from (task_id, artifact_id) to content hash.
    index: Arc<RwLock<HashMap<(String, String), ContentHash>>>,
}

impl InMemoryArtifactStore {
    /// Create a new in-memory artifact store.
    pub fn new() -> Self {
        Self {
            by_hash: Arc::new(RwLock::new(HashMap::new())),
            by_task: Arc::new(RwLock::new(HashMap::new())),
            index: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryArtifactStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtifactStore for InMemoryArtifactStore {
    fn append(&self, task_id: &str, artifact: Artifact) -> Result<ContentHash, ArtifactStoreError> {
        // Check if task is finalized
        {
            let by_task = self
                .by_task
                .read()
                .map_err(|e| ArtifactStoreError::StorageError {
                    message: format!("Lock poisoned: {}", e),
                })?;

            if let Some(task_artifacts) = by_task.get(task_id) {
                if task_artifacts.finalized {
                    return Err(ArtifactStoreError::TaskAlreadyFinalized {
                        task_id: task_id.to_string(),
                    });
                }
            }
        }

        // Compute content hash
        let content_hash = ContentHash::from_artifact(&artifact)?;

        // Create stored artifact
        let stored = StoredArtifact {
            artifact: artifact.clone(),
            content_hash: content_hash.clone(),
            committed: false,
            task_id: task_id.to_string(),
            stored_at: chrono::Utc::now(),
        };

        // Store by hash
        {
            let mut by_hash =
                self.by_hash
                    .write()
                    .map_err(|e| ArtifactStoreError::StorageError {
                        message: format!("Lock poisoned: {}", e),
                    })?;
            by_hash.insert(content_hash.clone(), stored.clone());
        }

        // Store in task collection
        {
            let mut by_task =
                self.by_task
                    .write()
                    .map_err(|e| ArtifactStoreError::StorageError {
                        message: format!("Lock poisoned: {}", e),
                    })?;

            by_task
                .entry(task_id.to_string())
                .or_insert_with(|| TaskArtifacts {
                    task_id: task_id.to_string(),
                    artifacts: Vec::new(),
                    finalized: false,
                    finalized_at: None,
                })
                .artifacts
                .push(stored);
        }

        // Update index
        {
            let mut index = self
                .index
                .write()
                .map_err(|e| ArtifactStoreError::StorageError {
                    message: format!("Lock poisoned: {}", e),
                })?;
            index.insert(
                (task_id.to_string(), artifact.artifact_id.clone()),
                content_hash.clone(),
            );
        }

        Ok(content_hash)
    }

    fn commit(&self, task_id: &str, artifact_id: &str) -> Result<(), ArtifactStoreError> {
        // Get content hash from index
        let hash = {
            let index = self
                .index
                .read()
                .map_err(|e| ArtifactStoreError::StorageError {
                    message: format!("Lock poisoned: {}", e),
                })?;

            index
                .get(&(task_id.to_string(), artifact_id.to_string()))
                .ok_or_else(|| ArtifactStoreError::ArtifactNotFound {
                    artifact_id: artifact_id.to_string(),
                })?
                .clone()
        };

        // Mark as committed in hash storage
        {
            let mut by_hash =
                self.by_hash
                    .write()
                    .map_err(|e| ArtifactStoreError::StorageError {
                        message: format!("Lock poisoned: {}", e),
                    })?;

            if let Some(stored) = by_hash.get_mut(&hash) {
                if stored.committed {
                    return Err(ArtifactStoreError::ArtifactAlreadyCommitted {
                        artifact_id: artifact_id.to_string(),
                    });
                }
                stored.committed = true;
            } else {
                return Err(ArtifactStoreError::ArtifactNotFound {
                    artifact_id: artifact_id.to_string(),
                });
            }
        }

        // Mark as committed in task storage
        {
            let mut by_task =
                self.by_task
                    .write()
                    .map_err(|e| ArtifactStoreError::StorageError {
                        message: format!("Lock poisoned: {}", e),
                    })?;

            if let Some(task_artifacts) = by_task.get_mut(task_id) {
                for stored in &mut task_artifacts.artifacts {
                    if stored.artifact.artifact_id == artifact_id {
                        stored.committed = true;
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn commit_task(&self, task_id: &str) -> Result<(), ArtifactStoreError> {
        let mut by_task = self
            .by_task
            .write()
            .map_err(|e| ArtifactStoreError::StorageError {
                message: format!("Lock poisoned: {}", e),
            })?;

        let task_artifacts =
            by_task
                .get_mut(task_id)
                .ok_or_else(|| ArtifactStoreError::TaskNotFound {
                    task_id: task_id.to_string(),
                })?;

        if task_artifacts.finalized {
            return Err(ArtifactStoreError::TaskAlreadyFinalized {
                task_id: task_id.to_string(),
            });
        }

        // Commit all artifacts and finalize the task
        for artifact in &mut task_artifacts.artifacts {
            artifact.committed = true;
        }

        task_artifacts.finalized = true;
        task_artifacts.finalized_at = Some(chrono::Utc::now());

        Ok(())
    }

    fn get_by_hash(&self, hash: &ContentHash) -> Result<StoredArtifact, ArtifactStoreError> {
        let by_hash = self
            .by_hash
            .read()
            .map_err(|e| ArtifactStoreError::StorageError {
                message: format!("Lock poisoned: {}", e),
            })?;

        by_hash
            .get(hash)
            .cloned()
            .ok_or_else(|| ArtifactStoreError::ArtifactNotFound {
                artifact_id: hash.as_str().to_string(),
            })
    }

    fn get_by_id(
        &self,
        task_id: &str,
        artifact_id: &str,
    ) -> Result<StoredArtifact, ArtifactStoreError> {
        // Get content hash from index
        let hash = {
            let index = self
                .index
                .read()
                .map_err(|e| ArtifactStoreError::StorageError {
                    message: format!("Lock poisoned: {}", e),
                })?;

            index
                .get(&(task_id.to_string(), artifact_id.to_string()))
                .ok_or_else(|| ArtifactStoreError::ArtifactNotFound {
                    artifact_id: artifact_id.to_string(),
                })?
                .clone()
        };

        // Get artifact by hash
        self.get_by_hash(&hash)
    }

    fn list_by_task(&self, task_id: &str) -> Result<Vec<StoredArtifact>, ArtifactStoreError> {
        let by_task = self
            .by_task
            .read()
            .map_err(|e| ArtifactStoreError::StorageError {
                message: format!("Lock poisoned: {}", e),
            })?;

        by_task
            .get(task_id)
            .map(|task_artifacts| task_artifacts.artifacts.clone())
            .ok_or_else(|| ArtifactStoreError::TaskNotFound {
                task_id: task_id.to_string(),
            })
    }

    fn get_task_artifacts(&self, task_id: &str) -> Result<TaskArtifacts, ArtifactStoreError> {
        let by_task = self
            .by_task
            .read()
            .map_err(|e| ArtifactStoreError::StorageError {
                message: format!("Lock poisoned: {}", e),
            })?;

        by_task
            .get(task_id)
            .cloned()
            .ok_or_else(|| ArtifactStoreError::TaskNotFound {
                task_id: task_id.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Part;

    fn create_test_artifact(id: &str, parts: Vec<Part>) -> Artifact {
        Artifact {
            artifact_id: id.to_string(),
            name: Some(format!("Test Artifact {}", id)),
            description: Some("A test artifact".to_string()),
            parts,
            metadata: None,
            extensions: None,
        }
    }

    #[test]
    fn test_append_and_retrieve() {
        let store = InMemoryArtifactStore::new();
        let artifact = create_test_artifact("art-1", vec![Part::text("Hello".to_string())]);

        // Append artifact
        let hash = store.append("task-1", artifact.clone()).unwrap();

        // Retrieve by hash
        let stored = store.get_by_hash(&hash).unwrap();
        assert_eq!(stored.artifact.artifact_id, "art-1");
        assert_eq!(stored.task_id, "task-1");
        assert!(!stored.committed);

        // Retrieve by ID
        let stored = store.get_by_id("task-1", "art-1").unwrap();
        assert_eq!(stored.artifact.artifact_id, "art-1");
    }

    #[test]
    fn test_commit_artifact() {
        let store = InMemoryArtifactStore::new();
        let artifact = create_test_artifact("art-1", vec![Part::text("Hello".to_string())]);

        // Append and commit
        store.append("task-1", artifact).unwrap();
        store.commit("task-1", "art-1").unwrap();

        // Verify committed
        let stored = store.get_by_id("task-1", "art-1").unwrap();
        assert!(stored.committed);

        // Try to commit again - should error
        let result = store.commit("task-1", "art-1");
        assert!(matches!(
            result,
            Err(ArtifactStoreError::ArtifactAlreadyCommitted { .. })
        ));
    }

    #[test]
    fn test_commit_task() {
        let store = InMemoryArtifactStore::new();

        // Append multiple artifacts
        store
            .append(
                "task-1",
                create_test_artifact("art-1", vec![Part::text("One".to_string())]),
            )
            .unwrap();
        store
            .append(
                "task-1",
                create_test_artifact("art-2", vec![Part::text("Two".to_string())]),
            )
            .unwrap();

        // Commit task
        store.commit_task("task-1").unwrap();

        // Verify all artifacts are committed
        let artifacts = store.list_by_task("task-1").unwrap();
        assert_eq!(artifacts.len(), 2);
        assert!(artifacts.iter().all(|a| a.committed));

        // Verify task is finalized
        let task_artifacts = store.get_task_artifacts("task-1").unwrap();
        assert!(task_artifacts.finalized);
        assert!(task_artifacts.finalized_at.is_some());

        // Try to append after finalization - should error
        let result = store.append(
            "task-1",
            create_test_artifact("art-3", vec![Part::text("Three".to_string())]),
        );
        assert!(matches!(
            result,
            Err(ArtifactStoreError::TaskAlreadyFinalized { .. })
        ));
    }

    #[test]
    fn test_list_by_task() {
        let store = InMemoryArtifactStore::new();

        // Append multiple artifacts to different tasks
        store
            .append(
                "task-1",
                create_test_artifact("art-1", vec![Part::text("One".to_string())]),
            )
            .unwrap();
        store
            .append(
                "task-1",
                create_test_artifact("art-2", vec![Part::text("Two".to_string())]),
            )
            .unwrap();
        store
            .append(
                "task-2",
                create_test_artifact("art-3", vec![Part::text("Three".to_string())]),
            )
            .unwrap();

        // List task-1 artifacts
        let artifacts = store.list_by_task("task-1").unwrap();
        assert_eq!(artifacts.len(), 2);
        assert_eq!(artifacts[0].artifact.artifact_id, "art-1");
        assert_eq!(artifacts[1].artifact.artifact_id, "art-2");

        // List task-2 artifacts
        let artifacts = store.list_by_task("task-2").unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].artifact.artifact_id, "art-3");
    }

    #[test]
    fn test_content_hash() {
        let artifact1 = create_test_artifact("art-1", vec![Part::text("Hello".to_string())]);
        let artifact2 = create_test_artifact("art-1", vec![Part::text("Hello".to_string())]);
        let artifact3 = create_test_artifact("art-1", vec![Part::text("World".to_string())]);

        let hash1 = ContentHash::from_artifact(&artifact1).unwrap();
        let hash2 = ContentHash::from_artifact(&artifact2).unwrap();
        let hash3 = ContentHash::from_artifact(&artifact3).unwrap();

        // Same content should produce same hash
        assert_eq!(hash1, hash2);

        // Different content should produce different hash
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_immutability() {
        let store = InMemoryArtifactStore::new();
        let artifact = create_test_artifact("art-1", vec![Part::text("Hello".to_string())]);

        // Append and commit
        let hash1 = store.append("task-1", artifact.clone()).unwrap();
        store.commit("task-1", "art-1").unwrap();

        // Try to append same artifact again to same task (different instance)
        let artifact2 = create_test_artifact("art-1", vec![Part::text("Hello".to_string())]);
        let hash2 = store.append("task-2", artifact2).unwrap();

        // Hashes should be equal (content-addressed)
        assert_eq!(hash1, hash2);
    }
}
