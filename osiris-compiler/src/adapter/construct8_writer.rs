//! CONSTRUCT8 bounded writer with pluggable storage backends.

use crate::domain::{Patch, PatchSet, Triple, MAX_MUTATION_UNITS};
use crate::port::{BoundedWriter, CommitResult, WriteError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "tracing")]
use tracing::{debug, error, info, warn};

/// Storage backend trait for CONSTRUCT8 writer.
///
/// Implementations provide actual persistence layer (Firestore, Spanner, etc.).
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Begins a new transaction.
    async fn begin_transaction(&self) -> Result<Box<dyn Transaction>, WriteError>;

    /// Returns the backend name for logging/debugging.
    fn backend_name(&self) -> &str;
}

/// Transaction trait for atomic commits.
///
/// All operations within a transaction are committed or rolled back atomically.
#[async_trait]
pub trait Transaction: Send + Sync {
    /// Adds triples to the transaction.
    async fn add_triples(&mut self, triples: &[Triple]) -> Result<(), WriteError>;

    /// Deletes triples from the transaction.
    async fn delete_triples(&mut self, triples: &[Triple]) -> Result<(), WriteError>;

    /// Commits the transaction.
    async fn commit(self: Box<Self>) -> Result<String, WriteError>;

    /// Rolls back the transaction.
    async fn rollback(self: Box<Self>) -> Result<(), WriteError>;
}

/// CONSTRUCT8 bounded writer implementation.
///
/// This writer:
/// - Enforces the 8-unit mutation limit per commit
/// - Provides atomic commit semantics via storage backend transactions
/// - Tracks mutation size before execution
/// - Supports SPARQL CONSTRUCT semantics for RDF state updates
///
/// # Backend Integration
///
/// The writer is backend-agnostic. Implement the `StorageBackend` trait
/// to integrate with Firestore, Spanner, or other databases.
///
/// # Example
///
/// ```rust,ignore
/// use osiris_compiler::adapter::Construct8Writer;
/// use osiris_compiler::domain::Patch;
///
/// let backend = MyStorageBackend::new();
/// let writer = Construct8Writer::new(backend);
///
/// let patch = Patch::with_additions(vec![
///     Triple::new("subject", "predicate", "object"),
/// ]);
///
/// let result = writer.commit_patch(patch).await?;
/// ```
#[derive(Clone)]
pub struct Construct8Writer {
    backend: Arc<dyn StorageBackend>,
}

impl Construct8Writer {
    /// Creates a new CONSTRUCT8 writer with the given storage backend.
    pub fn new(backend: impl StorageBackend + 'static) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    /// Returns the backend name.
    pub fn backend_name(&self) -> &str {
        self.backend.backend_name()
    }
}

#[async_trait]
impl BoundedWriter for Construct8Writer {
    async fn commit_patch(&self, patch: Patch) -> Result<CommitResult, WriteError> {
        #[cfg(feature = "tracing")]
        debug!(
            additions = patch.additions.len(),
            deletions = patch.deletions.len(),
            "Starting patch commit"
        );

        // Validate the patch first
        patch.validate()?;

        #[cfg(feature = "tracing")]
        info!(
            mutation_count = patch.mutation_count(),
            max_allowed = MAX_MUTATION_UNITS,
            "Patch validated"
        );

        // Begin transaction
        let mut tx = self.backend.begin_transaction().await?;

        // Apply deletions first (CONSTRUCT semantics: delete before insert)
        if !patch.deletions.is_empty() {
            #[cfg(feature = "tracing")]
            debug!(count = patch.deletions.len(), "Applying deletions");

            if let Err(e) = tx.delete_triples(&patch.deletions).await {
                #[cfg(feature = "tracing")]
                error!(error = ?e, "Deletion failed, rolling back");
                let _ = tx.rollback().await;
                return Err(e);
            }
        }

        // Apply additions
        if !patch.additions.is_empty() {
            #[cfg(feature = "tracing")]
            debug!(count = patch.additions.len(), "Applying additions");

            if let Err(e) = tx.add_triples(&patch.additions).await {
                #[cfg(feature = "tracing")]
                error!(error = ?e, "Addition failed, rolling back");
                let _ = tx.rollback().await;
                return Err(e);
            }
        }

        // Commit transaction
        let backend_commit_id = match tx.commit().await {
            Ok(id) => {
                #[cfg(feature = "tracing")]
                info!(commit_id = %id, "Transaction committed successfully");
                Some(id)
            }
            Err(e) => {
                #[cfg(feature = "tracing")]
                error!(error = ?e, "Commit failed");
                return Err(e);
            }
        };

        // Create result
        let result =
            CommitResult::new(Uuid::new_v4(), patch.additions.len(), patch.deletions.len());

        let result = if let Some(id) = backend_commit_id {
            result.with_backend_commit_id(id)
        } else {
            result
        };

        #[cfg(feature = "tracing")]
        info!(
            patch_set_id = %result.patch_set_id,
            additions = result.additions_count,
            deletions = result.deletions_count,
            "Patch committed successfully"
        );

        Ok(result)
    }

