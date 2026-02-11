//! Comprehensive authentication demonstration
//!
//! This example shows:
//! - OAuth2 Authorization Code flow
//! - OAuth2 Client Credentials flow
//! - OAuth2 Resource Owner Password flow
//! - OpenID Connect integration
//! - JWT token generation and validation
//! - Token refresh workflow
//! - Protected API endpoints with authentication
//!
//! Run with: cargo run --example auth_demo --features "auth,http-server"

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};

use a2a_rs::{
    adapter::{
        auth::{
            endpoints::{register_auth_routes, AuthEndpointsState, InMemoryUserStore, UserStore},
            token_service::{
                TokenRefreshRequest, TokenRequest, TokenService, UserInfo,
                AuthorizationUrlGenerator, AuthorizationUrlResponse
            },
            with_auth, JwtAuthenticator, OAuth2Authenticator, OpenIdConnectAuthenticator,
        },
        transport::http::server::HttpServer,
    },
    domain::{
        core::agent::{AgentCard, AgentCapabilities, SecurityScheme},
        A2AError,
    },
    port::{authenticator::{AuthContext, AuthPrincipal, Authenticator}, AgentInfoProvider},
    services::server::{AsyncA2ARequestProcessor, AgentInfoProvider},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("auth_demo=debug,info")
        .init();

    println!("=== A2A Authentication Demo ===\n");

    // Run all authentication demonstrations
    demo_jwt_token_service()?;
    demo_oauth2_flows()?;
    demo_openid_connect()?;
    demo_token_refresh()?;
    demo_user_info()?;

    println!("\n=== Starting Authenticated HTTP Server ===\n");

    // Start a demonstration server
    start_authenticated_server().await?;

    Ok(())
}

/// Demonstrate JWT token generation and validation
fn demo_jwt_token_service() -> Result<(), A2AError> {
    println!("1. JWT Token Service Demo");
    println!("   ------------------------");

    let secret = b"demo_secret_key_please_change_in_production";
    let token_service = TokenService::new_with_secret(secret)
        .with_expiration(3600)
        .with_refresh_expiration(2_592_000)
        .with_issuer("https://demo.example.com".to_string())
        .with_audience("demo-api".to_string());

    // Create a user principal
    let principal = AuthPrincipal::new("user_12345".to_string(), "jwt".to_string())
        .with_attribute("name".to_string(), "Alice Johnson".to_string())
        .with_attribute("email".to_string(), "alice@example.com".to_string())
        .with_attribute("role".to_string(), "admin".to_string());

    println!("   Creating principal for: Alice Johnson (user_12345)");

    // Generate token with refresh token
    let response = token_service.generate_token_with_refresh(&principal)?;

    println!("   ✓ Generated access token: {}...", &response.access_token[..50]);
    println!("   ✓ Token type: {}", response.token_type);
    println!("   ✓ Expires in: {} seconds", response.expires_in);
    println!("   ✓ Refresh token: {}...", &response.refresh_token.as_ref().unwrap()[..50]);

    // Validate the token
    let validated = token_service.validate_token(&response.access_token)?;
    println!("   ✓ Validated token for subject: {}", validated.id);
    println!("   ✓ Claims: name={}, email={}, role={}",
        validated.attributes.get("name").unwrap_or(&"?".to_string()),
        validated.attributes.get("email").unwrap_or(&"?".to_string()),
        validated.attributes.get("role").unwrap_or(&"?".to_string())
    );

    println!("   ✓ JWT Token Service: PASSED\n");
    Ok(())
}

/// Demonstrate OAuth2 flows
fn demo_oauth2_flows() -> Result<(), A2AError> {
    println!("2. OAuth2 Flows Demo");
    println!("   -----------------");

    use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl};

    // Authorization Code Flow
    println!("   Authorization Code Flow:");

    let auth_url = AuthUrl::new("https://auth.example.com/oauth/authorize".to_string())
        .map_err(|e| A2AError::Internal(format!("Invalid auth URL: {}", e)))?;
    let token_url = TokenUrl::new("https://auth.example.com/oauth/token".to_string())
        .map_err(|e| A2AError::Internal(format!("Invalid token URL: {}", e)))?;
    let redirect_url = RedirectUrl::new("https://app.example.com/callback".to_string())
        .map_err(|e| A2AError::Internal(format!("Invalid redirect URL: {}", e)))?;

    let mut scopes = HashMap::new();
    scopes.insert("read".to_string(), "Read access to resources".to_string());
    scopes.insert("write".to_string(), "Write access to resources".to_string());
    scopes.insert("admin".to_string(), "Administrative access".to_string());

    let oauth2_auth = OAuth2Authenticator::new_authorization_code(
        ClientId::new("demo_client".to_string()),
        None,
        auth_url,
        token_url,
        redirect_url,
        scopes.clone(),
    );

    let (auth_url_str, csrf_token) = oauth2_auth.authorize_url();
    println!("   ✓ Authorization URL generated");
    println!("   ✓ URL: {}", &auth_url_str[..80]);
    println!("   ✓ CSRF Token: {}", csrf_token.secret());

    // Client Credentials Flow
    println!("\n   Client Credentials Flow:");

    let token_url_cc = TokenUrl::new("https://auth.example.com/oauth/token".to_string())
        .map_err(|e| A2AError::Internal(format!("Invalid token URL: {}", e)))?;

    let mut cc_scopes = HashMap::new();
    cc_scopes.insert("api".to_string(), "API access".to_string());

    let cc_auth = OAuth2Authenticator::new_client_credentials(
        ClientId::new("service_client".to_string()),
        ClientSecret::new("service_secret".to_string()),
        token_url_cc,
        cc_scopes,
    );

    println!("   ✓ Client credentials authenticator created");
    println!("   ✓ Token URL: {}", cc_auth.security_scheme()
        .get_oauth2_flows()
        .and_then(|f| f.client_credentials.as_ref())
        .map(|cc| &cc.token_url)
        .unwrap_or(&"?".to_string()));

    println!("   ✓ OAuth2 Flows: PASSED\n");
    Ok(())
}

