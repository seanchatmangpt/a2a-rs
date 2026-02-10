# OAuth2 PKCE (Proof Key for Public Clients Exchange) Authenticator

## Overview

The **PkceAuthenticator** implements RFC 7636 Proof Key for Public Clients Exchange for secure OAuth2 authentication of public clients (applications without backend secrets, like SPAs and mobile apps).

**Location**: `osiris-edge/src/adapter/oauth_pkce.rs`

## Key Features

### 1. **Code Verifier & Challenge Generation**
- Generates cryptographically secure 128-character verifiers
- Supports SHA256 code challenge method (recommended)
- Optional plain-text challenge for compatibility (not recommended for production)
- Character set: `[A-Z][a-z][0-9]-._~` per RFC 7636

### 2. **PKCE Flow Implementation**
```
1. Generate code_verifier and code_challenge
2. Redirect user to authorization endpoint with code_challenge
3. User authorizes application
4. Receive authorization_code from redirect
5. Exchange code + verifier for access token (prevents code interception)
6. Manage sessions and tokens
7. Refresh tokens when expired
```

### 3. **Session Management**
- In-memory session storage with configurable limits
- Automatic cleanup of old sessions
- Token expiration tracking with configurable buffer
- Refresh token support for token rotation

### 4. **Token Validation**
- Basic JWT format validation
- Scope validation support
- Expiration checking with clock skew tolerance

### 5. **Security Features**
- **CSRF Protection**: State parameter validation in authorization response
- **Code Interception Prevention**: Code verifier prevents code theft
- **Cryptographic Randomness**: UUID-based seed with SHA256
- **No Client Secrets**: Suitable for public clients (SPAs, mobile apps)

## Architecture

### Port Trait: `Oauth2Authenticator`

Defines the interface for OAuth2 PKCE authentication:

```rust
#[async_trait]
pub trait Oauth2Authenticator: Send + Sync {
    // Code generation
    async fn generate_code_verifier_and_challenge(
        &self,
    ) -> Result<(CodeVerifier, CodeChallenge), EdgeError>;

    // Authorization flow
    async fn create_authorization_request(...)
        -> Result<AuthorizationRequest, EdgeError>;
    async fn validate_authorization_response(...)
        -> Result<String, EdgeError>;

    // Token exchange
    async fn exchange_code_for_token(...)
        -> Result<TokenResponse, EdgeError>;
    async fn refresh_access_token(...)
        -> Result<TokenResponse, EdgeError>;

    // Session management
    async fn create_session(...) -> Result<Oauth2Session, EdgeError>;
    async fn get_session(...) -> Result<Option<Oauth2Session>, EdgeError>;
    async fn store_session(...) -> Result<(), EdgeError>;
    async fn revoke_session(...) -> Result<(), EdgeError>;
    async fn is_session_valid(...) -> Result<bool, EdgeError>;
    async fn refresh_session_if_needed(...) -> Result<Oauth2Session, EdgeError>;

    // Token validation
    async fn validate_token(...)
        -> Result<serde_json::Value, EdgeError>;
}
```

### Adapter: `PkceAuthenticator`

Concrete implementation providing:

- **HTTP Client**: `reqwest`-based token endpoint requests
- **Session Storage**: In-memory `HashMap` with `RwLock`
- **Randomness**: UUID + SHA256 for code generation
- **Configuration**: `PkceConfig` builder pattern

## Domain Types

### `CodeVerifier`
- Generates and validates verifiers (43-128 chars)
- Character validation per RFC 7636
- Timestamp tracking

```rust
let verifier = CodeVerifier::new(verifier_string)?;
```

### `CodeChallenge`
- SHA256 hash of verifier (S256 method, recommended)
- Base64url encoding without padding
- Plain-text fallback support

```rust
let challenge = CodeChallenge::sha256(&verifier);
let challenge = CodeChallenge::plain(&verifier);
```

### `AuthorizationRequest`
- Wraps authorization endpoint details
- Stores code_challenge and code_verifier
- Includes state for CSRF protection

