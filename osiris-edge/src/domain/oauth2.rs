//! OAuth2 PKCE domain types
//!
//! Core types for OAuth2 Proof Key for Public Clients Exchange (PKCE) flow
//! without external dependencies. Implements RFC 7636 for secure public client authentication.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OAuth2 PKCE code verifier
///
/// A cryptographically random string used to create the challenge.
/// Must be 43-128 characters using unreserved characters [A-Z / a-z / 0-9 / "-" / "." / "_" / "~"]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeVerifier {
    /// The raw verifier string
    pub value: String,

    /// Creation timestamp (Unix epoch seconds)
    pub created_at: i64,
}

impl CodeVerifier {
    /// Create a new code verifier with the given value
    pub fn new(value: String) -> Result<Self, CodeVerifierError> {
        if value.len() < 43 || value.len() > 128 {
            return Err(CodeVerifierError::InvalidLength {
                actual: value.len(),
                min: 43,
                max: 128,
            });
        }

        // Validate characters: [A-Z] [a-z] [0-9] "-" "." "_" "~"
        if !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~'))
        {
            return Err(CodeVerifierError::InvalidCharacters);
        }

        Ok(Self {
            value,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        })
    }

    /// Get the verifier value
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// OAuth2 PKCE code challenge
///
/// Created by applying SHA256 to the code verifier and base64url encoding the result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeChallenge {
    /// The base64url-encoded SHA256 hash of the code verifier
    pub value: String,

    /// Method used to generate the challenge ("S256" for SHA256, "plain" for plain text)
    pub method: ChallengeMethod,

    /// Creation timestamp (Unix epoch seconds)
    pub created_at: i64,
}

impl CodeChallenge {
    /// Create a SHA256 code challenge from a verifier
    pub fn sha256(verifier: &CodeVerifier) -> Self {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(verifier.as_str().as_bytes());
        let hash = hasher.finalize();

        // Base64url encode without padding
        let challenge = Self::base64url_encode(&hash);

        Self {
            value: challenge,
            method: ChallengeMethod::S256,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        }
    }

    /// Create a plain text code challenge (not recommended in production)
    pub fn plain(verifier: &CodeVerifier) -> Self {
        Self {
            value: verifier.value.clone(),
            method: ChallengeMethod::Plain,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        }
    }

    /// Encode bytes to base64url without padding
    fn base64url_encode(data: &[u8]) -> String {
        use std::fmt::Write;

        const BASE64URL_ALPHABET: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

        let mut result = String::new();
        let mut i = 0;

        while i < data.len() {
            let b1 = data[i];
            let b2 = if i + 1 < data.len() { data[i + 1] } else { 0 };
            let b3 = if i + 2 < data.len() { data[i + 2] } else { 0 };

            let has_b2 = i + 1 < data.len();
            let has_b3 = i + 2 < data.len();

            let idx1 = (b1 >> 2) as usize;
            let idx2 = (((b1 & 0x03) << 4) | (b2 >> 4)) as usize;
            let idx3 = if has_b2 {
                (((b2 & 0x0f) << 2) | (b3 >> 6)) as usize
            } else {
                0
            };
            let idx4 = if has_b3 { (b3 & 0x3f) as usize } else { 0 };

            let _ = write!(result, "{}", BASE64URL_ALPHABET[idx1] as char);
            let _ = write!(result, "{}", BASE64URL_ALPHABET[idx2] as char);
            if has_b2 {
                let _ = write!(result, "{}", BASE64URL_ALPHABET[idx3] as char);
            }
            if has_b3 {
                let _ = write!(result, "{}", BASE64URL_ALPHABET[idx4] as char);
            }

            i += 3;
        }

        result
    }

    /// Get the challenge value
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// Method used to generate the code challenge
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ChallengeMethod {
    /// SHA256 hash method (recommended)
    #[serde(rename = "S256")]
    S256,

    /// Plain text method (not recommended)
    #[serde(rename = "plain")]
    Plain,
}

impl std::fmt::Display for ChallengeMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::S256 => write!(f, "S256"),
            Self::Plain => write!(f, "plain"),
        }
    }
}

/// OAuth2 authorization request (first leg of PKCE flow)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationRequest {
    /// Client identifier
    pub client_id: String,

    /// Authorization server endpoint
    pub authorization_endpoint: String,

    /// Redirect URI where user is sent after authorization
    pub redirect_uri: String,

    /// Requested scopes (space-separated)
    pub scope: String,

    /// PKCE code challenge
    pub code_challenge: CodeChallenge,

    /// PKCE code verifier (stored securely for later use)
    pub code_verifier: CodeVerifier,

    /// State parameter for CSRF protection
    pub state: String,

    /// Additional parameters
    #[serde(default)]
    pub additional_params: HashMap<String, String>,
}

/// OAuth2 authorization response (redirect from auth server)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationResponse {
    /// Authorization code (exchanged for access token)
    pub code: String,

    /// State parameter (must match request state)
    pub state: String,

    /// Error code (if authorization failed)
    pub error: Option<String>,

    /// Error description
    pub error_description: Option<String>,

    /// URI for error details
    pub error_uri: Option<String>,
}

