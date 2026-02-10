//! Adapter implementations for osiris-edge
//!
//! Concrete implementations of port traits.

pub mod auth_gate;
#[cfg(feature = "bigquery")]
pub mod bq_telemetry;
pub mod cache;
pub mod dynamic_policy_engine;
pub mod hierarchical_auth_adapter;
pub mod in_memory_tenant_manager;
pub mod instrumented_wip_gate;
pub mod kanban_wip;
pub mod metrics;
pub mod oauth_pkce;
pub mod protocol_detector;
pub mod pubsub_bus;
pub mod rate_limiter;
pub mod realtime_analytics;
pub mod refusal_engine;
pub mod tracing;
// pub mod unified_bridge;  // Temporarily disabled due to a2a-mcp dependency
#[cfg(feature = "ws")]
pub mod websocket;
pub mod workspace_normalizer;

pub use auth_gate::{
    BearerTokenExtractor, CompositeAuthGate, GoogleWorkspaceAuthGate, JwtAuthGate,
    ServiceAccountAuthGate,
};
#[cfg(feature = "bigquery")]
pub use bq_telemetry::{BigQueryConfig, BigQueryConfigBuilder, BigQueryTelemetrySink};
#[cfg(feature = "redis")]
pub use cache::RedisCache;
#[cfg(feature = "redis")]
pub use cache::RedisConfig;
pub use dynamic_policy_engine::DynamicPolicyEngine;
pub use hierarchical_auth_adapter::HierarchicalAuthAdapter;
pub use in_memory_tenant_manager::InMemoryTenantManager;
pub use instrumented_wip_gate::{InstrumentedPermit, InstrumentedWipGate};
pub use kanban_wip::KanbanWipGate;
pub use metrics::PrometheusCollector;
pub use oauth_pkce::{PkceAuthenticator, PkceConfig};
pub use protocol_detector::PathBasedDetector;
pub use pubsub_bus::InMemoryEventBus;
#[cfg(feature = "pubsub")]
pub use pubsub_bus::{GcsConfig, GcsPubSubBus};
pub use rate_limiter::TokenBucketRateLimiter;
pub use realtime_analytics::RealtimeAnalyticsEngine;
pub use refusal_engine::CryptoRefusalEngine;
pub use tracing::{
    OpenTelemetryManager, SpanEvent, SpanHandle, SpanMetrics, TraceContext, TraceContextInjector,
    TracingConfig, TracingError,
};
// pub use unified_bridge::{BridgeError, BridgeStatistics, UnifiedBridge};  // Temporarily disabled
#[cfg(feature = "ws")]
pub use websocket::WebSocketTransport;
pub use workspace_normalizer::WorkspaceNormalizer;
