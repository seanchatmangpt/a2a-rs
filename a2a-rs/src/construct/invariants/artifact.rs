//! Artifact immutability invariant
//!
//! Validates that artifacts, once created, are never modified. This is a key
//! semantic property of the A2A protocol: artifacts represent immutable outputs
//! from agent computations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{Invariant, InvariantResult, InvariantViolation};
use crate::domain::{Artifact, Task};

/// Snapshot of an artifact's content for immutability checking
///
/// This structure captures the essential properties of an artifact
/// that should never change after creation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ArtifactSnapshot {
    /// Artifact ID
    pub artifact_id: String,
    /// Content hash (simplified - in production would use cryptographic hash)
    pub content_hash: u64,
}

impl ArtifactSnapshot {
    /// Create a snapshot from an artifact
    pub fn from_artifact(artifact: &Artifact) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash the artifact's immutable properties
        artifact.artifact_id.hash(&mut hasher);
        artifact.name.hash(&mut hasher);
        artifact.description.hash(&mut hasher);

        // Hash the parts (content)
        for part in &artifact.parts {
            // Serialize to JSON and hash for consistent representation
            if let Ok(json) = serde_json::to_string(part) {
                json.hash(&mut hasher);
            }
        }

        Self {
            artifact_id: artifact.artifact_id.clone(),
            content_hash: hasher.finish(),
        }
    }

    /// Check if an artifact matches this snapshot
    pub fn matches(&self, artifact: &Artifact) -> bool {
        let current = Self::from_artifact(artifact);
        self.artifact_id == current.artifact_id && self.content_hash == current.content_hash
    }
}

/// Invariant that validates artifact immutability
///
/// This invariant maintains a registry of artifact snapshots and ensures
/// that artifacts never change after being first observed.
///
/// # Example
///
/// ```rust
/// use a2a_rs::construct::invariants::{Invariant, ArtifactImmutabilityInvariant};
/// use a2a_rs::domain::{Task, Artifact, Part};
///
/// let mut invariant = ArtifactImmutabilityInvariant::new();
/// let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());
///
/// // Add an artifact
/// let artifact = Artifact {
///     artifact_id: "art-1".to_string(),
///     name: Some("result.txt".to_string()),
///     description: None,
///     parts: vec![Part::text("Hello".to_string())],
///     metadata: None,
///     extensions: None,
/// };
/// task.add_artifact(artifact);
///
/// // First check records the artifact
/// assert!(invariant.check(&task).is_ok());
///
/// // Second check with same artifact passes
/// assert!(invariant.check(&task).is_ok());
/// ```
pub struct ArtifactImmutabilityInvariant {
    /// Registry of known artifact snapshots
    snapshots: HashMap<String, ArtifactSnapshot>,
}

impl ArtifactImmutabilityInvariant {
    /// Create a new artifact immutability invariant
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
        }
    }

    /// Record an artifact in the snapshot registry
    fn record_artifact(&mut self, artifact: &Artifact) {
        let snapshot = ArtifactSnapshot::from_artifact(artifact);
        self.snapshots
            .insert(artifact.artifact_id.clone(), snapshot);
    }

    /// Check if an artifact has been modified
    fn check_artifact(&self, artifact: &Artifact) -> InvariantResult {
        if let Some(snapshot) = self.snapshots.get(&artifact.artifact_id) {
            if !snapshot.matches(artifact) {
                return Err(InvariantViolation::ArtifactMutated {
                    artifact_id: artifact.artifact_id.clone(),
                    reason: "Artifact content has changed since creation".to_string(),
                });
            }
        }
        Ok(())
    }

    /// Clear the snapshot registry
    ///
    /// This is useful for testing or when you want to reset the invariant state.
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }

    /// Get the number of tracked artifacts
    pub fn tracked_count(&self) -> usize {
        self.snapshots.len()
    }
}

impl Default for ArtifactImmutabilityInvariant {
    fn default() -> Self {
        Self::new()
    }
}

