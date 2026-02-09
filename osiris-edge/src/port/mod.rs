//! Ports (interfaces) for osiris-edge
//!
//! Port traits define the interfaces for WIP limiting and work admission control.

pub mod auth_gate;
pub mod packet_normalizer;
pub mod refusal_engine;
pub mod wip_gate;

pub use auth_gate::{AuthGate, GoogleWorkspaceValidator, ServiceAccountValidator, TokenExtractor};
pub use packet_normalizer::{NormalizationError, PacketNormalizer};
pub use refusal_engine::RefusalEngine;
pub use wip_gate::{AsyncWipGate, WipGate, WipPermit};
