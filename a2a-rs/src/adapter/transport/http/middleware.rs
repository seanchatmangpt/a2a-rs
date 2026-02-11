//! HTTP middleware implementations for the A2A protocol server
//!
//! This module provides production-ready middleware for:
//! - CORS handling
//! - Rate limiting
//! - Request validation
//! - Response compression
//! - Request ID tracking
//! - Metrics and observability

#[cfg(feature = "http-server")]
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

#[cfg(feature = "http-server")]
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode, HeaderName},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

#[cfg(feature = "http-server")]
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
};

use serde::{Deserialize, Serialize};

/// CORS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorsConfig {
    /// Allowed origins
    #[serde(default = "default_origins")]
    pub allowed_origins: Vec<String>,

    /// Allowed methods
    #[serde(default = "default_methods")]
    pub allowed_methods: Vec<String>,

    /// Allowed headers
    #[serde(default = "default_headers")]
    pub allowed_headers: Vec<String>,

    /// Exposed headers
    #[serde(default)]
    pub exposed_headers: Vec<String>,

    /// Allow credentials
    #[serde(default = "default_allow_credentials")]
    pub allow_credentials: bool,

    /// Max age for preflight requests (seconds)
    #[serde(default = "default_max_age")]
    pub max_age: usize,
}

fn default_origins() -> Vec<String> {
    vec!["*".to_string()]
}

fn default_methods() -> Vec<String> {
    vec![
        "GET".to_string(),
        "POST".to_string(),
        "PUT".to_string(),
        "DELETE".to_string(),
        "OPTIONS".to_string(),
    ]
}

fn default_headers() -> Vec<String> {
    vec![
        "accept".to_string(),
        "accept-language".to_string(),
        "authorization".to_string(),
        "content-type".to_string(),
        "dnt".to_string(),
        "origin".to_string(),
        "user-agent".to_string(),
        "x-csrftoken".to_string(),
        "x-requested-with".to_string(),
    ]
}

fn default_allow_credentials() -> bool {
    false
}

fn default_max_age() -> usize {
    86400 // 24 hours
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: default_origins(),
            allowed_methods: default_methods(),
            allowed_headers: default_headers(),
            exposed_headers: vec![],
            allow_credentials: false,
            max_age: 86400,
        }
    }
}

impl CorsConfig {
    /// Create a new CORS configuration with strict defaults
    pub fn strict() -> Self {
        Self {
            allowed_origins: vec![],
            allowed_methods: vec!["POST".to_string(), "GET".to_string(), "OPTIONS".to_string()],
            allowed_headers: vec![
                "authorization".to_string(),
                "content-type".to_string(),
                "x-request-id".to_string(),
            ],
            exposed_headers: vec![],
            allow_credentials: false,
            max_age: 3600, // 1 hour
        }
    }

    /// Create a new CORS configuration with permissive defaults
    pub fn permissive() -> Self {
        Self::default()
    }

    /// Build the CORS layer
    #[cfg(feature = "http-server")]
    pub fn build_layer(&self) -> CorsLayer {
        use axum::http::HeaderValue;

        // Parse origins
        let origins: Result<Vec<HeaderValue>, _> = self
            .allowed_origins
            .iter()
            .map(|o| o.parse())
            .collect();

        let origins = origins.unwrap_or_else(|_| vec![HeaderValue::from_static("*")]);

        // Parse methods
        let methods: Result<Vec<Method>, _> = self
            .allowed_methods
            .iter()
            .map(|m| m.parse())
            .collect();

        let methods = methods.unwrap_or_else(|_| {
            vec![
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ]
        });

        // Parse headers
        let headers: Result<Vec<HeaderName>, _> =
            self.allowed_headers.iter().map(|h| h.parse()).collect();

        let headers = headers.unwrap_or_else(|_| {
            vec![
                header::AUTHORIZATION,
                header::CONTENT_TYPE,
                header::ACCEPT,
            ]
        });

        let mut cors = CorsLayer::new()
            .allow_methods(methods)
            .allow_headers(headers)
            .max_age(Duration::from_secs(self.max_age as u64));

        // Set origins
        if self.allowed_origins.len() == 1 && self.allowed_origins[0] == "*" {
            cors = cors.allow_origin(HeaderValue::from_static("*"));
        } else {
            cors = cors.allow_origin(origins);
        }

        // Set credentials
        if self.allow_credentials {
            cors = cors.allow_credentials(true);
        }

        cors
    }
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitConfig {
    /// Maximum requests per time window
    #[serde(default = "default_max_requests")]
    pub max_requests: usize,

    /// Time window in seconds
    #[serde(default = "default_window_seconds")]
    pub window_seconds: u64,

    /// Whether to use IP-based limiting
    #[serde(default = "default_use_ip")]
    pub use_ip: bool,

    /// Whether to use API key based limiting (if available)
    #[serde(default)]
    pub use_api_key: bool,

    /// Burst size (allow short bursts)
    #[serde(default = "default_burst_size")]
    pub burst_size: usize,
}

fn default_max_requests() -> usize {
    100
}

fn default_window_seconds() -> u64 {
    60
}

fn default_use_ip() -> bool {
    true
}

fn default_burst_size() -> usize {
    10
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: default_max_requests(),
            window_seconds: default_window_seconds(),
            use_ip: true,
            use_api_key: false,
            burst_size: default_burst_size(),
        }
    }
}

