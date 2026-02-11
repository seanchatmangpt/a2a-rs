//! HTTP transport implementations

#[cfg(feature = "http-client")]
pub mod client;

#[cfg(feature = "http-server")]
pub mod server;

#[cfg(feature = "http-server")]
pub mod middleware;

#[cfg(feature = "http-server")]
pub mod health;

#[cfg(feature = "http-server")]
pub mod openapi;

// Re-export HTTP implementations
#[cfg(feature = "http-client")]
pub use client::HttpClient;

#[cfg(feature = "http-server")]
pub use server::HttpServer;

#[cfg(feature = "http-server")]
pub use middleware::{CorsConfig, RateLimitConfig, ValidationConfig, CompressionConfig};

#[cfg(feature = "http-server")]
pub use health::{
    HealthChecker, HealthCheckResponse, HealthStatus, ComponentHealth,
    ReadinessCheckResponse, LivenessCheckResponse,
};

#[cfg(feature = "http-server")]
pub use openapi::{OpenApiBuilder, OpenApiInfo, OpenApiContact, OpenApiLicense};
