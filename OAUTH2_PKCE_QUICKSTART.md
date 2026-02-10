# OAuth2 PKCE Quickstart Guide

## What is PKCE?

**PKCE** (Proof Key for Public Clients Exchange) is a security extension for OAuth2 that protects public clients (apps without backend secrets) from authorization code interception attacks.

## When to Use

✅ **Use PKCE for**:
- Single Page Applications (SPAs)
- Mobile apps
- Command-line tools
- Desktop applications
- Any client that can't keep secrets

❌ **PKCE is optional for**:
- Server-side applications with backend secrets
- But recommended by OWASP for all clients

## 5-Minute Setup

### 1. Create Authenticator
```rust
use osiris_edge::PkceAuthenticator;

let auth = PkceAuthenticator::new()?;
// Optional: custom config
let auth = PkceAuthenticator::with_config(
    PkceConfig::new()
        .with_client_timeout(60)
        .with_max_sessions(5000)
)?;
```

### 2. Generate Code Verifier & Challenge
```rust
let (verifier, challenge) =
    auth.generate_code_verifier_and_challenge().await?;

// Store verifier securely (e.g., in browser sessionStorage)
// Send challenge to authorization endpoint
```

### 3. Create Authorization URL
```rust
let auth_request = auth.create_authorization_request(
    "your_client_id".to_string(),
    "https://auth.example.com/oauth/authorize".to_string(),
    "https://app.example.com/callback".to_string(),
    "openid profile email".to_string(),
    challenge,
    verifier,
).await?;

// Redirect user to auth URL with:
// ?client_id=...&code_challenge=...&state=...
```

### 4. Handle Authorization Response
```rust
let code = auth.validate_authorization_response(
    &auth_response,
    &expected_state,  // CSRF protection
).await?;
```

### 5. Exchange Code for Token
```rust
let token = auth.exchange_code_for_token(&TokenRequest {
    token_endpoint: "https://auth.example.com/oauth/token".to_string(),
    client_id: "your_client_id".to_string(),
    code,
    code_verifier: verifier.value,  // Proves code ownership
    redirect_uri: "https://app.example.com/callback".to_string(),
    client_secret: None,  // Public clients have no secret
    additional_params: Default::default(),
}).await?;
```

### 6. Manage Session
```rust
// Create session
let session = auth.create_session(&token, "openid profile email".to_string()).await?;

// Store session
auth.store_session(session.clone()).await?;

// Use session
let is_valid = auth.is_session_valid(&session.session_id, 300).await?;

// Refresh when needed
let refreshed = auth.refresh_session_if_needed(
    &session.session_id,
    300,  // Refresh if expires in < 5 minutes
    token_endpoint,
).await?;

// Cleanup
auth.revoke_session(&session.session_id).await?;
```

## Common Patterns

### With Axum Router
```rust
use axum::{
    routing::{get, post},
    Router, State, Json, extract::Query,
};

let auth = PkceAuthenticator::new()?;

let app = Router::new()
    .route("/login", get(login_handler))
    .route("/callback", post(callback_handler))
    .route("/profile", get(profile_handler))
    .with_state(auth);
```

### Login Endpoint
```rust
async fn login_handler(
    State(auth): State<PkceAuthenticator>,
) -> Json<serde_json::Value> {
    let (verifier, challenge) =
        auth.generate_code_verifier_and_challenge().await.unwrap();

    let auth_request = auth.create_authorization_request(
        "client_id".to_string(),
        "https://auth.example.com/authorize".to_string(),
        "https://app.example.com/callback".to_string(),
        "openid profile".to_string(),
        challenge,
        verifier,
    ).await.unwrap();

    Json(serde_json::json!({
        "authorization_url": format!(
            "{}?client_id={}&code_challenge={}&state={}",
            auth_request.authorization_endpoint,
            auth_request.client_id,
            auth_request.code_challenge.value,
            auth_request.state
        ),
        "state": auth_request.state,
    }))
}
```

