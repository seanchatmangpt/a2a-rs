//! Rate limiting port interface
//!
//! Defines the contract for rate limiting operations, supporting adaptive
//! rate limiting with per-agent controls and circuit breaker integration.

use async_trait::async_trait;
use std::time::Duration;

/// Metrics collected for rate limit decision making
#[derive(Debug, Clone)]
pub struct RateLimitMetrics {
    /// Current throughput (requests per second)
    pub throughput: f64,
    /// P99 latency in milliseconds
    pub latency_p99: Duration,
    /// Error rate (0.0 to 1.0)
    pub error_rate: f64,
    /// Current limit
    pub current_limit: u32,
    /// Available tokens
    pub available_tokens: f64,
}

/// Result of a rate limit check
#[derive(Debug, Clone)]
pub enum RateLimitDecision {
    /// Request is allowed to proceed
    Allowed,
    /// Request is rate limited, with retry-after duration
    Limited { retry_after: Duration },
    /// Circuit is open, requests blocked
    CircuitOpen { retry_after: Duration },
}

impl RateLimitDecision {
    /// Returns true if the request is allowed
    pub fn is_allowed(&self) -> bool {
        matches!(self, RateLimitDecision::Allowed)
    }

    /// Returns the retry-after duration if limited
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            RateLimitDecision::Allowed => None,
            RateLimitDecision::Limited { retry_after } => Some(*retry_after),
            RateLimitDecision::CircuitOpen { retry_after } => Some(*retry_after),
        }
    }
}

/// Rate limiter port interface
#[async_trait]
pub trait RateLimiter: Send + Sync {
    /// Check if a request is allowed for the given agent
    async fn check_rate_limit(&self, agent_id: &str) -> RateLimitDecision;

    /// Record the result of a request for adaptive learning
    async fn record_request(
        &self,
        agent_id: &str,
        latency: Duration,
        success: bool,
    ) -> Result<(), String>;

    /// Get current metrics for an agent
    async fn get_metrics(&self, agent_id: &str) -> Option<RateLimitMetrics>;

    /// Reset rate limits for an agent
    async fn reset(&self, agent_id: &str) -> Result<(), String>;

    /// Get all agent IDs currently being tracked
    async fn tracked_agents(&self) -> Vec<String>;
}

/// Async version of the rate limiter trait
pub type AsyncRateLimiter = dyn RateLimiter;
