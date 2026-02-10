//! Domain types for osiris-edge
//!
//! Core types and errors for the edge gateway.

pub mod analytics;
pub mod auth;
pub mod error;
pub mod packet;
pub mod protocol;
pub mod refusal;
pub mod tenant;

pub use analytics::*;
pub use auth::{AuthPrincipal, AuthRequest, PrincipalType, TokenValidationConfig};
pub use error::{EdgeError, WipError};
pub use packet::*;
pub use protocol::{BridgeConfig, DetectedProtocol, DetectionMethod, Protocol};
pub use refusal::{AuthErrorCode, RefusalReason, RefusalReceipt, TypeCheckErrorCode};
pub use tenant::*;
