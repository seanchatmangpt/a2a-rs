//! Domain types for Q invariants.
//!
//! Q invariants are predicates over state that must hold before and after
//! any state transition (commit). This module provides the jidoka "stop-the-line"
//! mechanism: if preserve(Q) cannot be proven, the commit is blocked.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A Q invariant: a predicate that must be preserved across state transitions.
///
/// Q invariants enforce safety properties of the system state. Before any
/// commit, the system must prove that the invariant holds in both the
/// pre-state and post-state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QInvariant {
    /// Unique identifier for this invariant
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Description of what this invariant guarantees
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// The predicate that must hold
    pub predicate: InvariantPredicate,

    /// Severity level if violated
    pub severity: InvariantSeverity,

    /// Whether this invariant is currently enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// Severity level for invariant violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InvariantSeverity {
    /// Critical: system cannot proceed
    Critical,
    /// Error: operation should be blocked
    Error,
    /// Warning: log but allow
    Warning,
}

/// Predicate types for invariants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InvariantPredicate {
    /// State field must equal a specific value
    #[serde(rename_all = "camelCase")]
    StateEquals {
        field: String,
        expected: serde_json::Value,
    },

    /// State field must satisfy a comparison
    #[serde(rename_all = "camelCase")]
    StateComparison {
        field: String,
        operator: ComparisonOperator,
        value: serde_json::Value,
    },

    /// Custom predicate expression
    #[serde(rename_all = "camelCase")]
    Custom {
        expression: String,
        #[serde(skip_serializing_if = "HashMap::is_empty", default)]
        parameters: HashMap<String, serde_json::Value>,
    },

    /// Conjunction of multiple predicates (all must hold)
    #[serde(rename_all = "camelCase")]
    And { predicates: Vec<InvariantPredicate> },

    /// Disjunction of multiple predicates (at least one must hold)
    #[serde(rename_all = "camelCase")]
    Or { predicates: Vec<InvariantPredicate> },

    /// Negation of a predicate
    #[serde(rename_all = "camelCase")]
    Not { predicate: Box<InvariantPredicate> },

    /// Type invariant: state must conform to a schema
    #[serde(rename_all = "camelCase")]
    TypeInvariant { schema: serde_json::Value },

    /// Relational invariant: relationship between multiple fields
    #[serde(rename_all = "camelCase")]
    Relational {
        left_field: String,
        operator: ComparisonOperator,
        right_field: String,
    },
}

/// Comparison operators for invariant predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ComparisonOperator {
    /// Equal to
    Eq,
    /// Not equal to
    Ne,
    /// Less than
    Lt,
    /// Less than or equal to
    Le,
    /// Greater than
    Gt,
    /// Greater than or equal to
    Ge,
    /// Contains (for collections)
    Contains,
    /// Matches (for regex)
    Matches,
}

/// State snapshot for invariant verification.
///
/// Represents the system state at a point in time, used to evaluate
/// invariants before and after state transitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateSnapshot {
    /// Unique identifier for this snapshot
    pub snapshot_id: String,

    /// State data as key-value pairs
    pub state: HashMap<String, serde_json::Value>,

    /// Timestamp of the snapshot
    #[cfg(feature = "timestamps")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    #[cfg(not(feature = "timestamps"))]
    pub timestamp: String,

    /// Optional metadata
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Result of verifying an invariant against a state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum InvariantCheckResult {
    /// Invariant holds in the given state
    #[serde(rename_all = "camelCase")]
    Satisfied {
        invariant_id: String,
        snapshot_id: String,
    },

    /// Invariant is violated in the given state
    #[serde(rename_all = "camelCase")]
    Violated {
        invariant_id: String,
        snapshot_id: String,
        reason: String,
        #[serde(skip_serializing_if = "HashMap::is_empty", default)]
        context: HashMap<String, serde_json::Value>,
    },

    /// Invariant verification failed (e.g., missing fields)
    #[serde(rename_all = "camelCase")]
    VerificationFailed {
        invariant_id: String,
        snapshot_id: String,
        error: String,
    },
}

/// Result of verifying preserve(Q) across a state transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreservationResult {
    /// The invariant being checked
    pub invariant_id: String,

    /// Pre-state verification result
    pub pre_state: InvariantCheckResult,

    /// Post-state verification result
    pub post_state: InvariantCheckResult,

    /// Whether the invariant is preserved (holds in both states)
    pub preserved: bool,
}

