//! JWT authentication implementation using jsonwebtoken crate
//!
//! Features:
//! - RS256 and HS256 token signing
//! - Agent-specific claims (agent_id, permissions)
//! - Token generation and validation
//! - Refresh token mechanism
//! - Configurable expiry and issuer

#[cfg(feature = "auth")]
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use async_trait::async_trait;

use crate::{
    domain::{A2AError, core::agent::SecurityScheme},
    port::authenticator::{AuthContext, AuthContextExtractor, AuthPrincipal, Authenticator},
};

/// JWT Claims structure for agent authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentClaims {
    /// Agent ID (subject)
    pub agent_id: String,
    /// Permissions granted to this agent
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Expiration time (Unix timestamp)
    pub exp: i64,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Not before (Unix timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nbf: Option<i64>,
    /// Issuer
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
    /// Audience
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aud: Option<String>,
    /// Token type (access or refresh)
    #[serde(default = "default_token_type")]
    pub token_type: String,
    /// Additional custom claims
    #[serde(flatten)]
    pub additional: HashMap<String, serde_json::Value>,
}

fn default_token_type() -> String {
    "access".to_string()
}

impl AgentClaims {
    /// Create new agent claims
    pub fn new(agent_id: String, permissions: Vec<String>, expiry_secs: i64) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            agent_id,
            permissions,
            exp: now + expiry_secs,
            iat: now,
            nbf: None,
            iss: None,
            aud: None,
            token_type: "access".to_string(),
            additional: HashMap::new(),
        }
    }

    /// Create a refresh token with longer expiry
    pub fn new_refresh(agent_id: String, expiry_secs: i64) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            agent_id,
            permissions: vec![],
            exp: now + expiry_secs,
            iat: now,
            nbf: None,
            iss: None,
            aud: None,
            token_type: "refresh".to_string(),
            additional: HashMap::new(),
        }
    }

    /// Set issuer
    pub fn with_issuer(mut self, issuer: String) -> Self {
        self.iss = Some(issuer);
        self
    }

    /// Set audience
    pub fn with_audience(mut self, audience: String) -> Self {
        self.aud = Some(audience);
        self
    }

    /// Set not before timestamp
    pub fn with_not_before(mut self, nbf: i64) -> Self {
        self.nbf = Some(nbf);
        self
    }

    /// Add custom claim
    pub fn with_claim(mut self, key: String, value: serde_json::Value) -> Self {
        self.additional.insert(key, value);
        self
    }

    /// Check if this is a refresh token
    pub fn is_refresh_token(&self) -> bool {
        self.token_type == "refresh"
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.exp <= now
    }
}

/// Token pair containing access and refresh tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenPair {
    /// Access token (short-lived)
    pub access_token: String,
    /// Refresh token (long-lived)
    pub refresh_token: String,
    /// Token type (always "Bearer")
    pub token_type: String,
    /// Access token expiry in seconds
    pub expires_in: i64,
}

/// Configuration for JWT token generation
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// Access token expiry in seconds (default: 15 minutes)
    pub access_token_expiry: i64,
    /// Refresh token expiry in seconds (default: 7 days)
    pub refresh_token_expiry: i64,
    /// Issuer
    pub issuer: Option<String>,
    /// Audience
    pub audience: Option<String>,
    /// Algorithm (RS256 or HS256)
    pub algorithm: Algorithm,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            access_token_expiry: 900,     // 15 minutes
            refresh_token_expiry: 604800, // 7 days
            issuer: None,
            audience: None,
            algorithm: Algorithm::RS256,
        }
    }
}

impl JwtConfig {
    /// Create a new configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set access token expiry
    pub fn with_access_expiry(mut self, seconds: i64) -> Self {
        self.access_token_expiry = seconds;
        self
    }

    /// Set refresh token expiry
    pub fn with_refresh_expiry(mut self, seconds: i64) -> Self {
        self.refresh_token_expiry = seconds;
        self
    }

