//! Retry policy port interface
//!
//! Defines the contract for retry logic with exponential backoff,
//! jitter, and circuit breaker integration.

use async_trait::async_trait;
use std::time::Duration;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (0 means no retries)
    pub max_attempts: u32,
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Multiplier for exponential backoff (typically 2.0)
    pub multiplier: f64,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Jitter factor (0.0 to 1.0) to prevent thundering herd
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            multiplier: 2.0,
            max_delay: Duration::from_secs(30),
            jitter_factor: 0.1,
        }
    }
}

/// Result of a retry attempt
#[derive(Debug, Clone)]
pub enum RetryDecision {
    /// Continue with retry after the specified delay
    Retry { delay: Duration, attempt: u32 },
    /// Stop retrying, operation failed
    Stop { attempts: u32 },
    /// Circuit breaker is open, do not retry
    CircuitOpen,
}

/// Trait to determine if an error is transient and should be retried
pub trait RetryableError {
    /// Returns true if the error is transient and the operation should be retried
    fn is_transient(&self) -> bool;
}

/// Retry policy port interface
#[async_trait]
pub trait RetryPolicy: Send + Sync {
    /// Determine whether to retry based on the attempt number and error
    async fn should_retry<E: RetryableError + Send>(
        &self,
        error: &E,
        attempt: u32,
    ) -> RetryDecision;

    /// Calculate the delay for the next retry attempt
    fn calculate_delay(&self, attempt: u32) -> Duration;

    /// Reset the retry state (useful for long-running operations)
    async fn reset(&self);

    /// Get the current configuration
    fn config(&self) -> &RetryConfig;
}

/// Wrapper for retrying async operations
#[async_trait]
pub trait AsyncRetry: Send + Sync {
    /// Execute an async operation with retry logic
    ///
    /// # Type Parameters
    /// * `T` - The successful result type
    /// * `E` - The error type (must implement RetryableError)
    /// * `F` - The async operation to retry
    async fn execute<T, E, F, Fut>(&self, operation: F) -> Result<T, E>
    where
        T: Send + 'static,
        E: RetryableError + Send + 'static,
        F: Fn() -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T, E>> + Send;
}
