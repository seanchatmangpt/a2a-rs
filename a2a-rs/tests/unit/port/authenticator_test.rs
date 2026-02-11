//! Unit tests for Authenticator port traits
//!
//! Tests the contract and behavior of the Authenticator, AuthContextExtractor,
//! and CompositeAuthenticator port traits using mock implementations.

use a2a_rs::domain::core::agent::SecurityScheme;
use a2a_rs::domain::error::A2AError;
use a2a_rs::port::authenticator::{
    AuthContext, AuthContextExtractor, AuthPrincipal, Authenticator, CompositeAuthenticator,
};
use async_trait::async_trait;
use std::collections::HashMap;

/// Mock implementation of Authenticator for testing
#[derive(Debug, Clone)]
struct MockAuthenticator {
    scheme: SecurityScheme,
    valid_credentials: HashMap<String, String>, // credential -> principal_id
}

impl MockAuthenticator {
    fn new(scheme: SecurityScheme) -> Self {
        Self {
            scheme,
            valid_credentials: HashMap::new(),
        }
    }

    fn with_credential(mut self, credential: String, principal_id: String) -> Self {
        self.valid_credentials.insert(credential, principal_id);
        self
    }
}

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn authenticate(&self, context: &AuthContext) -> Result<AuthPrincipal, A2AError> {
        self.validate_context(context)?;

        let principal_id = self
            .valid_credentials
            .get(&context.credential)
            .ok_or_else(|| {
                A2AError::InvalidRequest("Invalid credentials".to_string())
            })?;

        Ok(AuthPrincipal::new(principal_id.clone(), context.scheme_type.clone()))
    }

    fn security_scheme(&self) -> &SecurityScheme {
        &self.scheme
    }

    fn validate_context(&self, context: &AuthContext) -> Result<(), A2AError> {
        if context.scheme_type != self.scheme.scheme_type {
            return Err(A2AError::InvalidRequest(format!(
                "Invalid scheme type: expected {:?}, got {}",
                self.scheme.scheme_type, context.scheme_type
            )));
        }
        Ok(())
    }
}

/// Mock implementation of AuthContextExtractor for testing
#[derive(Debug, Clone)]
struct MockAuthContextExtractor;

#[async_trait]
impl AuthContextExtractor for MockAuthContextExtractor {
    #[cfg(feature = "http-server")]
    async fn extract_from_headers(&self, headers: &axum::http::HeaderMap) -> Option<AuthContext> {
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .map(|auth_value| {
                let parts: Vec<&str> = auth_value.splitn(2, ' ').collect();
                AuthContext {
                    scheme_type: parts.get(0).unwrap_or(&"").to_string(),
                    credential: parts.get(1).unwrap_or(&"").to_string(),
                    metadata: HashMap::new(),
                }
            })
    }

    #[cfg(not(feature = "http-server"))]
    async fn extract_from_headers(
        &self,
        headers: &HashMap<String, String>,
    ) -> Option<AuthContext> {
        headers.get("authorization").map(|auth_value| {
            let parts: Vec<&str> = auth_value.splitn(2, ' ').collect();
            AuthContext {
                scheme_type: parts.get(0).unwrap_or(&"").to_string(),
                credential: parts.get(1).unwrap_or(&"").to_string(),
                metadata: HashMap::new(),
            }
        })
    }

    async fn extract_from_query(&self, params: &HashMap<String, String>) -> Option<AuthContext> {
        params.get("token").map(|token| AuthContext {
            scheme_type: "bearer".to_string(),
            credential: token.clone(),
            metadata: HashMap::new(),
        })
    }

    async fn extract_from_cookies(&self, cookies: &str) -> Option<AuthContext> {
        cookies
            .split(';')
            .find_map(|cookie| {
                let parts: Vec<&str> = cookie.trim().splitn(2, '=').collect();
                if parts.get(0) == Some(&"session_token") {
                    Some(AuthContext {
                        scheme_type: "cookie".to_string(),
                        credential: parts.get(1).unwrap_or(&"").to_string(),
                        metadata: HashMap::new(),
                    })
                } else {
                    None
                }
            })
    }
}

/// Mock implementation of CompositeAuthenticator for testing
struct MockCompositeAuthenticator {
    authenticators: Vec<Box<dyn Authenticator + Send + Sync>>,
    schemes: Vec<SecurityScheme>,
}