### `AuthorizationResponse`
- OAuth2 server response
- Contains authorization code or error
- State parameter for validation

### `TokenRequest`
- Token exchange request parameters
- Includes code_verifier for security
- Optional client_secret for confidential clients

### `TokenResponse`
- Access token + metadata
- Expiration time
- Optional refresh token
- Additional claims from provider

### `RefreshTokenRequest`
- Refresh token grant parameters
- Maintains scope restrictions
- Optional client secret

### `Oauth2Session`
- Stores active OAuth2 session
- Tracks expiration and refresh capability
- Stores additional claims
- Includes creation and refresh timestamps

## Configuration

### `PkceConfig`

```rust
let config = PkceConfig::new()
    .with_client_timeout(30)              // HTTP timeout
    .with_expiration_buffer(300)          // 5-minute buffer
    .with_max_sessions(1000)              // Max stored sessions
    .with_user_agent("app/1.0".to_string());

let authenticator = PkceAuthenticator::with_config(config)?;
```

## Usage Example

### Complete Flow

```rust
use osiris_edge::{PkceAuthenticator, Oauth2Authenticator};

#[tokio::main]
async fn main() -> Result<()> {
    let auth = PkceAuthenticator::new()?;

    // 1. Generate verifier and challenge
    let (verifier, challenge) =
        auth.generate_code_verifier_and_challenge().await?;

    // 2. Create authorization request
    let auth_request = auth.create_authorization_request(
        "client_id".to_string(),
        "https://auth.example.com/authorize".to_string(),
        "https://app.example.com/callback".to_string(),
        "openid profile".to_string(),
        challenge,
        verifier,
    ).await?;

    // 3. Redirect user to: auth_request.authorization_endpoint?...

    // 4. After user authorizes, get authorization_code from redirect
    let auth_response = AuthorizationResponse {
        code: "auth_code_...".to_string(),
        state: auth_request.state.clone(),
        error: None,
        error_description: None,
        error_uri: None,
    };

    // 5. Validate response (CSRF check)
    let code = auth.validate_authorization_response(
        &auth_response,
        &auth_request.state
    ).await?;

    // 6. Exchange code for token
    let token_request = TokenRequest {
        token_endpoint: "https://auth.example.com/token".to_string(),
        client_id: "client_id".to_string(),
        code,
        code_verifier: auth_request.code_verifier.value,
        redirect_uri: auth_request.redirect_uri,
        client_secret: None,
        additional_params: Default::default(),
    };

    let token_response = auth.exchange_code_for_token(&token_request).await?;

    // 7. Create session
    let session = auth.create_session(
        &token_response,
        "openid profile".to_string()
    ).await?;

    // 8. Store and retrieve session
    auth.store_session(session.clone()).await?;
    let stored = auth.get_session(&session.session_id).await?;

    // 9. Check if session is valid
    let valid = auth.is_session_valid(&session.session_id, 300).await?;

    // 10. Refresh session if needed
    let refreshed = auth.refresh_session_if_needed(
        &session.session_id,
        300,  // buffer seconds
        "https://auth.example.com/token"
    ).await?;

    // 11. Validate access token
    let claims = auth.validate_token(
        &token_response.access_token,
        Some("openid")
    ).await?;

    // 12. Revoke session
    auth.revoke_session(&session.session_id).await?;

    Ok(())
}
```

## RFC 7636 Compliance

### PKCE Security Model

The PKCE flow adds protection against:
1. **Authorization Code Interception**: Code cannot be directly used without verifier
2. **Authorization Code Replay**: Verifier is one-time-use
3. **Open Redirector Attacks**: State parameter prevents misuse

### Implementation Details

**Code Verifier**:
- Length: 43-128 characters
- Character set: `[A-Z][a-z][0-9] - . _ ~`
- Generation: Cryptographically random

**Code Challenge**:
- Method S256 (SHA256): `BASE64URL(SHA256(verifier))`
- Method plain: verifier value (not recommended)
- Base64URL encoding: RFC 4648 without padding

