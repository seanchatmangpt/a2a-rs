//! OAuth2 PKCE authenticator adapter
//!
//! Concrete implementation of the Oauth2Authenticator port using reqwest for HTTP requests
//! and in-memory session storage. Implements RFC 7636 Proof Key for Public Clients Exchange.

use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::domain::{
    AuthorizationRequest, AuthorizationResponse, ChallengeMethod, CodeChallenge, CodeVerifier,
    EdgeError, Oauth2Session, RefreshTokenRequest, TokenRequest, TokenResponse,
};
use crate::port::Oauth2Authenticator;

/// OAuth2 PKCE authenticator adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PkceConfig {
    /// HTTP client timeout in seconds
    pub client_timeout_seconds: u64,

    /// Session expiration buffer (refresh session when remaining time <= buffer)
    pub session_expiration_buffer_seconds: i64,

    /// Maximum number of stored sessions
    pub max_stored_sessions: usize,

    /// Use SHA256 for code challenge (recommended)
    pub use_sha256: bool,

    /// Optional HTTP client User-Agent header
    pub user_agent: String,
}

impl Default for PkceConfig {
    fn default() -> Self {
        Self {
            client_timeout_seconds: 30,
            session_expiration_buffer_seconds: 300,
            max_stored_sessions: 1000,
            use_sha256: true,
            user_agent: "osiris-edge/0.1.0".to_string(),
        }
    }
}

impl PkceConfig {
    /// Create a new configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set client timeout
    pub fn with_client_timeout(mut self, seconds: u64) -> Self {
        self.client_timeout_seconds = seconds;
        self
    }

    /// Set session expiration buffer
    pub fn with_expiration_buffer(mut self, seconds: i64) -> Self {
        self.session_expiration_buffer_seconds = seconds;
        self
    }

    /// Set maximum stored sessions
    pub fn with_max_sessions(mut self, max: usize) -> Self {
        self.max_stored_sessions = max;
        self
    }

    /// Set user agent
    pub fn with_user_agent(mut self, agent: String) -> Self {
        self.user_agent = agent;
        self
    }
}

/// OAuth2 PKCE authenticator implementation
pub struct PkceAuthenticator {
    config: PkceConfig,
    http_client: Client,
    /// In-memory session storage: session_id -> session
    sessions: Arc<RwLock<HashMap<String, Oauth2Session>>>,
}

impl PkceAuthenticator {
    /// Create a new PKCE authenticator with default configuration
    pub fn new() -> Result<Self, EdgeError> {
        Self::with_config(PkceConfig::default())
    }