    /// Set issuer
    pub fn with_issuer(mut self, issuer: String) -> Self {
        self.issuer = Some(issuer);
        self
    }

    /// Set audience
    pub fn with_audience(mut self, audience: String) -> Self {
        self.audience = Some(audience);
        self
    }

    /// Set algorithm
    pub fn with_algorithm(mut self, algorithm: Algorithm) -> Self {
        self.algorithm = algorithm;
        self
    }
}

/// JWT authenticator using the jsonwebtoken crate
#[cfg(feature = "auth")]
#[derive(Clone)]
pub struct JwtAuthenticator {
    /// Encoding key for JWT signing (optional - only needed for token generation)
    encoding_key: Option<EncodingKey>,
    /// Decoding key for JWT verification
    decoding_key: DecodingKey,
    /// Validation rules
    validation: Validation,
    /// Security scheme configuration
    scheme: SecurityScheme,
    /// JWT configuration
    config: JwtConfig,
}

#[cfg(feature = "auth")]
impl JwtAuthenticator {
    /// Create a new JWT authenticator with HS256 secret
    pub fn new_with_secret(secret: &[u8]) -> Self {
        let mut config = JwtConfig::default();
        config.algorithm = Algorithm::HS256;

        Self {
            encoding_key: Some(EncodingKey::from_secret(secret)),
            decoding_key: DecodingKey::from_secret(secret),
            validation: Validation::new(Algorithm::HS256),
            scheme: SecurityScheme::Http {
                scheme: "bearer".to_string(),
                bearer_format: Some("JWT".to_string()),
                description: Some("JWT Bearer token authentication (HS256)".to_string()),
            },
            config,
        }
    }

    /// Create a new JWT authenticator with RSA keys (RS256)
    pub fn new_with_rsa_pem(private_pem: &[u8], public_pem: &[u8]) -> Result<Self, A2AError> {
        let encoding_key = EncodingKey::from_rsa_pem(private_pem)
            .map_err(|e| A2AError::Internal(format!("Invalid RSA private key: {}", e)))?;

        let decoding_key = DecodingKey::from_rsa_pem(public_pem)
            .map_err(|e| A2AError::Internal(format!("Invalid RSA public key: {}", e)))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;

        Ok(Self {
            encoding_key: Some(encoding_key),
            decoding_key,
            validation,
            scheme: SecurityScheme::Http {
                scheme: "bearer".to_string(),
                bearer_format: Some("JWT".to_string()),
                description: Some("JWT Bearer token authentication (RS256)".to_string()),
            },
            config: JwtConfig::default(),
        })
    }

    /// Create a new JWT authenticator with only public key (verification only)
    pub fn new_with_public_key(public_pem: &[u8]) -> Result<Self, A2AError> {
        let decoding_key = DecodingKey::from_rsa_pem(public_pem)
            .map_err(|e| A2AError::Internal(format!("Invalid RSA public key: {}", e)))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;

        Ok(Self {
            encoding_key: None,
            decoding_key,
            validation,
            scheme: SecurityScheme::Http {
                scheme: "bearer".to_string(),
                bearer_format: Some("JWT".to_string()),
                description: Some("JWT Bearer token authentication (RS256)".to_string()),
            },
            config: JwtConfig::default(),
        })
    }

    /// Set JWT configuration
    pub fn with_config(mut self, config: JwtConfig) -> Self {
        self.validation
            .set_issuer(&[config.issuer.clone().unwrap_or_default()]);
        self.validation
            .set_audience(&[config.audience.clone().unwrap_or_default()]);
        self.config = config;
        self
    }

    /// Set custom validation rules
    pub fn with_validation(mut self, validation: Validation) -> Self {
        self.validation = validation;
        self
    }

