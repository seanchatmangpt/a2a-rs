//! Ports (interfaces) for osiris-edge
//!
//! Port traits define the interfaces for WIP limiting and work admission control.

pub mod analytics_engine;
pub mod auth_gate;
pub mod hierarchical_auth;
pub mod packet_normalizer;
pub mod policy_engine;
pub mod protocol_detector;
pub mod refusal_engine;
pub mod tenant_manager;
pub mod wip_gate;

pub use analytics_engine::{AnalyticsConfig, AnalyticsConfigBuilder, AnalyticsEngine};
pub use auth_gate::{AuthGate, GoogleWorkspaceValidator, ServiceAccountValidator, TokenExtractor};
pub use hierarchical_auth::HierarchicalAuthGate;
pub use packet_normalizer::{NormalizationError, PacketNormalizer};
pub use policy_engine::{Decision, EvaluationContext, PolicyEngine};
pub use protocol_detector::ProtocolDetector;
pub use refusal_engine::RefusalEngine;
pub use tenant_manager::TenantManager;
pub use wip_gate::{AsyncWipGate, WipGate, WipPermit};