    async fn commit_patch_set(&self, patch_set: PatchSet) -> Result<CommitResult, WriteError> {
        #[cfg(feature = "tracing")]
        debug!(
            patches = patch_set.patches.len(),
            total_mutations = patch_set.total_mutation_count(),
            "Starting patch set commit"
        );

        // Validate all patches first
        patch_set.validate()?;

        #[cfg(feature = "tracing")]
        info!(
            patch_count = patch_set.patches.len(),
            total_mutations = patch_set.total_mutation_count(),
            "Patch set validated"
        );

        // Begin transaction
        let mut tx = self.backend.begin_transaction().await?;

        let mut total_additions = 0;
        let mut total_deletions = 0;

        // Apply patches in order
        for (idx, patch) in patch_set.patches.iter().enumerate() {
            #[cfg(feature = "tracing")]
            debug!(
                patch_index = idx,
                additions = patch.additions.len(),
                deletions = patch.deletions.len(),
                "Processing patch"
            );

            // Apply deletions
            if !patch.deletions.is_empty() {
                if let Err(e) = tx.delete_triples(&patch.deletions).await {
                    #[cfg(feature = "tracing")]
                    error!(patch_index = idx, error = ?e, "Deletion failed, rolling back");
                    let _ = tx.rollback().await;
                    return Err(e);
                }
                total_deletions += patch.deletions.len();
            }

            // Apply additions
            if !patch.additions.is_empty() {
                if let Err(e) = tx.add_triples(&patch.additions).await {
                    #[cfg(feature = "tracing")]
                    error!(patch_index = idx, error = ?e, "Addition failed, rolling back");
                    let _ = tx.rollback().await;
                    return Err(e);
                }
                total_additions += patch.additions.len();
            }
        }

        // Commit transaction
        let backend_commit_id = match tx.commit().await {
            Ok(id) => {
                #[cfg(feature = "tracing")]
                info!(commit_id = %id, "Transaction committed successfully");
                Some(id)
            }
            Err(e) => {
                #[cfg(feature = "tracing")]
                error!(error = ?e, "Commit failed");
                return Err(e);
            }
        };

        // Create result
        let result = CommitResult::new(patch_set.id, total_additions, total_deletions);

        let result = if let Some(id) = backend_commit_id {
            result.with_backend_commit_id(id)
        } else {
            result
        };

        #[cfg(feature = "tracing")]
        info!(
            patch_set_id = %result.patch_set_id,
            patches = patch_set.patches.len(),
            additions = result.additions_count,
            deletions = result.deletions_count,
            "Patch set committed successfully"
        );

        Ok(result)
    }

    async fn validate_patch(&self, patch: &Patch) -> Result<(), WriteError> {
        #[cfg(feature = "tracing")]
        debug!(
            additions = patch.additions.len(),
            deletions = patch.deletions.len(),
            "Validating patch"
        );

        patch.validate().map_err(|e| {
            #[cfg(feature = "tracing")]
            warn!(error = ?e, "Patch validation failed");
            WriteError::from(e)
        })
    }

