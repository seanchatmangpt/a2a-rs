//! Token bucket rate limiter implementation using tower-governor
//!
//! Provides rate limiting with support for:
//! - Per-IP address rate limits
//! - Per-tenant rate limits
//! - Global gateway-wide rate limits
//! - Token bucket algorithm with configurable refill rates
//! - Non-blocking operations (no queueing)

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
// use tower_governor::{governor, governor::state::NotKeyed};  // Not available on crates.io
use tracing::{debug, warn};

use crate::port::{RateLimitConfig, RateLimitError, RateLimitResult, RateLimiter};

/// Token bucket state with timestamps for window management
#[derive(Debug, Clone)]
struct TokenBucketState {
    /// Tokens available in current window
    tokens: f64,
    /// Last refill timestamp (seconds since epoch)
    last_refill: f64,
    /// Refill rate (tokens per second)
    refill_rate: f64,
    /// Maximum tokens to hold
    max_tokens: f64,
}

impl TokenBucketState {
    fn new(refill_rate: f64) -> Self {
        Self {
            tokens: refill_rate,
            last_refill: Self::now(),
            refill_rate,
            max_tokens: refill_rate,
        }
    }

    fn now() -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Self::now();
        let elapsed = now - self.last_refill;
        let new_tokens = elapsed * self.refill_rate;

        self.tokens = (self.tokens + new_tokens).min(self.max_tokens);
        self.last_refill = now;
    }

    /// Try to consume tokens
    fn try_consume(&mut self, tokens: f64) -> bool {
        self.refill();

        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    /// Get current tokens without consuming
    fn current_tokens(&mut self) -> f64 {
        self.refill();
        self.tokens
    }

    /// Get time until next token is available
    fn time_until_available(&mut self) -> f64 {
        self.refill();

        if self.tokens >= 1.0 {
            0.0
        } else {
            (1.0 - self.tokens) / self.refill_rate
        }
    }
}

/// Token bucket rate limiter implementation
///
/// Uses the token bucket algorithm for rate limiting at three levels:
/// - Per IP address
/// - Per tenant ID
/// - Global gateway-wide
///
/// Each limiter maintains its own state and refills at a constant rate.
#[derive(Clone)]
pub struct TokenBucketRateLimiter {
    config: RateLimitConfig,
    /// Per-IP rate limiters
    ip_limiters: Arc<RwLock<HashMap<String, TokenBucketState>>>,
    /// Per-tenant rate limiters
    tenant_limiters: Arc<RwLock<HashMap<String, TokenBucketState>>>,
    /// Global rate limiter
    global_limiter: Arc<RwLock<TokenBucketState>>,
}

impl TokenBucketRateLimiter {
    /// Create a new token bucket rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        let global_limiter = TokenBucketState::new(config.global_rps as f64);

        Self {
            config,
            ip_limiters: Arc::new(RwLock::new(HashMap::new())),
            tenant_limiters: Arc::new(RwLock::new(HashMap::new())),
            global_limiter: Arc::new(RwLock::new(global_limiter)),
        }
    }

    /// Create a rate limiter with default configuration
    pub fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }

    /// Create a rate limiter with strict limits (useful for testing)
    pub fn strict() -> Self {
        Self::new(RateLimitConfig::strict())
    }

    /// Get or create a per-IP limiter
    async fn get_or_create_ip_limiter(&self, ip: &str) -> TokenBucketState {
        let mut limiters = self.ip_limiters.write().await;

        limiters
            .entry(ip.to_string())
            .or_insert_with(|| TokenBucketState::new(self.config.per_ip_rps as f64))
            .clone()
    }

    /// Get or create a per-tenant limiter
    async fn get_or_create_tenant_limiter(&self, tenant_id: &str) -> TokenBucketState {
        let mut limiters = self.tenant_limiters.write().await;

        limiters
            .entry(tenant_id.to_string())
            .or_insert_with(|| TokenBucketState::new(self.config.per_tenant_rps as f64))
            .clone()
    }

    /// Update a per-IP limiter
    async fn update_ip_limiter(&self, ip: &str, state: TokenBucketState) {
        let mut limiters = self.ip_limiters.write().await;
        limiters.insert(ip.to_string(), state);
    }

    /// Update a per-tenant limiter
    async fn update_tenant_limiter(&self, tenant_id: &str, state: TokenBucketState) {
        let mut limiters = self.tenant_limiters.write().await;
        limiters.insert(tenant_id.to_string(), state);
    }
}

