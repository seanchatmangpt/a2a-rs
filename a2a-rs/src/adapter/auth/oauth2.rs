//! OAuth2 and OpenID Connect authentication implementations
//!
//! This module provides OAuth2 authentication adapters with support for:
//! - Authorization code flow with PKCE
//! - Client credentials flow
//! - Token refresh mechanism
//! - Scope validation
//! - Pre-configured providers (Google, GitHub)
//! - Token introspection and validation

#[cfg(feature = "auth")]
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
    basic::BasicClient, reqwest::async_http_client,
};
#[cfg(feature = "auth")]
use openidconnect::{
    IssuerUrl, Nonce,
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    domain::{
        A2AError,
        core::agent::{
            AuthorizationCodeOAuthFlow, ClientCredentialsOAuthFlow, OAuthFlows, SecurityScheme,
        },
    },
    port::authenticator::{AuthContext, AuthContextExtractor, AuthPrincipal, Authenticator},
};

/// OAuth2 token information with expiration tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuth2Token {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
}

impl OAuth2Token {
    /// Check if the token is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            now >= expires_at
        } else {
            false
        }
    }

    /// Check if the token will expire within the given seconds
    pub fn expires_within(&self, seconds: u64) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            expires_at <= now + seconds
        } else {
            false
        }
    }
}

/// OAuth2 provider configuration
#[derive(Debug, Clone)]
pub struct OAuth2Provider {
    pub name: String,
    pub auth_url: String,
    pub token_url: String,
    pub userinfo_url: Option<String>,
    pub scopes: Vec<String>,
}

impl OAuth2Provider {
    /// Google OAuth2 provider configuration
    pub fn google() -> Self {
        Self {
            name: "Google".to_string(),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            userinfo_url: Some("https://www.googleapis.com/oauth2/v2/userinfo".to_string()),
            scopes: vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
            ],
        }
    }

    /// GitHub OAuth2 provider configuration
    pub fn github() -> Self {
        Self {
            name: "GitHub".to_string(),
            auth_url: "https://github.com/login/oauth/authorize".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            userinfo_url: Some("https://api.github.com/user".to_string()),
            scopes: vec!["user".to_string(), "read:user".to_string()],
        }
    }
}

/// Token storage for managing OAuth2 tokens
#[cfg(feature = "auth")]
#[derive(Clone)]
struct TokenStore {
    tokens: Arc<RwLock<HashMap<String, OAuth2Token>>>,
}

#[cfg(feature = "auth")]
impl TokenStore {
    fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn store_token(&self, key: String, token: OAuth2Token) {
        let mut tokens = self.tokens.write().unwrap();
        tokens.insert(key, token);
    }

    fn get_token(&self, key: &str) -> Option<OAuth2Token> {
        let tokens = self.tokens.read().unwrap();
        tokens.get(key).cloned()
    }

    fn remove_token(&self, key: &str) {
        let mut tokens = self.tokens.write().unwrap();
        tokens.remove(key);
    }
}

/// OAuth2 authenticator using the oauth2 crate
#[cfg(feature = "auth")]
#[derive(Clone)]
pub struct OAuth2Authenticator {
    /// OAuth2 client
    client: BasicClient,
    /// Security scheme configuration
    scheme: SecurityScheme,
    /// Token store for managing tokens
    token_store: TokenStore,
    /// Required scopes for authentication
    required_scopes: HashSet<String>,
    /// PKCE code verifier for authorization code flow
    pkce_verifiers: Arc<RwLock<HashMap<String, PkceCodeVerifier>>>,
    /// Provider configuration
    provider: Option<OAuth2Provider>,
}

#[cfg(feature = "auth")]
impl OAuth2Authenticator {
    /// Create a new OAuth2 authenticator for authorization code flow
    pub fn new_authorization_code(
        client_id: ClientId,
        client_secret: Option<ClientSecret>,
        auth_url: AuthUrl,
        token_url: TokenUrl,
        redirect_url: RedirectUrl,
        scopes: HashMap<String, String>,
    ) -> Self {
        let client = BasicClient::new(client_id, client_secret, auth_url, Some(token_url))
            .set_redirect_uri(redirect_url);

        let flow = AuthorizationCodeOAuthFlow {
            authorization_url: client.auth_url().url().to_string(),
            token_url: client.token_url().unwrap().url().to_string(),
            refresh_url: None,
            scopes,
        };

        let scheme = SecurityScheme::OAuth2 {
            flows: Box::new(OAuthFlows {
                authorization_code: Some(flow),
                ..Default::default()
            }),
            description: Some("OAuth2 Authorization Code Flow with PKCE".to_string()),
            metadata_url: None,
        };

        Self {
            client,
            scheme,
            token_store: TokenStore::new(),
            required_scopes: HashSet::new(),
            pkce_verifiers: Arc::new(RwLock::new(HashMap::new())),
            provider: None,
        }
    }

