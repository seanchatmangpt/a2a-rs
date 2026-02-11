//! Authentication gate adapter implementations
//!
//! Provides concrete implementations of the AuthGate port using external
//! dependencies like jsonwebtoken and reqwest.

use async_trait::async_trait;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::domain::{AuthPrincipal, AuthRequest, EdgeError, PrincipalType, TokenValidationConfig};
use crate::port::auth_gate::{
    AuthGate, GoogleWorkspaceValidator, ServiceAccountValidator, TokenExtractor,
};

/// Standard JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    /// Subject (user ID)
    sub: String,

    /// Issuer
    #[serde(skip_serializing_if = "Option::is_none")]
    iss: Option<String>,

    /// Audience
    #[serde(skip_serializing_if = "Option::is_none")]
    aud: Option<serde_json::Value>,

    /// Expiration time (Unix timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    exp: Option<i64>,

    /// Issued at (Unix timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    iat: Option<i64>,

    /// Not before (Unix timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    nbf: Option<i64>,

    /// Email (for Google tokens)
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,

    /// Service account email
    #[serde(skip_serializing_if = "Option::is_none")]
    service_account_email: Option<String>,

    /// Additional claims
    #[serde(flatten)]
    additional: HashMap<String, serde_json::Value>,
}

/// Google OAuth2 token info response
#[derive(Debug, Deserialize)]
struct GoogleTokenInfo {
    /// User ID
    #[serde(skip_serializing_if = "Option::is_none")]
    sub: Option<String>,

    /// Email address
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,

    /// Email verified
    #[serde(skip_serializing_if = "Option::is_none")]
    email_verified: Option<bool>,

    /// Token audience
    #[serde(skip_serializing_if = "Option::is_none")]
    aud: Option<String>,

    /// Token issuer
    #[serde(skip_serializing_if = "Option::is_none")]
    iss: Option<String>,

    /// Token expiration
    #[serde(skip_serializing_if = "Option::is_none")]
    exp: Option<String>,

    /// Granted scopes (space-separated)
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,

    /// Error description if token is invalid
    #[serde(skip_serializing_if = "Option::is_none")]
    error_description: Option<String>,
}

/// Composite authentication gate supporting multiple validation methods
pub struct CompositeAuthGate {
    /// JWT validator for local token verification
    jwt_validator: Option<Arc<JwtAuthGate>>,

    /// Google Workspace token validator
    google_validator: Option<Arc<GoogleWorkspaceAuthGate>>,

    /// Service account validator
    service_account_validator: Option<Arc<ServiceAccountAuthGate>>,

    /// Token extractor
    token_extractor: Arc<BearerTokenExtractor>,

    /// Validation configuration
    config: TokenValidationConfig,
}

impl CompositeAuthGate {
    /// Create a new composite auth gate builder
    pub fn builder() -> CompositeAuthGateBuilder {
        CompositeAuthGateBuilder::default()
    }
}

/// Builder for CompositeAuthGate
#[derive(Default)]
pub struct CompositeAuthGateBuilder {
    jwt_validator: Option<Arc<JwtAuthGate>>,
    google_validator: Option<Arc<GoogleWorkspaceAuthGate>>,
    service_account_validator: Option<Arc<ServiceAccountAuthGate>>,
    config: Option<TokenValidationConfig>,
}

impl CompositeAuthGateBuilder {
    /// Add JWT validation
    pub fn with_jwt_validator(mut self, validator: JwtAuthGate) -> Self {
        self.jwt_validator = Some(Arc::new(validator));
        self
    }

    /// Add Google Workspace validation
    pub fn with_google_validator(mut self, validator: GoogleWorkspaceAuthGate) -> Self {
        self.google_validator = Some(Arc::new(validator));
        self
    }

    /// Add service account validation
    pub fn with_service_account_validator(mut self, validator: ServiceAccountAuthGate) -> Self {
        self.service_account_validator = Some(Arc::new(validator));
        self
    }

    /// Set validation configuration
    pub fn with_config(mut self, config: TokenValidationConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Build the composite auth gate
    pub fn build(self) -> CompositeAuthGate {
        CompositeAuthGate {
            jwt_validator: self.jwt_validator,
            google_validator: self.google_validator,
            service_account_validator: self.service_account_validator,
            token_extractor: Arc::new(BearerTokenExtractor),
            config: self.config.unwrap_or_default(),
        }
    }
}

#[async_trait]
impl AuthGate for CompositeAuthGate {
    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthPrincipal, EdgeError> {
        // Try JWT validation first (fastest, local validation)
        if let Some(jwt_validator) = &self.jwt_validator {
            if let Ok(principal) = jwt_validator.authenticate(request).await {
                return Ok(principal);
            }
        }

        // Try service account validation
        if let Some(sa_validator) = &self.service_account_validator {
            if let Ok(principal) = sa_validator.validate_service_account(&request.token).await {
                return Ok(principal);
            }
        }

        // Try Google Workspace validation (slowest, requires API call)
        if let Some(google_validator) = &self.google_validator {
            if let Ok(principal) = google_validator.validate_google_token(&request.token).await {
                return Ok(principal);
            }
        }

        Err(EdgeError::Authentication(
            "No validator accepted the token".to_string(),
        ))
    }

