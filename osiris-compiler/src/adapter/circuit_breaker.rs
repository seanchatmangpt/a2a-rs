//! Standard circuit breaker adapter implementation.
//!
//! This adapter provides a production-ready circuit breaker that monitors
//! external calls and prevents cascading failures through state-based control.

use crate::domain::CircuitBreakerError;
use crate::port::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerSnapshot, CircuitState};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// Internal state maintained by the circuit breaker.
#[derive(Debug, Clone)]
struct InternalState {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    call_count: u32,
    total_failures: u64,
    total_successes: u64,
    last_state_change: SystemTime,
    last_failure_time: Option<SystemTime>,
}

impl Default for InternalState {
    fn default() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            call_count: 0,
            total_failures: 0,
            total_successes: 0,
            last_state_change: SystemTime::now(),
            last_failure_time: None,
        }
    }
}

/// Standard circuit breaker adapter.
///
/// # Features
///
/// - Thread-safe through Arc<RwLock>
/// - Configurable thresholds and timeouts
/// - Half-open state for recovery testing
/// - Comprehensive state tracking and metrics
///
/// # Example
///
/// ```ignore
/// use osiris_compiler::adapter::StandardCircuitBreaker;
/// use osiris_compiler::port::{CircuitBreaker, CircuitBreakerConfig};
/// use std::time::Duration;
///
/// let config = CircuitBreakerConfig {
///     failure_threshold: 5,
///     success_threshold: 2,
///     timeout: Duration::from_secs(30),
///     half_open_max_calls: 1,
/// };
///
/// let breaker = StandardCircuitBreaker::new(config);
///
/// // Use the breaker to wrap external calls
/// let result = breaker.call(async {
///     Ok("Success".to_string())
/// }).await;
/// ```
#[derive(Debug)]
pub struct StandardCircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<InternalState>>,
}

impl StandardCircuitBreaker {
    /// Create a new circuit breaker with custom configuration.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(InternalState::default())),
        }
    }

    /// Create a circuit breaker with default configuration.
    pub fn default() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }

    /// Check if the timeout has passed since last failure.
    async fn should_attempt_recovery(&self) -> bool {
        let state = self.state.read().await;

        if let Some(last_failure) = state.last_failure_time {
            match last_failure.elapsed() {
                Ok(elapsed) => elapsed >= self.config.timeout,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// Transition from Open to HalfOpen state.
    async fn transition_to_half_open(&self) -> Result<(), CircuitBreakerError> {
        let mut state = self.state.write().await;

        if state.state == CircuitState::Open {
            state.state = CircuitState::HalfOpen;
            state.failure_count = 0;
            state.success_count = 0;
            state.call_count = 0;
            state.last_state_change = SystemTime::now();
            Ok(())
        } else {
            Err(CircuitBreakerError::InvalidStateTransition(format!(
                "Cannot transition to HalfOpen from {}",
                state.state
            )))
        }
    }

    /// Transition from HalfOpen to Closed state (recovery successful).
    async fn transition_to_closed(&self) -> Result<(), CircuitBreakerError> {
        let mut state = self.state.write().await;

        if state.state == CircuitState::HalfOpen {
            state.state = CircuitState::Closed;
            state.failure_count = 0;
            state.success_count = 0;
            state.call_count = 0;
            state.last_state_change = SystemTime::now();
            Ok(())
        } else {
            Err(CircuitBreakerError::InvalidStateTransition(format!(
                "Cannot transition to Closed from {}",
                state.state
            )))
        }
    }

    /// Transition to Open state (too many failures).
    async fn transition_to_open(&self) -> Result<(), CircuitBreakerError> {
        let mut state = self.state.write().await;

        if state.state != CircuitState::Open {
            state.state = CircuitState::Open;
            state.failure_count = 0;
            state.success_count = 0;
            state.call_count = 0;
            state.last_failure_time = Some(SystemTime::now());
            state.last_state_change = SystemTime::now();
            Ok(())
        } else {
            Ok(())
        }
    }
}

impl Clone for StandardCircuitBreaker {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            state: Arc::clone(&self.state),
        }
    }
}

