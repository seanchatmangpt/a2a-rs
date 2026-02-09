# Authentication Gate Documentation

## Overview

The authentication gate provides comprehensive token validation for the OSIRIS Edge Gateway. It follows hexagonal architecture principles with clear separation between domain types, port interfaces, and adapter implementations.

## Architecture

```
domain/auth.rs          -> Pure domain types (AuthPrincipal, AuthRequest, etc.)
port/auth_gate.rs       -> Trait definitions (interfaces)
adapter/auth_gate.rs    -> Concrete implementations using jsonwebtoken & reqwest
```

## Features

- **JWT Validation**: Local token verification using HMAC or RSA signatures
- **Google Workspace OAuth2**: Token validation via Google's tokeninfo endpoint
- **Service Account Tokens**: Internal service-to-service authentication
- **Composite Authentication**: Try multiple validators in sequence
- **Token Extraction**: Extract tokens from headers, query params, or cookies

## Domain Types

### AuthPrincipal

Represents an authenticated entity:

```rust
pub struct AuthPrincipal {
    pub subject: String,                      // User/service ID
    pub email: Option<String>,                // Email if available
    pub issuer: Option<String>,               // Token issuer
    pub audience: Option<String>,             // Token audience
    pub principal_type: PrincipalType,        // User, ServiceAccount, ApiKey, Anonymous
    pub claims: HashMap<String, serde_json::Value>,  // Additional claims
    pub expires_at: Option<i64>,              // Unix timestamp
}
```

### PrincipalType

```rust
pub enum PrincipalType {
    User,            // Regular user via OAuth2/OIDC
    ServiceAccount,  // Internal service
    ApiKey,          // API key authentication
    Anonymous,       // Unauthenticated
}
```

### AuthRequest

```rust
pub struct AuthRequest {
    pub token: String,
    pub token_type: Option<String>,
    pub metadata: HashMap<String, String>,
}
```

### TokenValidationConfig

```rust
pub struct TokenValidationConfig {
    pub expected_issuer: Option<String>,
    pub expected_audience: Option<String>,
    pub validate_expiration: bool,
    pub clock_skew_seconds: i64,
    pub required_claims: Vec<String>,
}
```

## Port Traits

### AuthGate

Main authentication interface:

```rust
#[async_trait]
pub trait AuthGate: Send + Sync {
    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthPrincipal, EdgeError>;
    async fn validate_token(&self, token: &str) -> Result<bool, EdgeError>;
    async fn authorize(&self, principal: &AuthPrincipal, resource: &str, action: &str) -> Result<bool, EdgeError>;
    fn validation_config(&self) -> &TokenValidationConfig;
}
```

### GoogleWorkspaceValidator

Specialized interface for Google OAuth2:

```rust
#[async_trait]
pub trait GoogleWorkspaceValidator: Send + Sync {
    async fn validate_google_token(&self, access_token: &str) -> Result<AuthPrincipal, EdgeError>;
    async fn validate_scopes(&self, access_token: &str, required_scopes: &[String]) -> Result<bool, EdgeError>;
}
```

### ServiceAccountValidator

For internal service authentication:

```rust
#[async_trait]
pub trait ServiceAccountValidator: Send + Sync {
    async fn validate_service_account(&self, token: &str) -> Result<AuthPrincipal, EdgeError>;
    async fn check_service_account_permission(&self, service_account_id: &str, action: &str) -> Result<bool, EdgeError>;
}
```

### TokenExtractor

Extract tokens from HTTP requests:

```rust
pub trait TokenExtractor: Send + Sync {
    fn extract_from_authorization(&self, authorization_header: &str) -> Option<String>;
    fn extract_from_query(&self, query_params: &HashMap<String, String>) -> Option<String>;
    fn extract_from_cookie(&self, cookie_header: &str) -> Option<String>;
}
```

## Adapter Implementations

### 1. JwtAuthGate

Validates JWT tokens using local signature verification.

**Features:**
- HMAC-SHA256 or RSA-SHA256 signatures
- Configurable issuer/audience validation
- Expiration checking with clock skew tolerance
- Support for custom claims

**Usage:**

```rust
// HMAC secret
let auth_gate = JwtAuthGate::new_with_secret(b"my-secret-key")
    .with_config(
        TokenValidationConfig::new()
            .with_issuer("https://auth.example.com".to_string())
            .with_audience("https://api.example.com".to_string())
    );

// RSA public key
let pem = std::fs::read("public_key.pem")?;
let auth_gate = JwtAuthGate::new_with_rsa_pem(&pem)?
    .with_config(config);

// Authenticate
let request = AuthRequest::new(token);
let principal = auth_gate.authenticate(&request).await?;
```

### 2. GoogleWorkspaceAuthGate

