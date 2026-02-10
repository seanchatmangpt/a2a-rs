//! Rate limiter port definitions
//!
//! Defines the interface for token bucket rate limiting with support
//! for per-IP, per-tenant, and global rate limit enforcement.

use async_trait::async_trait;
use std::fmt;

/// Result type for rate limiter operations
pub type RateLimitResult = Result<(), RateLimitError>;

/// Errors that can occur during rate limiting
#[derive(Debug, Clone)]
pub enum RateLimitError {
    /// Rate limit exceeded for the given key
    RateLimitExceeded {
        /// The key (IP, tenant ID, etc.) that hit the limit
        key: String,
        /// Current request rate (tokens consumed this period)
        current_rate: u32,
        /// Maximum allowed rate (tokens available per period)
        limit: u32,
        /// Retry-After duration in seconds
        retry_after_secs: u64,
    },

    /// Configuration error
    ConfigurationError(String),

    /// Invalid key format
    InvalidKey(String),
}

impl fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RateLimitError::RateLimitExceeded {
                key,
                current_rate,
                limit,
                retry_after_secs,
            } => {
                write!(
                    f,
                    "Rate limit exceeded for '{}': {}/{} requests, retry after {} seconds",
                    key, current_rate, limit, retry_after_secs
                )
            }
            RateLimitError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
            RateLimitError::InvalidKey(msg) => write!(f, "Invalid key: {}", msg),
        }
    }
}

impl std::error::Error for RateLimitError {}

/// Configuration for rate limiter limits
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Requests per second per IP address
    pub per_ip_rps: u32,
    /// Requests per second per tenant ID
    pub per_tenant_rps: u32,
    /// Global requests per second (for entire gateway)
    pub global_rps: u32,
    /// Number of seconds over which to measure rates
    pub window_secs: u64,
}

impl RateLimitConfig {
    /// Create a new rate limit configuration
    pub fn new(per_ip_rps: u32, per_tenant_rps: u32, global_rps: u32, window_secs: u64) -> Self {
        Self {
            per_ip_rps,
            per_tenant_rps,
            global_rps,
            window_secs,
        }
    }

    /// Default configuration: 1000 req/s per IP, 5000 req/s per tenant, 10000 req/s global
    pub fn default() -> Self {
        Self {
            per_ip_rps: 1000,
            per_tenant_rps: 5000,
            global_rps: 10000,
            window_secs: 1,
        }
    }

    /// Strict configuration for testing: 10 req/s per IP, 50 req/s per tenant
    pub fn strict() -> Self {
        Self {
            per_ip_rps: 10,
            per_tenant_rps: 50,
            global_rps: 100,
            window_secs: 1,
        }
    }
}

/// Rate limiter using token bucket algorithm
///
/// Enforces rate limits at three levels:
/// - Per IP address
/// - Per tenant ID
/// - Global gateway level
///
/// Each level uses a token bucket algorithm that refills at a constant rate.
/// When a request arrives, it consumes one or more tokens. If insufficient
/// tokens are available, the request is rejected.
#[async_trait]
pub trait RateLimiter: Send + Sync {
    /// Try to acquire tokens for a request
    ///
    /// Checks rate limits at multiple levels and returns an error if any limit is exceeded.
    /// The `key` parameter should be either an IP address or tenant ID, depending on context.
    ///
    /// # Arguments
    /// * `key` - The identifier to rate limit (IP address or tenant ID)
    /// * `tokens` - Number of tokens to consume (default: 1)
    ///
    /// # Returns
    /// - `Ok(())` if tokens were successfully acquired
    /// - `Err(RateLimitError::RateLimitExceeded)` if any limit was exceeded
    async fn check_rate_limit(&self, key: &str, tokens: u32) -> RateLimitResult;

    /// Try to acquire tokens for a request, by IP address
    ///
    /// Only checks the per-IP rate limit.
    async fn check_ip_limit(&self, ip: &str, tokens: u32) -> RateLimitResult;

    /// Try to acquire tokens for a request, by tenant ID
    ///
    /// Only checks the per-tenant rate limit.
    async fn check_tenant_limit(&self, tenant_id: &str, tokens: u32) -> RateLimitResult;

    /// Check the global rate limit
    ///
    /// Only checks the gateway-wide rate limit.
    async fn check_global_limit(&self, tokens: u32) -> RateLimitResult;

    /// Get the current rate for a key (requests in current window)
    ///
    /// Returns the number of requests seen for this key in the current time window.
    async fn get_rate(&self, key: &str) -> u32;

    /// Get the current global rate (requests in current window)
    async fn get_global_rate(&self) -> u32;

    /// Get the configured rate limit for a key
    async fn get_limit(&self, key: &str) -> u32;

    /// Reset all rate limits (useful for testing)
    async fn reset(&self);

    /// Get configuration
    fn config(&self) -> RateLimitConfig;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_error_display() {
        let err = RateLimitError::RateLimitExceeded {
            key: "192.168.1.1".to_string(),
            current_rate: 101,
            limit: 100,
            retry_after_secs: 1,
        };

        let msg = err.to_string();
        assert!(msg.contains("192.168.1.1"));
        assert!(msg.contains("101/100"));
        assert!(msg.contains("1 seconds"));
    }

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.per_ip_rps, 1000);
        assert_eq!(config.per_tenant_rps, 5000);
        assert_eq!(config.global_rps, 10000);
        assert_eq!(config.window_secs, 1);
    }

    #[test]
    fn test_rate_limit_config_strict() {
        let config = RateLimitConfig::strict();
        assert_eq!(config.per_ip_rps, 10);
        assert_eq!(config.per_tenant_rps, 50);
        assert_eq!(config.global_rps, 100);
        assert_eq!(config.window_secs, 1);
    }
}
