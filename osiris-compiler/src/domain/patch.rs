//! Patch types for bounded state mutations.

use super::triple::Triple;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum number of mutation units allowed per commit.
pub const MAX_MUTATION_UNITS: usize = 8;

/// Error types for patch operations.
#[derive(Debug, Clone, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PatchError {
    /// Patch exceeds the maximum allowed mutation units.
    #[error("Patch exceeds maximum mutation units: {actual} > {max}")]
    ExceedsLimit { actual: usize, max: usize },

    /// Patch is empty (no mutations).
    #[error("Patch is empty")]
    EmptyPatch,

    /// Invalid triple in patch.
    #[error("Invalid triple: {reason}")]
    InvalidTriple { reason: String },
}

/// Represents a bounded patch of RDF triples.
///
/// A patch contains additions and deletions that together must not exceed
/// the CONSTRUCT8 limit of 8 mutation units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Patch {
    /// Triples to add to the graph.
    pub additions: Vec<Triple>,
    /// Triples to delete from the graph.
    pub deletions: Vec<Triple>,
}

impl Patch {
    /// Creates a new empty patch.
    pub fn new() -> Self {
        Self {
            additions: Vec::new(),
            deletions: Vec::new(),
        }
    }

    /// Creates a patch with additions.
    pub fn with_additions(additions: Vec<Triple>) -> Self {
        Self {
            additions,
            deletions: Vec::new(),
        }
    }

    /// Creates a patch with deletions.
    pub fn with_deletions(deletions: Vec<Triple>) -> Self {
        Self {
            additions: Vec::new(),
            deletions,
        }
    }

    /// Adds a triple to the additions.
    pub fn add(&mut self, triple: Triple) {
        self.additions.push(triple);
    }

    /// Adds a triple to the deletions.
    pub fn delete(&mut self, triple: Triple) {
        self.deletions.push(triple);
    }

    /// Returns the total number of mutation units in this patch.
    ///
    /// Each triple addition or deletion counts as 1 mutation unit.
    pub fn mutation_count(&self) -> usize {
        self.additions.len() + self.deletions.len()
    }

    /// Validates that this patch is within the bounded limit.
    pub fn validate(&self) -> Result<(), PatchError> {
        if self.is_empty() {
            return Err(PatchError::EmptyPatch);
        }

        let count = self.mutation_count();
        if count > MAX_MUTATION_UNITS {
            return Err(PatchError::ExceedsLimit {
                actual: count,
                max: MAX_MUTATION_UNITS,
            });
        }

        Ok(())
    }

    /// Returns true if this patch has no mutations.
    pub fn is_empty(&self) -> bool {
        self.additions.is_empty() && self.deletions.is_empty()
    }
}

impl Default for Patch {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a collection of patches that should be applied atomically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchSet {
    /// Unique identifier for this patch set.
    pub id: uuid::Uuid,
    /// Individual patches in this set.
    pub patches: Vec<Patch>,
}

impl PatchSet {
    /// Creates a new patch set with a generated ID.
    pub fn new(patches: Vec<Patch>) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            patches,
        }
    }

    /// Validates all patches in this set.
    pub fn validate(&self) -> Result<(), PatchError> {
        if self.patches.is_empty() {
            return Err(PatchError::EmptyPatch);
        }

        for patch in &self.patches {
            patch.validate()?;
        }

        Ok(())
    }

    /// Returns the total mutation count across all patches.
    pub fn total_mutation_count(&self) -> usize {
        self.patches.iter().map(|p| p.mutation_count()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_mutation_count() {
        let mut patch = Patch::new();
        assert_eq!(patch.mutation_count(), 0);

        patch.add(Triple::new("s1", "p1", "o1"));
        assert_eq!(patch.mutation_count(), 1);

        patch.delete(Triple::new("s2", "p2", "o2"));
        assert_eq!(patch.mutation_count(), 2);
    }

    #[test]
    fn test_patch_validation_empty() {
        let patch = Patch::new();
        assert!(matches!(patch.validate(), Err(PatchError::EmptyPatch)));
    }

    #[test]
    fn test_patch_validation_exceeds_limit() {
        let mut patch = Patch::new();
        for i in 0..9 {
            patch.add(Triple::new(format!("s{}", i), "p", "o"));
        }

        let result = patch.validate();
        assert!(matches!(
            result,
            Err(PatchError::ExceedsLimit { actual: 9, max: 8 })
        ));
    }

    #[test]
    fn test_patch_validation_at_limit() {
        let mut patch = Patch::new();
        for i in 0..8 {
            patch.add(Triple::new(format!("s{}", i), "p", "o"));
        }

        assert!(patch.validate().is_ok());
    }
}
