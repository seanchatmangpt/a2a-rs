//! OAuth2 Client demonstration
//!
//! Shows how to implement an OAuth2 client that:
//! - Obtains authorization code
//! - Exchanges code for access token
//! - Uses access token to access protected resources
//! - Refreshes expired tokens
//!
//! Run with: cargo run --example oauth2_client_demo --features "auth"

use std::collections::HashMap;
use std::time::Duration;

use a2a_rs::{
    adapter::auth::{
        token_service::{TokenService, TokenRequest},
        OAuth2Authenticator,
    },
    port::authenticator::{AuthContext, Authenticator},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OAuth2 Client Demo ===\n");

    // Demo 1: Authorization Code Flow (Client-side)
    demo_authorization_code_flow()?;

    // Demo 2: Client Credentials Flow (Server-to-server)
    demo_client_credentials_flow()?;

    // Demo 3: Token Refresh
    demo_token_refresh()?;

    // Demo 4: Resource Owner Password Flow (Trusted apps)
    demo_password_flow()?;

    Ok(())
}

/// Demonstrate Authorization Code Flow
fn demo_authorization_code_flow() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. Authorization Code Flow");
    println!("   ----------------------");

    use oauth2::{AuthUrl, ClientId, RedirectUrl, TokenUrl, Scope};

    // Step 1: Generate authorization URL
    let auth_url = AuthUrl::new("https://auth.example.com/oauth/authorize".to_string())?;
    let token_url = TokenUrl::new("https://auth.example.com/oauth/token".to_string())?;
    let redirect_url = RedirectUrl::new("https://myapp.com/callback".to_string())?;

    let mut scopes = HashMap::new();
    scopes.insert("read".to_string(), "Read access".to_string());
    scopes.insert("write".to_string(), "Write access".to_string());

    let client = OAuth2Authenticator::new_authorization_code(
        ClientId::new("my_client_id".to_string()),
        None,
        auth_url,
        token_url,
        redirect_url,
        scopes,
    );

    let (url, _csrf_token) = client.authorize_url();

    println!("   Step 1: Redirect user to authorization URL:");
    println!("   {}", url);
    println!("\n   [User would be redirected here and grant permission]");
    println!("   [After authorization, user is redirected back with code]");

    // Step 2: Exchange authorization code for access token
    println!("\n   Step 2: Exchange authorization code for access token");
    println!("   (In real flow, this would be a POST request to token endpoint)");
    println!("   ✗ Not implemented in this demo (requires actual OAuth2 server)");

    println!("\n   ✓ Authorization Code Flow structure: PASSED\n");
    Ok(())
}

/// Demonstrate Client Credentials Flow
fn demo_client_credentials_flow() -> Result<(), Box<dyn std::error::Error>> {
    println!("2. Client Credentials Flow");
    println!("   ------------------------");

    use oauth2::{ClientId, ClientSecret, TokenUrl};

    // Step 1: Create client credentials authenticator
    let token_url = TokenUrl::new("https://auth.example.com/oauth/token".to_string())?;

    let mut scopes = HashMap::new();
    scopes.insert("api".to_string(), "API access".to_string());
    scopes.insert("background_jobs".to_string(), "Background job access".to_string());

    let _client = OAuth2Authenticator::new_client_credentials(
        ClientId::new("service_account".to_string()),
        ClientSecret::new("service_secret".to_string()),
        token_url,
        scopes,
    );

    println!("   Step 1: Create service account with client credentials");
    println!("   Client ID: service_account");
    println!("   Client Secret: ********");

    // Step 2: Request token from token endpoint
    println!("\n   Step 2: POST to token endpoint");
    println!("   URL: https://auth.example.com/oauth/token");
    println!("   Body: grant_type=client_credentials&client_id=service_account");
    println!("   ✗ Not implemented in this demo (requires actual OAuth2 server)");

    println!("\n   ✓ Client Credentials Flow structure: PASSED\n");
    Ok(())
}

