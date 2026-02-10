//! Authentication domain types
//!
//! Core types for authentication without external dependencies.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Authentication principal representing an authenticated entity
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthPrincipal {
    /// Unique identifier for the authenticated entity
    pub subject: String,

    /// Email address (if available)
    pub email: Option<String>,

    /// Token issuer
    pub issuer: Option<String>,

    /// Token audience
    pub audience: Option<String>,

    /// Principal type (user, service_account, etc.)
    pub principal_type: PrincipalType,

    /// Additional claims and attributes
    pub claims: HashMap<String, serde_json::Value>,

    /// Token expiration timestamp (Unix epoch)
    pub expires_at: Option<i64>,
}

/// Type of authenticated principal
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalType {
    /// Regular user authenticated via OAuth2/OIDC
    User,

    /// Service account for internal service-to-service calls
    ServiceAccount,

    /// API key authentication
    ApiKey,

    /// Anonymous/unauthenticated
    Anonymous,
}

/// Authentication request containing token and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthRequest {
    /// Authentication token (JWT, OAuth2 access token, etc.)
    pub token: String,

    /// Token type (Bearer, etc.)
    pub token_type: Option<String>,

    /// Optional metadata for validation context
    pub metadata: HashMap<String, String>,
}

impl AuthRequest {
    /// Create a new authentication request
    pub fn new(token: String) -> Self {
        Self {
            token,
            token_type: Some("Bearer".to_string()),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the request
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Token validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenValidationConfig {
    /// Expected issuer (optional)
    pub expected_issuer: Option<String>,

    /// Expected audience (optional)
    pub expected_audience: Option<String>,

    /// Whether to validate token expiration
    pub validate_expiration: bool,

    /// Clock skew tolerance in seconds
    pub clock_skew_seconds: i64,

    /// Additional required claims
    pub required_claims: Vec<String>,
}

impl Default for TokenValidationConfig {
    fn default() -> Self {
        Self {
            expected_issuer: None,
            expected_audience: None,
            validate_expiration: true,
            clock_skew_seconds: 60,
            required_claims: Vec::new(),
        }
    }
}

impl TokenValidationConfig {
    /// Create a new configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set expected issuer
    pub fn with_issuer(mut self, issuer: String) -> Self {
        self.expected_issuer = Some(issuer);
        self
    }

    /// Set expected audience
    pub fn with_audience(mut self, audience: String) -> Self {
        self.expected_audience = Some(audience);
        self
    }

    /// Add required claim
    pub fn with_required_claim(mut self, claim: String) -> Self {
        self.required_claims.push(claim);
        self
    }
}
