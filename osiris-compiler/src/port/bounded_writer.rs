//! BoundedWriter port trait for CONSTRUCT8 state mutations.

use crate::domain::{Patch, PatchError, PatchSet};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "tokio-runtime")]
use async_trait::async_trait;

/// Error types for bounded write operations.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WriteError {
    /// Patch validation failed.
    #[error("Patch validation failed: {0}")]
    ValidationFailed(#[from] PatchError),

    /// Commit failed due to conflict.
    #[error("Commit conflict: {reason}")]
    ConflictError { reason: String },

    /// Backend storage error.
    #[error("Storage error: {message}")]
    StorageError { message: String },

    /// Transaction rollback occurred.
    #[error("Transaction rolled back: {reason}")]
    RollbackError { reason: String },

    /// Operation timed out.
    #[error("Operation timed out")]
    TimeoutError,
}

/// Result of a commit operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    /// Unique identifier for the committed patch set.
    pub patch_set_id: uuid::Uuid,
    /// Number of triples added.
    pub additions_count: usize,
    /// Number of triples deleted.
    pub deletions_count: usize,
    /// Timestamp of the commit.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Backend-specific commit identifier (e.g., Firestore write ID).
    pub backend_commit_id: Option<String>,
}

impl CommitResult {
    /// Creates a new commit result.
    pub fn new(patch_set_id: uuid::Uuid, additions_count: usize, deletions_count: usize) -> Self {
        Self {
            patch_set_id,
            additions_count,
            deletions_count,
            timestamp: chrono::Utc::now(),
            backend_commit_id: None,
        }
    }

    /// Sets the backend commit identifier.
    pub fn with_backend_commit_id(mut self, id: String) -> Self {
        self.backend_commit_id = Some(id);
        self
    }
}

/// Port trait for bounded RDF state writers.
///
/// Implementations must:
/// - Enforce the 8-unit mutation limit per commit
/// - Provide atomic commit semantics (all-or-nothing)
/// - Track mutation size before execution
/// - Support SPARQL CONSTRUCT semantics for state updates
#[cfg(feature = "tokio-runtime")]
#[async_trait]
pub trait BoundedWriter: Send + Sync {
    /// Commits a single patch atomically.
    ///
    /// # Errors
    ///
    /// Returns `WriteError` if:
    /// - Patch validation fails (exceeds limit, empty, invalid triples)
    /// - Backend storage error occurs
    /// - Commit conflict detected
    /// - Transaction rollback required
    async fn commit_patch(&self, patch: Patch) -> Result<CommitResult, WriteError>;

    /// Commits a patch set atomically.
    ///
    /// All patches in the set are applied as a single transaction.
    /// If any patch fails, the entire set is rolled back.
    ///
    /// # Errors
    ///
    /// Returns `WriteError` if any patch fails validation or commit.
    async fn commit_patch_set(&self, patch_set: PatchSet) -> Result<CommitResult, WriteError>;

    /// Validates a patch without committing it.
    ///
    /// Checks that the patch:
    /// - Is not empty
    /// - Does not exceed the 8-unit limit
    /// - Contains valid triples
    ///
    /// # Errors
    ///
    /// Returns `WriteError::ValidationFailed` if validation fails.
    async fn validate_patch(&self, patch: &Patch) -> Result<(), WriteError>;

    /// Returns the maximum mutation units allowed per commit.
    fn max_mutation_units(&self) -> usize;
}

/// Synchronous version of BoundedWriter for non-async contexts.
#[cfg(not(feature = "tokio-runtime"))]
pub trait BoundedWriter: Send + Sync {
    /// Commits a single patch atomically.
    fn commit_patch(&self, patch: Patch) -> Result<CommitResult, WriteError>;

    /// Commits a patch set atomically.
    fn commit_patch_set(&self, patch_set: PatchSet) -> Result<CommitResult, WriteError>;

    /// Validates a patch without committing it.
    fn validate_patch(&self, patch: &Patch) -> Result<(), WriteError>;

    /// Returns the maximum mutation units allowed per commit.
    fn max_mutation_units(&self) -> usize;
}
