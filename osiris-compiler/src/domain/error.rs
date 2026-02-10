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

/// Errors that can occur in the circuit breaker domain.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CircuitBreakerError {
    /// The circuit is open and rejecting requests
    #[error("Circuit is open: too many recent failures")]
    CircuitOpen,

    /// The circuit is half-open and limited to probe requests
    #[error("Circuit is half-open: waiting for recovery signal")]
    CircuitHalfOpen,

    /// The underlying operation failed
    #[error("Operation failed: {0}")]
    OperationFailed(String),

    /// Configuration validation failed
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Timeout exceeded while waiting for response
    #[error("Request timeout: {0}")]
    Timeout(String),

    /// State transition is invalid
    #[error("Invalid state transition: {0}")]
    InvalidStateTransition(String),
}

/// Errors that can occur with Cloud Tasks queue operations.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum QueueError {
    /// Failed to enqueue a job
    #[error("Failed to enqueue job: {0}")]
    EnqueueFailed(String),

    /// Failed to dequeue a job
    #[error("Failed to dequeue job: {0}")]
    DequeueFailed(String),

    /// Job not found
    #[error("Job not found: {0}")]
    JobNotFound(String),

    /// Invalid job configuration
    #[error("Invalid job configuration: {0}")]
    InvalidJobConfig(String),

    /// OIDC token generation failed
    #[error("OIDC token generation failed: {0}")]
    TokenGenerationFailed(String),

    /// Job retry failed
    #[error("Job retry failed: {0}")]
    RetryFailed(String),

    /// Cloud Tasks API error
    #[error("Cloud Tasks API error: {0}")]
    ApiError(String),

    /// Authentication error
    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),
}
