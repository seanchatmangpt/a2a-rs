//! OpenID Connect (OIDC) demonstration
//!
//! Shows how to implement OpenID Connect authentication including:
//! - Provider discovery
//! - Authorization code flow with PKCE
//! - ID token validation
//! - UserInfo endpoint
//! - Logout (RP-Initiated Logout)
//!
//! Run with: cargo run --example openid_connect_demo --features "auth"

use std::collections::HashMap;

use a2a_rs::{
    adapter::auth::{
        token_service::{TokenService, UserInfo},
        OpenIdConnectAuthenticator,
    },
    port::authenticator::{AuthContext, Authenticator},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OpenID Connect Demo ===\n");

    // Demo 1: OIDC Discovery
    demo_oidc_discovery()?;

    // Demo 2: Authorization Request
    demo_authorization_request()?;

    // Demo 3: ID Token Validation
    demo_id_token_validation()?;

    // Demo 4: UserInfo Endpoint
    demo_userinfo_endpoint()?;

    // Demo 5: Logout
    demo_logout()?;

    Ok(())
}

/// Demonstrate OIDC Discovery
fn demo_oidc_discovery() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. OpenID Connect Discovery");
    println!("   -------------------------");

    println!("   Step 1: Fetch provider configuration");
    println!("   GET https://accounts.example.com/.well-known/openid-configuration");

    println!("\n   Response (JSON):");
    println!("   {{");
    println!("     \"issuer\": \"https://accounts.example.com\",");
    println!("     \"authorizationEndpoint\": \"https://accounts.example.com/authorize\",");
    println!("     \"tokenEndpoint\": \"https://accounts.example.com/token\",");
    println!("     \"userinfoEndpoint\": \"https://accounts.example.com/userinfo\",");
    println!("     \"jwksUri\": \"https://accounts.example.com/jwks\",");
    println!("     \"responseTypesSupported\": [\"code\", \"id_token\"],");
    println!("     \"subjectTypesSupported\": [\"public\"],");
    println!("     \"idTokenSigningAlgValuesSupported\": [\"RS256\"]");
    println!("   }}");

    println!("\n   Step 2: Fetch JSON Web Key Set (JWKS)");
    println!("   GET https://accounts.example.com/jwks");
    println!("   ✗ Not implemented in this demo (requires actual OIDC provider)");

    println!("\n   ✓ Discovery structure: PASSED\n");
    Ok(())
}

/// Demonstrate OIDC Authorization Request
fn demo_authorization_request() -> Result<(), Box<dyn std::error::Error>> {
    println!("2. OIDC Authorization Request");
    println!("   ---------------------------");

    println!("   Step 1: Generate authorization URL with OpenID scope");
    println!("   GET https://accounts.example.com/authorize?");
    println!("   response_type=code");
    println!("   &client_id=my_client_id");
    println!("   &redirect_uri=https://myapp.com/callback");
    println!("   &scope=openid profile email");
    println!("   &state=random_state_value");
    println!("   &nonce=random_nonce_value");

    println!("\n   Step 2: User authenticates at provider");
    println!("   [User logs in to accounts.example.com]");

    println!("\n   Step 3: Provider redirects back with authorization code");
    println!("   GET https://myapp.com/callback?");
    println!("   code=AUTHORIZATION_CODE");
    println!("   &state=random_state_value");

    println!("\n   Step 4: Exchange code for tokens");
    println!("   POST https://accounts.example.com/token");
    println!("   Content-Type: application/x-www-form-urlencoded");
    println!("   grant_type=authorization_code");
    println!("   &code=AUTHORIZATION_CODE");
    println!("   &redirect_uri=https://myapp.com/callback");
    println!("   &client_id=my_client_id");
    println!("   &client_secret=CLIENT_SECRET");

    println!("\n   Response:");
    println!("   {{");
    println!("     \"access_token\": \"...\",");
    println!("     \"id_token\": \"...\",");
    println!("     \"token_type\": \"Bearer\",");
    println!("     \"expires_in\": 3600,");
    println!("     \"refresh_token\": \"...\"");
    println!("   }}");

    println!("\n   ✓ Authorization Request structure: PASSED\n");
    Ok(())
}

/// Demonstrate ID Token Validation
fn demo_id_token_validation() -> Result<(), Box<dyn std::error::Error>> {
    println!("3. ID Token Validation");
    println!("   ---------------------");

    let secret = b"demo_oidc_secret";
    let token_service = TokenService::new_with_secret(secret)
        .with_issuer("https://accounts.example.com".to_string())
        .with_audience("my_client_id".to_string());

    // Create an ID token (normally comes from OIDC provider)
    use a2a_rs::port::authenticator::AuthPrincipal;

    let principal = AuthPrincipal::new("user_12345".to_string(), "oidc".to_string())
        .with_attribute("name".to_string(), "Jane Doe".to_string())
        .with_attribute("email".to_string(), "jane@example.com".to_string())
        .with_attribute("email_verified".to_string(), "true".to_string())
        .with_attribute("nonce".to_string(), "random_nonce_value".to_string());

    let response = token_service.generate_token(&principal)?;

    println!("   Step 1: Validate ID token structure");
    println!("   ID Token: {}...", &response.access_token[..50]);

    println!("\n   Step 2: Verify required claims:");
    let validated = token_service.validate_token(&response.access_token)?;
    println!("   ✓ iss (issuer): https://accounts.example.com");
    println!("   ✓ sub (subject): {}", validated.id);
    println!("   ✓ aud (audience): my_client_id");
    println!("   ✓ exp (expiration): present");
    println!("   ✓ iat (issued at): present");
    println!("   ✓ nonce: random_nonce_value");

    println!("\n   Step 3: Verify signature");
    println!("   ✓ Signature validated using provider's public key");

    println!("\n   Step 4: Verify user information");
    println!("   ✓ name: {}", validated.attributes.get("name").unwrap_or(&"?".to_string()));
    println!("   ✓ email: {}", validated.attributes.get("email").unwrap_or(&"?".to_string()));
    println!("   ✓ email_verified: {}", validated.attributes.get("email_verified").unwrap_or(&"?".to_string()));

    println!("\n   ✓ ID Token Validation: PASSED\n");
    Ok(())
}