impl MockCompositeAuthenticator {
    fn new() -> Self {
        Self {
            authenticators: Vec::new(),
            schemes: Vec::new(),
        }
    }

    fn add_authenticator(
        mut self,
        authenticator: Box<dyn Authenticator + Send + Sync>,
        scheme: SecurityScheme,
    ) -> Self {
        self.schemes.push(scheme);
        self.authenticators.push(authenticator);
        self
    }
}

#[async_trait]
impl CompositeAuthenticator for MockCompositeAuthenticator {
    async fn authenticate_any(&self, contexts: Vec<AuthContext>) -> Result<AuthPrincipal, A2AError> {
        for authenticator in &self.authenticators {
            for context in &contexts {
                if let Ok(principal) = authenticator.authenticate(context).await {
                    return Ok(principal);
                }
            }
        }
        Err(A2AError::InvalidRequest(
            "No valid authentication method found".to_string(),
        ))
    }

    fn supported_schemes(&self) -> Vec<&SecurityScheme> {
        self.schemes.iter().collect()
    }
}

// ============== Test Cases ==============

#[tokio::test]
async fn test_auth_context_new() {
    let context = AuthContext::new("bearer".to_string(), "token123".to_string());

    assert_eq!(context.scheme_type, "bearer");
    assert_eq!(context.credential, "token123");
    assert!(context.metadata.is_empty());
}

#[tokio::test]
async fn test_auth_context_with_metadata() {
    let context = AuthContext::new("apikey".to_string(), "key456".to_string())
        .with_metadata("location".to_string(), "header".to_string())
        .with_metadata("service".to_string(), "test".to_string());

    assert_eq!(context.get_metadata("location"), Some(&"header".to_string()));
    assert_eq!(context.get_metadata("service"), Some(&"test".to_string()));
    assert_eq!(context.get_metadata("nonexistent"), None);
}

#[tokio::test]
async fn test_auth_principal_new() {
    let principal = AuthPrincipal::new("user123".to_string(), "bearer".to_string());

    assert_eq!(principal.id, "user123");
    assert_eq!(principal.scheme, "bearer");
    assert!(principal.attributes.is_empty());
}

#[tokio::test]
async fn test_auth_principal_with_attribute() {
    let principal = AuthPrincipal::new("user456".to_string(), "apikey".to_string())
        .with_attribute("role".to_string(), "admin".to_string())
        .with_attribute("scope".to_string(), "read:write".to_string());

    assert_eq!(principal.attributes.get("role"), Some(&"admin".to_string()));
    assert_eq!(
        principal.attributes.get("scope"),
        Some(&"read:write".to_string())
    );
}

#[tokio::test]
async fn test_authenticator_success() {
    let scheme = SecurityScheme {
        scheme_type: "bearer".to_string(),
        description: Some("Bearer token auth".to_string()),
        name: None,
        in_: None,
        required: None,
    };

    let authenticator = MockAuthenticator::new(scheme).with_credential(
        "valid-token".to_string(),
        "user123".to_string(),
    );

    let context = AuthContext::new("bearer".to_string(), "valid-token".to_string());
    let result = authenticator.authenticate(&context).await;

    assert!(result.is_ok());
    let principal = result.unwrap();
    assert_eq!(principal.id, "user123");
    assert_eq!(principal.scheme, "bearer");
}

#[tokio::test]
async fn test_authenticator_invalid_credential() {
    let scheme = SecurityScheme {
        scheme_type: "bearer".to_string(),
        description: Some("Bearer token auth".to_string()),
        name: None,
        in_: None,
        required: None,
    };

    let authenticator = MockAuthenticator::new(scheme);

    let context = AuthContext::new("bearer".to_string(), "invalid-token".to_string());
    let result = authenticator.authenticate(&context).await;

    assert!(matches!(result, Err(A2AError::InvalidRequest(_))));
}

#[tokio::test]
async fn test_authenticator_invalid_scheme() {
    let scheme = SecurityScheme {
        scheme_type: "bearer".to_string(),
        description: Some("Bearer token auth".to_string()),
        name: None,
        in_: None,
        required: None,
    };

    let authenticator = MockAuthenticator::new(scheme);

    let context = AuthContext::new("apikey".to_string(), "some-key".to_string());
    let result = authenticator.validate_context(&context);

    assert!(matches!(result, Err(A2AError::InvalidRequest(_))));
    assert!(result.unwrap_err().to_string().contains("Invalid scheme type"));
}