    fn max_mutation_units(&self) -> usize {
        MAX_MUTATION_UNITS
    }
}

/// Serializable triple for storage backends.
///
/// This is a helper type that backends can use for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageTriple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

impl From<&Triple> for StorageTriple {
    fn from(triple: &Triple) -> Self {
        Self {
            subject: triple.subject.clone(),
            predicate: triple.predicate.clone(),
            object: triple.object.clone(),
        }
    }
}

impl From<Triple> for StorageTriple {
    fn from(triple: Triple) -> Self {
        Self {
            subject: triple.subject,
            predicate: triple.predicate,
            object: triple.object,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock storage backend for testing
    struct MockBackend {
        should_fail: bool,
    }

    impl MockBackend {
        fn new() -> Self {
            Self { should_fail: false }
        }

        fn with_failure() -> Self {
            Self { should_fail: true }
        }
    }

    struct MockTransaction {
        additions: Vec<Triple>,
        deletions: Vec<Triple>,
        should_fail: bool,
        committed: bool,
    }

    #[async_trait]
    impl StorageBackend for MockBackend {
        async fn begin_transaction(&self) -> Result<Box<dyn Transaction>, WriteError> {
            Ok(Box::new(MockTransaction {
                additions: Vec::new(),
                deletions: Vec::new(),
                should_fail: self.should_fail,
                committed: false,
            }))
        }

        fn backend_name(&self) -> &str {
            "MockBackend"
        }
    }

    #[async_trait]
    impl Transaction for MockTransaction {
        async fn add_triples(&mut self, triples: &[Triple]) -> Result<(), WriteError> {
            if self.should_fail {
                return Err(WriteError::StorageError {
                    message: "Mock failure".to_string(),
                });
            }
            self.additions.extend_from_slice(triples);
            Ok(())
        }

        async fn delete_triples(&mut self, triples: &[Triple]) -> Result<(), WriteError> {
            if self.should_fail {
                return Err(WriteError::StorageError {
                    message: "Mock failure".to_string(),
                });
            }
            self.deletions.extend_from_slice(triples);
            Ok(())
        }

        async fn commit(mut self: Box<Self>) -> Result<String, WriteError> {
            if self.should_fail {
                return Err(WriteError::StorageError {
                    message: "Mock commit failure".to_string(),
                });
            }
            self.committed = true;
            Ok("mock-commit-id".to_string())
        }

        async fn rollback(self: Box<Self>) -> Result<(), WriteError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_commit_patch_success() {
        let backend = MockBackend::new();
        let writer = Construct8Writer::new(backend);

        let mut patch = Patch::new();
        patch.add(Triple::new("s1", "p1", "o1"));
        patch.add(Triple::new("s2", "p2", "o2"));

        let result = writer.commit_patch(patch).await.unwrap();
        assert_eq!(result.additions_count, 2);
        assert_eq!(result.deletions_count, 0);
        assert_eq!(result.backend_commit_id, Some("mock-commit-id".to_string()));
    }

    #[tokio::test]
    async fn test_commit_patch_validation_fails() {
        let backend = MockBackend::new();
        let writer = Construct8Writer::new(backend);

        let mut patch = Patch::new();
        for i in 0..9 {
            patch.add(Triple::new(format!("s{}", i), "p", "o"));
        }

        let result = writer.commit_patch(patch).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WriteError::ValidationFailed(_)
        ));
    }

    #[tokio::test]
    async fn test_commit_patch_backend_fails() {
        let backend = MockBackend::with_failure();
        let writer = Construct8Writer::new(backend);

        let mut patch = Patch::new();
        patch.add(Triple::new("s1", "p1", "o1"));

        let result = writer.commit_patch(patch).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WriteError::StorageError { .. }
        ));
    }

    #[tokio::test]
    async fn test_validate_patch() {
        let backend = MockBackend::new();
        let writer = Construct8Writer::new(backend);

        let mut patch = Patch::new();
        patch.add(Triple::new("s1", "p1", "o1"));

        assert!(writer.validate_patch(&patch).await.is_ok());
    }

    #[tokio::test]
    async fn test_max_mutation_units() {
        let backend = MockBackend::new();
        let writer = Construct8Writer::new(backend);

        assert_eq!(writer.max_mutation_units(), MAX_MUTATION_UNITS);
    }
}