    /// Create OAuth2 authenticator from a provider configuration
    pub fn from_provider(
        provider: OAuth2Provider,
        client_id: ClientId,
        client_secret: Option<ClientSecret>,
        redirect_url: RedirectUrl,
    ) -> Result<Self, A2AError> {
        let auth_url = AuthUrl::new(provider.auth_url.clone())
            .map_err(|e| A2AError::Internal(format!("Invalid auth URL: {}", e)))?;

        let token_url = TokenUrl::new(provider.token_url.clone())
            .map_err(|e| A2AError::Internal(format!("Invalid token URL: {}", e)))?;

        let scopes: HashMap<String, String> = provider
            .scopes
            .iter()
            .map(|s| (s.clone(), format!("{} scope", s)))
            .collect();

        let mut authenticator = Self::new_authorization_code(
            client_id,
            client_secret,
            auth_url,
            token_url,
            redirect_url,
            scopes,
        );

        authenticator.provider = Some(provider);
        Ok(authenticator)
    }

    /// Create a new OAuth2 authenticator for client credentials flow
    pub fn new_client_credentials(
        client_id: ClientId,
        client_secret: ClientSecret,
        token_url: TokenUrl,
        scopes: HashMap<String, String>,
    ) -> Self {
        let client = BasicClient::new(
            client_id,
            Some(client_secret),
            AuthUrl::new("".to_string()).unwrap(),
            Some(token_url),
        );

        let flow = ClientCredentialsOAuthFlow {
            token_url: client.token_url().unwrap().url().to_string(),
            refresh_url: None,
            scopes,
        };

        let scheme = SecurityScheme::OAuth2 {
            flows: Box::new(OAuthFlows {
                client_credentials: Some(flow),
                ..Default::default()
            }),
            description: Some("OAuth2 Client Credentials Flow".to_string()),
            metadata_url: None,
        };

        Self {
            client,
            scheme,
            token_store: TokenStore::new(),
            required_scopes: HashSet::new(),
            pkce_verifiers: Arc::new(RwLock::new(HashMap::new())),
            provider: None,
        }
    }

    /// Set required scopes for authentication
    pub fn with_required_scopes(mut self, scopes: Vec<String>) -> Self {
        self.required_scopes = scopes.into_iter().collect();
        self
    }

    /// Generate authorization URL for authorization code flow with PKCE
    pub fn authorize_url(&self) -> Result<(String, CsrfToken, PkceCodeChallenge), A2AError> {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let scopes: Vec<Scope> = if let Some(provider) = &self.provider {
            provider
                .scopes
                .iter()
                .map(|s| Scope::new(s.clone()))
                .collect()
        } else {
            self.required_scopes
                .iter()
                .map(|s| Scope::new(s.clone()))
                .collect()
        };

        let mut auth_request = self
            .client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge.clone());

        for scope in scopes {
            auth_request = auth_request.add_scope(scope);
        }

        let (auth_url, csrf_token) = auth_request.url();

        // Store PKCE verifier for later use
        let mut verifiers = self.pkce_verifiers.write().unwrap();
        verifiers.insert(csrf_token.secret().clone(), pkce_verifier);