#[async_trait]
impl RateLimiter for TokenBucketRateLimiter {
    async fn check_rate_limit(&self, key: &str, tokens: u32) -> RateLimitResult {
        // Check if key looks like an IP address
        if key.contains('.') || key.contains(':') {
            self.check_ip_limit(key, tokens).await
        } else {
            self.check_tenant_limit(key, tokens).await
        }
    }

    async fn check_ip_limit(&self, ip: &str, tokens: u32) -> RateLimitResult {
        let tokens_f64 = tokens as f64;

        // Get or create IP limiter
        let mut ip_state = self.get_or_create_ip_limiter(ip).await;

        // Check global limit first
        let global_result = self.check_global_limit(tokens).await;
        if global_result.is_err() {
            return global_result;
        }

        // Check IP limit
        if ip_state.try_consume(tokens_f64) {
            // Update the limiter state
            self.update_ip_limiter(ip, ip_state).await;
            debug!(ip, tokens, "IP rate limit check passed");
            Ok(())
        } else {
            let current_rate = ((ip_state.max_tokens - ip_state.tokens).round()) as u32;
            let retry_after = ip_state.time_until_available().ceil() as u64;

            warn!(
                ip,
                current_rate,
                limit = self.config.per_ip_rps,
                retry_after_secs = retry_after,
                "IP rate limit exceeded"
            );

            Err(RateLimitError::RateLimitExceeded {
                key: ip.to_string(),
                current_rate,
                limit: self.config.per_ip_rps,
                retry_after_secs: retry_after,
            })
        }
    }

    async fn check_tenant_limit(&self, tenant_id: &str, tokens: u32) -> RateLimitResult {
        let tokens_f64 = tokens as f64;

        // Get or create tenant limiter
        let mut tenant_state = self.get_or_create_tenant_limiter(tenant_id).await;

        // Check global limit first
        let global_result = self.check_global_limit(tokens).await;
        if global_result.is_err() {
            return global_result;
        }

        // Check tenant limit
        if tenant_state.try_consume(tokens_f64) {
            // Update the limiter state
            self.update_tenant_limiter(tenant_id, tenant_state).await;
            debug!(tenant_id, tokens, "Tenant rate limit check passed");
            Ok(())
        } else {
            let current_rate = ((tenant_state.max_tokens - tenant_state.tokens).round()) as u32;
            let retry_after = tenant_state.time_until_available().ceil() as u64;

            warn!(
                tenant_id,
                current_rate,
                limit = self.config.per_tenant_rps,
                retry_after_secs = retry_after,
                "Tenant rate limit exceeded"
            );

            Err(RateLimitError::RateLimitExceeded {
                key: tenant_id.to_string(),
                current_rate,
                limit: self.config.per_tenant_rps,
                retry_after_secs: retry_after,
            })
        }
    }

    async fn check_global_limit(&self, tokens: u32) -> RateLimitResult {
        let tokens_f64 = tokens as f64;
        let mut global_state = self.global_limiter.write().await;

        if global_state.try_consume(tokens_f64) {
            debug!(tokens, "Global rate limit check passed");
            Ok(())
        } else {
            let current_rate = ((global_state.max_tokens - global_state.tokens).round()) as u32;
            let retry_after = global_state.time_until_available().ceil() as u64;

            warn!(
                current_rate,
                limit = self.config.global_rps,
                retry_after_secs = retry_after,
                "Global rate limit exceeded"
            );

            Err(RateLimitError::RateLimitExceeded {
                key: "global".to_string(),
                current_rate,
                limit: self.config.global_rps,
                retry_after_secs: retry_after,
            })
        }
    }