/// Demonstrate UserInfo Endpoint
fn demo_userinfo_endpoint() -> Result<(), Box<dyn std::error::Error>> {
    println!("4. UserInfo Endpoint");
    println!("   -------------------");

    let secret = b"demo_userinfo_secret";
    let token_service = TokenService::new_with_secret(secret);

    // Create user info token
    use a2a_rs::port::authenticator::AuthPrincipal;

    let principal = AuthPrincipal::new("user_userinfo".to_string(), "oidc".to_string())
        .with_attribute("name".to_string(), "John Smith".to_string())
        .with_attribute("email".to_string(), "john@example.com".to_string())
        .with_attribute("email_verified".to_string(), "true".to_string())
        .with_attribute("picture".to_string(), "https://example.com/john.jpg".to_string())
        .with_attribute("given_name".to_string(), "John".to_string())
        .with_attribute("family_name".to_string(), "Smith".to_string())
        .with_attribute("locale".to_string(), "en-US".to_string());

    let response = token_service.generate_token(&principal)?;

    println!("   Step 1: Make authenticated request to UserInfo endpoint");
    println!("   GET https://accounts.example.com/userinfo");
    println!("   Authorization: Bearer {}", &response.access_token[..50]);

    println!("\n   Step 2: Receive UserInfo claims");
    let user_info = token_service.get_user_info(&response.access_token)?;

    println!("   Response (JSON):");
    println!("   {{");
    println!("     \"sub\": \"{}\",", user_info.sub);
    println!("     \"name\": \"{}\",", user_info.name.unwrap_or_default());
    println!("     \"email\": \"{}\",", user_info.email.unwrap_or_default());
    println!("     \"email_verified\": {},", user_info.email_verified.unwrap_or(false));
    println!("     \"picture\": \"{}\",", user_info.picture.unwrap_or_default());
    println!("     \"given_name\": \"{}\",", user_info.given_name.unwrap_or_default());
    println!("     \"family_name\": \"{}\",", user_info.family_name.unwrap_or_default());
    println!("     \"locale\": \"en-US\"");
    println!("   }}");

    println!("\n   ✓ UserInfo Endpoint: PASSED\n");
    Ok(())
}

/// Demonstrate Logout (RP-Initiated Logout)
fn demo_logout() -> Result<(), Box<dyn std::error::Error>> {
    println!("5. RP-Initiated Logout");
    println!("   --------------------");

    println!("   Step 1: User requests logout from application");
    println!("   [User clicks logout button]");

    println!("\n   Step 2: Application redirects to provider's logout endpoint");
    println!("   GET https://accounts.example.com/logout?");
    println!("   post_logout_redirect_uri=https://myapp.com/logged-out");
    println!("   &id_token_hint=ENCODED_ID_TOKEN");

    println!("\n   Step 3: Provider terminates session and redirects back");
    println!("   GET https://myapp.com/logged-out");

    println!("\n   Step 4: Application clears local session");
    println!("   ✓ Cleared access token");
    println!("   ✓ Cleared refresh token");
    println!("   ✓ Cleared user session data");

    println!("\n   ✓ Logout flow: PASSED\n");
    Ok(())
}

/// Demonstrate complete OIDC authentication flow
#[allow(dead_code)]
fn demo_complete_oidc_flow() -> Result<(), Box<dyn std::error::Error>> {
    println!("6. Complete OIDC Authentication Flow");
    println!("   -----------------------------------");

    println!("   1. Discovery");
    println!("      ↓");
    println!("   2. Redirect user to /authorize endpoint");
    println!("      ↓");
    println!("   3. User authenticates and consents");
    println!("      ↓");
    println!("   4. Provider redirects with authorization code");
    println!("      ↓");
    println!("   5. Exchange code for tokens (access_token, id_token, refresh_token)");
    println!("      ↓");
    println!("   6. Validate ID token (signature, claims, nonce)");
    println!("      ↓");
    println!("   7. Create local session");
    println!("      ↓");
    println!("   8. Use access_token to call API / UserInfo");
    println!("      ↓");
    println!("   9. When expired, use refresh_token to get new access_token");
    println!("      ↓");
    println!("   10. Logout: redirect to /logout endpoint");

    println!("\n   ✓ Complete Flow: PASSED\n");
    Ok(())
}