Validates Google OAuth2 access tokens by calling Google's tokeninfo endpoint.

**Features:**
- OAuth2 access token validation
- Email verification
- Scope validation
- Client ID verification

**Usage:**

```rust
let auth_gate = GoogleWorkspaceAuthGate::new()
    .with_client_id("123456789.apps.googleusercontent.com".to_string())
    .with_required_scopes(vec![
        "https://www.googleapis.com/auth/userinfo.email".to_string(),
    ]);

let principal = auth_gate.validate_google_token(access_token).await?;
```

**Google API Endpoint:**
- `GET https://oauth2.googleapis.com/tokeninfo?access_token={token}`

### 3. ServiceAccountAuthGate

Validates service account JWTs with additional permission checks.

**Features:**
- JWT validation for service accounts
- Allowlist of service account IDs
- Per-service-account permission management
- Principal type enforcement

**Usage:**

```rust
let auth_gate = ServiceAccountAuthGate::new_with_secret(secret)
    .with_allowed_service_account("backend@project.iam.gserviceaccount.com".to_string())
    .with_permissions(
        "backend@project.iam.gserviceaccount.com".to_string(),
        vec!["read".to_string(), "write".to_string()],
    );

let principal = auth_gate.validate_service_account(token).await?;
let can_write = auth_gate
    .check_service_account_permission(&principal.subject, "write")
    .await?;
```

### 4. CompositeAuthGate

Combines multiple validators, trying each in sequence.

**Features:**
- Try JWT first (fastest, local)
- Fall back to service account validation
- Fall back to Google OAuth2 (slowest, remote API call)
- Fail if no validator accepts the token

**Usage:**

```rust
let composite = CompositeAuthGate::builder()
    .with_jwt_validator(jwt_gate)
    .with_google_validator(google_gate)
    .with_service_account_validator(sa_gate)
    .with_config(config)
    .build();

let principal = composite.authenticate(&request).await?;
```

### 5. BearerTokenExtractor

Extracts bearer tokens from various sources.

**Usage:**

```rust
let extractor = BearerTokenExtractor;

// From Authorization header
let token = extractor.extract_from_authorization("Bearer abc123");

// From query parameters
let mut params = HashMap::new();
params.insert("access_token".to_string(), "abc123".to_string());
let token = extractor.extract_from_query(&params);

// From cookies
let token = extractor.extract_from_cookie("session=xyz; access_token=abc123");
```

## Integration with Axum

Example middleware integration:

```rust
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

async fn auth_middleware(
    State(auth_gate): State<Arc<dyn AuthGate>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract token
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| {
            let extractor = BearerTokenExtractor;
            extractor.extract_from_authorization(h)
        })
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Authenticate
    let auth_request = AuthRequest::new(token);
    let principal = auth_gate
        .authenticate(&auth_request)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Add principal to request extensions
    request.extensions_mut().insert(principal);

    Ok(next.run(request).await)
}
```

## Error Handling

All authentication operations return `Result<T, EdgeError>`:

```rust
pub enum EdgeError {
    Authentication(String),
    Authorization(String),
    TokenValidation(String),
    InvalidToken(String),
    TokenExpired,
    MissingClaim(String),
    InvalidIssuer { expected: String, actual: String },
    InvalidAudience { expected: String, actual: String },
    HttpClient(String),
    Configuration(String),
    Internal(String),
}
```

## Security Considerations

1. **Secret Management**: Never hardcode secrets. Use environment variables or secret managers.
2. **Token Expiration**: Always validate expiration with appropriate clock skew.
3. **HTTPS Only**: Tokens should only be transmitted over HTTPS.
4. **Scope Validation**: For OAuth2, validate scopes match required permissions.
5. **Audience Validation**: Always validate the audience claim to prevent token reuse.
6. **Key Rotation**: Support multiple decoding keys for smooth key rotation.

## Testing

The implementation includes unit tests for token extraction:

```bash
cargo test -p osiris-edge auth_gate
```

For integration testing with real tokens, see `examples/auth_gate_demo.rs`:

```bash
cargo run -p osiris-edge --example auth_gate_demo
```

## Dependencies

- `jsonwebtoken`: JWT encoding/decoding and validation
- `reqwest`: HTTP client for Google OAuth2 validation
- `serde`/`serde_json`: Serialization
- `async-trait`: Async trait support

## Future Enhancements

- [ ] JWKS (JSON Web Key Set) support for automatic key rotation
- [ ] Redis-based token revocation list
- [ ] Rate limiting per principal
- [ ] Audit logging for authentication events
- [ ] OpenID Connect Discovery support
- [ ] Multi-tenant support with per-tenant configurations
- [ ] Certificate-based authentication (mTLS)