    async fn get_rate(&self, key: &str) -> u32 {
        if key.contains('.') || key.contains(':') {
            // It's an IP
            let mut ip_state = self.get_or_create_ip_limiter(key).await;
            ((ip_state.max_tokens - ip_state.current_tokens()).round()) as u32
        } else {
            // It's a tenant ID
            let mut tenant_state = self.get_or_create_tenant_limiter(key).await;
            ((tenant_state.max_tokens - tenant_state.current_tokens()).round()) as u32
        }
    }

    async fn get_global_rate(&self) -> u32 {
        let mut global_state = self.global_limiter.write().await;
        ((global_state.max_tokens - global_state.current_tokens()).round()) as u32
    }

    async fn get_limit(&self, key: &str) -> u32 {
        if key.contains('.') || key.contains(':') {
            self.config.per_ip_rps
        } else {
            self.config.per_tenant_rps
        }
    }

    async fn reset(&self) {
        let mut ip_limiters = self.ip_limiters.write().await;
        ip_limiters.clear();

        let mut tenant_limiters = self.tenant_limiters.write().await;
        tenant_limiters.clear();

        let mut global_limiter = self.global_limiter.write().await;
        *global_limiter = TokenBucketState::new(self.config.global_rps as f64);

        debug!("Rate limiters reset");
    }

    fn config(&self) -> RateLimitConfig {
        self.config.clone()
    }
}