    /// Generate an access token for an agent
    pub fn generate_access_token(
        &self,
        agent_id: String,
        permissions: Vec<String>,
    ) -> Result<String, A2AError> {
        let encoding_key = self
            .encoding_key
            .as_ref()
            .ok_or_else(|| A2AError::Internal("No encoding key configured".to_string()))?;

        let mut claims = AgentClaims::new(agent_id, permissions, self.config.access_token_expiry);

        if let Some(ref issuer) = self.config.issuer {
            claims = claims.with_issuer(issuer.clone());
        }

        if let Some(ref audience) = self.config.audience {
            claims = claims.with_audience(audience.clone());
        }

        let header = Header::new(self.config.algorithm);

        encode(&header, &claims, encoding_key)
            .map_err(|e| A2AError::Internal(format!("Failed to generate token: {}", e)))
    }

    /// Generate a refresh token for an agent
    pub fn generate_refresh_token(&self, agent_id: String) -> Result<String, A2AError> {
        let encoding_key = self
            .encoding_key
            .as_ref()
            .ok_or_else(|| A2AError::Internal("No encoding key configured".to_string()))?;

        let mut claims = AgentClaims::new_refresh(agent_id, self.config.refresh_token_expiry);

        if let Some(ref issuer) = self.config.issuer {
            claims = claims.with_issuer(issuer.clone());
        }

        if let Some(ref audience) = self.config.audience {
            claims = claims.with_audience(audience.clone());
        }

        let header = Header::new(self.config.algorithm);

        encode(&header, &claims, encoding_key)
            .map_err(|e| A2AError::Internal(format!("Failed to generate refresh token: {}", e)))
    }

    /// Generate both access and refresh tokens
    pub fn generate_token_pair(
        &self,
        agent_id: String,
        permissions: Vec<String>,
    ) -> Result<TokenPair, A2AError> {
        let access_token = self.generate_access_token(agent_id.clone(), permissions)?;
        let refresh_token = self.generate_refresh_token(agent_id)?;

        Ok(TokenPair {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.access_token_expiry,
        })
    }

    /// Refresh an access token using a valid refresh token
    pub fn refresh_access_token(
        &self,
        refresh_token: &str,
        permissions: Vec<String>,
    ) -> Result<String, A2AError> {
        // Validate the refresh token
        let token_data = decode::<AgentClaims>(refresh_token, &self.decoding_key, &self.validation)
            .map_err(|e| A2AError::Internal(format!("Invalid refresh token: {}", e)))?;

        // Ensure it's actually a refresh token
        if !token_data.claims.is_refresh_token() {
            return Err(A2AError::Internal(
                "Token is not a refresh token".to_string(),
            ));
        }

        // Generate new access token
        self.generate_access_token(token_data.claims.agent_id, permissions)
    }

    /// Validate a token and return claims without authentication
    pub fn validate_token(&self, token: &str) -> Result<AgentClaims, A2AError> {
        let token_data = decode::<AgentClaims>(token, &self.decoding_key, &self.validation)
            .map_err(|e| A2AError::Internal(format!("Token validation failed: {}", e)))?;

        Ok(token_data.claims)
    }
}

#[cfg(feature = "auth")]
#[async_trait]
impl Authenticator for JwtAuthenticator {
    async fn authenticate(&self, context: &AuthContext) -> Result<AuthPrincipal, A2AError> {
        self.validate_context(context)?;

        let token = &context.credential;

        let token_data = decode::<AgentClaims>(token, &self.decoding_key, &self.validation)
            .map_err(|e| A2AError::Internal(format!("JWT validation failed: {}", e)))?;

        // Ensure it's an access token, not a refresh token
        if token_data.claims.is_refresh_token() {
            return Err(A2AError::Internal(
                "Refresh tokens cannot be used for authentication".to_string(),
            ));
        }

        let mut principal =
            AuthPrincipal::new(token_data.claims.agent_id.clone(), "jwt".to_string());

        // Add permissions
        principal = principal.with_attribute(
            "permissions".to_string(),
            token_data.claims.permissions.join(","),
        );

        // Add JWT metadata
        if let Some(iss) = token_data.claims.iss {
            principal = principal.with_attribute("issuer".to_string(), iss);
        }
        if let Some(aud) = token_data.claims.aud {
            principal = principal.with_attribute("audience".to_string(), aud);
        }
        principal = principal.with_attribute("exp".to_string(), token_data.claims.exp.to_string());
        principal = principal.with_attribute("iat".to_string(), token_data.claims.iat.to_string());
        principal = principal.with_attribute("agent_id".to_string(), token_data.claims.agent_id);

        // Add additional claims
        for (key, value) in token_data.claims.additional {
            if let Ok(string_value) = serde_json::to_string(&value) {
                principal = principal.with_attribute(key, string_value);
            }
        }

        Ok(principal)
    }

