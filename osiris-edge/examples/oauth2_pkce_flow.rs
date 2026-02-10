//! Example demonstrating the OAuth2 PKCE authentication flow
//!
//! This example shows how to use the PkceAuthenticator to implement
//! RFC 7636 Proof Key for Public Clients Exchange for secure public client authentication.

use osiris_edge::{
    AuthorizationRequest, AuthorizationResponse, CodeChallenge, CodeVerifier, Oauth2Authenticator,
    Oauth2Session, PkceAuthenticator, RefreshTokenRequest, TokenRequest, TokenResponse,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OAuth2 PKCE Authentication Flow Example ===\n");

    // Step 1: Initialize the PKCE authenticator
    println!("Step 1: Initializing PKCE Authenticator...");
    let authenticator = PkceAuthenticator::new()?;
    println!("✓ Authenticator created\n");

    // Step 2: Generate code verifier and challenge
    println!("Step 2: Generating Code Verifier and Challenge...");
    let (verifier, challenge) = authenticator.generate_code_verifier_and_challenge().await?;
    println!("✓ Code Verifier: {} chars", verifier.value.len());
    println!("✓ Code Challenge: {}", challenge.value);
    println!("✓ Challenge Method: {:?}\n", challenge.method);

    // Step 3: Create authorization request
    println!("Step 3: Creating Authorization Request...");
    let auth_request = authenticator
        .create_authorization_request(
            "my-public-client-id".to_string(),
            "https://auth.example.com/oauth/authorize".to_string(),
            "https://app.example.com/callback".to_string(),
            "openid profile email".to_string(),
            challenge,
            verifier.clone(),
        )
        .await?;

    println!("✓ Authorization Request Created:");
    println!("  - Client ID: {}", auth_request.client_id);
    println!("  - Redirect URI: {}", auth_request.redirect_uri);
    println!("  - Scope: {}", auth_request.scope);
    println!("  - State: {} (CSRF protection)", auth_request.state);
    println!(
        "  - Challenge Method: {:?}\n",
        auth_request.code_challenge.method
    );

    // Step 4: Simulate authorization response (user would be redirected to auth server)
    println!("Step 4: Simulating Authorization Response...");
    let auth_response = AuthorizationResponse {
        code: "auth_code_abc123xyz".to_string(),
        state: auth_request.state.clone(),
        error: None,
        error_description: None,
        error_uri: None,
    };
    println!("✓ Authorization Code: {}\n", auth_response.code);

    // Step 5: Validate authorization response
    println!("Step 5: Validating Authorization Response...");
    let auth_code = authenticator
        .validate_authorization_response(&auth_response, &auth_request.state)
        .await?;
    println!("✓ Authorization code validated: {}\n", auth_code);

    // Step 6: Prepare token exchange request
    println!("Step 6: Preparing Token Exchange Request...");
    let token_request = TokenRequest {
        token_endpoint: "https://auth.example.com/oauth/token".to_string(),
        client_id: "my-public-client-id".to_string(),
        code: auth_code,
        code_verifier: verifier.value.clone(),
        redirect_uri: "https://app.example.com/callback".to_string(),
        client_secret: None, // Public clients don't have secrets
        additional_params: Default::default(),
    };
    println!("✓ Token Exchange Request prepared");
    println!("  - Token Endpoint: {}", token_request.token_endpoint);
    println!(
        "  - Code Verifier: {} chars\n",
        token_request.code_verifier.len()
    );

    // Step 7: Create a mock token response
    println!("Step 7: Simulating Token Exchange (in production, makes HTTP request)...");
    let token_response = TokenResponse {
        access_token: "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1c2VyXzEyMyIsIm5hbWUiOiJKb2huIERvZSIsImlhdCI6MTUxNjIzOTAyMn0.signature".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: Some(3600), // 1 hour
        refresh_token: Some("refresh_token_def456uvw".to_string()),
        scope: Some("openid profile email".to_string()),
        additional_params: Default::default(),
    };
    println!("✓ Token Response received:");
    println!(
        "  - Access Token: {}...",
        &token_response.access_token[0..50]
    );
    println!("  - Token Type: {}", token_response.token_type);
    println!(
        "  - Expires In: {} seconds",
        token_response.expires_in.unwrap_or(0)
    );
    println!("  - Refresh Token: Available\n");

    // Step 8: Create session from token response
    println!("Step 8: Creating OAuth2 Session...");
    let session = authenticator
        .create_session(&token_response, "openid profile email".to_string())
        .await?;
    println!("✓ Session created:");
    println!("  - Session ID: {}", session.session_id);
    println!("  - Access Token: Valid");
    println!("  - Expires At: {:?}", session.expires_at);
    println!("  - Can Refresh: {}\n", session.can_refresh());

    // Step 9: Retrieve session
    println!("Step 9: Retrieving Stored Session...");
    let retrieved_session = authenticator.get_session(&session.session_id).await?;
    if let Some(sess) = retrieved_session {
        println!("✓ Session retrieved: {}", sess.session_id);
        println!("  - Scope: {}", sess.scope);
        println!("  - Created At: {}\n", sess.created_at);
    }

    // Step 10: Check session validity
    println!("Step 10: Checking Session Validity...");
    let is_valid = authenticator
        .is_session_valid(&session.session_id, 300) // 5 minute buffer
        .await?;
    println!("✓ Session is valid: {}\n", is_valid);

    // Step 11: Validate access token
    println!("Step 11: Validating Access Token...");
    let claims = authenticator
        .validate_token(&token_response.access_token, Some("openid"))
        .await?;
    println!("✓ Token validated");
    println!("  - Claims: {}\n", serde_json::to_string_pretty(&claims)?);

    // Step 12: Demonstrate refresh token flow
    println!("Step 12: Demonstrating Refresh Token Flow...");
    if let Some(refresh_token) = &token_response.refresh_token {
        let refresh_request = RefreshTokenRequest {
            token_endpoint: "https://auth.example.com/oauth/token".to_string(),
            client_id: "my-public-client-id".to_string(),
            refresh_token: refresh_token.clone(),
            client_secret: None,
            scope: Some("openid profile email".to_string()),
            additional_params: Default::default(),
        };
        println!("✓ Refresh Token Request prepared (in production, makes HTTP request)");
        println!("  - Using refresh token for token rotation\n");
    }

    // Step 13: Revoke session
    println!("Step 13: Revoking Session...");
    authenticator.revoke_session(&session.session_id).await?;
    println!("✓ Session revoked\n");

    // Step 14: Verify session was revoked
    println!("Step 14: Verifying Session Revocation...");
    let revoked_session = authenticator.get_session(&session.session_id).await?;
    println!(
        "✓ Session exists after revocation: {}",
        revoked_session.is_some()
    );

    println!("\n=== OAuth2 PKCE Flow Complete ===");
    println!("\nKey Features Demonstrated:");
    println!("  ✓ Code verifier/challenge generation (RFC 7636)");
    println!("  ✓ Authorization request with CSRF protection (state)");
    println!("  ✓ Authorization response validation");
    println!("  ✓ Token exchange without client secret");
    println!("  ✓ Session creation and storage");
    println!("  ✓ Token validation");
    println!("  ✓ Refresh token support");
    println!("  ✓ Session revocation");

    Ok(())
}