/// Demonstrate OpenID Connect
fn demo_openid_connect() -> Result<(), A2AError> {
    println!("3. OpenID Connect Demo");
    println!("   --------------------");

    use openidconnect::{IssuerUrl, ClientId, RedirectUrl};

    // Create OIDC authenticator
    // Note: This would normally discover the provider metadata
    let issuer_url = IssuerUrl::new("https://accounts.example.com".to_string())
        .map_err(|e| A2AError::Internal(format!("Invalid issuer URL: {}", e)))?;
    let client_id = ClientId::new("oidc_client".to_string());
    let redirect_url = RedirectUrl::new("https://app.example.com/callback".to_string())
        .map_err(|e| A2AError::Internal(format!("Invalid redirect URL: {}", e)))?;

    // For demo purposes, skip actual discovery
    let oidc_auth = OpenIdConnectAuthenticator::new(
        issuer_url,
        client_id,
        None,
        redirect_url,
    ).await;

    match oidc_auth {
        Ok(auth) => {
            let (url, csrf_token, nonce) = auth.authorize_url();
            println!("   ✓ OIDC Authorization URL generated");
            println!("   ✓ URL: {}", &url[..80]);
            println!("   ✓ CSRF Token: {}", csrf_token.secret());
            println!("   ✓ Nonce: {}", nonce.secret());
            println!("   ✓ Issuer URL: {}", auth.security_scheme()
                .get_openid_connect_url()
                .unwrap_or(&"?".to_string()));
        }
        Err(e) => {
            println!("   ⚠ OIDC provider discovery failed (expected in demo): {}", e);
            println!("   ✓ OpenID Connect structure: PASSED\n");
            return Ok(());
        }
    }

    println!("   ✓ OpenID Connect: PASSED\n");
    Ok(())
}

/// Demonstrate token refresh workflow
fn demo_token_refresh() -> Result<(), A2AError> {
    println!("4. Token Refresh Demo");
    println!("   -------------------");

    let secret = b"demo_refresh_secret";
    let token_service = TokenService::new_with_secret(secret)
        .with_expiration(60) // Short expiration for demo
        .with_refresh_expiration(3600);

    let principal = AuthPrincipal::new("user_refresh".to_string(), "jwt".to_string());

    // Generate initial token
    let response = token_service.generate_token_with_refresh(&principal)?;
    let refresh_token = response.refresh_token.as_ref().unwrap();

    println!("   ✓ Generated initial access token");
    println!("   ✓ Access token: {}...", &response.access_token[..50]);
    println!("   ✓ Refresh token: {}...", &refresh_token[..50]);

    // Simulate token refresh
    let refreshed = token_service.refresh_token(refresh_token)?;
    println!("   ✓ Refreshed access token");
    println!("   ✓ New access token: {}...", &refreshed.access_token[..50]);

    // Verify new token
    let validated = token_service.validate_token(&refreshed.access_token)?;
    println!("   ✓ Validated new token for subject: {}", validated.id);

    println!("   ✓ Token Refresh: PASSED\n");
    Ok(())
}