**Verification**:
- Server stores: `hash(code_verifier) == code_challenge`
- Cannot reverse: SHA256 is one-way
- Prevents code reuse: Different verifier per request

## Security Considerations

### For Public Clients (SPAs, Mobile Apps)

✓ **Do Use PKCE**
- Prevents code interception attacks
- No secrets to compromise
- Works with auth servers supporting PKCE

✓ **Validate State**
- Prevents CSRF attacks
- Must match request value

✓ **Use S256 Method**
- SHA256 recommended over plain
- Industry standard

✗ **Don't Store Client Secrets**
- Public clients have no secrets
- Verifier provides security instead

### Token Storage

⚠️ **In-Memory Storage** (Default)
- Suitable for single-server deployments
- Lost on restart
- For multi-server, implement persistent backend

**Optional Persistence**:
```rust
// Future: implement persistence adapter
// adapter/persistent_oauth_storage.rs
```

## Testing

### Unit Tests

```bash
cargo test -p osiris-edge --lib oauth_pkce
```

**Coverage**:
- PKCE generation and validation
- Authorization flow (success/error)
- Token exchange simulation
- Session lifecycle
- Token validation

### Integration Tests

```bash
cargo test --example oauth2_pkce_flow
```

**Example Output**:
```
=== OAuth2 PKCE Authentication Flow Example ===

Step 1: Initializing PKCE Authenticator...
✓ Authenticator created

Step 2: Generating Code Verifier and Challenge...
✓ Code Verifier: 128 chars
✓ Code Challenge: [base64url string]
✓ Challenge Method: S256
...
```

## Error Handling

### `EdgeError` Variants

| Error | Cause | Recovery |
|-------|-------|----------|
| `Authentication` | Invalid token/code | Reauthenticate |
| `TokenValidation` | Malformed/expired token | Refresh or reauth |
| `InvalidToken` | Wrong format | Validate token format |
| `TokenExpired` | Explicit expiration | Refresh token |
| `HttpClient` | Network/HTTP error | Retry with backoff |
| `Configuration` | Config issue | Fix configuration |
| `Internal` | Unexpected error | Log and escalate |

### Example Error Handling

```rust
match auth.exchange_code_for_token(&request).await {
    Ok(response) => {
        // Use token
    }
    Err(EdgeError::TokenValidation(msg)) => {
        eprintln!("Token validation failed: {}", msg);
        // Redirect to login
    }
    Err(EdgeError::HttpClient(msg)) => {
        eprintln!("Network error: {}", msg);
        // Retry with exponential backoff
    }
    Err(e) => {
        eprintln!("Unexpected error: {}", e);
    }
}
```

## Performance Characteristics

### Time Complexity

| Operation | Complexity | Note |
|-----------|-----------|------|
| Generate verifier | O(1) | Fixed 128 chars |
| Create challenge | O(n) | SHA256 is linear |
| Store session | O(1) | HashMap insertion |
| Lookup session | O(1) | HashMap access |
| Cleanup old | O(n log n) | Sort + delete |

### Space Complexity

- Per session: ~2KB (token + metadata)
- 1000 sessions: ~2MB
- Configurable via `max_stored_sessions`

## Future Enhancements

1. **Persistent Storage**
   - Redis backend
   - Database storage
   - Cloud key-value stores

2. **Metrics & Observability**
   - Token exchange duration
   - Session hit rate
   - Error rates by type

3. **Advanced Features**
   - PAR (Pushed Authorization Request)
   - JWT client assertions
   - FAPI compliance

4. **Provider-Specific Adapters**
   - Google Workspace
   - Azure AD
   - Okta

## References

- RFC 7636: Proof Key for Public Clients Exchange
- RFC 6749: OAuth 2.0 Authorization Framework
- RFC 6234: US Secure Hash and HMAC Algorithms
- OWASP: OAuth 2.0 Security Best Practices