### Callback Endpoint
```rust
#[derive(serde::Deserialize)]
struct CallbackQuery {
    code: String,
    state: String,
}

async fn callback_handler(
    State(auth): State<PkceAuthenticator>,
    Query(query): Query<CallbackQuery>,
) -> Result<Json<serde_json::Value>> {
    // Validate state (CSRF protection)
    let auth_response = AuthorizationResponse {
        code: query.code,
        state: query.state.clone(),
        error: None,
        error_description: None,
        error_uri: None,
    };

    let code = auth.validate_authorization_response(
        &auth_response,
        &stored_state,  // Retrieved from session
    ).await?;

    // Exchange code for token
    let token = auth.exchange_code_for_token(&TokenRequest {
        token_endpoint: "https://auth.example.com/token".to_string(),
        client_id: "client_id".to_string(),
        code,
        code_verifier: stored_verifier.value,
        redirect_uri: "https://app.example.com/callback".to_string(),
        client_secret: None,
        additional_params: Default::default(),
    }).await?;

    // Create session
    let session = auth.create_session(&token, "openid profile".to_string()).await?;
    auth.store_session(session.clone()).await?;

    Ok(Json(serde_json::json!({
        "session_id": session.session_id,
        "token": session.access_token,
    })))
}
```

### Protected Endpoint
```rust
async fn profile_handler(
    State(auth): State<PkceAuthenticator>,
    headers: http::HeaderMap,
) -> Result<Json<Profile>> {
    // Extract session ID from header
    let session_id = extract_session_from_header(&headers)?;

    // Check if session is valid
    if !auth.is_session_valid(&session_id, 300).await? {
        return Err(EdgeError::TokenExpired.into());
    }

    // Get session
    let session = auth.get_session(&session_id).await?
        .ok_or(EdgeError::Authentication("Session not found".to_string()))?;

    // Validate token
    let claims = auth.validate_token(&session.access_token, Some("openid")).await?;

    Ok(Json(Profile {
        user_id: claims["sub"].as_str().unwrap().to_string(),
        email: claims["email"].as_str().map(|s| s.to_string()),
    }))
}
```

## Configuration Options

```rust
let config = PkceConfig::new()
    .with_client_timeout(30)           // HTTP timeout in seconds
    .with_expiration_buffer(300)       // Refresh 5 min before expiration
    .with_max_sessions(1000)           // Max stored sessions
    .with_user_agent("MyApp/1.0".to_string());

let auth = PkceAuthenticator::with_config(config)?;
```

## Error Handling

```rust
use osiris_edge::EdgeError;

match auth.exchange_code_for_token(&request).await {
    Ok(token) => { /* Success */ },
    Err(EdgeError::TokenValidation(msg)) => {
        eprintln!("Token validation failed: {}", msg);
        // Redirect to login
    }
    Err(EdgeError::HttpClient(msg)) => {
        eprintln!("Network error: {}", msg);
        // Retry with backoff
    }
    Err(EdgeError::Authentication(msg)) => {
        eprintln!("Authentication failed: {}", msg);
        // Redirect to login
    }
    Err(e) => {
        eprintln!("Unexpected error: {}", e);
        // Log and escalate
    }
}
```

## RFC 7636 Compliance Details

### Code Verifier
- **Length**: 43-128 characters
- **Charset**: `[A-Z] [a-z] [0-9] - . _ ~` (unreserved characters)
- **Generated**: Cryptographically random via UUID + SHA256
- **Usage**: Sent with token exchange to prove code ownership

### Code Challenge
- **Method S256** (recommended):
  - `challenge = BASE64URL(SHA256(verifier))`
  - Sent during authorization
  - Server stores hash of verifier
- **Method plain** (fallback, not recommended):
  - `challenge = verifier`
  - Only for legacy systems

### State Parameter
- **Purpose**: CSRF protection
- **Generated**: UUID per authorization request
- **Validation**: Must match response value

## Security Best Practices

1. **Use S256 Method**
   ```rust
   let challenge = CodeChallenge::sha256(&verifier);  // ✅
   // NOT: CodeChallenge::plain(&verifier);  // ❌
   ```