    fn security_scheme(&self) -> &SecurityScheme {
        &self.scheme
    }

    fn validate_context(&self, context: &AuthContext) -> Result<(), A2AError> {
        if context.scheme_type != "bearer" {
            return Err(A2AError::Internal(format!(
                "Invalid authentication scheme: expected 'bearer', got '{}'",
                context.scheme_type
            )));
        }
        Ok(())
    }
}

/// JWT extractor for Bearer tokens
#[derive(Clone)]
pub struct JwtExtractor;

#[async_trait]
impl AuthContextExtractor for JwtExtractor {
    #[cfg(feature = "http-server")]
    async fn extract_from_headers(&self, headers: &axum::http::HeaderMap) -> Option<AuthContext> {
        headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|auth| {
                let parts: Vec<&str> = auth.splitn(2, ' ').collect();
                if parts.len() == 2 && parts[0].to_lowercase() == "bearer" {
                    Some(
                        AuthContext::new("bearer".to_string(), parts[1].to_string())
                            .with_metadata("format".to_string(), "JWT".to_string()),
                    )
                } else {
                    None
                }
            })
    }

    #[cfg(not(feature = "http-server"))]
    async fn extract_from_headers(&self, headers: &HashMap<String, String>) -> Option<AuthContext> {
        headers
            .get("authorization")
            .or_else(|| headers.get("Authorization"))
            .and_then(|auth| {
                let parts: Vec<&str> = auth.splitn(2, ' ').collect();
                if parts.len() == 2 && parts[0].to_lowercase() == "bearer" {
                    Some(
                        AuthContext::new("bearer".to_string(), parts[1].to_string())
                            .with_metadata("format".to_string(), "JWT".to_string()),
                    )
                } else {
                    None
                }
            })
    }

    async fn extract_from_query(&self, _params: &HashMap<String, String>) -> Option<AuthContext> {
        // JWTs are typically not passed in query parameters for security reasons
        None
    }

    async fn extract_from_cookies(&self, _cookies: &str) -> Option<AuthContext> {
        // JWTs can be passed in cookies, but we'll keep this simple for now
        None
    }
}

#[cfg(not(feature = "auth"))]
/// Placeholder when auth feature is not enabled
pub struct JwtAuthenticator;

#[cfg(not(feature = "auth"))]
impl JwtAuthenticator {
    pub fn new_with_secret(_secret: &[u8]) -> Self {
        compile_error!("JWT authentication requires the 'auth' feature");
    }
}

#[cfg(all(test, feature = "auth"))]
mod tests {
    use super::*;

    #[test]
    fn test_agent_claims_creation() {
        let claims = AgentClaims::new(
            "agent-123".to_string(),
            vec!["read".to_string(), "write".to_string()],
            3600,
        );

        assert_eq!(claims.agent_id, "agent-123");
        assert_eq!(claims.permissions.len(), 2);
        assert_eq!(claims.token_type, "access");
        assert!(!claims.is_refresh_token());
    }