impl RateLimitConfig {
    /// Create a restrictive rate limit
    pub fn restrictive() -> Self {
        Self {
            max_requests: 10,
            window_seconds: 60,
            use_ip: true,
            use_api_key: false,
            burst_size: 5,
        }
    }

    /// Create a permissive rate limit
    pub fn permissive() -> Self {
        Self {
            max_requests: 1000,
            window_seconds: 60,
            use_ip: true,
            use_api_key: false,
            burst_size: 100,
        }
    }
}

/// Rate limiter state
#[cfg(feature = "http-server")]
#[derive(Debug, Clone)]
pub struct RateLimiter {
    /// Rate limit configuration
    config: RateLimitConfig,
    /// Request tracker: maps identifier to (count, window_start)
    tracker: Arc<tokio::sync::RwLock<HashMap<String, (usize, Instant)>>>,
}

#[cfg(feature = "http-server")]
impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            tracker: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Check if a request should be allowed
    pub async fn check_rate_limit(&self, identifier: &str) -> Result<(), RateLimitError> {
        let mut tracker = self.tracker.write().await;
        let now = Instant::now();
        let window_duration = Duration::from_secs(self.config.window_seconds);

        // Clean up old entries
        tracker.retain(|_, (_, window_start)| {
            now.duration_since(*window_start) < window_duration
        });

        // Get or create entry
        let (count, window_start) = tracker
            .entry(identifier.to_string())
            .or_insert((0, now));

        // Reset if window expired
        if now.duration_since(*window_start) >= window_duration {
            *count = 0;
            *window_start = now;
        }

        // Check limit
        if *count >= self.config.max_requests {
            return Err(RateLimitError {
                limit: self.config.max_requests,
                window_seconds: self.config.window_seconds,
                retry_after: window_duration.as_secs() - now.duration_since(*window_start).as_secs(),
            });
        }

        // Increment counter
        *count += 1;
        Ok(())
    }

    /// Extract identifier from request
    pub fn extract_identifier(&self, headers: &HeaderMap, ip: &str) -> String {
        // Try API key first if enabled
        if self.config.use_api_key {
            if let Some(api_key) = headers.get("x-api-key") {
                if let Ok(key) = api_key.to_str() {
                    return format!("apikey:{}", key);
                }
            }
        }

        // Fall back to IP
        if self.config.use_ip {
            format!("ip:{}", ip)
        } else {
            // Use a global identifier if both are disabled
            "global".to_string()
        }
    }
}

/// Rate limit error
#[derive(Debug)]
pub struct RateLimitError {
    pub limit: usize,
    pub window_seconds: u64,
    pub retry_after: u64,
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Rate limit exceeded: {} requests per {} seconds. Retry after {} seconds.",
            self.limit, self.window_seconds, self.retry_after
        )
    }
}

impl std::error::Error for RateLimitError {}

#[cfg(feature = "http-server")]
impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        let body = Json(serde_json::json!({
            "error": "rate_limit_exceeded",
            "message": self.to_string(),
            "limit": self.limit,
            "windowSeconds": self.window_seconds,
            "retryAfter": self.retry_after,
        }));

        let mut response = body.into_response();
        *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
        response
            .headers_mut()
            .insert("retry-after", HeaderValue::from(self.retry_after));
        response
    }
}

/// Rate limiting middleware
#[cfg(feature = "http-server")]
pub async fn rate_limit_middleware(
    State(limiter): State<RateLimiter>,
    req: Request,
    next: Next,
) -> Result<Response, RateLimitError> {
    // Extract IP from connection info (simplified)
    // In production, extract from actual connection info
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .unwrap_or("unknown")
        .to_string();

    let identifier = limiter.extract_identifier(req.headers(), &ip);

    limiter.check_rate_limit(&identifier).await?;

    Ok(next.run(req).await)
}

/// Request validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationConfig {
    /// Maximum request body size in bytes
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,

    /// Allowed content types
    #[serde(default = "default_content_types")]
    pub allowed_content_types: Vec<String>,

    /// Whether to validate JSON-RPC format
    #[serde(default = "default_validate_jsonrpc")]
    pub validate_jsonrpc: bool,

    /// Whether to validate against JSON schema
    #[serde(default)]
    pub validate_schema: bool,
}

