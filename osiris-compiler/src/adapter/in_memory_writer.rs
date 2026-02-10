//! In-memory implementation of BoundedWriter for testing and development.

use crate::domain::{Patch, PatchSet, Triple, MAX_MUTATION_UNITS};
use crate::port::{BoundedWriter, CommitResult, WriteError};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// In-memory triple store for testing.
#[derive(Debug, Clone)]
struct TripleStore {
    /// Triples indexed by subject
    triples: HashMap<String, Vec<Triple>>,
    /// Commit history
    commits: Vec<CommitResult>,
}

impl TripleStore {
    fn new() -> Self {
        Self {
            triples: HashMap::new(),
            commits: Vec::new(),
        }
    }

    fn add_triple(&mut self, triple: Triple) {
        self.triples
            .entry(triple.subject.clone())
            .or_default()
            .push(triple);
    }

    fn remove_triple(&mut self, triple: &Triple) -> bool {
        if let Some(triples) = self.triples.get_mut(&triple.subject) {
            if let Some(pos) = triples.iter().position(|t| t == triple) {
                triples.remove(pos);
                return true;
            }
        }
        false
    }

    fn record_commit(&mut self, result: CommitResult) {
        self.commits.push(result);
    }
}

/// In-memory implementation of BoundedWriter.
///
/// This implementation stores triples in memory and is suitable for:
/// - Testing and development
/// - Single-node deployments without persistence requirements
/// - Prototyping CONSTRUCT8 semantics
///
/// For production use with persistence, use Firestore or Spanner backends.
#[derive(Debug, Clone)]
pub struct InMemoryWriter {
    store: Arc<RwLock<TripleStore>>,
}

impl InMemoryWriter {
    /// Creates a new in-memory writer.
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(TripleStore::new())),
        }
    }

    /// Returns the number of triples currently stored.
    pub fn triple_count(&self) -> usize {
        let store = self.store.read().unwrap();
        store.triples.values().map(|v| v.len()).sum()
    }

    /// Returns the commit history.
    pub fn commit_history(&self) -> Vec<CommitResult> {
        let store = self.store.read().unwrap();
        store.commits.clone()
    }

    /// Clears all triples and commit history.
    pub fn clear(&self) {
        let mut store = self.store.write().unwrap();
        store.triples.clear();
        store.commits.clear();
    }
}

impl Default for InMemoryWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BoundedWriter for InMemoryWriter {
    async fn commit_patch(&self, patch: Patch) -> Result<CommitResult, WriteError> {
        // Validate the patch first
        patch.validate()?;

        // Acquire write lock
        let mut store = self.store.write().map_err(|e| WriteError::StorageError {
            message: format!("Failed to acquire lock: {}", e),
        })?;

        // Apply deletions
        for triple in &patch.deletions {
            store.remove_triple(triple);
        }

        // Apply additions
        for triple in &patch.additions {
            store.add_triple(triple.clone());
        }

        // Create commit result
        let result =
            CommitResult::new(Uuid::new_v4(), patch.additions.len(), patch.deletions.len());

        // Record commit
        store.record_commit(result.clone());

        Ok(result)
    }

    async fn commit_patch_set(&self, patch_set: PatchSet) -> Result<CommitResult, WriteError> {
        // Validate all patches first
        patch_set.validate()?;

        // For atomicity, collect all operations first
        let mut all_additions = Vec::new();
        let mut all_deletions = Vec::new();

        for patch in &patch_set.patches {
            all_additions.extend(patch.additions.clone());
            all_deletions.extend(patch.deletions.clone());
        }

        // Acquire write lock
        let mut store = self.store.write().map_err(|e| WriteError::StorageError {
            message: format!("Failed to acquire lock: {}", e),
        })?;

        // Apply all deletions
        for triple in &all_deletions {
            store.remove_triple(triple);
        }

        // Apply all additions
        for triple in &all_additions {
            store.add_triple(triple.clone());
        }

        // Create commit result
        let result = CommitResult::new(patch_set.id, all_additions.len(), all_deletions.len());

        // Record commit
        store.record_commit(result.clone());

        Ok(result)
    }

    async fn validate_patch(&self, patch: &Patch) -> Result<(), WriteError> {
        patch.validate().map_err(WriteError::from)
    }

    fn max_mutation_units(&self) -> usize {
        MAX_MUTATION_UNITS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_commit_single_patch() {
        let writer = InMemoryWriter::new();
        let mut patch = Patch::new();
        patch.add(Triple::new("s1", "p1", "o1"));
        patch.add(Triple::new("s2", "p2", "o2"));

        let result = writer.commit_patch(patch).await.unwrap();
        assert_eq!(result.additions_count, 2);
        assert_eq!(result.deletions_count, 0);
        assert_eq!(writer.triple_count(), 2);
    }

    #[tokio::test]
    async fn test_commit_patch_with_deletions() {
        let writer = InMemoryWriter::new();

        // Add some triples
        let mut patch1 = Patch::new();
        patch1.add(Triple::new("s1", "p1", "o1"));
        patch1.add(Triple::new("s2", "p2", "o2"));
        writer.commit_patch(patch1).await.unwrap();

        // Delete one
        let mut patch2 = Patch::new();
        patch2.delete(Triple::new("s1", "p1", "o1"));
        let result = writer.commit_patch(patch2).await.unwrap();

        assert_eq!(result.deletions_count, 1);
        assert_eq!(writer.triple_count(), 1);
    }

    #[tokio::test]
    async fn test_validate_patch_exceeds_limit() {
        let writer = InMemoryWriter::new();
        let mut patch = Patch::new();

        // Add 9 triples (exceeds limit of 8)
        for i in 0..9 {
            patch.add(Triple::new(format!("s{}", i), "p", "o"));
        }

        let result = writer.validate_patch(&patch).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WriteError::ValidationFailed(_)
        ));
    }

    #[tokio::test]
    async fn test_commit_patch_set() {
        let writer = InMemoryWriter::new();

        let mut patch1 = Patch::new();
        patch1.add(Triple::new("s1", "p1", "o1"));

        let mut patch2 = Patch::new();
        patch2.add(Triple::new("s2", "p2", "o2"));

        let patch_set = PatchSet::new(vec![patch1, patch2]);
        let result = writer.commit_patch_set(patch_set).await.unwrap();

        assert_eq!(result.additions_count, 2);
        assert_eq!(writer.triple_count(), 2);
    }

    #[tokio::test]
    async fn test_commit_history() {
        let writer = InMemoryWriter::new();

        let mut patch = Patch::new();
        patch.add(Triple::new("s1", "p1", "o1"));
        writer.commit_patch(patch).await.unwrap();

        let history = writer.commit_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].additions_count, 1);
    }
}