impl PreservationResult {
    /// Checks if the invariant is preserved across the transition.
    pub fn is_preserved(&self) -> bool {
        self.preserved
    }

    /// Returns true if this result should block a commit.
    pub fn should_block(&self, severity: InvariantSeverity) -> bool {
        !self.preserved
            && matches!(
                severity,
                InvariantSeverity::Critical | InvariantSeverity::Error
            )
    }
}

/// A commit operation that requires invariant verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    /// Unique commit identifier
    pub commit_id: String,

    /// Pre-state snapshot
    pub pre_state: StateSnapshot,

    /// Post-state snapshot
    pub post_state: StateSnapshot,

    /// Description of the changes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Optional metadata
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Result of verifying all invariants for a commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitVerificationResult {
    /// The commit being verified
    pub commit_id: String,

    /// Results for each invariant
    pub invariant_results: Vec<PreservationResult>,

    /// Whether the commit should be allowed
    pub allowed: bool,

    /// List of violations that blocked the commit
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub blocking_violations: Vec<String>,
}

impl CommitVerificationResult {
    /// Creates a new verification result.
    pub fn new(commit_id: String, invariant_results: Vec<PreservationResult>) -> Self {
        let blocking_violations: Vec<String> = invariant_results
            .iter()
            .filter(|r| !r.preserved)
            .map(|r| r.invariant_id.clone())
            .collect();

        let allowed = blocking_violations.is_empty();

        Self {
            commit_id,
            invariant_results,
            allowed,
            blocking_violations,
        }
    }

    /// Returns true if the commit should be allowed.
    pub fn is_allowed(&self) -> bool {
        self.allowed
    }

    /// Returns true if the commit should be blocked.
    pub fn is_blocked(&self) -> bool {
        !self.allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invariant_predicate_serialization() {
        let predicate = InvariantPredicate::StateEquals {
            field: "status".to_string(),
            expected: serde_json::json!("active"),
        };

        let json = serde_json::to_string(&predicate).unwrap();
        let deserialized: InvariantPredicate = serde_json::from_str(&json).unwrap();

        match deserialized {
            InvariantPredicate::StateEquals { field, .. } => assert_eq!(field, "status"),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_preservation_result_blocking() {
        let result = PreservationResult {
            invariant_id: "inv-1".to_string(),
            pre_state: InvariantCheckResult::Satisfied {
                invariant_id: "inv-1".to_string(),
                snapshot_id: "snap-1".to_string(),
            },
            post_state: InvariantCheckResult::Violated {
                invariant_id: "inv-1".to_string(),
                snapshot_id: "snap-2".to_string(),
                reason: "State violated".to_string(),
                context: HashMap::new(),
            },
            preserved: false,
        };

        assert!(result.should_block(InvariantSeverity::Critical));
        assert!(result.should_block(InvariantSeverity::Error));
        assert!(!result.should_block(InvariantSeverity::Warning));
    }

    #[test]
    fn test_commit_verification_result() {
        let results = vec![
            PreservationResult {
                invariant_id: "inv-1".to_string(),
                pre_state: InvariantCheckResult::Satisfied {
                    invariant_id: "inv-1".to_string(),
                    snapshot_id: "snap-1".to_string(),
                },
                post_state: InvariantCheckResult::Satisfied {
                    invariant_id: "inv-1".to_string(),
                    snapshot_id: "snap-2".to_string(),
                },
                preserved: true,
            },
            PreservationResult {
                invariant_id: "inv-2".to_string(),
                pre_state: InvariantCheckResult::Satisfied {
                    invariant_id: "inv-2".to_string(),
                    snapshot_id: "snap-1".to_string(),
                },
                post_state: InvariantCheckResult::Violated {
                    invariant_id: "inv-2".to_string(),
                    snapshot_id: "snap-2".to_string(),
                    reason: "Violation".to_string(),
                    context: HashMap::new(),
                },
                preserved: false,
            },
        ];

        let commit_result = CommitVerificationResult::new("commit-1".to_string(), results);

        assert!(commit_result.is_blocked());
        assert_eq!(commit_result.blocking_violations.len(), 1);
        assert_eq!(commit_result.blocking_violations[0], "inv-2");
    }
}