#[async_trait]
impl CircuitBreaker for StandardCircuitBreaker {
    async fn call_with_timeout<F, T>(
        &self,
        operation: F,
        timeout: Option<Duration>,
    ) -> Result<T, CircuitBreakerError>
    where
        F: std::future::Future<Output = Result<T, String>> + Send,
        T: Send,
    {
        // Check current state
        let current_state = self.state.read().await.state;

        match current_state {
            CircuitState::Closed => {
                // Execute operation in closed state
                let timeout_duration = timeout.unwrap_or(self.config.timeout);

                match tokio::time::timeout(timeout_duration, operation).await {
                    Ok(Ok(result)) => {
                        // Success in closed state
                        self.record_success().ok();
                        Ok(result)
                    }
                    Ok(Err(err)) => {
                        // Failure in closed state
                        self.record_failure(err.clone()).ok();

                        // Check if we should open the circuit
                        let failure_count = self.state.read().await.failure_count;
                        if failure_count >= self.config.failure_threshold {
                            self.transition_to_open().await.ok();
                        }

                        Err(CircuitBreakerError::OperationFailed(err))
                    }
                    Err(_) => {
                        // Timeout
                        self.record_failure("Timeout".to_string()).ok();

                        let failure_count = self.state.read().await.failure_count;
                        if failure_count >= self.config.failure_threshold {
                            self.transition_to_open().await.ok();
                        }

                        Err(CircuitBreakerError::Timeout(
                            "Operation timeout".to_string(),
                        ))
                    }
                }
            }

            CircuitState::Open => {
                // Check if enough time has passed for recovery attempt
                if self.should_attempt_recovery().await {
                    self.transition_to_half_open().await?;
                    // Retry in half-open state
                    self.call_with_timeout(operation, timeout).await
                } else {
                    Err(CircuitBreakerError::CircuitOpen)
                }
            }

            CircuitState::HalfOpen => {
                // Check if we've reached the call limit for half-open state
                let call_count = self.state.read().await.call_count;
                if call_count >= self.config.half_open_max_calls {
                    return Err(CircuitBreakerError::CircuitHalfOpen);
                }

                // Allow the call in half-open state
                let timeout_duration = timeout.unwrap_or(self.config.timeout);

                match tokio::time::timeout(timeout_duration, operation).await {
                    Ok(Ok(result)) => {
                        // Success in half-open state
                        self.record_success().ok();

                        let success_count = self.state.read().await.success_count;
                        if success_count >= self.config.success_threshold {
                            self.transition_to_closed().await?;
                        }

                        Ok(result)
                    }
                    Ok(Err(err)) => {
                        // Failure in half-open state - reopen immediately
                        self.transition_to_open().await?;
                        Err(CircuitBreakerError::OperationFailed(err))
                    }
                    Err(_) => {
                        // Timeout in half-open state - reopen immediately
                        self.transition_to_open().await?;
                        Err(CircuitBreakerError::Timeout(
                            "Operation timeout in half-open state".to_string(),
                        ))
                    }
                }
            }
        }
    }

    fn state(&self) -> CircuitState {
        match self.state.try_read() {
            Ok(state) => state.state,
            Err(_) => CircuitState::Open, // Return Open as a safe default on lock contention
        }
    }

    fn snapshot(&self) -> CircuitBreakerSnapshot {
        match self.state.try_read() {
            Ok(state) => CircuitBreakerSnapshot {
                state: state.state,
                failure_count: state.failure_count,
                success_count: state.success_count,
                call_count: state.call_count,
                total_failures: state.total_failures,
                total_successes: state.total_successes,
                last_state_change: state.last_state_change,
            },
            Err(_) => {
                // Return a snapshot representing contention
                CircuitBreakerSnapshot {
                    state: CircuitState::Open,
                    failure_count: 0,
                    success_count: 0,
                    call_count: 0,
                    total_failures: 0,
                    total_successes: 0,
                    last_state_change: SystemTime::now(),
                }
            }
        }
    }

