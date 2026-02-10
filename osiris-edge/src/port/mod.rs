//! Ports (interfaces) for osiris-edge
//!
//! Port traits define the interfaces for WIP limiting, work admission control,
//! and bidirectional communication transport.

pub mod analytics_engine;
pub mod auth_gate;
pub mod cache;
pub mod event_bus;
pub mod hierarchical_auth;
pub mod metrics;
pub mod oauth2_authenticator;
pub mod packet_normalizer;
pub mod policy_engine;
pub mod protocol_detector;
pub mod rate_limiter;
pub mod refusal_engine;
pub mod telemetry_sink;
pub mod tenant_manager;
pub mod transport;
pub mod wip_gate;

pub use analytics_engine::{AnalyticsConfig, AnalyticsConfigBuilder, AnalyticsEngine};
pub use auth_gate::{AuthGate, GoogleWorkspaceValidator, ServiceAccountValidator, TokenExtractor};
pub use cache::{Cache, CacheConfig, CacheError};
pub use event_bus::{EventBus, PubMessage, ReceivedMessage, SubscriptionConfig, TopicConfig};
pub use hierarchical_auth::HierarchicalAuthGate;
pub use metrics::{MetricsCollector, MetricsError};
pub use oauth2_authenticator::Oauth2Authenticator;
pub use packet_normalizer::{NormalizationError, PacketNormalizer};
pub use policy_engine::{Decision, EvaluationContext, PolicyEngine};
pub use protocol_detector::ProtocolDetector;
pub use rate_limiter::{RateLimitConfig, RateLimitError, RateLimitResult, RateLimiter};
pub use refusal_engine::RefusalEngine;
pub use telemetry_sink::{
    BatchConfig, CycleTimeRecord, RefusalRecord, TelemetrySink, TelemetrySinkError,
    TelemetrySinkStats, WipStateRecord,
};
pub use tenant_manager::TenantManager;
pub use transport::{Transport, TransportConfig, TransportError, TransportStatus};
pub use wip_gate::{AsyncWipGate, WipGate, WipPermit};