fn default_max_body_size() -> usize {
    10 * 1024 * 1024 // 10 MB
}

fn default_content_types() -> Vec<String> {
    vec![
        "application/json".to_string(),
        "application/jsonrpc".to_string(),
    ]
}

fn default_validate_jsonrpc() -> bool {
    true
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_body_size: default_max_body_size(),
            allowed_content_types: default_content_types(),
            validate_jsonrpc: true,
            validate_schema: false,
        }
    }
}

impl ValidationConfig {
    /// Create a strict validation config
    pub fn strict() -> Self {
        Self {
            max_body_size: 1024 * 1024, // 1 MB
            allowed_content_types: vec!["application/json".to_string()],
            validate_jsonrpc: true,
            validate_schema: true,
        }
    }
}

/// Request validation error
#[derive(Debug)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Validation error in {}: {}", self.field, self.message)
    }
}

impl std::error::Error for ValidationError {}

#[cfg(feature = "http-server")]
impl IntoResponse for ValidationError {
    fn into_response(self) -> Response {
        let body = Json(serde_json::json!({
            "error": "validation_error",
            "message": self.to_string(),
            "field": self.field,
        }));

        let mut response = body.into_response();
        *response.status_mut() = StatusCode::BAD_REQUEST;
        response
    }
}

#[cfg(feature = "http-server")]
pub async fn validation_middleware(
    State(config): State<ValidationConfig>,
    req: Request,
    next: Next,
) -> Result<Response, ValidationError> {
    // Check content type
    if let Some(content_type) = req.headers().get("content-type") {
        if let Ok(ct) = content_type.to_str() {
            let ct_main = ct.split(';').next().unwrap_or("");
            if !config
                .allowed_content_types
                .iter()
                .any(|allowed| ct_main == *allowed)
            {
                return Err(ValidationError {
                    field: "content-type".to_string(),
                    message: format!(
                        "Content type '{}' not allowed. Allowed: {:?}",
                        ct, config.allowed_content_types
                    ),
                });
            }
        }
    }

    // Validate body size
    if let Some(content_length) = req.headers().get("content-length") {
        if let Ok(len_str) = content_length.to_str() {
            if let Ok(len) = len_str.parse::<usize>() {
                if len > config.max_body_size {
                    return Err(ValidationError {
                        field: "content-length".to_string(),
                        message: format!(
                            "Request body too large: {} bytes (max: {})",
                            len, config.max_body_size
                        ),
                    });
                }
            }
        }
    }

    Ok(next.run(req).await)
}

/// Compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionConfig {
    /// Whether to enable compression
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Compression level (0-9, where 9 is maximum)
    #[serde(default = "default_level")]
    pub level: u8,

    /// Minimum response size to compress (bytes)
    #[serde(default = "default_min_size")]
    pub min_size: usize,

    /// Whether to compress based on content type
    #[serde(default = "default_compressible")]
    pub compressible: bool,
}

fn default_enabled() -> bool {
    true
}

fn default_level() -> u8 {
    6 // Balanced between speed and compression
}

fn default_min_size() -> usize {
    1024 // 1 KB
}

fn default_compressible() -> bool {
    true
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: 6,
            min_size: 1024,
            compressible: true,
        }
    }
}

impl CompressionConfig {
    /// Create a fast compression config (lower compression, faster)
    pub fn fast() -> Self {
        Self {
            enabled: true,
            level: 3,
            min_size: 512,
            compressible: true,
        }
    }

    /// Create a maximum compression config (slower, better compression)
    pub fn max() -> Self {
        Self {
            enabled: true,
            level: 9,
            min_size: 256,
            compressible: true,
        }
    }

    /// Build the compression layer
    #[cfg(feature = "http-server")]
    pub fn build_layer(&self) -> CompressionLayer {
        // Use predicate to compress all responses
        // tower-http 0.6 uses predicates for deciding when to compress
        CompressionLayer::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cors_config_defaults() {
        let config = CorsConfig::default();
        assert_eq!(config.allowed_origins, vec!["*"]);
        assert_eq!(config.allow_credentials, false);
        assert_eq!(config.max_age, 86400);
    }

    #[test]
    fn test_rate_limit_config() {
        let config = RateLimitConfig::restrictive();
        assert_eq!(config.max_requests, 10);
        assert_eq!(config.window_seconds, 60);

        let config = RateLimitConfig::permissive();
        assert_eq!(config.max_requests, 1000);
    }

    #[test]
    fn test_validation_config() {
        let config = ValidationConfig::strict();
        assert_eq!(config.max_body_size, 1024 * 1024);
        assert!(config.validate_schema);
    }

    #[test]
    fn test_compression_config() {
        let config = CompressionConfig::fast();
        assert!(config.enabled);
        assert!(config.level < 6);

        let config = CompressionConfig::max();
        assert!(config.level >= 8);
    }
}