2. **Validate State Parameter**
   ```rust
   auth.validate_authorization_response(&response, &expected_state).await?;
   // Prevents CSRF attacks
   ```

3. **Use HTTPS Only**
   - PKCE prevents code interception
   - HTTPS prevents token theft

4. **Store Verifier Securely**
   - In browser: sessionStorage (not localStorage)
   - In mobile: secure storage
   - Never in URL or query parameters

5. **Monitor Token Exchange**
   ```rust
   // Log failed exchanges for anomaly detection
   match auth.exchange_code_for_token(&request).await {
       Err(e) => {
           warn!("Token exchange failed: {:?}", e);
           // Alert if multiple failures
       }
       Ok(_) => { /* Continue */ }
   }
   ```

## Troubleshooting

### "Code verifier invalid"
- **Cause**: Verifier doesn't match hash in request
- **Fix**: Use same verifier from generation; don't modify

### "State parameter mismatch"
- **Cause**: Attacker trying CSRF attack
- **Fix**: Validate state before exchanging code; ignore mismatches

### "Token expired"
- **Cause**: Too much time between token issue and use
- **Fix**: Refresh session: `refresh_session_if_needed(..., 300)`

### "Session not found"
- **Cause**: Session was revoked or storage was cleared
- **Fix**: Clear cookies/storage and restart login flow

## Run the Example

```bash
# Complete working example with 14 steps
cargo run -p osiris-edge --example oauth2_pkce_flow

# Output:
# === OAuth2 PKCE Authentication Flow Example ===
# Step 1: Initializing PKCE Authenticator...
# ✓ Authenticator created
# ...
# === OAuth2 PKCE Flow Complete ===
```

## Testing

```bash
# Run unit tests
cargo test -p osiris-edge --lib oauth_pkce -- --nocapture

# Test specific feature
cargo test -p osiris-edge code_verifier --lib
cargo test -p osiris-edge code_challenge --lib
```

## Next Steps

1. **Read Full Documentation**: `docs/OAUTH2_PKCE.md`
2. **Review Example**: `examples/oauth2_pkce_flow.rs`
3. **Integrate with Your Router**: Use patterns above with your auth server
4. **Add Persistence**: Implement Redis adapter for multi-server deployments
5. **Monitor in Production**: Track token exchange duration and failures

## Resources

- [RFC 7636 - PKCE](https://tools.ietf.org/html/rfc7636)
- [OAuth 2.0 Security Best Practices](https://tools.ietf.org/html/draft-ietf-oauth-security-topics)
- [OpenID Connect Core 1.0](https://openid.net/specs/openid-connect-core-1_0.html)
- [OWASP Authentication Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html)

## Quick Reference

```rust
// Full minimal example
use osiris_edge::{PkceAuthenticator, Oauth2Authenticator};

#[tokio::main]
async fn main() -> Result<()> {
    let auth = PkceAuthenticator::new()?;

    // 1. Generate
    let (verifier, challenge) = auth.generate_code_verifier_and_challenge().await?;

    // 2. Authorize (redirect user)
    let auth_req = auth.create_authorization_request(
        "client_id".into(), "https://auth.example.com/authorize".into(),
        "https://app.example.com/callback".into(), "openid profile".into(),
        challenge, verifier,
    ).await?;

    // 3. Validate response
    let code = auth.validate_authorization_response(&response, &auth_req.state).await?;

    // 4. Exchange
    let token = auth.exchange_code_for_token(&TokenRequest {
        token_endpoint: "https://auth.example.com/token".into(),
        client_id: "client_id".into(),
        code, code_verifier: auth_req.code_verifier.value,
        redirect_uri: "https://app.example.com/callback".into(),
        client_secret: None,
        additional_params: Default::default(),
    }).await?;

    // 5. Use
    let session = auth.create_session(&token, "openid profile".into()).await?;
    auth.store_session(session).await?;

    Ok(())
}
```

Happy authenticating! 🔐