    async fn validate_token(&self, token: &str) -> Result<bool, EdgeError> {
        let request = AuthRequest::new(token.to_string());
        match self.authenticate(&request).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn authorize(
        &self,
        _principal: &AuthPrincipal,
        _resource: &str,
        _action: &str,
    ) -> Result<bool, EdgeError> {
        // Default implementation - can be extended with RBAC/ABAC
        Ok(true)
    }

    fn validation_config(&self) -> &TokenValidationConfig {
        &self.config
    }
}

/// JWT-based authentication gate
pub struct JwtAuthGate {
    /// Decoding keys (supports multiple keys for key rotation)
    decoding_keys: Vec<DecodingKey>,

    /// Validation configuration
    validation: Validation,

    /// Token validation config
    config: TokenValidationConfig,
}

impl JwtAuthGate {
    /// Create a new JWT auth gate with a secret key
    pub fn new_with_secret(secret: &[u8]) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        Self {
            decoding_keys: vec![DecodingKey::from_secret(secret)],
            validation,
            config: TokenValidationConfig::default(),
        }
    }

    /// Create a new JWT auth gate with an RSA public key
    pub fn new_with_rsa_pem(pem: &[u8]) -> Result<Self, EdgeError> {
        let decoding_key = DecodingKey::from_rsa_pem(pem)
            .map_err(|e| EdgeError::Configuration(format!("Invalid RSA PEM: {}", e)))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;

        Ok(Self {
            decoding_keys: vec![decoding_key],
            validation,
            config: TokenValidationConfig::default(),
        })
    }

