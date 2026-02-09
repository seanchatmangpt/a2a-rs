//! Port trait for deterministic operation ordering.

use crate::domain::{Operation, OrderingError};

/// A deterministic orderer that establishes total order over operations.
///
/// This trait defines the contract for ordering admissible operations
/// in a deterministic, repeatable manner. Implementations must guarantee:
///
/// 1. **Determinism**: Same inputs always produce same output order
/// 2. **Totality**: Every pair of operations can be compared
/// 3. **Transitivity**: If A < B and B < C, then A < C
/// 4. **Stability**: Multiple invocations with same input yield same order
/// 5. **Repeatability**: The ordering can be reproduced across runs
///
/// # Law-Based Resolution
///
/// The orderer resolves concurrency through deterministic rules (laws),
/// not negotiation or randomness:
///
/// - Priority levels establish precedence
/// - Timestamps provide causal ordering
/// - Stable tiebreakers (e.g., UUID) ensure repeatability
///
/// # Usage
///
/// ```ignore
/// use osiris_compiler::port::DeterministicOrderer;
/// use osiris_compiler::domain::Operation;
///
/// let orderer = MyOrderer::new();
/// let operations = vec![op1, op2, op3];
/// let ordered = orderer.order(operations)?;
/// ```
pub trait DeterministicOrderer {
    /// Order a set of operations deterministically.
    ///
    /// Given a collection of operations, returns them in a deterministic
    /// total order. The same set of operations will always produce the
    /// same ordering, regardless of input order or timing.
    ///
    /// # Errors
    ///
    /// Returns `OrderingError` if:
    /// - Operations contain invalid data
    /// - Circular dependencies are detected
    /// - Unresolvable conflicts exist
    ///
    /// # Guarantees
    ///
    /// - **Determinism**: `order(ops)` always returns the same sequence
    /// - **Completeness**: All input operations appear in output
    /// - **Uniqueness**: No operation appears more than once
    fn order(&self, operations: Vec<Operation>) -> Result<Vec<Operation>, OrderingError>;

    /// Validate that an operation is admissible for ordering.
    ///
    /// Checks whether an operation meets the criteria for being ordered.
    /// This may include validation of:
    /// - Operation structure
    /// - Priority bounds
    /// - Source identifiers
    /// - Timestamp validity
    ///
    /// # Errors
    ///
    /// Returns `OrderingError::InvalidOperation` if validation fails.
    fn validate(&self, operation: &Operation) -> Result<(), OrderingError> {
        // Default implementation accepts all operations
        let _ = operation;
        Ok(())
    }

    /// Check if two operations conflict.
    ///
    /// Determines whether two operations have unresolvable conflicts
    /// that prevent deterministic ordering. Conflicts might arise from:
    /// - Mutually exclusive resource access
    /// - Incompatible operation types
    /// - Semantic dependencies
    ///
    /// The default implementation assumes no conflicts.
    fn conflicts(&self, a: &Operation, b: &Operation) -> bool {
        let _ = (a, b);
        false
    }
}

#[cfg(feature = "async")]
use async_trait::async_trait;

#[cfg(feature = "async")]
/// Async version of the deterministic orderer.
///
/// Provides the same guarantees as `DeterministicOrderer` but allows
/// for asynchronous validation and conflict detection. Useful when
/// ordering logic needs to consult external systems or databases.
#[async_trait]
pub trait AsyncDeterministicOrderer {
    /// Order operations asynchronously.
    async fn order(&self, operations: Vec<Operation>) -> Result<Vec<Operation>, OrderingError>;

    /// Validate operation asynchronously.
    async fn validate(&self, operation: &Operation) -> Result<(), OrderingError> {
        let _ = operation;
        Ok(())
    }

    /// Check for conflicts asynchronously.
    async fn conflicts(&self, a: &Operation, b: &Operation) -> bool {
        let _ = (a, b);
        false
    }
}