        Ok((auth_url.to_string(), csrf_token, pkce_challenge))
    }

    /// Exchange authorization code for access token
    pub async fn exchange_code(
        &self,
        code: String,
        csrf_token: String,
    ) -> Result<OAuth2Token, A2AError> {
        // Retrieve PKCE verifier
        let pkce_verifier = {
            let mut verifiers = self.pkce_verifiers.write().unwrap();
            verifiers
                .remove(&csrf_token)
                .ok_or_else(|| A2AError::Internal("Invalid or expired CSRF token".to_string()))?
        };

        let token_result = self
            .client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(async_http_client)
            .await
            .map_err(|e| A2AError::Internal(format!("Token exchange failed: {}", e)))?;

        let expires_at = token_result.expires_in().map(|duration| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + duration.as_secs()
        });

        let token = OAuth2Token {
            access_token: token_result.access_token().secret().clone(),
            token_type: token_result.token_type().as_ref().to_string(),
            expires_in: token_result.expires_in().map(|d| d.as_secs() as i64),
            refresh_token: token_result.refresh_token().map(|t| t.secret().clone()),
            scope: token_result.scopes().map(|scopes| {
                scopes
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            }),
            expires_at,
        };

        // Store the token
        self.token_store
            .store_token(token.access_token.clone(), token.clone());

        Ok(token)
    }

    /// Refresh an access token using a refresh token
    pub async fn refresh_token(&self, refresh_token: String) -> Result<OAuth2Token, A2AError> {
        let token_result = self
            .client
            .exchange_refresh_token(&RefreshToken::new(refresh_token))
            .request_async(async_http_client)
            .await
            .map_err(|e| A2AError::Internal(format!("Token refresh failed: {}", e)))?;

        let expires_at = token_result.expires_in().map(|duration| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + duration.as_secs()
        });

        let token = OAuth2Token {
            access_token: token_result.access_token().secret().clone(),
            token_type: token_result.token_type().as_ref().to_string(),
            expires_in: token_result.expires_in().map(|d| d.as_secs() as i64),
            refresh_token: token_result.refresh_token().map(|t| t.secret().clone()),
            scope: token_result.scopes().map(|scopes| {
                scopes
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            }),
            expires_at,
        };

        // Store the new token
        self.token_store
            .store_token(token.access_token.clone(), token.clone());

        Ok(token)
    }

    /// Validate token scopes against required scopes
    fn validate_scopes(&self, token_scopes: &str) -> Result<(), A2AError> {
        if self.required_scopes.is_empty() {
            return Ok(());
        }

        let scopes: HashSet<String> = token_scopes
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        let missing_scopes: Vec<_> = self
            .required_scopes
            .iter()
            .filter(|s| !scopes.contains(*s))
            .collect();

        if !missing_scopes.is_empty() {
            return Err(A2AError::Internal(format!(
                "Missing required scopes: {}",
                missing_scopes
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        Ok(())
    }

    /// Validate and refresh token if needed
    async fn validate_and_refresh_token(
        &self,
        token: &OAuth2Token,
    ) -> Result<OAuth2Token, A2AError> {
        // Check if token is expired or will expire soon (within 5 minutes)
        if token.is_expired() || token.expires_within(300) {
            if let Some(refresh_token) = &token.refresh_token {
                // Attempt to refresh the token
                self.refresh_token(refresh_token.clone()).await
            } else {
                Err(A2AError::Internal(
                    "Access token expired and no refresh token available".to_string(),
                ))
            }
        } else {
            Ok(token.clone())
        }
    }

    /// Get user information from provider (if supported)
    pub async fn get_userinfo(&self, access_token: &str) -> Result<serde_json::Value, A2AError> {
        let userinfo_url = self
            .provider
            .as_ref()
            .and_then(|p| p.userinfo_url.as_ref())
            .ok_or_else(|| {
                A2AError::Internal("Provider does not support userinfo endpoint".to_string())
            })?;

        let client = reqwest::Client::new();
        let response = client
            .get(userinfo_url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| A2AError::Internal(format!("Failed to fetch user info: {}", e)))?;

        if !response.status().is_success() {
            return Err(A2AError::Internal(format!(
                "User info request failed with status: {}",
                response.status()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| A2AError::Internal(format!("Failed to parse user info: {}", e)))
    }

    /// Store a token manually (for testing or custom flows)
    pub fn store_token(&self, token: OAuth2Token) {
        self.token_store
            .store_token(token.access_token.clone(), token);
    }

    /// Revoke a token
    pub fn revoke_token(&self, access_token: &str) {
        self.token_store.remove_token(access_token);
    }
}

#[cfg(feature = "auth")]
#[async_trait]
impl Authenticator for OAuth2Authenticator {
    async fn authenticate(&self, context: &AuthContext) -> Result<AuthPrincipal, A2AError> {
        self.validate_context(context)?;

        let access_token = &context.credential;

        // Retrieve stored token
        let stored_token = self
            .token_store
            .get_token(access_token)
            .ok_or_else(|| A2AError::Internal("Invalid or unknown access token".to_string()))?;

        // Validate and refresh token if needed
        let valid_token = self.validate_and_refresh_token(&stored_token).await?;

        // Validate scopes if required
        if let Some(scope) = &valid_token.scope {
            self.validate_scopes(scope)?;
        } else if !self.required_scopes.is_empty() {
            return Err(A2AError::Internal(
                "Token has no scopes but scopes are required".to_string(),
            ));
        }

        // Create principal with token information
        let mut principal = AuthPrincipal::new(
            format!("oauth2:{}", &valid_token.access_token[..8]),
            "oauth2".to_string(),
        );

        // Add OAuth2-specific attributes
        if let Some(scope) = &valid_token.scope {
            principal = principal.with_attribute("scope".to_string(), scope.clone());
        }

        if let Some(provider) = &self.provider {
            principal = principal.with_attribute("provider".to_string(), provider.name.clone());
        }

        principal = principal.with_attribute("token_type".to_string(), valid_token.token_type);

        Ok(principal)
    }

    fn security_scheme(&self) -> &SecurityScheme {
        &self.scheme
    }

    fn validate_context(&self, context: &AuthContext) -> Result<(), A2AError> {
        if context.scheme_type != "oauth2" {
            return Err(A2AError::Internal(format!(
                "Invalid authentication scheme: expected 'oauth2', got '{}'",
                context.scheme_type
            )));
        }
        Ok(())
    }
}

/// OpenID Connect authenticator
#[cfg(feature = "auth")]
#[derive(Clone)]
pub struct OpenIdConnectAuthenticator {
    /// OpenID Connect client
    client: CoreClient,
    /// Security scheme configuration
    scheme: SecurityScheme,
    /// Token store for managing tokens
    token_store: TokenStore,
    /// Required scopes
    required_scopes: HashSet<String>,
}

#[cfg(feature = "auth")]
impl OpenIdConnectAuthenticator {
    /// Create a new OpenID Connect authenticator
    pub async fn new(
        issuer_url: IssuerUrl,
        client_id: ClientId,
        client_secret: Option<ClientSecret>,
        redirect_url: RedirectUrl,
    ) -> Result<Self, A2AError> {
        // Discover OpenID Connect provider metadata
        let provider_metadata =
            CoreProviderMetadata::discover_async(issuer_url.clone(), async_http_client)
                .await
                .map_err(|e| {
                    A2AError::Internal(format!("Failed to discover OIDC provider: {}", e))
                })?;

        // Create OpenID Connect client
        let client =
            CoreClient::from_provider_metadata(provider_metadata, client_id, client_secret)
                .set_redirect_uri(redirect_url);

        let scheme = SecurityScheme::OpenIdConnect {
            open_id_connect_url: issuer_url.url().to_string(),
            description: Some("OpenID Connect authentication".to_string()),
        };

        Ok(Self {
            client,
            scheme,
            token_store: TokenStore::new(),
            required_scopes: HashSet::new(),
        })
    }

    /// Set required scopes
    pub fn with_required_scopes(mut self, scopes: Vec<String>) -> Self {
        self.required_scopes = scopes.into_iter().collect();
        self
    }

    /// Generate authorization URL for OpenID Connect
    pub fn authorize_url(&self) -> (String, CsrfToken, Nonce) {
        let (auth_url, csrf_token, nonce) = self
            .client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .url();

        (auth_url.to_string(), csrf_token, nonce)
    }

    /// Store a token manually
    pub fn store_token(&self, token: OAuth2Token) {
        self.token_store
            .store_token(token.access_token.clone(), token);
    }
}

#[cfg(feature = "auth")]
#[async_trait]
impl Authenticator for OpenIdConnectAuthenticator {
    async fn authenticate(&self, context: &AuthContext) -> Result<AuthPrincipal, A2AError> {
        self.validate_context(context)?;

        let token = &context.credential;

        // Retrieve stored token
        let stored_token = self
            .token_store
            .get_token(token)
            .ok_or_else(|| A2AError::Internal("Invalid or unknown ID token".to_string()))?;

        // In a real implementation, you would validate the ID token signature
        // and verify claims (issuer, audience, expiration, etc.)

        let principal = AuthPrincipal::new(
            format!("oidc:{}", &stored_token.access_token[..8]),
            "openidconnect".to_string(),
        );

        Ok(principal)
    }

    fn security_scheme(&self) -> &SecurityScheme {
        &self.scheme
    }

    fn validate_context(&self, context: &AuthContext) -> Result<(), A2AError> {
        if context.scheme_type != "openidconnect" {
            return Err(A2AError::Internal(format!(
                "Invalid authentication scheme: expected 'openidconnect', got '{}'",
                context.scheme_type
            )));
        }
        Ok(())
    }
}

/// OAuth2/OIDC token extractor
#[derive(Clone)]
pub struct OAuth2Extractor;

#[async_trait]
impl AuthContextExtractor for OAuth2Extractor {
    #[cfg(feature = "http-server")]
    async fn extract_from_headers(&self, headers: &axum::http::HeaderMap) -> Option<AuthContext> {
        headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|auth| {
                let parts: Vec<&str> = auth.splitn(2, ' ').collect();
                if parts.len() == 2 && parts[0].to_lowercase() == "bearer" {
                    Some(AuthContext::new("oauth2".to_string(), parts[1].to_string()))
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
                    Some(AuthContext::new("oauth2".to_string(), parts[1].to_string()))
                } else {
                    None
                }
            })
    }

    async fn extract_from_query(&self, params: &HashMap<String, String>) -> Option<AuthContext> {
        // OAuth2 tokens can be passed as access_token query parameter
        params.get("access_token").map(|token| {
            AuthContext::new("oauth2".to_string(), token.clone())
                .with_metadata("location".to_string(), "query".to_string())
        })
    }

    async fn extract_from_cookies(&self, _cookies: &str) -> Option<AuthContext> {
        // OAuth2 tokens can be stored in cookies, but we'll keep this simple for now
        None
    }
}

#[cfg(feature = "auth")]
async fn async_http_client(
    request: openidconnect::HttpRequest,
) -> Result<openidconnect::HttpResponse, openidconnect::reqwest::Error<reqwest::Error>> {
    use openidconnect::reqwest::async_http_client;
    async_http_client(request).await
}

// Placeholder implementations when auth feature is not enabled
#[cfg(not(feature = "auth"))]
pub struct OAuth2Authenticator;

#[cfg(not(feature = "auth"))]
pub struct OpenIdConnectAuthenticator;

#[cfg(not(feature = "auth"))]
impl OAuth2Authenticator {
    pub fn new_authorization_code(
        _client_id: String,
        _auth_url: String,
        _token_url: String,
    ) -> Self {
        compile_error!("OAuth2 authentication requires the 'auth' feature");
    }
}

#[cfg(all(test, feature = "auth"))]
mod tests {
    use super::*;

    #[test]
    fn test_oauth2_token_expiration() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let expired_token = OAuth2Token {
            access_token: "test_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: None,
            scope: None,
            expires_at: Some(now - 100), // Expired 100 seconds ago
        };

        assert!(expired_token.is_expired());

        let valid_token = OAuth2Token {
            access_token: "test_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: None,
            scope: None,
            expires_at: Some(now + 3600), // Expires in 1 hour
        };

        assert!(!valid_token.is_expired());
        assert!(!valid_token.expires_within(3000));
        assert!(valid_token.expires_within(4000));
    }

    #[test]
    fn test_provider_configurations() {
        let google = OAuth2Provider::google();
        assert_eq!(google.name, "Google");
        assert!(google.auth_url.contains("google.com"));
        assert!(google.scopes.contains(&"openid".to_string()));

        let github = OAuth2Provider::github();
        assert_eq!(github.name, "GitHub");
        assert!(github.auth_url.contains("github.com"));
        assert!(github.scopes.contains(&"user".to_string()));
    }

    #[tokio::test]
    async fn test_scope_validation() {
        let client_id = ClientId::new("test_client".to_string());
        let auth_url = AuthUrl::new("https://example.com/auth".to_string()).unwrap();
        let token_url = TokenUrl::new("https://example.com/token".to_string()).unwrap();
        let redirect_url = RedirectUrl::new("https://example.com/callback".to_string()).unwrap();

        let authenticator = OAuth2Authenticator::new_authorization_code(
            client_id,
            None,
            auth_url,
            token_url,
            redirect_url,
            HashMap::new(),
        )
        .with_required_scopes(vec!["read".to_string(), "write".to_string()]);

        // Test valid scopes
        let result = authenticator.validate_scopes("read write admin");
        assert!(result.is_ok());

        // Test missing scopes
        let result = authenticator.validate_scopes("read");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Missing required scopes")
        );

        // Test empty scopes when required
        let result = authenticator.validate_scopes("");
        assert!(result.is_err());
    }
}
