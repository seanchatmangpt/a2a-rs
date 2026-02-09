//! Invariants system for state validation
//!
//! This module provides a framework for defining and checking invariants that
//! ensure state transitions preserve protocol semantics. Invariants are conditions
//! that must hold true at all times during the lifecycle of protocol entities.
//!
//! # Architecture
//!
//! - **Invariant trait**: Core abstraction for checkable conditions
//! - **InvariantViolation**: Error type for violated invariants
//! - **InvariantRegistry**: Manages and executes invariants in deterministic order
//! - **Standard invariants**: Pre-built invariants for common validation scenarios
//!
//! # Example
//!
//! ```rust
//! use a2a_rs::construct::invariants::{Invariant, InvariantRegistry, TaskStateInvariant};
//! use a2a_rs::domain::{Task, TaskState};
//!
//! // Create a registry with standard invariants
//! let mut registry = InvariantRegistry::new();
//! registry.register("task_state", Box::new(TaskStateInvariant::new()));
//!
//! // Check invariants on a task
//! let task = Task::new("task-1".to_string(), "ctx-1".to_string());
//! let result = registry.check_all(&task);
//! assert!(result.is_ok());
//! ```

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

mod artifact;
mod event;
mod task_state;

pub use artifact::{ArtifactImmutabilityInvariant, ArtifactSnapshot};
pub use event::{EventOrderingInvariant, EventSequence};
pub use task_state::TaskStateInvariant;

use crate::domain::A2AError;

/// Error type for invariant violations
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum InvariantViolation {
    /// Task state transition violates FSM rules
    #[error("Task state invariant violated: {reason}")]
    TaskStateViolation { reason: String },

    /// Artifact was modified after creation
    #[error("Artifact immutability violated: artifact_id={artifact_id}, reason={reason}")]
    ArtifactMutated { artifact_id: String, reason: String },

    /// Event ordering constraint violated
    #[error("Event ordering violated: {reason}")]
    EventOrderingViolation { reason: String },

    /// Custom invariant violation
    #[error("Invariant '{name}' violated: {reason}")]
    Custom { name: String, reason: String },

    /// Multiple invariants violated
    #[error("Multiple invariants violated: {}", .violations.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("; "))]
    Multiple { violations: Vec<InvariantViolation> },
}

impl From<InvariantViolation> for A2AError {
    fn from(violation: InvariantViolation) -> Self {
        A2AError::ValidationError {
            field: "invariant".to_string(),
            message: violation.to_string(),
        }
    }
}

/// Result type for invariant checks
pub type InvariantResult = Result<(), InvariantViolation>;

/// Trait for types that can be checked against invariants
///
/// Invariants are conditions that must hold true at all times. Unlike
/// simple validation (which checks structural correctness), invariants
/// enforce semantic constraints and state machine properties.
pub trait Invariant<T>: Send + Sync {
    /// Check if the invariant holds for the given value
    ///
    /// # Errors
    ///
    /// Returns an `InvariantViolation` if the invariant does not hold.
    fn check(&self, value: &T) -> InvariantResult;

    /// Get a human-readable name for this invariant
    fn name(&self) -> &str;

    /// Get a description of what this invariant checks
    fn description(&self) -> &str {
        "No description provided"
    }
}

/// Registry for managing and executing invariants in deterministic order
///
/// The registry uses a BTreeMap to ensure invariants are always checked
/// in the same order (sorted by their registration key). This provides
/// deterministic behavior across different runs and platforms.
///
/// # Example
///
/// ```rust
/// use a2a_rs::construct::invariants::{InvariantRegistry, TaskStateInvariant};
/// use a2a_rs::domain::Task;
///
/// let mut registry = InvariantRegistry::<Task>::new();
/// registry.register("task_state", Box::new(TaskStateInvariant::new()));
///
/// let task = Task::new("task-1".to_string(), "ctx-1".to_string());
/// let result = registry.check_all(&task);
/// assert!(result.is_ok());
/// ```
pub struct InvariantRegistry<T> {
    /// Invariants stored in sorted order by key
    invariants: BTreeMap<String, Box<dyn Invariant<T>>>,
}

