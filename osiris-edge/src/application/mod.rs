//! Application layer - JSON-RPC routing and request handlers
//!
//! Wires adapters to ports and provides high-level request handling.

pub mod analytics_sse;
pub mod router;
pub mod tenant_api;

pub use analytics_sse::{
    analytics_health_handler, analytics_snapshot_handler, analytics_sse_handler,
    analytics_timeseries_handler,
};
pub use router::{RouterState, create_router};
pub use tenant_api::*;