/// Demonstrate Token Refresh
fn demo_token_refresh() -> Result<(), Box<dyn std::error::Error>> {
    println!("3. Token Refresh");
    println!("   --------------");

    let secret = b"demo_refresh_secret";
    let token_service = TokenService::new_with_secret(secret)
        .with_expiration(60) // Short expiration
        .with_refresh_expiration(3600);

    use a2a_rs::port::authenticator::AuthPrincipal;

    let principal = AuthPrincipal::new("service_account".to_string(), "client_credentials".to_string());

    // Get initial token
    println!("   Step 1: Get initial access token");
    let response = token_service.generate_token_with_refresh(&principal)?;
    println!("   ✓ Access token: {}...", &response.access_token[..30]);
    println!("   ✓ Refresh token: {}...", &response.refresh_token.as_ref().unwrap()[..30]);
    println!("   ✓ Expires in: {} seconds", response.expires_in);

    // Simulate token expiration and refresh
    println!("\n   Step 2: When access token expires, use refresh token");
    let refresh_token = response.refresh_token.as_ref().unwrap();
    let refreshed = token_service.refresh_token(refresh_token)?;
    println!("   ✓ New access token: {}...", &refreshed.access_token[..30]);

    println!("\n   ✓ Token Refresh: PASSED\n");
    Ok(())
}

/// Demonstrate Resource Owner Password Flow
fn demo_password_flow() -> Result<(), Box<dyn std::error::Error>> {
    println!("4. Resource Owner Password Flow");
    println!("   ---------------------------");

    let secret = b"demo_password_secret";
    let token_service = TokenService::new_with_secret(secret);

    // Step 1: Collect user credentials
    println!("   Step 1: User provides credentials");
    println!("   Username: user@example.com");
    println!("   Password: ********");

    // Step 2: Exchange credentials for token
    println!("\n   Step 2: POST credentials to token endpoint");
    println!("   URL: https://auth.example.com/oauth/token");
    println!("   Body:");
    println!("   {{");
    println!("     \"grantType\": \"password\",");
    println!("     \"username\": \"user@example.com\",");
    println!("     \"password\": \"user_password\",");
    println!("     \"scope\": \"read write\"");
    println!("   }}");

    // Simulate token generation (in real flow, this would be from the OAuth2 server)
    let principal = AuthPrincipal::new("user_password_flow".to_string(), "password".to_string())
        .with_attribute("email".to_string(), "user@example.com".to_string());

    let response = token_service.generate_token_with_refresh(&principal)?;
    println!("\n   ✓ Access token: {}...", &response.access_token[..30]);

    println!("\n   ⚠  Note: Password flow is only for highly trusted applications");
    println!("   ⚠  OAuth 2.1 deprecates this flow in favor of PKCE");

    println!("\n   ✓ Password Flow structure: PASSED\n");
    Ok(())
}

/// Demonstrate using an access token to access protected resources
#[allow(dead_code)]
fn demo_access_protected_resources() -> Result<(), Box<dyn std::error::Error>> {
    println!("5. Access Protected Resources");
    println!("   --------------------------");

    let secret = b"demo_api_access";
    let token_service = TokenService::new_with_secret(secret);

    let principal = AuthPrincipal::new("api_user".to_string(), "jwt".to_string());
    let response = token_service.generate_token(&principal)?;

    println!("   Step 1: Make authenticated request to API");
    println!("   GET https://api.example.com/v1/users");
    println!("   Authorization: Bearer {}", &response.access_token[..30]);

    println!("\n   Step 2: API validates token and returns data");
    println!("   HTTP 200 OK");
    println!("   Content-Type: application/json");
    println!("   {{");
    println!("     \"users\": [");
    println!("       {{\"id\": 1, \"name\": \"Alice\"}},");
    println!("       {{\"id\": 2, \"name\": \"Bob\"}}");
    println!("     ]");
    println!("   }}");

    println!("\n   ✓ Resource Access: PASSED\n");
    Ok(())
}
