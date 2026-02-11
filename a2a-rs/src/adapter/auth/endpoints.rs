//! HTTP endpoints for authentication (token generation, refresh, user info)

#[cfg(feature = "http-server")]
use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};

#[cfg(feature = "http-server")]
use crate::{
    adapter::auth::token_service::{
        TokenRefreshRequest, TokenRequest, UserInfo,
        AuthorizationUrlGenerator
    },
    domain::A2AError,
    port::authenticator::AuthPrincipal,
};

#[cfg(feature = "http-server")]
use serde_json::{json, Value};

/// Authentication endpoints state
#[cfg(feature = "http-server")]
pub struct AuthEndpointsState<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// Token service for generating and validating tokens
    pub token_service: Option<Arc<crate::adapter::auth::token_service::TokenService>>,
    /// User store for validating credentials (in production, use a proper user database)
    pub user_store: Option<Arc<dyn UserStore>>,
    /// Additional state
    pub additional_state: S,
}

#[cfg(feature = "http-server")]
impl<S> Clone for AuthEndpointsState<S>
where
    S: Clone + Send + Sync,
{
    fn clone(&self) -> Self {
        Self {
            token_service: self.token_service.clone(),
            user_store: self.user_store.clone(),
            additional_state: self.additional_state.clone(),
        }
    }
}

/// User store trait for credential validation
#[async_trait::async_trait]
pub trait UserStore: Send + Sync {
    /// Validate user credentials and return principal if valid
    async fn validate_credentials(
        &self,
        username: &str,
        password: &str,
    ) -> Result<AuthPrincipal, A2AError>;

    /// Get user info by principal ID
    async fn get_user_info(&self, user_id: &str) -> Result<UserInfo, A2AError>;
}

/// In-memory user store for testing (in production, use a real database)
#[derive(Clone)]
pub struct InMemoryUserStore {
    users: std::collections::HashMap<String, (String, AuthPrincipal)>,
}

impl InMemoryUserStore {
    pub fn new() -> Self {
        Self {
            users: std::collections::HashMap::new(),
        }
    }

    pub fn add_user(
        mut self,
        username: String,
        password: String,
        principal: AuthPrincipal,
    ) -> Self {
        // In production, store password hash only!
        self.users.insert(username, (password, principal));
        self
    }
}

impl Default for InMemoryUserStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl UserStore for InMemoryUserStore {
    async fn validate_credentials(
        &self,
        username: &str,
        password: &str,
    ) -> Result<AuthPrincipal, A2AError> {
        self.users
            .get(username)
            .filter(|(p, _)| p == password)
            .map(|(_, principal)| principal.clone())
            .ok_or_else(|| {
                A2AError::Internal("Invalid username or password".to_string())
            })
    }

    async fn get_user_info(&self, user_id: &str) -> Result<UserInfo, A2AError> {
        self.users
            .values()
            .find(|(_, p)| p.id == user_id)
            .map(|(_, principal)| UserInfo {
                sub: principal.id.clone(),
                name: principal.attributes.get("name").cloned(),
                email: principal.attributes.get("email").cloned(),
                email_verified: principal
                    .attributes
                    .get("email_verified")
                    .and_then(|v| v.parse().ok()),
                picture: principal.attributes.get("picture").cloned(),
                given_name: principal.attributes.get("given_name").cloned(),
                family_name: principal.attributes.get("family_name").cloned(),
            })
            .ok_or_else(|| A2AError::Internal("User not found".to_string()))
    }
}

/// Token endpoint - OAuth2 token endpoint
#[cfg(feature = "http-server")]
pub async fn token_endpoint(
    State(state): State<AuthEndpointsState<()>>,
    Json(request): Json<TokenRequest>,
) -> impl IntoResponse
{
    // Validate grant type
    match request.grant_type.as_str() {
        "password" => {
            // Resource Owner Password Credentials flow
            let token_service = state.token_service.as_ref().unwrap();
            let user_store = state.user_store.as_ref().unwrap();

            let username = request.username.unwrap_or_default();
            let password = request.password.unwrap_or_default();

            match user_store
                .validate_credentials(&username, &password)
                .await
            {
                Ok(principal) => match token_service.generate_token_with_refresh(&principal) {
                    Ok(response) => (StatusCode::OK, Json(response)).into_response(),
                    Err(e) => {
                        tracing::error!("Token generation failed: {}", e);
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({
                                "error": "server_error",
                                "error_description": "Failed to generate token"
                            })),
                        )
                            .into_response()
                    }
                },
                Err(_) => (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({
                        "error": "invalid_grant",
                        "error_description": "Invalid username or password"
                    })),
                )
                    .into_response(),
            }
        }
        "client_credentials" => {
            // Client Credentials flow
            let token_service = state.token_service.as_ref().unwrap();

            // Validate client_id and client_secret
            let client_id = request.client_id.unwrap_or_default();
            let _client_secret = request.client_secret.unwrap_or_default();

            // In production, validate against registered clients
            let principal = AuthPrincipal::new(
                client_id.clone(),
                "client_credentials".to_string(),
            );

            match token_service.generate_token(&principal) {
                Ok(response) => (StatusCode::OK, Json(response)).into_response(),
                Err(e) => {
                    tracing::error!("Token generation failed: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "server_error",
                            "error_description": "Failed to generate token"
                        })),
                    )
                        .into_response()
                }
            }
        }
        "authorization_code" => {
            // Authorization Code flow
            // This would require exchanging an authorization code for a token
            // For simplicity, return an error
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "unsupported_grant_type",
                    "error_description": "Authorization code flow not yet implemented"
                })),
            )
                .into_response()
        }
        _ => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "unsupported_grant_type",
                "error_description": format!("Grant type '{}' is not supported", request.grant_type)
            })),
        )
            .into_response(),
    }
}