    fn reset(&self) -> Result<(), CircuitBreakerError> {
        match self.state.try_write() {
            Ok(mut state) => {
                *state = InternalState::default();
                Ok(())
            }
            Err(_) => Err(CircuitBreakerError::InvalidStateTransition(
                "Cannot acquire write lock to reset state".to_string(),
            )),
        }
    }

    fn open(&self) -> Result<(), CircuitBreakerError> {
        match self.state.try_write() {
            Ok(mut state) => {
                if state.state != CircuitState::Open {
                    state.state = CircuitState::Open;
                    state.last_state_change = SystemTime::now();
                    state.last_failure_time = Some(SystemTime::now());
                }
                Ok(())
            }
            Err(_) => Err(CircuitBreakerError::InvalidStateTransition(
                "Cannot acquire write lock to open circuit".to_string(),
            )),
        }
    }

    fn record_success(&self) -> Result<(), CircuitBreakerError> {
        match self.state.try_write() {
            Ok(mut state) => {
                state.call_count += 1;
                state.total_successes += 1;

                if state.state == CircuitState::HalfOpen || state.state == CircuitState::Closed {
                    state.success_count += 1;
                    state.failure_count = 0; // Reset failure count on success
                }

                Ok(())
            }
            Err(_) => Err(CircuitBreakerError::InvalidStateTransition(
                "Cannot acquire write lock to record success".to_string(),
            )),
        }
    }

    fn record_failure(&self, reason: String) -> Result<(), CircuitBreakerError> {
        match self.state.try_write() {
            Ok(mut state) => {
                state.call_count += 1;
                state.total_failures += 1;
                state.failure_count += 1;
                state.success_count = 0; // Reset success count on failure
                state.last_failure_time = Some(SystemTime::now());

                Ok(())
            }
            Err(_) => Err(CircuitBreakerError::InvalidStateTransition(format!(
                "Cannot acquire write lock to record failure: {}",
                reason
            ))),
        }
    }