impl<T> InvariantRegistry<T> {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            invariants: BTreeMap::new(),
        }
    }

    /// Create a registry with standard invariants pre-registered
    ///
    /// This is a convenience method that registers the most common
    /// invariants used in the A2A protocol.
    pub fn with_standard_invariants() -> Self
    where
        Self: Default,
    {
        Self::default()
    }

    /// Register an invariant with the given key
    ///
    /// If an invariant with the same key already exists, it will be replaced.
    /// Keys are used to determine the order of checking (lexicographic sort).
    ///
    /// # Arguments
    ///
    /// * `key` - Unique identifier for this invariant (also determines check order)
    /// * `invariant` - The invariant to register
    pub fn register(&mut self, key: impl Into<String>, invariant: Box<dyn Invariant<T>>) {
        self.invariants.insert(key.into(), invariant);
    }

    /// Remove an invariant by key
    ///
    /// Returns `true` if an invariant was removed, `false` if no invariant
    /// with that key existed.
    pub fn unregister(&mut self, key: &str) -> bool {
        self.invariants.remove(key).is_some()
    }

    /// Check all registered invariants against the given value
    ///
    /// Invariants are checked in lexicographic order by their registration key.
    /// This ensures deterministic behavior.
    ///
    /// # Errors
    ///
    /// - If a single invariant fails, returns that violation
    /// - If multiple invariants fail, returns `InvariantViolation::Multiple`
    ///   containing all violations
    ///
    /// # Stop-on-first-error behavior
    ///
    /// By default, this method stops at the first violation. Use
    /// `check_all_collect` to collect all violations.
    pub fn check_all(&self, value: &T) -> InvariantResult {
        let mut violations = Vec::new();

        for (_key, invariant) in &self.invariants {
            if let Err(violation) = invariant.check(value) {
                violations.push(violation);
            }
        }

        if violations.is_empty() {
            Ok(())
        } else if violations.len() == 1 {
            Err(violations.into_iter().next().unwrap())
        } else {
            Err(InvariantViolation::Multiple { violations })
        }
    }

    /// Check all invariants and collect all violations
    ///
    /// Unlike `check_all`, this method always checks every invariant
    /// and returns all violations found.
    pub fn check_all_collect(&self, value: &T) -> Vec<InvariantViolation> {
        let mut violations = Vec::new();

        for (_key, invariant) in &self.invariants {
            if let Err(violation) = invariant.check(value) {
                violations.push(violation);
            }
        }

        violations
    }

    /// Get the number of registered invariants
    pub fn len(&self) -> usize {
        self.invariants.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.invariants.is_empty()
    }

    /// Get the keys of all registered invariants in check order
    pub fn keys(&self) -> Vec<&str> {
        self.invariants.keys().map(|s| s.as_str()).collect()
    }
}

impl<T> Default for InvariantRegistry<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysPass;
    impl Invariant<i32> for AlwaysPass {
        fn check(&self, _value: &i32) -> InvariantResult {
            Ok(())
        }
        fn name(&self) -> &str {
            "always_pass"
        }
    }

    struct AlwaysFail;
    impl Invariant<i32> for AlwaysFail {
        fn check(&self, _value: &i32) -> InvariantResult {
            Err(InvariantViolation::Custom {
                name: "always_fail".to_string(),
                reason: "This invariant always fails".to_string(),
            })
        }
        fn name(&self) -> &str {
            "always_fail"
        }
    }

    struct MustBePositive;
    impl Invariant<i32> for MustBePositive {
        fn check(&self, value: &i32) -> InvariantResult {
            if *value > 0 {
                Ok(())
            } else {
                Err(InvariantViolation::Custom {
                    name: "must_be_positive".to_string(),
                    reason: format!("Value {} is not positive", value),
                })
            }
        }
        fn name(&self) -> &str {
            "must_be_positive"
        }
    }

    #[test]
    fn test_empty_registry() {
        let registry = InvariantRegistry::<i32>::new();
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
        assert!(registry.check_all(&42).is_ok());
    }

    #[test]
    fn test_single_passing_invariant() {
        let mut registry = InvariantRegistry::new();
        registry.register("pass", Box::new(AlwaysPass));

        assert_eq!(registry.len(), 1);
        assert!(registry.check_all(&42).is_ok());
    }

    #[test]
    fn test_single_failing_invariant() {
        let mut registry = InvariantRegistry::new();
        registry.register("fail", Box::new(AlwaysFail));

        let result = registry.check_all(&42);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            InvariantViolation::Custom { .. }
        ));
    }

    #[test]
    fn test_multiple_invariants_all_pass() {
        let mut registry = InvariantRegistry::new();
        registry.register("pass1", Box::new(AlwaysPass));
        registry.register("pass2", Box::new(AlwaysPass));
        registry.register("positive", Box::new(MustBePositive));

        assert!(registry.check_all(&42).is_ok());
    }

    #[test]
    fn test_multiple_invariants_one_fails() {
        let mut registry = InvariantRegistry::new();
        registry.register("pass", Box::new(AlwaysPass));
        registry.register("positive", Box::new(MustBePositive));

        let result = registry.check_all(&-5);
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_invariants_multiple_fail() {
        let mut registry = InvariantRegistry::new();
        registry.register("fail1", Box::new(AlwaysFail));
        registry.register("positive", Box::new(MustBePositive));

        let result = registry.check_all(&-5);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            InvariantViolation::Multiple { .. }
        ));
    }

    #[test]
    fn test_check_all_collect() {
        let mut registry = InvariantRegistry::new();
        registry.register("fail1", Box::new(AlwaysFail));
        registry.register("positive", Box::new(MustBePositive));

        let violations = registry.check_all_collect(&-5);
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_unregister() {
        let mut registry = InvariantRegistry::new();
        registry.register("fail", Box::new(AlwaysFail));
        assert_eq!(registry.len(), 1);

        assert!(registry.unregister("fail"));
        assert_eq!(registry.len(), 0);
        assert!(registry.check_all(&42).is_ok());

        assert!(!registry.unregister("nonexistent"));
    }

    #[test]
    fn test_deterministic_order() {
        let mut registry = InvariantRegistry::new();
        // Register in reverse alphabetical order
        registry.register("z_last", Box::new(AlwaysPass));
        registry.register("a_first", Box::new(AlwaysPass));
        registry.register("m_middle", Box::new(AlwaysPass));

        let keys = registry.keys();
        assert_eq!(keys, vec!["a_first", "m_middle", "z_last"]);
    }

    #[test]
    fn test_replace_invariant() {
        let mut registry = InvariantRegistry::new();
        registry.register("test", Box::new(AlwaysPass));
        assert!(registry.check_all(&-5).is_ok());

        registry.register("test", Box::new(MustBePositive));
        assert!(registry.check_all(&-5).is_err());
    }
}
