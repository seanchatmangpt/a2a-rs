//! Axum middleware for rate limiting
//!
//! Integrates the RateLimiter port with Axum HTTP requests.
//! Extracts IP address from X-Forwarded-For or socket address,
//! and enforces rate limits before routing to handlers.

use axum::{
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::warn;

use crate::port::{RateLimitError, RateLimiter};

/// Configuration for rate limit middleware
#[derive(Debug, Clone)]
pub struct RateLimitMiddlewareConfig {
    /// Check per-IP limits
    pub check_ip: bool,
    /// Check per-tenant limits (requires X-Tenant-ID header)
    pub check_tenant: bool,
    /// Check global limits
    pub check_global: bool,
}

impl RateLimitMiddlewareConfig {
    /// Create a new configuration with all checks enabled
    pub fn all() -> Self {
        Self {
            check_ip: true,
            check_tenant: true,
            check_global: true,
        }
    }

    /// Create a configuration that only checks global limits
    pub fn global_only() -> Self {
        Self {
            check_ip: false,
            check_tenant: false,
            check_global: true,
        }
    }

    /// Create a configuration that checks IP and global limits
    pub fn ip_and_global() -> Self {
        Self {
            check_ip: true,
            check_tenant: false,
            check_global: true,
        }
    }
}

/// Error response for rate limit exceeded
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitErrorResponse {
    pub error: String,
    pub message: String,
    pub retry_after_secs: u64,
    pub limit: u32,
    pub current: u32,
}

impl RateLimitErrorResponse {
    /// Create a new error response from a RateLimitError
    pub fn from_error(error: &RateLimitError) -> Self {
        match error {
            RateLimitError::RateLimitExceeded {
                key: _,
                current_rate,
                limit,
                retry_after_secs,
            } => Self {
                error: "RateLimitExceeded".to_string(),
                message: format!(
                    "Rate limit exceeded: {}/{} requests allowed",
                    current_rate, limit
                ),
                retry_after_secs: *retry_after_secs,
                limit: *limit,
                current: *current_rate,
            },
            RateLimitError::ConfigurationError(msg) => Self {
                error: "ConfigurationError".to_string(),
                message: format!("Rate limiter configuration error: {}", msg),
                retry_after_secs: 0,
                limit: 0,
                current: 0,
            },
            RateLimitError::InvalidKey(msg) => Self {
                error: "InvalidKey".to_string(),
                message: format!("Invalid key for rate limiting: {}", msg),
                retry_after_secs: 0,
                limit: 0,
                current: 0,
            },
        }
    }
}

impl IntoResponse for RateLimitErrorResponse {
    fn into_response(self) -> Response {
        let retry_after = self.retry_after_secs.to_string();
        (
            StatusCode::TOO_MANY_REQUESTS,
            [("Retry-After", retry_after)],
            axum::Json(self),
        )
            .into_response()
    }
}

/// Extract the client IP address from the request
///
/// Checks X-Forwarded-For header first (for proxied requests),
/// then falls back to socket address if available.
fn extract_ip<B>(req: &Request<B>) -> String {
    // Try X-Forwarded-For header first
    if let Some(forwarded) = req.headers().get("X-Forwarded-For") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // X-Forwarded-For can contain multiple IPs, use the first one
            if let Some(ip) = forwarded_str.split(',').next() {
                return ip.trim().to_string();
            }
        }
    }

    // Try X-Real-IP header
    if let Some(real_ip) = req.headers().get("X-Real-IP") {
        if let Ok(ip_str) = real_ip.to_str() {
            return ip_str.to_string();
        }
    }

    // Fall back to socket address from ConnectInfo if available
    if let Some(conn_info) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return conn_info.ip().to_string();
    }

    "unknown".to_string()
}

/// Extract the tenant ID from the request headers
///
/// Looks for the X-Tenant-ID header.
fn extract_tenant_id<B>(req: &Request<B>) -> Option<String> {
    req.headers()
        .get("X-Tenant-ID")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
}

/// Create a rate limiting middleware layer for Axum
///
/// # Example
///
/// ```no_run
/// use osiris_edge::TokenBucketRateLimiter;
/// use osiris_edge::{RateLimitConfig, RateLimitMiddlewareConfig};
/// use osiris_edge::rate_limit_middleware::rate_limit_layer;
/// use axum::Router;
/// use std::sync::Arc;
///
/// # async fn example() {
/// let limiter = TokenBucketRateLimiter::default();
/// let config = RateLimitMiddlewareConfig::all();
/// let middleware = rate_limit_layer(Arc::new(limiter), config);
///
/// let app = Router::new()
///     .layer(middleware);
/// # }
/// ```
pub fn rate_limit_layer(
    limiter: Arc<dyn RateLimiter>,
    config: RateLimitMiddlewareConfig,
) -> impl Clone {
    use std::future::Future;

    type HandlerFuture = std::pin::Pin<
        Box<
            dyn Future<
                    Output = Result<
                        Response,
                        RateLimitErrorResponse,
                    >,
                > + Send,
        >,
    >;

    axum::middleware::from_fn_with_state::<_, (), HandlerFuture>(
        (),
        move |_state: (), req: Request<axum::body::Body>, next: Next| {
            let limiter = Arc::clone(&limiter);
            let config = config.clone();

            async move {
                let ip = extract_ip(&req);

                // Check global limit if enabled
                if config.check_global {
                    if let Err(e) = limiter.check_global_limit(1).await {
                        warn!("Global rate limit exceeded for request from {}", ip);
                        return Err(RateLimitErrorResponse::from_error(&e));
                    }
                }

                // Check IP limit if enabled
                if config.check_ip {
                    if let Err(e) = limiter.check_ip_limit(&ip, 1).await {
                        warn!("IP rate limit exceeded for {}", ip);
                        return Err(RateLimitErrorResponse::from_error(&e));
                    }
                }

                // Check tenant limit if enabled and tenant ID is present
                if config.check_tenant {
                    if let Some(tenant_id) = extract_tenant_id(&req) {
                        if let Err(e) = limiter.check_tenant_limit(&tenant_id, 1).await {
                            warn!("Tenant rate limit exceeded for {}", tenant_id);
                            return Err(RateLimitErrorResponse::from_error(&e));
                        }
                    }
                }

                // All checks passed, continue to next handler
                Ok(next.run(req).await)
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_error_response_from_error() {
        let error = RateLimitError::RateLimitExceeded {
            key: "192.168.1.1".to_string(),
            current_rate: 101,
            limit: 100,
            retry_after_secs: 2,
        };

        let response = RateLimitErrorResponse::from_error(&error);
        assert_eq!(response.error, "RateLimitExceeded");
        assert_eq!(response.limit, 100);
        assert_eq!(response.current, 101);
        assert_eq!(response.retry_after_secs, 2);
    }

    #[test]
    fn test_rate_limit_config_all() {
        let config = RateLimitMiddlewareConfig::all();
        assert!(config.check_ip);
        assert!(config.check_tenant);
        assert!(config.check_global);
    }

    #[test]
    fn test_rate_limit_config_global_only() {
        let config = RateLimitMiddlewareConfig::global_only();
        assert!(!config.check_ip);
        assert!(!config.check_tenant);
        assert!(config.check_global);
    }

    #[test]
    fn test_rate_limit_config_ip_and_global() {
        let config = RateLimitMiddlewareConfig::ip_and_global();
        assert!(config.check_ip);
        assert!(!config.check_tenant);
        assert!(config.check_global);
    }
}
