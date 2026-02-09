//! Lambda-based deterministic orderer implementation.
//!
//! This adapter implements deterministic ordering using a law-based approach
//! rather than negotiation. It establishes total order through:
//!
//! 1. Priority-based precedence (higher priority first)
//! 2. Timestamp-based causality (earlier first)
//! 3. UUID-based tiebreaker (stable, deterministic)
//!
//! The name "lambda" (Λ) reflects its foundation in formal ordering laws,
//! similar to lambda calculus establishing formal computation rules.

use crate::domain::{Operation, OrderingError};
use crate::port::DeterministicOrderer;
#[allow(unused_imports)] // HashMap used in tests
use std::collections::{HashMap, HashSet};

/// Configuration for the lambda orderer.
#[derive(Debug, Clone)]
pub struct LambdaOrdererConfig {
    /// Maximum allowed priority level (for validation)
    pub max_priority: u32,

    /// Whether to enforce strict source validation
    pub strict_sources: bool,

    /// Enable conflict detection between operations
    pub detect_conflicts: bool,
}

impl Default for LambdaOrdererConfig {
    fn default() -> Self {
        Self {
            max_priority: 100,
            strict_sources: false,
            detect_conflicts: false,
        }
    }
}

/// Deterministic orderer using lambda (Λ) ordering laws.
///
/// This orderer implements deterministic scheduling by establishing
/// a total order over operations through well-defined rules:
///
/// # Ordering Laws (Λ-Laws)
///
/// For any two operations A and B:
///
/// ```text
/// A < B ⟺ priority(A) > priority(B)  ∨
///         (priority(A) = priority(B) ∧ timestamp(A) < timestamp(B))  ∨
///         (priority(A) = priority(B) ∧ timestamp(A) = timestamp(B) ∧ id(A) < id(B))
/// ```
///
/// # Properties
///
/// - **Determinism**: Same input → same output order
/// - **Totality**: All operation pairs can be compared
/// - **Transitivity**: A < B ∧ B < C → A < C
/// - **Stability**: Multiple runs produce identical results
/// - **Repeatability**: Order is reproducible across systems
///
/// # Example
///
/// ```
/// use osiris_compiler::adapter::LambdaOrderer;
/// use osiris_compiler::domain::{Operation, OperationKind};
/// use osiris_compiler::port::DeterministicOrderer;
///
/// let orderer = LambdaOrderer::default();
///
/// let ops = vec![
///     Operation::new(OperationKind::Parse { input: "main.rs".into() }, 1),
///     Operation::new(OperationKind::TypeCheck { module_id: "main".into() }, 2),
/// ];
///
/// let ordered = orderer.order(ops).unwrap();
/// // TypeCheck comes first (higher priority)
/// ```
#[derive(Debug, Clone)]
pub struct LambdaOrderer {
    config: LambdaOrdererConfig,
}

impl LambdaOrderer {
    /// Create a new lambda orderer with the given configuration.
    pub fn new(config: LambdaOrdererConfig) -> Self {
        Self { config }
    }

    /// Create a lambda orderer with default configuration.
    pub fn default() -> Self {
        Self::new(LambdaOrdererConfig::default())
    }

    /// Detect circular dependencies in operations.
    ///
    /// This is a placeholder for dependency analysis. In a real compiler,
    /// this would check for cycles in the operation dependency graph.
    fn detect_cycles(&self, operations: &[Operation]) -> Result<(), OrderingError> {
        // Simple check: if any operation depends on itself (via source)
        let mut sources = HashSet::new();

        for op in operations {
            if let Some(source) = &op.source {
                if !sources.insert(source.clone()) {
                    return Err(OrderingError::CircularDependency(format!(
                        "Duplicate source: {}",
                        source
                    )));
                }
            }
        }

        Ok(())
    }

    /// Detect conflicts between operations.
    ///
    /// Conflicts arise when operations cannot be meaningfully ordered
    /// relative to each other. This implementation checks for basic
    /// semantic conflicts based on operation types.
    fn detect_operation_conflicts(&self, operations: &[Operation]) -> Result<(), OrderingError> {
        if !self.config.detect_conflicts {
            return Ok(());
        }

        // Group operations by kind to detect conflicts
        let mut parse_ops = 0;
        let mut codegen_ops = 0;

        for op in operations {
            match op.kind {
                crate::domain::OperationKind::Parse { .. } => parse_ops += 1,
                crate::domain::OperationKind::CodeGen { .. } => codegen_ops += 1,
                _ => {}
            }
        }

        // Example: can't have multiple parse operations (simplistic rule)
        if parse_ops > 1 && self.config.strict_sources {
            return Err(OrderingError::Conflict(
                "Multiple parse operations detected".into(),
            ));
        }

        Ok(())
    }
}

impl DeterministicOrderer for LambdaOrderer {
    fn order(&self, mut operations: Vec<Operation>) -> Result<Vec<Operation>, OrderingError> {
        // Validate all operations first
        for op in &operations {
            self.validate(op)?;
        }

        // Detect cycles and conflicts
        self.detect_cycles(&operations)?;
        self.detect_operation_conflicts(&operations)?;

        // Apply deterministic ordering using built-in Ord implementation
        // Rust's sort() is stable, ensuring deterministic results
        operations.sort();

        Ok(operations)
    }