#[tokio::test]
async fn test_authenticator_security_scheme() {
    let scheme = SecurityScheme {
        scheme_type: "apikey".to_string(),
        description: Some("API key authentication".to_string()),
        name: Some("X-API-Key".to_string()),
        in_: Some("header".to_string()),
        required: Some(true),
    };

    let authenticator = MockAuthenticator::new(scheme.clone());

    let returned_scheme = authenticator.security_scheme();
    assert_eq!(returned_scheme.scheme_type, "apikey");
    assert_eq!(returned_scheme.name, Some(&"X-API-Key".to_string()));
}

#[tokio::test]
async fn test_context_extractor_from_headers() {
    let extractor = MockAuthContextExtractor;

    #[cfg(feature = "http-server")]
    {
        use axum::http::HeaderValue;

        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer token123"),
        );

        let result = extractor.extract_from_headers(&headers).await;

        assert!(result.is_some());
        let context = result.unwrap();
        assert_eq!(context.scheme_type, "Bearer");
        assert_eq!(context.credential, "token123");
    }

    #[cfg(not(feature = "http-server"))]
    {
        let mut headers = HashMap::new();
        headers.insert("authorization".to_string(), "Bearer token123".to_string());

        let result = extractor.extract_from_headers(&headers).await;

        assert!(result.is_some());
        let context = result.unwrap();
        assert_eq!(context.scheme_type, "Bearer");
        assert_eq!(context.credential, "token123");
    }
}

#[tokio::test]
async fn test_context_extractor_from_headers_missing() {
    let extractor = MockAuthContextExtractor;

    #[cfg(feature = "http-server")]
    {
        let headers = axum::http::HeaderMap::new();
        let result = extractor.extract_from_headers(&headers).await;
        assert!(result.is_none());
    }

    #[cfg(not(feature = "http-server"))]
    {
        let headers = HashMap::new();
        let result = extractor.extract_from_headers(&headers).await;
        assert!(result.is_none());
    }
}

#[tokio::test]
async fn test_context_extractor_from_query() {
    let extractor = MockAuthContextExtractor;

    let mut params = HashMap::new();
    params.insert("token".to_string(), "query-token-456".to_string());

    let result = extractor.extract_from_query(&params).await;

    assert!(result.is_some());
    let context = result.unwrap();
    assert_eq!(context.scheme_type, "bearer");
    assert_eq!(context.credential, "query-token-456");
}

#[tokio::test]
async fn test_context_extractor_from_query_missing() {
    let extractor = MockAuthContextExtractor;
    let params = HashMap::new();

    let result = extractor.extract_from_query(&params).await;

    assert!(result.is_none());
}

#[tokio::test]
async fn test_context_extractor_from_cookies() {
    let extractor = MockAuthContextExtractor;

    let cookies = "session_id=abc123; session_token=secret789; other=value";

    let result = extractor.extract_from_cookies(cookies).await;

    assert!(result.is_some());
    let context = result.unwrap();
    assert_eq!(context.scheme_type, "cookie");
    assert_eq!(context.credential, "secret789");
}

#[tokio::test]
async fn test_context_extractor_from_cookies_missing() {
    let extractor = MockAuthContextExtractor;

    let cookies = "session_id=abc123; other=value";

    let result = extractor.extract_from_cookies(cookies).await;

    assert!(result.is_none());
}

