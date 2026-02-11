//! Token service for JWT generation and refresh

#[cfg(feature = "auth")]
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
#[cfg(feature = "auth")]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    domain::{A2AError, core::agent::SecurityScheme},
    port::authenticator::{AuthPrincipal},
};

#[cfg(test)]
#[cfg(feature = "auth")]
mod tests;

/// Token generation response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// Token refresh request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRefreshRequest {
    pub refresh_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// Token generation request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRequest {
    pub grant_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
}

/// User info for OpenID Connect
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub sub: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub given_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_name: Option<String>,
}

/// Authorization URL response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationUrlResponse {
    pub url: String,
    pub csrf_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

/// Token service for JWT generation and validation
#[cfg(feature = "auth")]
#[derive(Clone)]
pub struct TokenService {
    /// Encoding key for JWT signing
    encoding_key: EncodingKey,
    /// Decoding key for JWT verification
    decoding_key: DecodingKey,
    /// Token expiration time in seconds (default 1 hour)
    expiration_secs: i64,
    /// Refresh token expiration time in seconds (default 30 days)
    refresh_expiration_secs: i64,
    /// Issuer claim
    issuer: Option<String>,
    /// Audience claim
    audience: Option<String>,
    /// Algorithm
    algorithm: jsonwebtoken::Algorithm,
}

#[cfg(feature = "auth")]
impl TokenService {
    /// Create a new token service with HMAC secret
    pub fn new_with_secret(secret: &[u8]) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            expiration_secs: 3600, // 1 hour
            refresh_expiration_secs: 2_592_000, // 30 days
            issuer: None,
            audience: None,
            algorithm: jsonwebtoken::Algorithm::HS256,
        }
    }

    /// Create with RSA key pair
    pub fn new_with_rsa(private_pem: &[u8], public_pem: &[u8]) -> Result<Self, A2AError> {
        let encoding_key = EncodingKey::from_rsa_pem(private_pem)
            .map_err(|e| A2AError::Internal(format!("Invalid RSA private key: {}", e)))?;
        let decoding_key = DecodingKey::from_rsa_pem(public_pem)
            .map_err(|e| A2AError::Internal(format!("Invalid RSA public key: {}", e)))?;

        Ok(Self {
            encoding_key,
            decoding_key,
            expiration_secs: 3600,
            refresh_expiration_secs: 2_592_000,
            issuer: None,
            audience: None,
            algorithm: jsonwebtoken::Algorithm::RS256,
        })
    }

    /// Set token expiration time
    pub fn with_expiration(mut self, secs: i64) -> Self {
        self.expiration_secs = secs;
        self
    }

    /// Set refresh token expiration time
    pub fn with_refresh_expiration(mut self, secs: i64) -> Self {
        self.refresh_expiration_secs = secs;
        self
    }

    /// Set issuer claim
    pub fn with_issuer(mut self, issuer: String) -> Self {
        self.issuer = Some(issuer);
        self
    }

    /// Set audience claim
    pub fn with_audience(mut self, audience: String) -> Self {
        self.audience = Some(audience);
        self
    }

    /// Get current time as Unix timestamp
    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    /// Generate an access token for the given principal
    pub fn generate_token(&self, principal: &AuthPrincipal) -> Result<TokenResponse, A2AError> {
        let now = self.now();
        let exp = now + self.expiration_secs;

        let mut claims = HashMap::new();
        claims.insert("sub".to_string(), serde_json::Value::String(principal.id.clone()));
        claims.insert("exp".to_string(), serde_json::Value::Number(exp.into()));
        claims.insert("iat".to_string(), serde_json::Value::Number(now.into()));

        if let Some(iss) = &self.issuer {
            claims.insert("iss".to_string(), serde_json::Value::String(iss.clone()));
        }

        if let Some(aud) = &self.audience {
            claims.insert("aud".to_string(), serde_json::Value::String(aud.clone()));
        }

        // Add principal attributes as claims
        for (key, value) in &principal.attributes {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(value) {
                claims.insert(key.clone(), json_val);
            } else {
                claims.insert(key.clone(), serde_json::Value::String(value.clone()));
            }
        }

        let header = Header::new(self.algorithm);
        let token = encode(&header, &claims, &self.encoding_key)
            .map_err(|e| A2AError::Internal(format!("Token encoding failed: {}", e)))?;

        Ok(TokenResponse {
            access_token: token,
            token_type: "Bearer".to_string(),
            expires_in: self.expiration_secs,
            refresh_token: None,
            scope: None,
        })
    }

    /// Generate a token with refresh token
    pub fn generate_token_with_refresh(
        &self,
        principal: &AuthPrincipal,
    ) -> Result<TokenResponse, A2AError> {
        let access_response = self.generate_token(principal)?;

        let now = self.now();
        let exp = now + self.refresh_expiration_secs;

        let refresh_claims = serde_json::json!({
            "sub": principal.id,
            "exp": exp,
            "iat": now,
            "type": "refresh"
        });

        let header = Header::new(self.algorithm);
        let refresh_token = encode(&header, &refresh_claims, &self.encoding_key)
            .map_err(|e| A2AError::Internal(format!("Refresh token encoding failed: {}", e)))?;

        Ok(TokenResponse {
            access_token: access_response.access_token,
            token_type: access_response.token_type,
            expires_in: access_response.expires_in,
            refresh_token: Some(refresh_token),
            scope: None,
        })
    }

    /// Refresh an access token using a refresh token
    pub fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<TokenResponse, A2AError> {
        let validation = Validation::new(self.algorithm);
        let token_data = decode::<serde_json::Value>(
            refresh_token,
            &self.decoding_key,
            &validation,
        )
            .map_err(|e| A2AError::Internal(format!("Invalid refresh token: {}", e)))?;

        // Verify this is a refresh token
        let token_type = token_data
            .claims
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("access");

        if token_type != "refresh" {
            return Err(A2AError::Internal(
                "Invalid token type: expected refresh token".to_string(),
            ));
        }

        // Extract subject
        let sub = token_data
            .claims
            .get("sub")
            .and_then(|v| v.as_str())
            .ok_or_else(|| A2AError::Internal("Missing subject in refresh token".to_string()))?;

        // Create a principal from the refresh token
        let principal = AuthPrincipal::new(sub.to_string(), "refresh".to_string());

        self.generate_token_with_refresh(&principal)
    }

    /// Validate a token and return the principal
    pub fn validate_token(&self, token: &str) -> Result<AuthPrincipal, A2AError> {
        let mut validation = Validation::new(self.algorithm);

        if let Some(iss) = &self.issuer {
            validation.iss = Some(std::collections::HashSet::from([iss.clone()]));
        }

        if let Some(aud) = &self.audience {
            validation.aud = Some(std::collections::HashSet::from([aud.clone()]));
        }

        let token_data = decode::<serde_json::Value>(token, &self.decoding_key, &validation)
            .map_err(|e| A2AError::Internal(format!("Token validation failed: {}", e)))?;

        let sub = token_data
            .claims
            .get("sub")
            .and_then(|v| v.as_str())
            .ok_or_else(|| A2AError::Internal("Missing subject in token".to_string()))?;

        let mut principal = AuthPrincipal::new(sub.to_string(), "jwt".to_string());

        // Extract additional claims
        if let Some(obj) = token_data.claims.as_object() {
            for (key, value) in obj {
                if key != "sub" && key != "exp" && key != "iat" && key != "iss" && key != "aud" && key != "type" {
                    if let Ok(str_val) = serde_json::to_string(value) {
                        principal = principal.with_attribute(key.clone(), str_val);
                    }
                }
            }
        }

        Ok(principal)
    }

    /// Get user info from token (OpenID Connect)
    pub fn get_user_info(&self, token: &str) -> Result<UserInfo, A2AError> {
        let principal = self.validate_token(token)?;

        Ok(UserInfo {
            sub: principal.id,
            name: principal.attributes.get("name").cloned(),
            email: principal.attributes.get("email").cloned(),
            email_verified: principal
                .attributes
                .get("email_verified")
                .and_then(|v| v.parse::<bool>().ok()),
            picture: principal.attributes.get("picture").cloned(),
            given_name: principal.attributes.get("given_name").cloned(),
            family_name: principal.attributes.get("family_name").cloned(),
        })
    }

    /// Get security scheme for this token service
    pub fn security_scheme(&self) -> SecurityScheme {
        SecurityScheme::Http {
            scheme: "bearer".to_string(),
            bearer_format: Some("JWT".to_string()),
            description: Some("JWT Bearer token authentication".to_string()),
        }
    }
}

