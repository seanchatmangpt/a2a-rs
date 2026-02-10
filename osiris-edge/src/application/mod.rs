//! Application layer - JSON-RPC routing and request handlers
//!
//! Wires adapters to ports and provides high-level request handling.

pub mod analytics_sse;
pub mod metrics_handler;
pub mod rate_limit_middleware;
pub mod router;
pub mod tenant_api;

pub use analytics_sse::{
    analytics_health_handler, analytics_snapshot_handler, analytics_sse_handler,
    analytics_timeseries_handler,
};
pub use metrics_handler::{
    MetricsErrorResponse, MetricsResponse, error_tracking_middleware, metrics_handler,
    simple_request_metrics_middleware,
};
pub use rate_limit_middleware::{
    RateLimitErrorResponse, RateLimitMiddlewareConfig, rate_limit_layer,
};
pub use router::{RouterState, create_router};
pub use tenant_api::*;