/// OAuth2 token request (second leg of PKCE flow)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRequest {
    /// Token endpoint URL
    pub token_endpoint: String,

    /// Client identifier
    pub client_id: String,

    /// Authorization code (from authorization response)
    pub code: String,

    /// Code verifier (must match the code challenge)
    pub code_verifier: String,

    /// Redirect URI (must match authorization request)
    pub redirect_uri: String,

    /// Optional client secret (for confidential clients)
    pub client_secret: Option<String>,

    /// Additional parameters
    #[serde(default)]
    pub additional_params: HashMap<String, String>,
}

/// OAuth2 token response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TokenResponse {
    /// The access token
    pub access_token: String,

    /// Token type (usually "Bearer")
    pub token_type: String,

    /// Seconds until token expiration
    pub expires_in: Option<i64>,

    /// Refresh token (if issued)
    pub refresh_token: Option<String>,

    /// Granted scopes (if different from requested)
    pub scope: Option<String>,

    /// Additional parameters from the server
    #[serde(flatten)]
    pub additional_params: HashMap<String, serde_json::Value>,
}

impl TokenResponse {
    /// Check if token is expired (with optional buffer)
    pub fn is_expired(&self, buffer_seconds: i64) -> bool {
        self.expires_in
            .map_or(false, |expires_in| expires_in <= buffer_seconds)
    }
}

/// OAuth2 refresh token request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshTokenRequest {
    /// Token endpoint URL
    pub token_endpoint: String,

    /// Client identifier
    pub client_id: String,

    /// Refresh token
    pub refresh_token: String,

    /// Optional client secret (for confidential clients)
    pub client_secret: Option<String>,

    /// Requested scopes (may be subset of original)
    pub scope: Option<String>,

    /// Additional parameters
    #[serde(default)]
    pub additional_params: HashMap<String, String>,
}

/// OAuth2 session storing credentials and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Oauth2Session {
    /// Session identifier
    pub session_id: String,

    /// Access token
    pub access_token: String,

    /// Token type
    pub token_type: String,

    /// Expiration timestamp (Unix epoch seconds)
    pub expires_at: Option<i64>,

    /// Refresh token (if available)
    pub refresh_token: Option<String>,

    /// Granted scopes
    pub scope: String,

    /// Session creation timestamp
    pub created_at: i64,

    /// Last refresh timestamp
    pub last_refreshed_at: Option<i64>,

    /// Additional claims/metadata
    pub claims: HashMap<String, serde_json::Value>,
}

impl Oauth2Session {
    /// Check if token is expired (with buffer for safety)
    pub fn is_expired(&self, buffer_seconds: i64) -> bool {
        self.expires_at.map_or(false, |expires_at| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            now >= expires_at - buffer_seconds
        })
    }

    /// Check if token can be refreshed
    pub fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }
}

/// Code verifier error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeVerifierError {
    /// Invalid length (must be 43-128 characters)
    InvalidLength {
        actual: usize,
        min: usize,
        max: usize,
    },

    /// Invalid characters (must be [A-Z][a-z][0-9]-._~)
    InvalidCharacters,
}

impl std::fmt::Display for CodeVerifierError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLength { actual, min, max } => {
                write!(
                    f,
                    "Invalid code verifier length: {} (must be {}-{})",
                    actual, min, max
                )
            }
            Self::InvalidCharacters => {
                write!(f, "Invalid characters in code verifier")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_verifier_creation() {
        let verifier = CodeVerifier::new("a".repeat(43)).unwrap();
        assert_eq!(verifier.value.len(), 43);
    }

    #[test]
    fn test_code_verifier_too_short() {
        let result = CodeVerifier::new("a".repeat(42));
        assert!(matches!(
            result,
            Err(CodeVerifierError::InvalidLength { .. })
        ));
    }

    #[test]
    fn test_code_verifier_too_long() {
        let result = CodeVerifier::new("a".repeat(129));
        assert!(matches!(
            result,
            Err(CodeVerifierError::InvalidLength { .. })
        ));
    }

    #[test]
    fn test_code_verifier_invalid_chars() {
        let result = CodeVerifier::new("a".repeat(42) + "!");
        assert_eq!(result, Err(CodeVerifierError::InvalidCharacters));
    }

    #[test]
    fn test_code_challenge_sha256() {
        let verifier = CodeVerifier::new("a".repeat(43)).unwrap();
        let challenge = CodeChallenge::sha256(&verifier);
        assert_eq!(challenge.method, ChallengeMethod::S256);
        assert!(!challenge.value.is_empty());
    }

    #[test]
    fn test_code_challenge_plain() {
        let verifier = CodeVerifier::new("a".repeat(43)).unwrap();
        let challenge = CodeChallenge::plain(&verifier);
        assert_eq!(challenge.method, ChallengeMethod::Plain);
        assert_eq!(challenge.value, verifier.value);
    }

    #[test]
    fn test_oauth2_session_expiration() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let expired_session = Oauth2Session {
            session_id: "test".to_string(),
            access_token: "token".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: Some(now - 100),
            refresh_token: None,
            scope: "read".to_string(),
            created_at: now - 1000,
            last_refreshed_at: None,
            claims: HashMap::new(),
        };

        assert!(expired_session.is_expired(0));
    }

    #[test]
    fn test_oauth2_session_not_expired() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let valid_session = Oauth2Session {
            session_id: "test".to_string(),
            access_token: "token".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: Some(now + 3600),
            refresh_token: Some("refresh".to_string()),
            scope: "read".to_string(),
            created_at: now,
            last_refreshed_at: None,
            claims: HashMap::new(),
        };

        assert!(!valid_session.is_expired(60));
        assert!(valid_session.can_refresh());
    }
}
