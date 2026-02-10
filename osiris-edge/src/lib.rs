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
    PathBasedDetector,
    RealtimeAnalyticsEngine,
    WorkspaceNormalizer,
};
pub use application::{
    TenantApiState, analytics_health_handler, analytics_snapshot_handler, analytics_sse_handler,
    analytics_timeseries_handler,
};
pub use domain::{
    AnalyticsSnapshot, Anomaly, AnomalySeverity, AnomalyType, BottleneckSignal, BottleneckType,
    BridgeConfig, Condition, DetectedProtocol, DetectionMethod, Effect, EventType,
    HierarchicalIdentity, LittlesLawMetrics, OrganizationId, PacketContext, PacketPayload,
    PacketSource, PercentileLatency, Permission, Policy, PolicyRule, Protocol, RefusalRules, Role,
    RoleBinding, Scope, TeamId, TenantConfig, TenantId, TypedPacket, UserId, WipError, WipLimits,
    WipSnapshot, WorkMetrics,
};
pub use port::{
    AnalyticsConfig, AnalyticsEngine, AsyncWipGate, Decision, EvaluationContext,
    HierarchicalAuthGate, NormalizationError, PacketNormalizer, PolicyEngine, ProtocolDetector,
    TenantManager, WipGate, WipPermit,
};
// pub use services::{UnifiedServer, UnifiedServerConfig};  // Temporarily disabled
