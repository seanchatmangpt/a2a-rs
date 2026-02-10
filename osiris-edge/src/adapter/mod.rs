//! Adapter implementations for osiris-edge
//!
//! Concrete implementations of port traits.

pub mod auth_gate;
pub mod dynamic_policy_engine;
pub mod hierarchical_auth_adapter;
pub mod in_memory_tenant_manager;
pub mod instrumented_wip_gate;
pub mod kanban_wip;
pub mod protocol_detector;
pub mod realtime_analytics;
pub mod refusal_engine;
// pub mod unified_bridge;  // Temporarily disabled due to a2a-mcp dependency
pub mod workspace_normalizer;

pub use auth_gate::{
    BearerTokenExtractor, CompositeAuthGate, GoogleWorkspaceAuthGate, JwtAuthGate,
    ServiceAccountAuthGate,
};
pub use dynamic_policy_engine::DynamicPolicyEngine;
pub use hierarchical_auth_adapter::HierarchicalAuthAdapter;
pub use in_memory_tenant_manager::InMemoryTenantManager;
pub use instrumented_wip_gate::{InstrumentedPermit, InstrumentedWipGate};
pub use kanban_wip::KanbanWipGate;
pub use protocol_detector::PathBasedDetector;
pub use realtime_analytics::RealtimeAnalyticsEngine;
pub use refusal_engine::CryptoRefusalEngine;
// pub use unified_bridge::{BridgeError, BridgeStatistics, UnifiedBridge};  // Temporarily disabled
pub use workspace_normalizer::WorkspaceNormalizer;