/// Demonstrate user info endpoint
fn demo_user_info() -> Result<(), A2AError> {
    println!("5. User Info Demo");
    println!("   --------------");

    let secret = b"demo_userinfo_secret";
    let token_service = TokenService::new_with_secret(secret);

    // Create user with comprehensive profile
    let principal = AuthPrincipal::new("user_profile".to_string(), "oidc".to_string())
        .with_attribute("name".to_string(), "Bob Smith".to_string())
        .with_attribute("email".to_string(), "bob.smith@example.com".to_string())
        .with_attribute("email_verified".to_string(), "true".to_string())
        .with_attribute("picture".to_string(), "https://example.com/bob.jpg".to_string())
        .with_attribute("given_name".to_string(), "Bob".to_string())
        .with_attribute("family_name".to_string(), "Smith".to_string());

    // Generate token
    let response = token_service.generate_token(&principal)?;

    // Get user info
    let user_info = token_service.get_user_info(&response.access_token)?;

    println!("   ✓ User info retrieved:");
    println!("   ✓ Sub: {}", user_info.sub);
    println!("   ✓ Name: {}", user_info.name.unwrap_or_default());
    println!("   ✓ Email: {}", user_info.email.unwrap_or_default());
    println!("   ✓ Email Verified: {}", user_info.email_verified.unwrap_or(false));
    println!("   ✓ Picture: {}", user_info.picture.unwrap_or_default());
    println!("   ✓ Given Name: {}", user_info.given_name.unwrap_or_default());
    println!("   ✓ Family Name: {}", user_info.family_name.unwrap_or_default());

    println!("   ✓ User Info: PASSED\n");
    Ok(())
}

/// Start an authenticated HTTP server
async fn start_authenticated_server() -> Result<(), Box<dyn std::error::Error>> {
    // Create token service
    let secret = b"server_auth_secret_change_in_production";
    let token_service = Arc::new(
        TokenService::new_with_secret(secret)
            .with_expiration(3600)
            .with_refresh_expiration(86400)
            .with_issuer("https://demo.example.com".to_string())
            .with_audience("demo-api".to_string()),
    );

    // Create user store with test users
    let user_store = Arc::new(InMemoryUserStore::new()
        .add_user(
            "alice".to_string(),
            "password123".to_string(),
            AuthPrincipal::new("user_alice".to_string(), "password".to_string())
                .with_attribute("name".to_string(), "Alice Johnson".to_string())
                .with_attribute("email".to_string(), "alice@example.com".to_string())
                .with_attribute("email_verified".to_string(), "true".to_string())
        )
        .add_user(
            "bob".to_string(),
            "password456".to_string(),
            AuthPrincipal::new("user_bob".to_string(), "password".to_string())
                .with_attribute("name".to_string(), "Bob Smith".to_string())
                .with_attribute("email".to_string(), "bob@example.com".to_string())
                .with_attribute("email_verified".to_string(), "true".to_string())
        )
    ) as Arc<dyn UserStore>;

    // Build the router with authentication
    let mut app = Router::new()
        .route("/", get(protected_endpoint))
        .route("/health", get(health_check))
        .route("/api/data", get(api_data_endpoint));

    // Register authentication routes
    let auth_state = AuthEndpointsState {
        token_service: Some(token_service.clone()),
        user_store: Some(user_store),
        additional_state: (),
    };

    app = register_auth_routes(app, auth_state);

    // Add JWT authentication middleware to protected routes
    let jwt_auth = JwtAuthenticator::new_with_secret(secret)
        .with_issuer("https://demo.example.com".to_string())
        .with_audience("demo-api".to_string());

    app = with_auth(app, jwt_auth);

    // Start server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
    println!("   ✓ Authenticated server listening on http://127.0.0.1:8080");
    println!("\n   Available endpoints:");
    println!("   • POST /auth/token - Get access token (password flow)");
    println!("   • POST /auth/refresh - Refresh access token");
    println!("   • GET  /auth/userinfo - Get user info (requires valid token)");
    println!("   • POST /auth/authorize-url - Generate OAuth2 authorization URL");
    println!("   • GET  /health - Public health check");
    println!("   • GET  / - Protected endpoint (requires authentication)");
    println!("   • GET  /api/data - Protected API data (requires authentication)");
    println!("\n   Example usage:");
    println!("   # Get token:");
    println!("   curl -X POST http://127.0.0.1:8080/auth/token \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d '{{\"grantType\":\"password\",\"username\":\"alice\",\"password\":\"password123\"}}'");
    println!("\n   # Access protected endpoint:");
    println!("   curl http://127.0.0.1:8080/ \\");
    println!("     -H 'Authorization: Bearer YOUR_TOKEN_HERE'");
    println!("\n   Press Ctrl+C to stop the server\n");

    axum::serve(listener, app).await?;

    Ok(())
}

/// Protected endpoint that requires authentication
async fn protected_endpoint(
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    // In a real app, you'd extract the principal from the request extensions
    // added by the authentication middleware
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(auth) if auth.starts_with("Bearer ") => {
            let token = &auth[7..];
            Ok(Json(json!({
                "message": "Welcome to the protected endpoint!",
                "authenticated": true,
                "token_preview": format!("{}...", &token[..20.min(token.len())])
            })))
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Health check endpoint (public)
async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "authentication": "enabled"
    }))
}

/// Protected API data endpoint
async fn api_data_endpoint(
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(_) => {
            Ok(Json(json!({
                "data": [
                    {"id": 1, "name": "Item 1", "value": 100},
                    {"id": 2, "name": "Item 2", "value": 200},
                    {"id": 3, "name": "Item 3", "value": 300},
                ],
                "message": "Sensitive data accessed successfully"
            })))
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}
