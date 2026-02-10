//! Domain types for osiris-edge
//!
//! Core types and errors for the edge gateway.

pub mod analytics;
pub mod auth;
pub mod error;
pub mod oauth2;
pub mod packet;
pub mod protocol;
pub mod refusal;
pub mod tenant;

pub use analytics::*;
pub use auth::{AuthPrincipal, AuthRequest, PrincipalType, TokenValidationConfig};
pub use error::{EdgeError, EventBusError, WipError};
pub use oauth2::{
    AuthorizationRequest, AuthorizationResponse, ChallengeMethod, CodeChallenge, CodeVerifier,
    Oauth2Session, RefreshTokenRequest, TokenRequest, TokenResponse,
};
pub use packet::*;
pub use protocol::{BridgeConfig, DetectedProtocol, DetectionMethod, Protocol};
pub use refusal::{AuthErrorCode, RefusalReason, RefusalReceipt, TypeCheckErrorCode};
pub use tenant::*;
