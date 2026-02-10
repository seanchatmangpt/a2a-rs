//! Port trait for circuit breaker pattern.
//!
//! The circuit breaker prevents cascading failures by monitoring external calls
//! and stopping requests when a service exhibits failure patterns.

use crate::domain::CircuitBreakerError;
use async_trait::async_trait;
use std::time::Duration;

/// Configuration for circuit breaker behavior.
///
/// # Fields
///
/// - `failure_threshold`: Number of consecutive failures before opening the circuit
/// - `success_threshold`: Number of consecutive successes in half-open state to close the circuit
/// - `timeout`: Duration to wait before attempting recovery (half-open state)
/// - `half_open_max_calls`: Maximum calls allowed while in half-open state
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures to trigger circuit open
    pub failure_threshold: u32,

    /// Number of successes to transition from half-open to closed
    pub success_threshold: u32,

    /// Duration to wait before attempting recovery
    pub timeout: Duration,

    /// Maximum concurrent calls allowed in half-open state
    pub half_open_max_calls: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(30),
            half_open_max_calls: 1,
        }
    }
}

/// State of the circuit breaker.
///
/// # Variants
///
/// - `Closed`: Normal state, requests pass through
/// - `Open`: Circuit is open, requests are rejected immediately
/// - `HalfOpen`: Testing recovery, limited requests allowed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed: requests pass through normally
    Closed,

    /// Circuit is open: requests are rejected
    Open,

    /// Circuit is half-open: limited requests allowed for testing recovery
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "Closed"),
            CircuitState::Open => write!(f, "Open"),
            CircuitState::HalfOpen => write!(f, "HalfOpen"),
        }
    }
}

/// Snapshot of circuit breaker state for metrics and monitoring.
#[derive(Debug, Clone)]
pub struct CircuitBreakerSnapshot {
    /// Current state
    pub state: CircuitState,

    /// Number of consecutive failures
    pub failure_count: u32,

    /// Number of consecutive successes (in half-open state)
    pub success_count: u32,

    /// Number of calls processed in current window
    pub call_count: u32,

    /// Total failures recorded
    pub total_failures: u64,

    /// Total successes recorded
    pub total_successes: u64,

    /// Timestamp of last state transition
    pub last_state_change: std::time::SystemTime,
}

/// Trait for circuit breaker implementations.
///
/// The circuit breaker monitors external calls and prevents cascading failures
/// by stopping requests when a service exhibits failure patterns.
///
/// # States
///
/// - **Closed**: Normal operation. Requests pass through and failures are tracked.
/// - **Open**: Too many failures detected. Requests are rejected immediately
///   without calling the external service.
/// - **Half-Open**: Recovery testing phase. Limited requests are allowed through
///   to test if the service has recovered.
///
/// # Usage
///
/// ```ignore
/// use osiris_compiler::port::CircuitBreaker;
/// use osiris_compiler::adapter::StandardCircuitBreaker;
///
/// let breaker = StandardCircuitBreaker::default();
/// match breaker.call(|| async { external_service().await }).await {
///     Ok(result) => println!("Success: {}", result),
///     Err(CircuitBreakerError::CircuitOpen) => println!("Service unavailable"),
///     Err(e) => println!("Error: {}", e),
/// }
/// ```
#[async_trait]
pub trait CircuitBreaker: Send + Sync {
    /// Call an external operation through the circuit breaker.
    ///
    /// This method wraps a potentially failing operation and handles state
    /// transitions based on success/failure patterns.
    ///
    /// # Arguments
    ///
    /// * `operation` - Async function that returns a Result<T, String>
    ///
    /// # Returns
    ///
    /// - `Ok(T)` if operation succeeds
    /// - `Err(CircuitBreakerError::CircuitOpen)` if circuit is open
    /// - `Err(CircuitBreakerError::CircuitHalfOpen)` if all half-open slots are used
    /// - `Err(CircuitBreakerError::OperationFailed)` if operation fails
    /// - `Err(CircuitBreakerError::Timeout)` if operation times out
    async fn call<F, T>(&self, operation: F) -> Result<T, CircuitBreakerError>
    where
        F: std::future::Future<Output = Result<T, String>> + Send,
        T: Send,
    {
        self.call_with_timeout(operation, None).await
    }

    /// Call an operation with explicit timeout.
    ///
    /// Similar to `call()` but allows specifying a custom timeout.
    async fn call_with_timeout<F, T>(
        &self,
        operation: F,
        timeout: Option<Duration>,
    ) -> Result<T, CircuitBreakerError>
    where
        F: std::future::Future<Output = Result<T, String>> + Send,
        T: Send;

    /// Get the current state of the circuit breaker.
    fn state(&self) -> CircuitState;

    /// Get a snapshot of the circuit breaker state for monitoring.
    fn snapshot(&self) -> CircuitBreakerSnapshot;

    /// Manually reset the circuit to closed state.
    ///
    /// This can be used for explicit recovery or testing.
    fn reset(&self) -> Result<(), CircuitBreakerError>;

    /// Manually open the circuit.
    ///
    /// Useful for administrative actions or explicit failure handling.
    fn open(&self) -> Result<(), CircuitBreakerError>;

    /// Record a successful call.
    ///
    /// Called internally by the breaker but can be used for manual tracking.
    fn record_success(&self) -> Result<(), CircuitBreakerError>;

    /// Record a failed call.
    ///
    /// Called internally by the breaker but can be used for manual tracking.
    fn record_failure(&self, reason: String) -> Result<(), CircuitBreakerError>;

    /// Validate the configuration.
    fn validate_config(&self) -> Result<(), CircuitBreakerError>;
}