    fn validate_config(&self) -> Result<(), CircuitBreakerError> {
        if self.config.failure_threshold == 0 {
            return Err(CircuitBreakerError::InvalidConfig(
                "failure_threshold must be > 0".to_string(),
            ));
        }

        if self.config.success_threshold == 0 {
            return Err(CircuitBreakerError::InvalidConfig(
                "success_threshold must be > 0".to_string(),
            ));
        }

        if self.config.timeout == Duration::ZERO {
            return Err(CircuitBreakerError::InvalidConfig(
                "timeout must be > 0".to_string(),
            ));
        }

        if self.config.half_open_max_calls == 0 {
            return Err(CircuitBreakerError::InvalidConfig(
                "half_open_max_calls must be > 0".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_closed_state() {
        let breaker = StandardCircuitBreaker::default();

        assert_eq!(breaker.state(), CircuitState::Closed);

        let result = breaker
            .call(async { Ok::<String, String>("success".into()) })
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_circuit_breaker_failure_opens_circuit() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 1,
            timeout: Duration::from_millis(100),
            half_open_max_calls: 1,
        };
        let breaker = StandardCircuitBreaker::new(config);

        // First failure
        let _ = breaker
            .call(async { Err::<String, String>("error 1".into()) })
            .await;

        assert_eq!(breaker.state(), CircuitState::Closed);

        // Second failure - should open circuit
        let _ = breaker
            .call(async { Err::<String, String>("error 2".into()) })
            .await;

        assert_eq!(breaker.state(), CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_open_rejects_calls() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 1,
            timeout: Duration::from_millis(100),
            half_open_max_calls: 1,
        };
        let breaker = StandardCircuitBreaker::new(config);

        // Open the circuit
        breaker.open().unwrap();
        assert_eq!(breaker.state(), CircuitState::Open);

        // Call should be rejected immediately
        let result = breaker
            .call(async { Ok::<String, String>("should not execute".into()) })
            .await;

        assert!(matches!(result, Err(CircuitBreakerError::CircuitOpen)));
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_recovery() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 1,
            timeout: Duration::from_millis(100),
            half_open_max_calls: 2,
        };
        let breaker = StandardCircuitBreaker::new(config);

        // Trigger circuit open
        let _ = breaker
            .call(async { Err::<String, String>("error".into()) })
            .await;

        assert_eq!(breaker.state(), CircuitState::Open);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Next call transitions to half-open and succeeds
        let result = breaker
            .call(async { Ok::<String, String>("success".into()) })
            .await;

        assert!(result.is_ok());
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_max_calls() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 2,
            timeout: Duration::from_millis(50),
            half_open_max_calls: 1,
        };
        let breaker = StandardCircuitBreaker::new(config);

        // Open circuit
        breaker.open().unwrap();

        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Force transition to half-open
        breaker.transition_to_half_open().await.unwrap();
        assert_eq!(breaker.state(), CircuitState::HalfOpen);

        // First call succeeds
        let result1 = breaker
            .call(async { Ok::<String, String>("success1".into()) })
            .await;
        assert!(result1.is_ok());

        // Second call should be rejected (max calls exceeded)
        let result2 = breaker
            .call(async { Ok::<String, String>("success2".into()) })
            .await;

        assert!(matches!(result2, Err(CircuitBreakerError::CircuitHalfOpen)));
    }

    #[tokio::test]
    async fn test_circuit_breaker_timeout() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 1,
            timeout: Duration::from_millis(50),
            half_open_max_calls: 1,
        };
        let breaker = StandardCircuitBreaker::new(config);

        let result = breaker
            .call_with_timeout(
                async {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok::<String, String>("too late".into())
                },
                Some(Duration::from_millis(50)),
            )
            .await;

        assert!(matches!(result, Err(CircuitBreakerError::Timeout(_))));
    }

    #[test]
    fn test_circuit_breaker_snapshot() {
        let breaker = StandardCircuitBreaker::default();
        let snapshot = breaker.snapshot();

        assert_eq!(snapshot.state, CircuitState::Closed);
        assert_eq!(snapshot.failure_count, 0);
        assert_eq!(snapshot.success_count, 0);
    }

    #[test]
    fn test_circuit_breaker_reset() {
        let breaker = StandardCircuitBreaker::default();

        breaker.open().unwrap();
        assert_eq!(breaker.state(), CircuitState::Open);

        breaker.reset().unwrap();
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_validate_config() {
        let config = CircuitBreakerConfig::default();
        let breaker = StandardCircuitBreaker::new(config);

        assert!(breaker.validate_config().is_ok());
    }

    #[test]
    fn test_circuit_breaker_validate_zero_failure_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 0,
            ..Default::default()
        };
        let breaker = StandardCircuitBreaker::new(config);

        assert!(breaker.validate_config().is_err());
    }

    #[tokio::test]
    async fn test_circuit_breaker_records_metrics() {
        let breaker = StandardCircuitBreaker::default();

        // Successful call
        let _ = breaker
            .call(async { Ok::<String, String>("success".into()) })
            .await;

        let snapshot = breaker.snapshot();
        assert_eq!(snapshot.total_successes, 1);
        assert_eq!(snapshot.call_count, 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker_cloning() {
        let breaker1 = StandardCircuitBreaker::default();

        breaker1.open().unwrap();

        let breaker2 = breaker1.clone();
        assert_eq!(breaker2.state(), CircuitState::Open);

        // Both should share the same underlying state
        breaker2.reset().unwrap();
        assert_eq!(breaker1.state(), CircuitState::Closed);
    }
}