    #[test]
    fn test_refresh_token_creation() {
        let claims = AgentClaims::new_refresh("agent-123".to_string(), 604800);

        assert_eq!(claims.agent_id, "agent-123");
        assert!(claims.permissions.is_empty());
        assert_eq!(claims.token_type, "refresh");
        assert!(claims.is_refresh_token());
    }

    #[tokio::test]
    async fn test_jwt_token_generation_hs256() {
        let secret = b"test-secret-key-for-testing";
        let jwt = JwtAuthenticator::new_with_secret(secret);

        let token = jwt
            .generate_access_token("agent-123".to_string(), vec!["read".to_string()])
            .expect("Failed to generate token");

        assert!(!token.is_empty());

        // Validate the token
        let claims = jwt
            .validate_token(&token)
            .expect("Failed to validate token");
        assert_eq!(claims.agent_id, "agent-123");
        assert_eq!(claims.permissions, vec!["read"]);
    }

    #[tokio::test]
    async fn test_token_pair_generation() {
        let secret = b"test-secret-key-for-testing";
        let jwt = JwtAuthenticator::new_with_secret(secret);

        let token_pair = jwt
            .generate_token_pair("agent-123".to_string(), vec!["read".to_string()])
            .expect("Failed to generate token pair");

        assert!(!token_pair.access_token.is_empty());
        assert!(!token_pair.refresh_token.is_empty());
        assert_eq!(token_pair.token_type, "Bearer");
    }

    #[tokio::test]
    async fn test_refresh_token_flow() {
        let secret = b"test-secret-key-for-testing";
        let jwt = JwtAuthenticator::new_with_secret(secret);

        // Generate initial token pair
        let token_pair = jwt
            .generate_token_pair("agent-123".to_string(), vec!["read".to_string()])
            .expect("Failed to generate token pair");

        // Use refresh token to get new access token
        let new_access_token = jwt
            .refresh_access_token(
                &token_pair.refresh_token,
                vec!["read".to_string(), "write".to_string()],
            )
            .expect("Failed to refresh token");

        assert!(!new_access_token.is_empty());

        // Validate the new token
        let claims = jwt
            .validate_token(&new_access_token)
            .expect("Failed to validate");
        assert_eq!(claims.agent_id, "agent-123");
        assert_eq!(claims.permissions, vec!["read", "write"]);
    }

    #[tokio::test]
    async fn test_authentication_with_access_token() {
        let secret = b"test-secret-key-for-testing";
        let jwt = JwtAuthenticator::new_with_secret(secret);

        let token = jwt
            .generate_access_token("agent-123".to_string(), vec!["read".to_string()])
            .expect("Failed to generate token");

        let context = AuthContext::new("bearer".to_string(), token);

        let principal = jwt
            .authenticate(&context)
            .await
            .expect("Authentication failed");

        assert_eq!(principal.id, "agent-123");
        assert_eq!(principal.scheme, "jwt");
    }

    #[tokio::test]
    async fn test_refresh_token_rejected_for_auth() {
        let secret = b"test-secret-key-for-testing";
        let jwt = JwtAuthenticator::new_with_secret(secret);

        let refresh_token = jwt
            .generate_refresh_token("agent-123".to_string())
            .expect("Failed to generate refresh token");

        let context = AuthContext::new("bearer".to_string(), refresh_token);

        let result = jwt.authenticate(&context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_jwt_config() {
        let config = JwtConfig::new()
            .with_access_expiry(1800)
            .with_refresh_expiry(2592000)
            .with_issuer("test-issuer".to_string())
            .with_audience("test-audience".to_string())
            .with_algorithm(Algorithm::HS256);

        assert_eq!(config.access_token_expiry, 1800);
        assert_eq!(config.refresh_token_expiry, 2592000);
        assert_eq!(config.issuer, Some("test-issuer".to_string()));
        assert_eq!(config.audience, Some("test-audience".to_string()));
        assert_eq!(config.algorithm, Algorithm::HS256);
    }
}
