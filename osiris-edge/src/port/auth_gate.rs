//! Authentication gate port - defines the authentication interface
//!
//! This port provides async trait definitions for authentication and authorization
//! following hexagonal architecture principles. Adapters implement these traits
//! using external dependencies (jsonwebtoken, reqwest, etc.).

use crate::domain::{AuthPrincipal, AuthRequest, EdgeError, TokenValidationConfig};
use async_trait::async_trait;

/// Port interface for authentication gate
///
/// Authenticates incoming requests and validates tokens using various methods:
/// - JWT validation (local signature verification)
/// - OAuth2 token introspection (Google Workspace, etc.)
/// - Service account token validation
#[async_trait]
pub trait AuthGate: Send + Sync {
    /// Authenticate a request and return the authenticated principal
    ///
    /// This method validates the token in the request and returns information
    /// about the authenticated entity.
    ///
    /// # Arguments
    ///
    /// * `request` - The authentication request containing the token
    ///
    /// # Returns
    ///
    /// * `Ok(AuthPrincipal)` - Successfully authenticated principal
    /// * `Err(EdgeError)` - Authentication failed
    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthPrincipal, EdgeError>;

    /// Validate a token without full authentication
    ///
    /// This is useful for checking if a token is valid without extracting
    /// all the principal information.
    ///
    /// # Arguments
    ///
    /// * `token` - The token string to validate
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Token is valid
    /// * `Ok(false)` - Token is invalid
    /// * `Err(EdgeError)` - Validation error occurred
    async fn validate_token(&self, token: &str) -> Result<bool, EdgeError>;

    /// Check if the authenticated principal has access to a resource
    ///
    /// # Arguments
    ///
    /// * `principal` - The authenticated principal
    /// * `resource` - The resource identifier
    /// * `action` - The action being performed (read, write, delete, etc.)
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Access granted
    /// * `Ok(false)` - Access denied
    /// * `Err(EdgeError)` - Authorization check failed
    async fn authorize(
        &self,
        principal: &AuthPrincipal,
        resource: &str,
        action: &str,
    ) -> Result<bool, EdgeError>;

    /// Get the validation configuration
    fn validation_config(&self) -> &TokenValidationConfig;
}

/// Port interface for extracting authentication tokens from HTTP requests
#[async_trait]
pub trait TokenExtractor: Send + Sync {
    /// Extract token from Authorization header
    ///
    /// Supports Bearer token format: "Authorization: Bearer <token>"
    ///
    /// # Arguments
    ///
    /// * `authorization_header` - The Authorization header value
    ///
    /// # Returns
    ///
    /// * `Some(token)` - Token extracted successfully
    /// * `None` - No valid token found
    fn extract_from_authorization(&self, authorization_header: &str) -> Option<String>;

    /// Extract token from query parameters
    ///
    /// # Arguments
    ///
    /// * `query_params` - Map of query parameters
    ///
    /// # Returns
    ///
    /// * `Some(token)` - Token extracted successfully
    /// * `None` - No valid token found
    fn extract_from_query(
        &self,
        query_params: &std::collections::HashMap<String, String>,
    ) -> Option<String>;

    /// Extract token from cookies
    ///
    /// # Arguments
    ///
    /// * `cookie_header` - The Cookie header value
    ///
    /// # Returns
    ///
    /// * `Some(token)` - Token extracted successfully
    /// * `None` - No valid token found
    fn extract_from_cookie(&self, cookie_header: &str) -> Option<String>;
}

/// Port interface for Google Workspace OAuth2 token validation
///
/// This trait specifically handles Google Workspace OAuth2 tokens,
/// which require calling Google's token info endpoint for validation.
#[async_trait]
pub trait GoogleWorkspaceValidator: Send + Sync {
    /// Validate a Google Workspace OAuth2 token
    ///
    /// Makes a request to Google's tokeninfo endpoint to validate the token
    /// and extract user information.
    ///
    /// # Arguments
    ///
    /// * `access_token` - The OAuth2 access token
    ///
    /// # Returns
    ///
    /// * `Ok(AuthPrincipal)` - Token is valid, returns principal info
    /// * `Err(EdgeError)` - Token validation failed
    async fn validate_google_token(&self, access_token: &str) -> Result<AuthPrincipal, EdgeError>;

    /// Validate that the token has required scopes
    ///
    /// # Arguments
    ///
    /// * `access_token` - The OAuth2 access token
    /// * `required_scopes` - List of required OAuth2 scopes
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Token has all required scopes
    /// * `Ok(false)` - Token is missing required scopes
    /// * `Err(EdgeError)` - Validation error
    async fn validate_scopes(
        &self,
        access_token: &str,
        required_scopes: &[String],
    ) -> Result<bool, EdgeError>;
}

/// Port interface for service account token validation
///
/// Used for validating tokens from internal services using service accounts.
#[async_trait]
pub trait ServiceAccountValidator: Send + Sync {
    /// Validate a service account token
    ///
    /// # Arguments
    ///
    /// * `token` - The service account JWT token
    ///
    /// # Returns
    ///
    /// * `Ok(AuthPrincipal)` - Token is valid, returns service account info
    /// * `Err(EdgeError)` - Token validation failed
    async fn validate_service_account(&self, token: &str) -> Result<AuthPrincipal, EdgeError>;

    /// Check if a service account has permission for an action
    ///
    /// # Arguments
    ///
    /// * `service_account_id` - The service account identifier
    /// * `action` - The action being performed
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Service account has permission
    /// * `Ok(false)` - Service account lacks permission
    /// * `Err(EdgeError)` - Permission check failed
    async fn check_service_account_permission(
        &self,
        service_account_id: &str,
        action: &str,
    ) -> Result<bool, EdgeError>;
}
