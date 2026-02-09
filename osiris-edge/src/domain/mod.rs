//! Domain types for osiris-edge
//!
//! Core types and errors for the edge gateway.

pub mod auth;
pub mod error;
pub mod packet;
pub mod refusal;

pub use auth::{AuthPrincipal, AuthRequest, PrincipalType, TokenValidationConfig};
pub use error::{EdgeError, WipError};
pub use packet::*;
pub use refusal::{AuthErrorCode, RefusalReason, RefusalReceipt, TypeCheckErrorCode};
