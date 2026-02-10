//! Core domain types for compiler operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use uuid::Uuid;

/// An admissible operation in the Osiris compiler.
///
/// Represents a single unit of work that can be ordered and executed
/// deterministically. Operations have timestamps, unique IDs, and
/// priority levels to establish total ordering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    /// Unique identifier for this operation
    pub id: Uuid,

    /// Timestamp when the operation was created
    pub timestamp: DateTime<Utc>,

    /// Priority level (higher = more important)
    pub priority: u32,

    /// The actual operation payload
    pub kind: OperationKind,

    /// Optional source identifier for conflict resolution
    pub source: Option<String>,
}

/// Types of operations the compiler can execute.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum OperationKind {
    /// Parse source code
    Parse { input: String },

    /// Type check a module
    TypeCheck { module_id: String },

    /// Optimize intermediate representation
    Optimize { ir_id: String, level: u8 },

    /// Generate code
    CodeGen { target: String },

    /// Link modules
    Link { modules: Vec<String> },
}

impl Operation {
    /// Create a new operation with the current timestamp.
    pub fn new(kind: OperationKind, priority: u32) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            priority,
            kind,
            source: None,
        }
    }

    /// Create a new operation with a specific source identifier.
    pub fn with_source(mut self, source: String) -> Self {
        self.source = Some(source);
        self
    }
}

/// Deterministic ordering of operations.
///
/// This establishes a total order by:
/// 1. Priority (higher first)
/// 2. Timestamp (earlier first)
/// 3. UUID (lexicographic as tiebreaker)
///
/// This ensures that given the same set of operations,
/// they will always be ordered the same way.
impl Ord for Operation {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority comes first
        match other.priority.cmp(&self.priority) {
            Ordering::Equal => {
                // Earlier timestamp comes first
                match self.timestamp.cmp(&other.timestamp) {
                    Ordering::Equal => {
                        // UUID as stable tiebreaker
                        self.id.cmp(&other.id)
                    }
                    ord => ord,
                }
            }
            ord => ord,
        }
    }
}

impl PartialOrd for Operation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_operation_ordering_by_priority() {
        let op1 = Operation::new(OperationKind::Parse { input: "a".into() }, 1);
        let op2 = Operation::new(OperationKind::Parse { input: "b".into() }, 2);

        // Higher priority comes first
        assert!(op2 < op1);
    }

    #[test]
    fn test_operation_ordering_by_timestamp() {
        let op1 = Operation::new(OperationKind::Parse { input: "a".into() }, 1);
        sleep(Duration::from_millis(10));
        let op2 = Operation::new(OperationKind::Parse { input: "b".into() }, 1);

        // Earlier timestamp comes first (same priority)
        assert!(op1 < op2);
    }

    #[test]
    fn test_operation_ordering_stability() {
        let mut ops = vec![
            Operation::new(OperationKind::Parse { input: "1".into() }, 2),
            Operation::new(OperationKind::Parse { input: "2".into() }, 1),
            Operation::new(OperationKind::Parse { input: "3".into() }, 2),
            Operation::new(OperationKind::Parse { input: "4".into() }, 1),
        ];

        // Sort multiple times - should always get same result
        ops.sort();
        let first_sort = ops.clone();

        ops.sort();
        assert_eq!(ops, first_sort);
    }
}