    fn validate(&self, operation: &Operation) -> Result<(), OrderingError> {
        // Check priority bounds
        if operation.priority > self.config.max_priority {
            return Err(OrderingError::InvalidOperation(format!(
                "Priority {} exceeds maximum {}",
                operation.priority, self.config.max_priority
            )));
        }

        // If strict sources enabled, require source identifier
        if self.config.strict_sources && operation.source.is_none() {
            return Err(OrderingError::InvalidOperation(
                "Source identifier required in strict mode".into(),
            ));
        }

        Ok(())
    }

    fn conflicts(&self, a: &Operation, b: &Operation) -> bool {
        if !self.config.detect_conflicts {
            return false;
        }

        // Example conflict detection: same operation kind with same target
        match (&a.kind, &b.kind) {
            (
                crate::domain::OperationKind::TypeCheck { module_id: m1 },
                crate::domain::OperationKind::TypeCheck { module_id: m2 },
            ) => m1 == m2,
            (
                crate::domain::OperationKind::CodeGen { target: t1 },
                crate::domain::OperationKind::CodeGen { target: t2 },
            ) => t1 == t2,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::OperationKind;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_deterministic_ordering() {
        let orderer = LambdaOrderer::default();

        let ops = vec![
            Operation::new(OperationKind::Parse { input: "1".into() }, 1),
            Operation::new(OperationKind::Parse { input: "2".into() }, 2),
            Operation::new(OperationKind::Parse { input: "3".into() }, 1),
        ];

        let ordered1 = orderer.order(ops.clone()).unwrap();
        let ordered2 = orderer.order(ops).unwrap();

        // Same input should produce same order
        assert_eq!(ordered1, ordered2);

        // Higher priority comes first
        assert_eq!(ordered1[0].priority, 2);
    }

    #[test]
    fn test_priority_ordering() {
        let orderer = LambdaOrderer::default();

        let ops = vec![
            Operation::new(
                OperationKind::Parse {
                    input: "low".into(),
                },
                1,
            ),
            Operation::new(
                OperationKind::Parse {
                    input: "high".into(),
                },
                10,
            ),
            Operation::new(
                OperationKind::Parse {
                    input: "mid".into(),
                },
                5,
            ),
        ];

        let ordered = orderer.order(ops).unwrap();

        // Should be ordered: high (10), mid (5), low (1)
        assert_eq!(ordered[0].priority, 10);
        assert_eq!(ordered[1].priority, 5);
        assert_eq!(ordered[2].priority, 1);
    }

    #[test]
    fn test_timestamp_ordering() {
        let orderer = LambdaOrderer::default();

        let op1 = Operation::new(OperationKind::Parse { input: "1".into() }, 1);
        sleep(Duration::from_millis(10));
        let op2 = Operation::new(OperationKind::Parse { input: "2".into() }, 1);
        sleep(Duration::from_millis(10));
        let op3 = Operation::new(OperationKind::Parse { input: "3".into() }, 1);

        let ordered = orderer
            .order(vec![op3.clone(), op1.clone(), op2.clone()])
            .unwrap();

        // Same priority, so ordered by timestamp (earliest first)
        assert_eq!(ordered[0].id, op1.id);
        assert_eq!(ordered[1].id, op2.id);
        assert_eq!(ordered[2].id, op3.id);
    }

    #[test]
    fn test_validation_max_priority() {
        let config = LambdaOrdererConfig {
            max_priority: 10,
            ..Default::default()
        };
        let orderer = LambdaOrderer::new(config);

        let op = Operation::new(
            OperationKind::Parse {
                input: "test".into(),
            },
            100,
        );

        let result = orderer.order(vec![op]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            OrderingError::InvalidOperation(_)
        ));
    }

    #[test]
    fn test_validation_strict_sources() {
        let config = LambdaOrdererConfig {
            strict_sources: true,
            ..Default::default()
        };
        let orderer = LambdaOrderer::new(config);

        let op = Operation::new(
            OperationKind::Parse {
                input: "test".into(),
            },
            1,
        );

        let result = orderer.order(vec![op]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            OrderingError::InvalidOperation(_)
        ));
    }

    #[test]
    fn test_conflict_detection() {
        let config = LambdaOrdererConfig {
            detect_conflicts: true,
            ..Default::default()
        };
        let orderer = LambdaOrderer::new(config);

        let op1 = Operation::new(
            OperationKind::TypeCheck {
                module_id: "main".into(),
            },
            1,
        );
        let op2 = Operation::new(
            OperationKind::TypeCheck {
                module_id: "main".into(),
            },
            2,
        );

        assert!(orderer.conflicts(&op1, &op2));
    }

    #[test]
    fn test_repeatability() {
        let orderer = LambdaOrderer::default();

        let ops = vec![
            Operation::new(OperationKind::Parse { input: "a".into() }, 1),
            Operation::new(
                OperationKind::TypeCheck {
                    module_id: "b".into(),
                },
                2,
            ),
            Operation::new(
                OperationKind::Optimize {
                    ir_id: "c".into(),
                    level: 2,
                },
                3,
            ),
            Operation::new(OperationKind::CodeGen { target: "d".into() }, 1),
            Operation::new(
                OperationKind::Link {
                    modules: vec!["e".into()],
                },
                2,
            ),
        ];

        // Run ordering multiple times
        let mut results = Vec::new();
        for _ in 0..5 {
            results.push(orderer.order(ops.clone()).unwrap());
        }

        // All results should be identical
        for i in 1..results.len() {
            assert_eq!(results[0], results[i]);
        }
    }
}
