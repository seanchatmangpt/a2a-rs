//! Adapter implementations for osiris-edge
//!
//! Concrete implementations of port traits.

pub mod auth_gate;
pub mod kanban_wip;
pub mod refusal_engine;
pub mod workspace_normalizer;

pub use auth_gate::{
    BearerTokenExtractor, CompositeAuthGate, GoogleWorkspaceAuthGate, JwtAuthGate,
    ServiceAccountAuthGate,
};
pub use kanban_wip::KanbanWipGate;
pub use refusal_engine::CryptoRefusalEngine;
pub use workspace_normalizer::WorkspaceNormalizer;