#[tokio::test]
async fn test_composite_authenticator_success() {
    let bearer_scheme = SecurityScheme {
        scheme_type: "bearer".to_string(),
        description: None,
        name: None,
        in_: None,
        required: None,
    };

    let apikey_scheme = SecurityScheme {
        scheme_type: "apikey".to_string(),
        description: None,
        name: None,
        in_: None,
        required: None,
    };

    let bearer_auth = Box::new(
        MockAuthenticator::new(bearer_scheme.clone())
            .with_credential("valid-bearer".to_string(), "user1".to_string()),
    ) as Box<dyn Authenticator + Send + Sync>;

    let apikey_auth = Box::new(
        MockAuthenticator::new(apikey_scheme.clone())
            .with_credential("valid-apikey".to_string(), "user2".to_string()),
    ) as Box<dyn Authenticator + Send + Sync>;

    let composite = MockCompositeAuthenticator::new()
        .add_authenticator(bearer_auth, bearer_scheme)
        .add_authenticator(apikey_auth, apikey_scheme);

    let contexts = vec![
        AuthContext::new("bearer".to_string(), "valid-bearer".to_string()),
        AuthContext::new("apikey".to_string(), "valid-apikey".to_string()),
    ];

    let result = composite.authenticate_any(contexts).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_composite_authenticator_no_valid_method() {
    let scheme = SecurityScheme {
        scheme_type: "bearer".to_string(),
        description: None,
        name: None,
        in_: None,
        required: None,
    };

    let auth = Box::new(MockAuthenticator::new(scheme)) as Box<dyn Authenticator + Send + Sync>;

    let composite = MockCompositeAuthenticator::new()
        .add_authenticator(auth, SecurityScheme {
            scheme_type: "bearer".to_string(),
            description: None,
            name: None,
            in_: None,
            required: None,
        });

    let contexts = vec![AuthContext::new(
        "bearer".to_string(),
        "invalid-token".to_string(),
    )];

    let result = composite.authenticate_any(contexts).await;

    assert!(matches!(result, Err(A2AError::InvalidRequest(_))));
}

#[tokio::test]
async fn test_composite_authenticator_supported_schemes() {
    let bearer_scheme = SecurityScheme {
        scheme_type: "bearer".to_string(),
        description: Some("Bearer auth".to_string()),
        name: None,
        in_: None,
        required: None,
    };

    let apikey_scheme = SecurityScheme {
        scheme_type: "apikey".to_string(),
        description: Some("API key auth".to_string()),
        name: None,
        in_: None,
        required: None,
    };

    let auth1 = Box::new(MockAuthenticator::new(bearer_scheme.clone()))
        as Box<dyn Authenticator + Send + Sync>;
    let auth2 = Box::new(MockAuthenticator::new(apikey_scheme.clone()))
        as Box<dyn Authenticator + Send + Sync>;

    let composite = MockCompositeAuthenticator::new()
        .add_authenticator(auth1, bearer_scheme)
        .add_authenticator(auth2, apikey_scheme);

    let schemes = composite.supported_schemes();

    assert_eq!(schemes.len(), 2);
    assert_eq!(schemes[0].scheme_type, "bearer");
    assert_eq!(schemes[1].scheme_type, "apikey");
}

#[tokio::test]
async fn test_authenticate_with_metadata() {
    let scheme = SecurityScheme {
        scheme_type: "oauth2".to_string(),
        description: None,
        name: None,
        in_: None,
        required: None,
    };

    let authenticator = MockAuthenticator::new(scheme).with_credential(
        "oauth-token".to_string(),
        "user3".to_string(),
    );

    let mut metadata = HashMap::new();
    metadata.insert("scope".to_string(), "read:write".to_string());

    let context = AuthContext {
        scheme_type: "oauth2".to_string(),
        credential: "oauth-token".to_string(),
        metadata,
    };

    let result = authenticator.authenticate(&context).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_multiple_authenticators_different_schemes() {
    let bearer_scheme = SecurityScheme {
        scheme_type: "bearer".to_string(),
        description: None,
        name: None,
        in_: None,
        required: None,
    };

    let apikey_scheme = SecurityScheme {
        scheme_type: "apikey".to_string(),
        description: None,
        name: None,
        in_: None,
        required: None,
    };

    let bearer_auth = MockAuthenticator::new(bearer_scheme).with_credential(
        "bearer-token".to_string(),
        "user-bearer".to_string(),
    );

    let apikey_auth = MockAuthenticator::new(apikey_scheme).with_credential(
        "api-key".to_string(),
        "user-apikey".to_string(),
    );

    // Test bearer
    let bearer_context = AuthContext::new("bearer".to_string(), "bearer-token".to_string());
    let result = bearer_auth.authenticate(&bearer_context).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().id, "user-bearer");

    // Test apikey
    let apikey_context = AuthContext::new("apikey".to_string(), "api-key".to_string());
    let result = apikey_auth.authenticate(&apikey_context).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().id, "user-apikey");
}