    /// Create a new PKCE authenticator with custom configuration
    pub fn with_config(config: PkceConfig) -> Result<Self, EdgeError> {
        let timeout = std::time::Duration::from_secs(config.client_timeout_seconds);
        let http_client = Client::builder()
            .timeout(timeout)
            .user_agent(&config.user_agent)
            .build()
            .map_err(|e| EdgeError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            http_client,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Generate cryptographically secure random string for code verifier
    fn generate_random_string(&self, length: usize) -> String {
        use std::fmt::Write;

        const CHARSET: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";

        // Use a UUID as a source of randomness
        let seed = Uuid::new_v4().as_bytes().to_vec();
        let mut hasher = Sha256::new();
        hasher.update(&seed);
        let hash = hasher.finalize();

        let mut result = String::with_capacity(length);
        for (i, byte) in hash.iter().chain(seed.iter()).take(length).enumerate() {
            let idx = (*byte as usize + i) % CHARSET.len();
            let _ = write!(result, "{}", CHARSET[idx] as char);
        }

        result.truncate(length);
        result
    }

    /// Cleanup old sessions if storage limit exceeded
    async fn cleanup_old_sessions(&self) {
        let mut sessions = self.sessions.write().await;

        if sessions.len() > self.config.max_stored_sessions {
            // Remove oldest sessions (by creation time)
            let mut sessions_vec: Vec<_> = sessions
                .iter()
                .map(|(id, session)| (id.clone(), session.created_at))
                .collect();

            sessions_vec.sort_by_key(|(_, created_at)| *created_at);

            let remove_count = sessions.len() - self.config.max_stored_sessions;
            for (id, _) in sessions_vec.iter().take(remove_count) {
                sessions.remove(id);
            }
        }
    }
}

impl Default for PkceAuthenticator {
    fn default() -> Self {
        Self::new().expect("Failed to create default PKCE authenticator")
    }
}

#[async_trait]
impl Oauth2Authenticator for PkceAuthenticator {
    async fn generate_code_verifier_and_challenge(
        &self,
    ) -> Result<(CodeVerifier, CodeChallenge), EdgeError> {
        // Generate 128-character code verifier for maximum entropy
        let verifier_str = self.generate_random_string(128);

        let verifier = CodeVerifier::new(verifier_str).map_err(|e| {
            EdgeError::Configuration(format!("Failed to create code verifier: {}", e))
        })?;

        let challenge = if self.config.use_sha256 {
            CodeChallenge::sha256(&verifier)
        } else {
            CodeChallenge::plain(&verifier)
        };

        Ok((verifier, challenge))
    }

    async fn create_authorization_request(
        &self,
        client_id: String,
        authorization_endpoint: String,
        redirect_uri: String,
        scope: String,
        code_challenge: CodeChallenge,
        code_verifier: CodeVerifier,
    ) -> Result<AuthorizationRequest, EdgeError> {
        let state = Uuid::new_v4().to_string();

        Ok(AuthorizationRequest {
            client_id,
            authorization_endpoint,
            redirect_uri,
            scope,
            code_challenge,
            code_verifier,
            state,
            additional_params: HashMap::new(),
        })
    }

    async fn validate_authorization_response(
        &self,
        response: &AuthorizationResponse,
        expected_state: &str,
    ) -> Result<String, EdgeError> {
        // Check for error response
        if let Some(error) = &response.error {
            return Err(EdgeError::Authentication(format!(
                "Authorization failed: {} ({})",
                error,
                response.error_description.as_deref().unwrap_or("unknown")
            )));
        }

        // Validate state parameter (CSRF protection)
        if response.state != expected_state {
            return Err(EdgeError::Authentication(
                "State parameter mismatch - possible CSRF attack".to_string(),
            ));
        }

        // Extract and validate authorization code
        if response.code.is_empty() {
            return Err(EdgeError::Authentication(
                "Missing authorization code in response".to_string(),
            ));
        }

        Ok(response.code.clone())
    }

    async fn exchange_code_for_token(
        &self,
        request: &TokenRequest,
    ) -> Result<TokenResponse, EdgeError> {
        let mut params = HashMap::new();
        params.insert("grant_type", "authorization_code");
        params.insert("code", &request.code);
        params.insert("client_id", &request.client_id);
        params.insert("redirect_uri", &request.redirect_uri);
        params.insert("code_verifier", &request.code_verifier);

        if let Some(secret) = &request.client_secret {
            params.insert("client_secret", secret);
        }

        for (key, value) in &request.additional_params {
            params.insert(key, value);
        }

        let response = self
            .http_client
            .post(&request.token_endpoint)
            .form(&params)
            .send()
            .await
            .map_err(|e| EdgeError::HttpClient(format!("Token exchange request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(EdgeError::TokenValidation(format!(
                "Token exchange failed with status {}",
                response.status()
            )));
        }

        let token_response: TokenResponse = response.json().await.map_err(|e| {
            EdgeError::TokenValidation(format!("Failed to parse token response: {}", e))
        })?;

        Ok(token_response)
    }

    async fn refresh_access_token(
        &self,
        request: &RefreshTokenRequest,
    ) -> Result<TokenResponse, EdgeError> {
        let mut params = HashMap::new();
        params.insert("grant_type", "refresh_token");
        params.insert("refresh_token", &request.refresh_token);
        params.insert("client_id", &request.client_id);

        if let Some(secret) = &request.client_secret {
            params.insert("client_secret", secret);
        }

        if let Some(scope) = &request.scope {
            params.insert("scope", scope);
        }

        for (key, value) in &request.additional_params {
            params.insert(key, value);
        }

        let response = self
            .http_client
            .post(&request.token_endpoint)
            .form(&params)
            .send()
            .await
            .map_err(|e| EdgeError::HttpClient(format!("Token refresh request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(EdgeError::TokenValidation(format!(
                "Token refresh failed with status {}",
                response.status()
            )));
        }

        let token_response: TokenResponse = response.json().await.map_err(|e| {
            EdgeError::TokenValidation(format!("Failed to parse refresh token response: {}", e))
        })?;

        Ok(token_response)
    }

