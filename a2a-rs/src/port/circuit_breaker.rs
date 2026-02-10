//! Circuit breaker port interface
//!
//! Defines the contract for circuit breaker operations, providing fault tolerance
//! and preventing cascade failures through automatic failure detection and recovery.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are blocked
    Open,
    /// Circuit is half-open, testing if service recovered
    HalfOpen,
}

/// Configuration for circuit breaker behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// Number of successes in half-open state before closing
    pub success_threshold: u32,
    /// Duration to wait before transitioning from open to half-open
    pub timeout: Duration,
    /// Minimum number of requests before evaluating failure rate
    pub minimum_request_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
            minimum_request_threshold: 10,
        }
    }
}

/// Metrics for circuit breaker monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CircuitBreakerMetrics {
    /// Current state of the circuit
    pub state: CircuitState,
    /// Total number of successful requests
    pub success_count: u64,
    /// Total number of failed requests
    pub failure_count: u64,
    /// Number of consecutive failures
    pub consecutive_failures: u32,
    /// Number of consecutive successes in half-open state
    pub consecutive_successes: u32,
    /// Timestamp of last state change (milliseconds since epoch)
    pub last_state_change: u64,
}

/// Result of a circuit breaker check
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitBreakerDecision {
    /// Request is allowed to proceed
    Allowed,
    /// Request is rejected due to open circuit
    Rejected {
        /// Time remaining until circuit transitions to half-open
        retry_after: Duration,
    },
}

impl CircuitBreakerDecision {
    /// Returns true if the request is allowed
    pub fn is_allowed(&self) -> bool {
        matches!(self, CircuitBreakerDecision::Allowed)
    }

    /// Returns the retry-after duration if rejected
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            CircuitBreakerDecision::Allowed => None,
            CircuitBreakerDecision::Rejected { retry_after } => Some(*retry_after),
        }
    }
}

/// Circuit breaker port interface
#[async_trait]
pub trait CircuitBreaker: Send + Sync {
    /// Check if a request should be allowed for the given agent
    async fn check(&self, agent_id: &str) -> CircuitBreakerDecision;

    /// Record a successful request
    async fn record_success(&self, agent_id: &str) -> Result<(), String>;

    /// Record a failed request
    async fn record_failure(&self, agent_id: &str) -> Result<(), String>;

    /// Get current metrics for an agent's circuit breaker
    async fn get_metrics(&self, agent_id: &str) -> Option<CircuitBreakerMetrics>;

    /// Get the current state of an agent's circuit breaker
    async fn get_state(&self, agent_id: &str) -> CircuitState;

    /// Manually reset a circuit breaker to closed state
    async fn reset(&self, agent_id: &str) -> Result<(), String>;

    /// Get all agent IDs currently being tracked
    async fn tracked_agents(&self) -> Vec<String>;

    /// Force a circuit breaker to a specific state (for testing/admin)
    async fn force_state(&self, agent_id: &str, state: CircuitState) -> Result<(), String>;
}

/// Async version of the circuit breaker trait
pub type AsyncCircuitBreaker = dyn CircuitBreaker;