impl std::fmt::Debug for TokenBucketRateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenBucketRateLimiter")
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new_rate_limiter() {
        let limiter = TokenBucketRateLimiter::new(RateLimitConfig::default());
        assert_eq!(limiter.config.per_ip_rps, 1000);
        assert_eq!(limiter.config.per_tenant_rps, 5000);
        assert_eq!(limiter.config.global_rps, 10000);
    }

    #[tokio::test]
    async fn test_ip_rate_limit_allow() {
        let limiter = TokenBucketRateLimiter::strict();
        let ip = "192.168.1.1";

        // Should allow first few requests
        for _ in 0..5 {
            assert!(limiter.check_ip_limit(ip, 1).await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_ip_rate_limit_exceed() {
        let config = RateLimitConfig::new(5, 50, 100, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let ip = "192.168.1.1";

        // Consume all tokens
        for _ in 0..5 {
            let _ = limiter.check_ip_limit(ip, 1).await;
        }

        // Next request should fail
        let result = limiter.check_ip_limit(ip, 1).await;
        assert!(result.is_err());

        match result {
            Err(RateLimitError::RateLimitExceeded {
                key,
                limit,
                retry_after_secs,
                ..
            }) => {
                assert_eq!(key, ip);
                assert_eq!(limit, 5);
                assert!(retry_after_secs > 0);
            }
            _ => panic!("Expected RateLimitExceeded error"),
        }
    }

    #[tokio::test]
    async fn test_tenant_rate_limit_allow() {
        let limiter = TokenBucketRateLimiter::strict();
        let tenant_id = "tenant-123";

        // Should allow first few requests
        for _ in 0..5 {
            assert!(limiter.check_tenant_limit(tenant_id, 1).await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_tenant_rate_limit_exceed() {
        let config = RateLimitConfig::new(100, 10, 200, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let tenant_id = "tenant-123";

        // Consume all tokens
        for _ in 0..10 {
            let _ = limiter.check_tenant_limit(tenant_id, 1).await;
        }

        // Next request should fail
        let result = limiter.check_tenant_limit(tenant_id, 1).await;
        assert!(result.is_err());

        match result {
            Err(RateLimitError::RateLimitExceeded {
                key,
                limit,
                retry_after_secs,
                ..
            }) => {
                assert_eq!(key, tenant_id);
                assert_eq!(limit, 10);
                assert!(retry_after_secs > 0);
            }
            _ => panic!("Expected RateLimitExceeded error"),
        }
    }

    #[tokio::test]
    async fn test_global_rate_limit_exceed() {
        let config = RateLimitConfig::new(100, 100, 5, 1);
        let limiter = TokenBucketRateLimiter::new(config);

        // Consume all global tokens
        for _ in 0..5 {
            let _ = limiter.check_global_limit(1).await;
        }

        // Next request should fail
        let result = limiter.check_global_limit(1).await;
        assert!(result.is_err());

        match result {
            Err(RateLimitError::RateLimitExceeded { key, limit, .. }) => {
                assert_eq!(key, "global");
                assert_eq!(limit, 5);
            }
            _ => panic!("Expected RateLimitExceeded error"),
        }
    }

    #[tokio::test]
    async fn test_get_rate() {
        let limiter = TokenBucketRateLimiter::strict();
        let ip = "192.168.1.1";

        // Consume some tokens
        for _ in 0..3 {
            let _ = limiter.check_ip_limit(ip, 1).await;
        }

        let rate = limiter.get_rate(ip).await;
        assert_eq!(rate, 3);
    }

    #[tokio::test]
    async fn test_get_global_rate() {
        let limiter = TokenBucketRateLimiter::strict();

        // Consume some tokens
        for _ in 0..5 {
            let _ = limiter.check_global_limit(1).await;
        }

        let rate = limiter.get_global_rate().await;
        assert_eq!(rate, 5);
    }

    #[tokio::test]
    async fn test_reset() {
        let limiter = TokenBucketRateLimiter::strict();
        let ip = "192.168.1.1";

        // Consume tokens
        for _ in 0..5 {
            let _ = limiter.check_ip_limit(ip, 1).await;
        }

        // Verify consumed
        let rate_before = limiter.get_rate(ip).await;
        assert_eq!(rate_before, 5);

        // Reset
        limiter.reset().await;

        // Verify reset
        let rate_after = limiter.get_rate(ip).await;
        assert_eq!(rate_after, 0);
    }

    #[tokio::test]
    async fn test_multi_ip_isolation() {
        let limiter = TokenBucketRateLimiter::strict();
        let ip1 = "192.168.1.1";
        let ip2 = "192.168.1.2";

        // Consume tokens for IP1
        for _ in 0..5 {
            let _ = limiter.check_ip_limit(ip1, 1).await;
        }

        // IP2 should not be affected
        for _ in 0..5 {
            assert!(limiter.check_ip_limit(ip2, 1).await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_multi_tenant_isolation() {
        let limiter = TokenBucketRateLimiter::strict();
        let tenant1 = "tenant-1";
        let tenant2 = "tenant-2";

        // Consume tokens for tenant1
        for _ in 0..5 {
            let _ = limiter.check_tenant_limit(tenant1, 1).await;
        }

        // tenant2 should not be affected
        for _ in 0..5 {
            assert!(limiter.check_tenant_limit(tenant2, 1).await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_token_bucket_refill() {
        let config = RateLimitConfig::new(10, 50, 100, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let ip = "192.168.1.1";

        // Consume all tokens
        for _ in 0..10 {
            let _ = limiter.check_ip_limit(ip, 1).await;
        }

        // Verify we're at limit
        assert!(limiter.check_ip_limit(ip, 1).await.is_err());

        // Wait a bit for tokens to refill
        tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;

        // Should have refilled tokens
        assert!(limiter.check_ip_limit(ip, 1).await.is_ok());
    }

    #[tokio::test]
    async fn test_get_limit() {
        let limiter = TokenBucketRateLimiter::strict();

        let ip = "192.168.1.1";
        assert_eq!(limiter.get_limit(ip).await, 10);

        let tenant = "tenant-123";
        assert_eq!(limiter.get_limit(tenant).await, 50);
    }

    #[tokio::test]
    async fn test_check_rate_limit_auto_detection() {
        let limiter = TokenBucketRateLimiter::strict();

        // Should auto-detect IP
        let result = limiter.check_rate_limit("192.168.1.1", 1).await;
        assert!(result.is_ok());

        // Should auto-detect tenant ID
        let result = limiter.check_rate_limit("tenant-123", 1).await;
        assert!(result.is_ok());
    }
}