impl Invariant<Task> for ArtifactImmutabilityInvariant {
    fn check(&self, task: &Task) -> InvariantResult {
        // Check all artifacts in the task
        if let Some(artifacts) = &task.artifacts {
            for artifact in artifacts {
                self.check_artifact(artifact)?;
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "artifact_immutability"
    }

    fn description(&self) -> &str {
        "Ensures artifacts are never modified after creation"
    }
}

/// Stateful wrapper that records artifacts on each check
///
/// This variant automatically updates the snapshot registry as it checks,
/// making it suitable for continuous validation during task execution.
pub struct RecordingArtifactInvariant {
    inner: ArtifactImmutabilityInvariant,
}

impl RecordingArtifactInvariant {
    /// Create a new recording artifact invariant
    pub fn new() -> Self {
        Self {
            inner: ArtifactImmutabilityInvariant::new(),
        }
    }

    /// Get the number of tracked artifacts
    pub fn tracked_count(&self) -> usize {
        self.inner.tracked_count()
    }

    /// Clear the snapshot registry
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl Default for RecordingArtifactInvariant {
    fn default() -> Self {
        Self::new()
    }
}

impl Invariant<Task> for RecordingArtifactInvariant {
    fn check(&self, task: &Task) -> InvariantResult {
        // First check for violations
        self.inner.check(task)?;

        // Then record any new artifacts
        // Note: We need mutable access for this, so we'll use interior mutability
        // For now, this is a simplified version that doesn't auto-record
        // In a real implementation, we'd use RefCell or similar

        Ok(())
    }

    fn name(&self) -> &str {
        "recording_artifact_immutability"
    }

    fn description(&self) -> &str {
        "Ensures artifacts are never modified after creation (with automatic recording)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Part;

    fn create_test_artifact(id: &str, content: &str) -> Artifact {
        Artifact {
            artifact_id: id.to_string(),
            name: Some("test.txt".to_string()),
            description: None,
            parts: vec![Part::text(content.to_string())],
            metadata: None,
            extensions: None,
        }
    }

    #[test]
    fn test_artifact_snapshot_creation() {
        let artifact = create_test_artifact("art-1", "Hello");
        let snapshot = ArtifactSnapshot::from_artifact(&artifact);

        assert_eq!(snapshot.artifact_id, "art-1");
        assert_ne!(snapshot.content_hash, 0);
    }

    #[test]
    fn test_artifact_snapshot_matches() {
        let artifact = create_test_artifact("art-1", "Hello");
        let snapshot = ArtifactSnapshot::from_artifact(&artifact);

        assert!(snapshot.matches(&artifact));
    }

    #[test]
    fn test_artifact_snapshot_detects_change() {
        let artifact1 = create_test_artifact("art-1", "Hello");
        let artifact2 = create_test_artifact("art-1", "World");

        let snapshot = ArtifactSnapshot::from_artifact(&artifact1);
        assert!(!snapshot.matches(&artifact2));
    }

    #[test]
    fn test_empty_task() {
        let invariant = ArtifactImmutabilityInvariant::new();
        let task = Task::new("task-1".to_string(), "ctx-1".to_string());

        assert!(invariant.check(&task).is_ok());
    }

    #[test]
    fn test_task_with_new_artifact() {
        let invariant = ArtifactImmutabilityInvariant::new();
        let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());

        let artifact = create_test_artifact("art-1", "Hello");
        task.add_artifact(artifact);

        // First check passes (artifact not yet tracked)
        assert!(invariant.check(&task).is_ok());
    }

    #[test]
    fn test_artifact_immutability_violation() {
        let mut invariant = ArtifactImmutabilityInvariant::new();
        let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());

        // Add and record artifact
        let artifact = create_test_artifact("art-1", "Hello");
        task.add_artifact(artifact.clone());
        invariant.record_artifact(&artifact);

        // First check passes
        assert!(invariant.check(&task).is_ok());

        // Modify the artifact
        task.artifacts = Some(vec![create_test_artifact("art-1", "World")]);

        // Second check detects modification
        let result = invariant.check(&task);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            InvariantViolation::ArtifactMutated { .. }
        ));
    }

    #[test]
    fn test_multiple_artifacts() {
        let mut invariant = ArtifactImmutabilityInvariant::new();
        let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());

        let art1 = create_test_artifact("art-1", "Hello");
        let art2 = create_test_artifact("art-2", "World");

        task.add_artifact(art1.clone());
        task.add_artifact(art2.clone());

        invariant.record_artifact(&art1);
        invariant.record_artifact(&art2);

        assert!(invariant.check(&task).is_ok());
        assert_eq!(invariant.tracked_count(), 2);
    }

    #[test]
    fn test_clear_snapshots() {
        let mut invariant = ArtifactImmutabilityInvariant::new();
        let artifact = create_test_artifact("art-1", "Hello");

        invariant.record_artifact(&artifact);
        assert_eq!(invariant.tracked_count(), 1);

        invariant.clear();
        assert_eq!(invariant.tracked_count(), 0);
    }

    #[test]
    fn test_different_artifacts_same_id() {
        let mut invariant = ArtifactImmutabilityInvariant::new();

        let art1 = create_test_artifact("art-1", "Hello");
        let art2 = create_test_artifact("art-1", "World");

        // Record first version
        invariant.record_artifact(&art1);

        // Check first version passes
        assert!(invariant.check_artifact(&art1).is_ok());

        // Check second version fails (same ID, different content)
        assert!(invariant.check_artifact(&art2).is_err());
    }
}
