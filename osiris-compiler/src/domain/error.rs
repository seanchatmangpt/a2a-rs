//! Domain-level error types.

use thiserror::Error;

/// Errors that can occur in the ordering domain.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum OrderingError {
    /// An operation failed validation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Circular dependency detected
    #[error("Circular dependency detected involving: {0}")]
    CircularDependency(String),

    /// Conflicting operations cannot be ordered
    #[error("Conflicting operations: {0}")]
    Conflict(String),

    /// Generic ordering failure
    #[error("Failed to order operations: {0}")]
    OrderingFailed(String),
}