/// OAuth2 authorization URL generator
#[cfg(feature = "auth")]
pub struct AuthorizationUrlGenerator {
    pub authorization_url: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub state: Option<String>,
}

#[cfg(feature = "auth")]
impl AuthorizationUrlGenerator {
    pub fn new(
        authorization_url: String,
        client_id: String,
        redirect_uri: String,
        scopes: Vec<String>,
    ) -> Self {
        Self {
            authorization_url,
            client_id,
            redirect_uri,
            scopes,
            state: None,
        }
    }

    pub fn with_state(mut self, state: String) -> Self {
        self.state = Some(state);
        self
    }

    pub fn generate(&self) -> Result<AuthorizationUrlResponse, A2AError> {
        use oauth2::{AuthUrl, ClientId, CsrfToken, RedirectUrl, Scope};

        let auth_url = AuthUrl::new(self.authorization_url.clone())
            .map_err(|e| A2AError::Internal(format!("Invalid authorization URL: {}", e)))?;
        let redirect_url =
            RedirectUrl::new(self.redirect_uri.clone())
                .map_err(|e| A2AError::Internal(format!("Invalid redirect URL: {}", e)))?;

        let client = oauth2::basic::BasicClient::new(
            ClientId::new(self.client_id.clone()),
            None,
            auth_url,
            None,
        )
        .set_redirect_uri(redirect_url);

        let (url, csrf_token) = client
            .authorize_url(CsrfToken::new_random)
            .add_scopes(self.scopes.iter().map(|s| Scope::new(s.clone())))
            .url();

        Ok(AuthorizationUrlResponse {
            url: url.to_string(),
            csrf_token: csrf_token.secret().to_string(),
            nonce: None,
        })
    }
}

#[cfg(not(feature = "auth"))]
/// Placeholder when auth feature is not enabled
pub struct TokenService;

#[cfg(not(feature = "auth"))]
impl TokenService {
    pub fn new_with_secret(_secret: &[u8]) -> Self {
        compile_error!("Token service requires the 'auth' feature");
    }
}