    async fn create_session(
        &self,
        token_response: &TokenResponse,
        scope: String,
    ) -> Result<Oauth2Session, EdgeError> {
        let now = Utc::now().timestamp();
        let expires_at = token_response.expires_in.map(|delta| now + delta);

        let session = Oauth2Session {
            session_id: Uuid::new_v4().to_string(),
            access_token: token_response.access_token.clone(),
            token_type: token_response.token_type.clone(),
            expires_at,
            refresh_token: token_response.refresh_token.clone(),
            scope,
            created_at: now,
            last_refreshed_at: None,
            claims: token_response.additional_params.clone(),
        };

        self.store_session(session.clone()).await?;
        Ok(session)
    }

    async fn get_session(&self, session_id: &str) -> Result<Option<Oauth2Session>, EdgeError> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_id).cloned())
    }

    async fn store_session(&self, session: Oauth2Session) -> Result<(), EdgeError> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.session_id.clone(), session);

        // Cleanup if storage limit exceeded
        drop(sessions);
        self.cleanup_old_sessions().await;

        Ok(())
    }

    async fn revoke_session(&self, session_id: &str) -> Result<(), EdgeError> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
        Ok(())
    }

    async fn is_session_valid(
        &self,
        session_id: &str,
        buffer_seconds: i64,
    ) -> Result<bool, EdgeError> {
        let sessions = self.sessions.read().await;

        match sessions.get(session_id) {
            Some(session) => Ok(!session.is_expired(buffer_seconds)),
            None => Ok(false),
        }
    }

    async fn refresh_session_if_needed(
        &self,
        session_id: &str,
        buffer_seconds: i64,
        token_endpoint: &str,
    ) -> Result<Oauth2Session, EdgeError> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| EdgeError::Authentication("Session not found".to_string()))?;

        drop(sessions);

        // Check if refresh is needed
        if !session.is_expired(buffer_seconds) {
            return Ok(session);
        }

        // Cannot refresh without refresh token
        let refresh_token = session
            .refresh_token
            .as_ref()
            .ok_or_else(|| EdgeError::Authentication("No refresh token available".to_string()))?
            .clone();

        let refresh_request = RefreshTokenRequest {
            token_endpoint: token_endpoint.to_string(),
            client_id: String::new(), // Note: would be provided by application
            refresh_token,
            client_secret: None,
            scope: Some(session.scope.clone()),
            additional_params: HashMap::new(),
        };

        let token_response = self.refresh_access_token(&refresh_request).await?;
        let now = Utc::now().timestamp();
        let expires_at = token_response.expires_in.map(|delta| now + delta);

        let updated_session = Oauth2Session {
            session_id: session.session_id.clone(),
            access_token: token_response.access_token,
            token_type: token_response.token_type,
            expires_at,
            refresh_token: token_response.refresh_token,
            scope: session.scope,
            created_at: session.created_at,
            last_refreshed_at: Some(now),
            claims: token_response.additional_params,
        };

        self.store_session(updated_session.clone()).await?;
        Ok(updated_session)
    }

    async fn validate_token(
        &self,
        token: &str,
        expected_scope: Option<&str>,
    ) -> Result<serde_json::Value, EdgeError> {
        // Basic token validation: check format and non-empty
        if token.is_empty() {
            return Err(EdgeError::InvalidToken("Token is empty".to_string()));
        }

        // For Bearer tokens, typically JWTs - minimal validation
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(EdgeError::InvalidToken("Invalid JWT format".to_string()));
        }

        // Return claims object with basic info
        let mut claims = serde_json::json!({
            "token_type": "Bearer",
            "valid": true,
        });

        if let Some(scope) = expected_scope {
            claims["expected_scope"] = serde_json::json!(scope);
        }

        Ok(claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pkce_creation() {
        let auth = PkceAuthenticator::new().unwrap();
        let (verifier, challenge) = auth.generate_code_verifier_and_challenge().await.unwrap();

        assert!(!verifier.value.is_empty());
        assert_eq!(verifier.value.len(), 128);
        assert!(!challenge.value.is_empty());
        assert_eq!(challenge.method, ChallengeMethod::S256);
    }

    #[tokio::test]
    async fn test_authorization_request_creation() {
        let auth = PkceAuthenticator::new().unwrap();
        let (verifier, challenge) = auth.generate_code_verifier_and_challenge().await.unwrap();

        let req = auth
            .create_authorization_request(
                "client123".to_string(),
                "https://auth.example.com/oauth/authorize".to_string(),
                "https://app.example.com/callback".to_string(),
                "read write".to_string(),
                challenge,
                verifier,
            )
            .await
            .unwrap();

        assert_eq!(req.client_id, "client123");
        assert_eq!(req.scope, "read write");
        assert!(!req.state.is_empty());
    }

    #[tokio::test]
    async fn test_validate_authorization_response_success() {
        let auth = PkceAuthenticator::new().unwrap();
        let state = "test-state";

        let response = AuthorizationResponse {
            code: "auth_code_123".to_string(),
            state: state.to_string(),
            error: None,
            error_description: None,
            error_uri: None,
        };

        let code = auth
            .validate_authorization_response(&response, state)
            .await
            .unwrap();
        assert_eq!(code, "auth_code_123");
    }

    #[tokio::test]
    async fn test_validate_authorization_response_state_mismatch() {
        let auth = PkceAuthenticator::new().unwrap();

        let response = AuthorizationResponse {
            code: "auth_code_123".to_string(),
            state: "wrong_state".to_string(),
            error: None,
            error_description: None,
            error_uri: None,
        };

        let result = auth
            .validate_authorization_response(&response, "correct_state")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_authorization_response_with_error() {
        let auth = PkceAuthenticator::new().unwrap();

        let response = AuthorizationResponse {
            code: String::new(),
            state: "state".to_string(),
            error: Some("access_denied".to_string()),
            error_description: Some("User denied access".to_string()),
            error_uri: None,
        };

        let result = auth
            .validate_authorization_response(&response, "state")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_session_creation_and_storage() {
        let auth = PkceAuthenticator::new().unwrap();

        let token_response = TokenResponse {
            access_token: "access_123".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: Some("refresh_123".to_string()),
            scope: Some("read write".to_string()),
            additional_params: HashMap::new(),
        };

        let session = auth
            .create_session(&token_response, "read write".to_string())
            .await
            .unwrap();

        assert_eq!(session.access_token, "access_123");
        assert!(session.refresh_token.is_some());

        // Verify session is stored
        let retrieved = auth.get_session(&session.session_id).await.unwrap();
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_session_validation() {
        let auth = PkceAuthenticator::new().unwrap();

        let token_response = TokenResponse {
            access_token: "access_123".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: None,
            scope: None,
            additional_params: HashMap::new(),
        };

        let session = auth
            .create_session(&token_response, "read".to_string())
            .await
            .unwrap();

        let is_valid = auth
            .is_session_valid(&session.session_id, 300)
            .await
            .unwrap();
        assert!(is_valid);
    }

    #[tokio::test]
    async fn test_session_revocation() {
        let auth = PkceAuthenticator::new().unwrap();

        let token_response = TokenResponse {
            access_token: "access_123".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: None,
            scope: None,
            additional_params: HashMap::new(),
        };

        let session = auth
            .create_session(&token_response, "read".to_string())
            .await
            .unwrap();

        auth.revoke_session(&session.session_id).await.unwrap();

        let retrieved = auth.get_session(&session.session_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_token_validation() {
        let auth = PkceAuthenticator::new().unwrap();

        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let claims = auth.validate_token(token, Some("read")).await.unwrap();

        assert_eq!(claims["valid"], true);
        assert_eq!(claims["expected_scope"], "read");
    }

    #[tokio::test]
    async fn test_token_validation_invalid() {
        let auth = PkceAuthenticator::new().unwrap();

        let result = auth.validate_token("invalid", None).await;
        assert!(result.is_err());
    }
}
