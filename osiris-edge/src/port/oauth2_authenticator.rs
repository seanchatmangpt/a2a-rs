//! OAuth2 PKCE authenticator port
//!
//! Defines the async trait interface for OAuth2 Proof Key for Public Clients Exchange (PKCE)
//! authentication. Implements RFC 7636 for secure public client authentication flow.

use async_trait::async_trait;

use crate::domain::{
    AuthorizationRequest, AuthorizationResponse, CodeChallenge, CodeVerifier, EdgeError,
    Oauth2Session, RefreshTokenRequest, TokenRequest, TokenResponse,
};

/// Port interface for OAuth2 PKCE authentication
///
/// This trait handles the complete OAuth2 PKCE flow for public clients:
/// 1. Generate code verifier and challenge
/// 2. Initiate authorization request
/// 3. Exchange authorization code for tokens
/// 4. Refresh access tokens
/// 5. Validate and manage sessions
#[async_trait]
pub trait Oauth2Authenticator: Send + Sync {
    /// Generate a new PKCE code verifier and challenge
    ///
    /// # Returns
    ///
    /// * `Ok((verifier, challenge))` - Generated verifier and challenge
    /// * `Err(EdgeError)` - Generation failed
    async fn generate_code_verifier_and_challenge(
        &self,
    ) -> Result<(CodeVerifier, CodeChallenge), EdgeError>;

    /// Create an authorization request URL for redirecting the user
    ///
    /// # Arguments
    ///
    /// * `client_id` - OAuth2 client identifier
    /// * `authorization_endpoint` - Authorization server endpoint
    /// * `redirect_uri` - URI where user will be sent after authorization
    /// * `scope` - Space-separated scopes to request
    /// * `code_challenge` - PKCE code challenge (from generate_code_verifier_and_challenge)
    ///
    /// # Returns
    ///
    /// * `Ok(request)` - Authorization request with URL ready for user redirect
    /// * `Err(EdgeError)` - Request creation failed
    async fn create_authorization_request(
        &self,
        client_id: String,
        authorization_endpoint: String,
        redirect_uri: String,
        scope: String,
        code_challenge: CodeChallenge,
        code_verifier: CodeVerifier,
    ) -> Result<AuthorizationRequest, EdgeError>;

    /// Validate authorization response and extract authorization code
    ///
    /// # Arguments
    ///
    /// * `response` - Authorization response from OAuth2 server
    /// * `expected_state` - State parameter from the original request (for CSRF protection)
    ///
    /// # Returns
    ///
    /// * `Ok(code)` - Authorization code for token exchange
    /// * `Err(EdgeError)` - Validation failed or authorization denied
    async fn validate_authorization_response(
        &self,
        response: &AuthorizationResponse,
        expected_state: &str,
    ) -> Result<String, EdgeError>;

    /// Exchange authorization code for tokens (token endpoint request)
    ///
    /// # Arguments
    ///
    /// * `request` - Token request with code, verifier, and endpoint info
    ///
    /// # Returns
    ///
    /// * `Ok(response)` - Token response with access token
    /// * `Err(EdgeError)` - Token exchange failed
    async fn exchange_code_for_token(
        &self,
        request: &TokenRequest,
    ) -> Result<TokenResponse, EdgeError>;

    /// Refresh an access token using a refresh token
    ///
    /// # Arguments
    ///
    /// * `request` - Refresh token request with endpoint and credentials
    ///
    /// # Returns
    ///
    /// * `Ok(response)` - New token response with updated access token
    /// * `Err(EdgeError)` - Refresh failed
    async fn refresh_access_token(
        &self,
        request: &RefreshTokenRequest,
    ) -> Result<TokenResponse, EdgeError>;

    /// Create and store an OAuth2 session from a token response
    ///
    /// # Arguments
    ///
    /// * `token_response` - The token response from the OAuth2 server
    /// * `scope` - The scopes that were granted
    ///
    /// # Returns
    ///
    /// * `Ok(session)` - Created OAuth2 session
    /// * `Err(EdgeError)` - Session creation failed
    async fn create_session(
        &self,
        token_response: &TokenResponse,
        scope: String,
    ) -> Result<Oauth2Session, EdgeError>;

    /// Retrieve a stored OAuth2 session
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    ///
    /// # Returns
    ///
    /// * `Ok(Some(session))` - Session found and retrieved
    /// * `Ok(None)` - Session not found
    /// * `Err(EdgeError)` - Retrieval failed
    async fn get_session(&self, session_id: &str) -> Result<Option<Oauth2Session>, EdgeError>;

    /// Store an OAuth2 session
    ///
    /// # Arguments
    ///
    /// * `session` - The OAuth2 session to store
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Session stored successfully
    /// * `Err(EdgeError)` - Storage failed
    async fn store_session(&self, session: Oauth2Session) -> Result<(), EdgeError>;

    /// Invalidate/revoke a session
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier to revoke
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Session revoked
    /// * `Err(EdgeError)` - Revocation failed
    async fn revoke_session(&self, session_id: &str) -> Result<(), EdgeError>;

    /// Check if a session is valid and not expired
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `buffer_seconds` - Number of seconds before actual expiration to consider invalid
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Session is valid and not expired
    /// * `Ok(false)` - Session is invalid or expired
    /// * `Err(EdgeError)` - Validation check failed
    async fn is_session_valid(
        &self,
        session_id: &str,
        buffer_seconds: i64,
    ) -> Result<bool, EdgeError>;

    /// Refresh a session if it has expired or is about to expire
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `buffer_seconds` - Number of seconds before expiration to trigger refresh
    /// * `token_endpoint` - The token endpoint for refresh requests
    ///
    /// # Returns
    ///
    /// * `Ok(session)` - Refreshed session (or original if still valid)
    /// * `Err(EdgeError)` - Refresh failed
    async fn refresh_session_if_needed(
        &self,
        session_id: &str,
        buffer_seconds: i64,
        token_endpoint: &str,
    ) -> Result<Oauth2Session, EdgeError>;

    /// Validate an access token's integrity and claims
    ///
    /// # Arguments
    ///
    /// * `token` - The access token to validate
    /// * `expected_scope` - Optional scope that must be present
    ///
    /// # Returns
    ///
    /// * `Ok(claims)` - Token is valid, returns claims as JSON object
    /// * `Err(EdgeError)` - Token validation failed
    async fn validate_token(
        &self,
        token: &str,
        expected_scope: Option<&str>,
    ) -> Result<serde_json::Value, EdgeError>;
}