    /// Set the validation configuration
    pub fn with_config(mut self, config: TokenValidationConfig) -> Self {
        if let Some(issuer) = &config.expected_issuer {
            self.validation.iss = Some(HashSet::from([issuer.clone()]));
        }
        if let Some(audience) = &config.expected_audience {
            self.validation.aud = Some(HashSet::from([audience.clone()]));
        }
        self.validation.validate_exp = config.validate_expiration;
        self.validation.leeway = config.clock_skew_seconds as u64;

        self.config = config;
        self
    }
}

#[async_trait]
impl AuthGate for JwtAuthGate {
    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthPrincipal, EdgeError> {
        let token = &request.token;

        // Try each decoding key (for key rotation support)
        let mut last_error = None;
        for key in &self.decoding_keys {
            match decode::<JwtClaims>(token, key, &self.validation) {
                Ok(token_data) => {
                    let claims = token_data.claims;

                    // Validate required claims
                    for required_claim in &self.config.required_claims {
                        if !claims.additional.contains_key(required_claim) {
                            return Err(EdgeError::MissingClaim(required_claim.clone()));
                        }
                    }

                    // Build principal
                    let mut principal_claims = HashMap::new();
                    principal_claims.extend(claims.additional.into_iter().map(|(k, v)| (k, v)));

                    let principal_type = if claims.service_account_email.is_some() {
                        PrincipalType::ServiceAccount
                    } else {
                        PrincipalType::User
                    };

                    return Ok(AuthPrincipal {
                        subject: claims.sub,
                        email: claims.email,
                        issuer: claims.iss,
                        audience: extract_audience(&claims.aud),
                        principal_type,
                        claims: principal_claims,
                        expires_at: claims.exp,
                    });
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        Err(EdgeError::TokenValidation(format!(
            "JWT validation failed: {}",
            last_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Unknown error".to_string())
        )))
    }

    async fn validate_token(&self, token: &str) -> Result<bool, EdgeError> {
        let request = AuthRequest::new(token.to_string());
        match self.authenticate(&request).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn authorize(
        &self,
        _principal: &AuthPrincipal,
        _resource: &str,
        _action: &str,
    ) -> Result<bool, EdgeError> {
        // Default implementation
        Ok(true)
    }

    fn validation_config(&self) -> &TokenValidationConfig {
        &self.config
    }
}

/// Google Workspace OAuth2 authentication gate
pub struct GoogleWorkspaceAuthGate {
    /// HTTP client for token validation
    client: reqwest::Client,

    /// Expected OAuth2 client ID
    expected_client_id: Option<String>,

    /// Required scopes
    required_scopes: Vec<String>,

    /// Token validation config
    config: TokenValidationConfig,
}

impl GoogleWorkspaceAuthGate {
    /// Create a new Google Workspace auth gate
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            expected_client_id: None,
            required_scopes: Vec::new(),
            config: TokenValidationConfig::default(),
        }
    }

    /// Set the expected client ID
    pub fn with_client_id(mut self, client_id: String) -> Self {
        self.expected_client_id = Some(client_id);
        self
    }

    /// Set required scopes
    pub fn with_required_scopes(mut self, scopes: Vec<String>) -> Self {
        self.required_scopes = scopes;
        self
    }

    /// Set validation configuration
    pub fn with_config(mut self, config: TokenValidationConfig) -> Self {
        self.config = config;
        self
    }
}

impl Default for GoogleWorkspaceAuthGate {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GoogleWorkspaceValidator for GoogleWorkspaceAuthGate {
    async fn validate_google_token(&self, access_token: &str) -> Result<AuthPrincipal, EdgeError> {
        let url = format!(
            "https://oauth2.googleapis.com/tokeninfo?access_token={}",
            access_token
        );

        let response = self.client.get(&url).send().await.map_err(|e| {
            EdgeError::HttpClient(format!("Token validation request failed: {}", e))
        })?;

        if !response.status().is_success() {
            return Err(EdgeError::TokenValidation(format!(
                "Google token validation failed with status: {}",
                response.status()
            )));
        }

        let token_info: GoogleTokenInfo = response
            .json()
            .await
            .map_err(|e| EdgeError::HttpClient(format!("Failed to parse token info: {}", e)))?;

        // Check for error in response
        if let Some(error_desc) = token_info.error_description {
            return Err(EdgeError::TokenValidation(error_desc));
        }

        // Validate client ID if configured
        if let Some(expected_client_id) = &self.expected_client_id {
            if let Some(aud) = &token_info.aud {
                if aud != expected_client_id {
                    return Err(EdgeError::InvalidAudience {
                        expected: expected_client_id.clone(),
                        actual: aud.clone(),
                    });
                }
            }
        }

        // Validate scopes if required
        if !self.required_scopes.is_empty() {
            if let Some(scope_str) = &token_info.scope {
                let granted_scopes: HashSet<String> =
                    scope_str.split_whitespace().map(String::from).collect();
                let required_scopes: HashSet<String> =
                    self.required_scopes.iter().cloned().collect();

                if !required_scopes.is_subset(&granted_scopes) {
                    return Err(EdgeError::Authorization(
                        "Missing required scopes".to_string(),
                    ));
                }
            } else {
                return Err(EdgeError::MissingClaim("scope".to_string()));
            }
        }

        // Parse expiration
        let expires_at = token_info
            .exp
            .and_then(|exp_str| exp_str.parse::<i64>().ok());

        Ok(AuthPrincipal {
            subject: token_info.sub.unwrap_or_default(),
            email: token_info.email,
            issuer: token_info.iss,
            audience: token_info.aud,
            principal_type: PrincipalType::User,
            claims: HashMap::new(),
            expires_at,
        })
    }

    async fn validate_scopes(
        &self,
        access_token: &str,
        required_scopes: &[String],
    ) -> Result<bool, EdgeError> {
        let principal = self.validate_google_token(access_token).await?;

        // For now, return true if token is valid
        // In production, you'd check principal claims for scopes
        Ok(principal.principal_type == PrincipalType::User)
    }
}

#[async_trait]
impl AuthGate for GoogleWorkspaceAuthGate {
    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthPrincipal, EdgeError> {
        self.validate_google_token(&request.token).await
    }

    async fn validate_token(&self, token: &str) -> Result<bool, EdgeError> {
        match self.validate_google_token(token).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn authorize(
        &self,
        _principal: &AuthPrincipal,
        _resource: &str,
        _action: &str,
    ) -> Result<bool, EdgeError> {
        Ok(true)
    }

    fn validation_config(&self) -> &TokenValidationConfig {
        &self.config
    }
}

/// Service account authentication gate
pub struct ServiceAccountAuthGate {
    /// Allowed service account IDs
    allowed_service_accounts: HashSet<String>,

    /// JWT validator for service account tokens
    jwt_validator: JwtAuthGate,

    /// Service account permissions map (service_account_id -> allowed_actions)
    permissions: HashMap<String, HashSet<String>>,
}

impl ServiceAccountAuthGate {
    /// Create a new service account auth gate
    pub fn new_with_secret(secret: &[u8]) -> Self {
        Self {
            allowed_service_accounts: HashSet::new(),
            jwt_validator: JwtAuthGate::new_with_secret(secret),
            permissions: HashMap::new(),
        }
    }

    /// Create with RSA public key
    pub fn new_with_rsa_pem(pem: &[u8]) -> Result<Self, EdgeError> {
        Ok(Self {
            allowed_service_accounts: HashSet::new(),
            jwt_validator: JwtAuthGate::new_with_rsa_pem(pem)?,
            permissions: HashMap::new(),
        })
    }

    /// Add an allowed service account
    pub fn with_allowed_service_account(mut self, service_account_id: String) -> Self {
        self.allowed_service_accounts.insert(service_account_id);
        self
    }

    /// Add permissions for a service account
    pub fn with_permissions(mut self, service_account_id: String, actions: Vec<String>) -> Self {
        self.permissions
            .insert(service_account_id, actions.into_iter().collect());
        self
    }

    /// Set validation configuration
    pub fn with_config(mut self, config: TokenValidationConfig) -> Self {
        self.jwt_validator = self.jwt_validator.with_config(config);
        self
    }
}

#[async_trait]
impl ServiceAccountValidator for ServiceAccountAuthGate {
    async fn validate_service_account(&self, token: &str) -> Result<AuthPrincipal, EdgeError> {
        let request = AuthRequest::new(token.to_string());
        let mut principal = self.jwt_validator.authenticate(&request).await?;

        // Verify it's a service account
        if principal.principal_type != PrincipalType::ServiceAccount {
            return Err(EdgeError::Authentication(
                "Token is not a service account token".to_string(),
            ));
        }

        // Check if service account is allowed
        if !self.allowed_service_accounts.is_empty()
            && !self.allowed_service_accounts.contains(&principal.subject)
        {
            return Err(EdgeError::Authorization(format!(
                "Service account {} is not allowed",
                principal.subject
            )));
        }

        // Update principal type
        principal.principal_type = PrincipalType::ServiceAccount;

        Ok(principal)
    }

    async fn check_service_account_permission(
        &self,
        service_account_id: &str,
        action: &str,
    ) -> Result<bool, EdgeError> {
        if let Some(allowed_actions) = self.permissions.get(service_account_id) {
            Ok(allowed_actions.contains(action))
        } else {
            // If no permissions configured, allow all (default permissive)
            Ok(true)
        }
    }
}

#[async_trait]
impl AuthGate for ServiceAccountAuthGate {
    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthPrincipal, EdgeError> {
        self.validate_service_account(&request.token).await
    }

    async fn validate_token(&self, token: &str) -> Result<bool, EdgeError> {
        match self.validate_service_account(token).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn authorize(
        &self,
        principal: &AuthPrincipal,
        _resource: &str,
        action: &str,
    ) -> Result<bool, EdgeError> {
        self.check_service_account_permission(&principal.subject, action)
            .await
    }

    fn validation_config(&self) -> &TokenValidationConfig {
        self.jwt_validator.validation_config()
    }
}

/// Bearer token extractor
pub struct BearerTokenExtractor;

impl TokenExtractor for BearerTokenExtractor {
    fn extract_from_authorization(&self, authorization_header: &str) -> Option<String> {
        let parts: Vec<&str> = authorization_header.splitn(2, ' ').collect();
        if parts.len() == 2 && parts[0].eq_ignore_ascii_case("bearer") {
            Some(parts[1].to_string())
        } else {
            None
        }
    }

    fn extract_from_query(&self, query_params: &HashMap<String, String>) -> Option<String> {
        query_params
            .get("access_token")
            .or_else(|| query_params.get("token"))
            .cloned()
    }

    fn extract_from_cookie(&self, cookie_header: &str) -> Option<String> {
        for cookie in cookie_header.split(';') {
            let parts: Vec<&str> = cookie.trim().splitn(2, '=').collect();
            if parts.len() == 2 && parts[0] == "access_token" {
                return Some(parts[1].to_string());
            }
        }
        None
    }
}

/// Helper function to extract audience from JWT claim
fn extract_audience(aud: &Option<serde_json::Value>) -> Option<String> {
    match aud {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Array(arr)) => {
            arr.first().and_then(|v| v.as_str().map(String::from))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bearer_token_extractor() {
        let extractor = BearerTokenExtractor;

        // Test authorization header
        let token = extractor.extract_from_authorization("Bearer abc123");
        assert_eq!(token, Some("abc123".to_string()));

        // Test case insensitive
        let token = extractor.extract_from_authorization("bearer xyz789");
        assert_eq!(token, Some("xyz789".to_string()));

        // Test invalid format
        let token = extractor.extract_from_authorization("Token abc123");
        assert_eq!(token, None);
    }

    #[test]
    fn test_extract_from_query() {
        let extractor = BearerTokenExtractor;
        let mut params = HashMap::new();
        params.insert("access_token".to_string(), "token123".to_string());

        let token = extractor.extract_from_query(&params);
        assert_eq!(token, Some("token123".to_string()));
    }

    #[test]
    fn test_extract_from_cookie() {
        let extractor = BearerTokenExtractor;
        let cookie_header = "session_id=xyz; access_token=abc123; other=value";

        let token = extractor.extract_from_cookie(cookie_header);
        assert_eq!(token, Some("abc123".to_string()));
    }
}