/// Token refresh endpoint
#[cfg(feature = "http-server")]
pub async fn refresh_token_endpoint(
    State(state): State<AuthEndpointsState<()>>,
    Json(request): Json<TokenRefreshRequest>,
) -> impl IntoResponse
{
    let token_service = state.token_service.as_ref().unwrap();

    match token_service.refresh_token(&request.refresh_token) {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => {
            tracing::error!("Token refresh failed: {}", e);
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "invalid_grant",
                    "error_description": "Invalid or expired refresh token"
                })),
            )
                .into_response()
        }
    }
}

/// User info endpoint - OpenID Connect
#[cfg(feature = "http-server")]
pub async fn user_info_endpoint(
    State(state): State<AuthEndpointsState<()>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse
{
    // Extract Bearer token from Authorization header
    let token = match headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        Some(auth) => {
            let parts: Vec<&str> = auth.splitn(2, ' ').collect();
            if parts.len() == 2 && parts[0].to_lowercase() == "bearer" {
                parts[1]
            } else {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({
                        "error": "invalid_token",
                        "error_description": "Missing or invalid Authorization header"
                    })),
                )
                    .into_response();
            }
        }
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "invalid_token",
                    "error_description": "Missing Authorization header"
                })),
            )
                .into_response()
        }
    };

    let token_service = state.token_service.as_ref().unwrap();

    match token_service.get_user_info(token) {
        Ok(user_info) => (StatusCode::OK, Json(user_info)).into_response(),
        Err(e) => {
            tracing::error!("Failed to get user info: {}", e);
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "invalid_token",
                    "error_description": "Invalid or expired access token"
                })),
            )
                .into_response()
        }
    }
}

/// Authorization URL endpoint - OAuth2 authorization URL generation
#[cfg(feature = "http-server")]
pub async fn authorization_url_endpoint(
    State(_state): State<AuthEndpointsState<()>>,
    Json(request): Json<Value>,
) -> impl IntoResponse
{
    let authorization_url = request
        .get("authorizationUrl")
        .and_then(|v| v.as_str())
        .unwrap_or("https://example.com/oauth/authorize");

    let client_id = request
        .get("clientId")
        .and_then(|v| v.as_str())
        .unwrap_or("default_client");

    let redirect_uri = request
        .get("redirectUri")
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost:3000/callback");

    let scopes: Vec<String> = request
        .get("scopes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_else(|| vec!["openid".to_string(), "profile".to_string(), "email".to_string()]);

    let generator = AuthorizationUrlGenerator::new(
        authorization_url.to_string(),
        client_id.to_string(),
        redirect_uri.to_string(),
        scopes,
    );

    match generator.generate() {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => {
            tracing::error!("Failed to generate authorization URL: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "server_error",
                    "error_description": "Failed to generate authorization URL"
                })),
            )
                .into_response()
        }
    }
}

/// Register authentication routes on a router
#[cfg(feature = "http-server")]
pub fn register_auth_routes(
    router: axum::Router,
    state: AuthEndpointsState<()>,
) -> axum::Router {
    // Create a separate router for auth routes with state
    let auth_router = axum::Router::new()
        .route("/auth/token", axum::routing::post(token_endpoint))
        .route("/auth/refresh", axum::routing::post(refresh_token_endpoint))
        .route("/auth/userinfo", axum::routing::get(user_info_endpoint))
        .route("/auth/authorize-url", axum::routing::post(authorization_url_endpoint))
        .with_state(state);

    // Merge the auth router with the existing router
    router.merge(auth_router)
}
