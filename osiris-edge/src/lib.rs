//! Osiris-Edge: Edge-optimized A2A agent runtime
//!
//! Provides Kanban-style WIP (Work-in-Progress) limiting for bounded concurrency
//! and deterministic overload protection.
//!
//! # Features
//!
//! - **Hard WIP limits**: Semaphore-based hard cap on concurrent work
//! - **Deterministic rejection**: No queuing - immediate failure when at capacity
//! - **Bounded response times**: Prevents unbounded queueing delays
//! - **Zero-cost abstraction**: Port/adapter pattern with minimal overhead
//!
//! # Example
//!
//! ```no_run
//! use osiris_edge::{KanbanWipGate, AsyncWipGate};
//!
//! # async fn example() {
//! // Create a gate allowing max 5 concurrent work items
//! let gate = KanbanWipGate::new(5);
//!
//! // Try to execute work
//! match gate.try_acquire().await {
//!     Ok(permit) => {
//!         // Do work while holding permit
//!         println!("Working...");
//!         // Permit auto-released on drop
//!     }
//!     Err(e) => {
//!         // WIP limit reached - emit refusal receipt
//!         eprintln!("Work rejected: {}", e);
//!     }
//! }
//!
//! // Or use the execute helper
//! let result = gate.execute(|| async {
//!     // Do work
//!     Ok::<_, osiris_edge::WipError>(42)
//! }).await;
//! # }
//! ```

pub mod adapter;
pub mod application;
pub mod domain;
pub mod port;
pub mod services;

// Re-export core types
pub use adapter::{
    // BridgeError, BridgeStatistics, UnifiedBridge,  // Temporarily disabled
    DynamicPolicyEngine,
    HierarchicalAuthAdapter,
    InMemoryTenantManager,
    InstrumentedWipGate,
    KanbanWipGate,
    OpenTelemetryManager,
    PathBasedDetector,
    PkceAuthenticator,
    PkceConfig,
    PrometheusCollector,
    RealtimeAnalyticsEngine,
    SpanEvent,
    SpanHandle,
    SpanMetrics,
    TokenBucketRateLimiter,
    TraceContext,
    TraceContextInjector,
    TracingConfig,
    TracingError,
    WorkspaceNormalizer,
};

#[cfg(feature = "redis")]
pub use adapter::RedisCache;

#[cfg(feature = "ws")]
pub use adapter::WebSocketTransport;
pub use application::{
    RateLimitErrorResponse, RateLimitMiddlewareConfig, TenantApiState, analytics_health_handler,
    analytics_snapshot_handler, analytics_sse_handler, analytics_timeseries_handler,
    error_tracking_middleware, metrics_handler, rate_limit_layer,
    simple_request_metrics_middleware,
};
pub use domain::{
    AnalyticsSnapshot, Anomaly, AnomalySeverity, AnomalyType, AuthorizationRequest,
    AuthorizationResponse, BottleneckSignal, BottleneckType, BridgeConfig, ChallengeMethod,
    CodeChallenge, CodeVerifier, Condition, DetectedProtocol, DetectionMethod, Effect, EventType,
    HierarchicalIdentity, LittlesLawMetrics, Oauth2Session, OrganizationId, PacketContext,
    PacketPayload, PacketSource, PercentileLatency, Permission, Policy, PolicyRule, Protocol,
    RefreshTokenRequest, RefusalRules, Role, RoleBinding, Scope, TeamId, TenantConfig, TenantId,
    TokenRequest, TokenResponse, TypedPacket, UserId, WipError, WipLimits, WipSnapshot,
    WorkMetrics,
};
pub use port::{
    AnalyticsConfig, AnalyticsEngine, AsyncWipGate, Cache, CacheConfig, CacheError, Decision,
    EvaluationContext, HierarchicalAuthGate, MetricsCollector, MetricsError, NormalizationError,
    Oauth2Authenticator, PacketNormalizer, PolicyEngine, ProtocolDetector, RateLimitConfig,
    RateLimitError, RateLimitResult, RateLimiter, TenantManager, Transport, TransportConfig,
    TransportError, TransportStatus, WipGate, WipPermit,
};
// pub use services::{UnifiedServer, UnifiedServerConfig};  // Temporarily disabled
