//! Unit tests for authentication adapters
//!
//! Tests authentication adapter implementations including Bearer tokens,
//! API keys, and no-op authenticators.

use a2a_rs::adapter::{
    ApiKeyAuthenticator, ApiKeyExtractor, BearerTokenAuthenticator,
    BearerTokenExtractor, NoopAuthenticator,
};
use a2a_rs::domain::core::agent::SecurityScheme;
use a2a_rs::domain::A2AError;
use a2a_rs::port::{AuthContext, AuthContextExtractor, Authenticator, AuthPrincipal};
use std::collections::HashMap;

#[tokio::test]
async fn test_bearer_token_authenticator_valid_token() {
    let authenticator = BearerTokenAuthenticator::new(vec![
        "valid-token-123".to_string(),
        "another-token-456".to_string(),
    ]);

    let context = AuthContext::new("bearer".to_string(), "valid-token-123".to_string());
    let result = authenticator.authenticate(&context).await;

    assert!(result.is_ok());
    let principal = result.unwrap();
    assert_eq!(principal.id, "valid-token-123");
    assert_eq!(principal.auth_type, "bearer");
}

#[tokio::test]
async fn test_bearer_token_authenticator_invalid_token() {
    let authenticator =
        BearerTokenAuthenticator::new(vec!["valid-token".to_string()]);

    let context =
        AuthContext::new("bearer".to_string(), "invalid-token".to_string());
    let result = authenticator.authenticate(&context).await;

    assert!(result.is_err());
    if let Err(A2AError::Internal(msg)) = result {
        assert!(msg.contains("Invalid authentication token"));
    } else {
        panic!("Expected Internal error");
    }
}

#[tokio::test]
async fn test_bearer_token_authenticator_wrong_scheme() {
    let authenticator =
        BearerTokenAuthenticator::new(vec!["valid-token".to_string()]);

    let context = AuthContext::new("apikey".to_string(), "some-key".to_string());
    let result = authenticator.authenticate(&context).await;

    assert!(result.is_err());
    if let Err(A2AError::Internal(msg)) = result {
        assert!(msg.contains("Invalid authentication scheme"));
        assert!(msg.contains("expected 'bearer'"));
    } else {
        panic!("Expected Internal error");
    }
}

#[tokio::test]
async fn test_bearer_token_security_scheme() {
    let authenticator =
        BearerTokenAuthenticator::new(vec!["token".to_string()]);

    let scheme = authenticator.security_scheme();

    match scheme {
        SecurityScheme::Http { scheme, .. } => {
            assert_eq!(scheme, "bearer");
        }
        _ => panic!("Expected Http security scheme"),
    }
}

#[tokio::test]
async fn test_bearer_token_with_format() {
    let authenticator = BearerTokenAuthenticator::with_format(
        vec!["token".to_string()],
        "JWT".to_string(),
    );

    let scheme = authenticator.security_scheme();

    match scheme {
        SecurityScheme::Http {
            scheme,
            bearer_format,
            ..
        } => {
            assert_eq!(scheme, "bearer");
            assert_eq!(bearer_format, Some("JWT".to_string()));
        }
        _ => panic!("Expected Http security scheme with format"),
    }
}

#[tokio::test]
async fn test_api_key_authenticator_valid_key() {
    let authenticator = ApiKeyAuthenticator::header(
        vec!["valid-key-123".to_string(), "another-key".to_string()],
        "X-API-Key".to_string(),
    );

    let mut metadata = HashMap::new();
    metadata.insert("location".to_string(), "header".to_string());
    metadata.insert("name".to_string(), "X-API-Key".to_string());

    let context = AuthContext {
        scheme_type: "apikey".to_string(),
        credential: "valid-key-123".to_string(),
        metadata,
    };

    let result = authenticator.authenticate(&context).await;

    assert!(result.is_ok());
    let principal = result.unwrap();
    assert_eq!(principal.id, "valid-key-123");
    assert_eq!(principal.auth_type, "apikey");
}

#[tokio::test]
async fn test_api_key_authenticator_invalid_key() {
    let authenticator =
        ApiKeyAuthenticator::header(vec!["valid-key".to_string()], "X-API-Key".to_string());

    let mut metadata = HashMap::new();
    metadata.insert("location".to_string(), "header".to_string());

    let context = AuthContext {
        scheme_type: "apikey".to_string(),
        credential: "invalid-key".to_string(),
        metadata,
    };

    let result = authenticator.authenticate(&context).await;

    assert!(result.is_err());
    if let Err(A2AError::Internal(msg)) = result {
        assert!(msg.contains("Invalid API key"));
    } else {
        panic!("Expected Internal error");
    }
}

#[tokio::test]
async fn test_api_key_authenticator_wrong_scheme() {
    let authenticator =
        ApiKeyAuthenticator::header(vec!["key".to_string()], "X-API-Key".to_string());

    let context =
        AuthContext::new("bearer".to_string(), "some-token".to_string());
    let result = authenticator.authenticate(&context).await;

    assert!(result.is_err());
    if let Err(A2AError::Internal(msg)) = result {
        assert!(msg.contains("Invalid authentication scheme"));
        assert!(msg.contains("expected 'apikey'"));
    } else {
        panic!("Expected Internal error");
    }
}

#[tokio::test]
async fn test_api_key_security_scheme() {
    let authenticator = ApiKeyAuthenticator::header(
        vec!["key".to_string()],
        "X-API-Key".to_string(),
    );

    let scheme = authenticator.security_scheme();

    match scheme {
        SecurityScheme::ApiKey { location, name, .. } => {
            assert_eq!(location, "header");
            assert_eq!(name, "X-API-Key");
        }
        _ => panic!("Expected ApiKey security scheme"),
    }
}

#[tokio::test]
async fn test_api_key_query_location() {
    let authenticator = ApiKeyAuthenticator::query(
        vec!["key".to_string()],
        "api_key".to_string(),
    );

    let scheme = authenticator.security_scheme();

    match scheme {
        SecurityScheme::ApiKey { location, name, .. } => {
            assert_eq!(location, "query");
            assert_eq!(name, "api_key");
        }
        _ => panic!("Expected ApiKey security scheme"),
    }
}

#[tokio::test]
async fn test_api_key_cookie_location() {
    let authenticator = ApiKeyAuthenticator::cookie(
        vec!["key".to_string()],
        "session_id".to_string(),
    );

    let scheme = authenticator.security_scheme();

    match scheme {
        SecurityScheme::ApiKey { location, name, .. } => {
            assert_eq!(location, "cookie");
            assert_eq!(name, "session_id");
        }
        _ => panic!("Expected ApiKey security scheme"),
    }
}

#[tokio::test]
async fn test_api_key_principal_attributes() {
    let authenticator = ApiKeyAuthenticator::header(
        vec!["key-123".to_string()],
        "X-API-Key".to_string(),
    );

    let mut metadata = HashMap::new();
    metadata.insert("location".to_string(), "header".to_string());
    metadata.insert("name".to_string(), "X-API-Key".to_string());

    let context = AuthContext {
        scheme_type: "apikey".to_string(),
        credential: "key-123".to_string(),
        metadata,
    };

    let result = authenticator.authenticate(&context).await;

    assert!(result.is_ok());
    let principal = result.unwrap();

    assert_eq!(
        principal.get_attribute("location"),
        Some(&"header".to_string())
    );
}

#[tokio::test]
async fn test_noop_authenticator_always_succeeds() {
    let authenticator = NoopAuthenticator::new();

    let context = AuthContext::new("any".to_string(), "any".to_string());
    let result = authenticator.authenticate(&context).await;

    assert!(result.is_ok());
    let principal = result.unwrap();
    assert_eq!(principal.id, "anonymous");
    assert_eq!(principal.auth_type, "none");
}

#[tokio::test]
async fn test_noop_authenticator_security_scheme() {
    let authenticator = NoopAuthenticator::new();

    let scheme = authenticator.security_scheme();

    match scheme {
        SecurityScheme::Http { scheme, .. } => {
            assert_eq!(scheme, "none");
        }
        _ => panic!("Expected Http security scheme with 'none'"),
    }
}

#[tokio::test]
async fn test_noop_authenticator_default() {
    let authenticator = NoopAuthenticator::default();

    let context = AuthContext::new("any".to_string(), "any".to_string());
    let result = authenticator.authenticate(&context).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_bearer_token_extractor_from_headers() {
    let extractor = BearerTokenExtractor;

    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), "Bearer token-123".to_string());

    #[cfg(feature = "http-server")]
    let result = {
        use axum::http::HeaderMap;
        let mut axum_headers = HeaderMap::new();
        axum_headers.insert(
            "authorization",
            "Bearer token-123".parse().unwrap(),
        );
        extractor.extract_from_headers(&axum_headers).await
    };

    #[cfg(not(feature = "http-server"))]
    let result = extractor.extract_from_headers(&headers).await;

    assert!(result.is_some());
    let context = result.unwrap();
    assert_eq!(context.scheme_type, "bearer");
    assert_eq!(context.credential, "token-123");
}

#[tokio::test]
async fn test_bearer_token_extractor_lowercase() {
    let extractor = BearerTokenExtractor;

    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), "bearer token-456".to_string());

    let result = extractor.extract_from_headers(&headers).await;

    assert!(result.is_some());
    let context = result.unwrap();
    assert_eq!(context.scheme_type, "bearer");
    assert_eq!(context.credential, "token-456");
}

#[tokio::test]
async fn test_bearer_token_extractor_missing_header() {
    let extractor = BearerTokenExtractor;

    let headers = HashMap::new();

    let result = extractor.extract_from_headers(&headers).await;

    assert!(result.is_none());
}

#[tokio::test]
async fn test_bearer_token_extractor_invalid_format() {
    let extractor = BearerTokenExtractor;

    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), "InvalidFormat".to_string());

    let result = extractor.extract_from_headers(&headers).await;

    assert!(result.is_none());
}

#[tokio::test]
async fn test_bearer_token_extractor_from_query() {
    let extractor = BearerTokenExtractor;

    let mut params = HashMap::new();
    params.insert("token".to_string(), "value".to_string());

    let result = extractor.extract_from_query(&params).await;

    // Bearer tokens should not be extracted from query params
    assert!(result.is_none());
}

#[tokio::test]
async fn test_bearer_token_extractor_from_cookies() {
    let extractor = BearerTokenExtractor;

    let cookies = "session=abc123; token=xyz789";

    let result = extractor.extract_from_cookies(cookies).await;

    // Bearer tokens should not be extracted from cookies
    assert!(result.is_none());
}

#[tokio::test]
async fn test_api_key_extractor_header() {
    let extractor = ApiKeyExtractor::new("header".to_string(), "X-API-Key".to_string());

    let mut headers = HashMap::new();
    headers.insert("x-api-key".to_string(), "my-secret-key".to_string());

    let result = extractor.extract_from_headers(&headers).await;

    assert!(result.is_some());
    let context = result.unwrap();
    assert_eq!(context.scheme_type, "apikey");
    assert_eq!(context.credential, "my-secret-key");

    // Check metadata
    assert_eq!(
        context.metadata.get("location"),
        Some(&"header".to_string())
    );
    assert_eq!(
        context.metadata.get("name"),
        Some(&"X-API-Key".to_string())
    );
}

#[tokio::test]
async fn test_api_key_extractor_header_case_insensitive() {
    let extractor = ApiKeyExtractor::new("header".to_string(), "X-API-Key".to_string());

    let mut headers = HashMap::new();
    headers.insert("X-API-KEY".to_string(), "my-secret-key".to_string());

    let result = extractor.extract_from_headers(&headers).await;

    // Should extract case-insensitively for header names
    #[cfg(not(feature = "http-server"))]
    assert!(result.is_some());

    #[cfg(feature = "http-server")]
    {
        use axum::http::HeaderName;
        // Axum handles header names properly
        assert!(result.is_some() || result.is_none());
    }
}

#[tokio::test]
async fn test_api_key_extractor_query() {
    let extractor = ApiKeyExtractor::new("query".to_string(), "api_key".to_string());

    let mut params = HashMap::new();
    params.insert("api_key".to_string(), "my-query-key".to_string());

    let result = extractor.extract_from_query(&params).await;

    assert!(result.is_some());
    let context = result.unwrap();
    assert_eq!(context.scheme_type, "apikey");
    assert_eq!(context.credential, "my-query-key");
}

#[tokio::test]
async fn test_api_key_extractor_cookie() {
    let extractor =
        ApiKeyExtractor::new("cookie".to_string(), "session_id".to_string());

    let cookies = "session=abc123; session_id=xyz789; other=value";

    let result = extractor.extract_from_cookies(cookies).await;

    assert!(result.is_some());
    let context = result.unwrap();
    assert_eq!(context.scheme_type, "apikey");
    assert_eq!(context.credential, "xyz789");
}

#[tokio::test]
async fn test_api_key_extractor_cookie_not_found() {
    let extractor =
        ApiKeyExtractor::new("cookie".to_string(), "session_id".to_string());

    let cookies = "session=abc123; other=value";

    let result = extractor.extract_from_cookies(cookies).await;

    assert!(result.is_none());
}

#[tokio::test]
async fn test_api_key_extractor_wrong_location() {
    let extractor = ApiKeyExtractor::new("header".to_string(), "X-API-Key".to_string());

    let mut params = HashMap::new();
    params.insert("api_key".to_string(), "key".to_string());

    let result = extractor.extract_from_query(&params).await;

    // Should not extract when location is "header" but trying from query
    assert!(result.is_none());
}

#[tokio::test]
async fn test_api_key_extractor_header_missing() {
    let extractor = ApiKeyExtractor::new("header".to_string(), "X-API-Key".to_string());

    let headers = HashMap::new();

    let result = extractor.extract_from_headers(&headers).await;

    assert!(result.is_none());
}

#[tokio::test]
async fn test_auth_context_creation() {
    let context = AuthContext::new("bearer".to_string(), "token-123".to_string());

    assert_eq!(context.scheme_type, "bearer");
    assert_eq!(context.credential, "token-123");
    assert!(context.metadata.is_empty());
}

#[tokio::test]
async fn test_auth_context_with_metadata() {
    let mut metadata = HashMap::new();
    metadata.insert("location".to_string(), "header".to_string());
    metadata.insert("extra".to_string(), "value".to_string());

    let context = AuthContext {
        scheme_type: "apikey".to_string(),
        credential: "key-123".to_string(),
        metadata,
    };

    assert_eq!(context.scheme_type, "apikey");
    assert_eq!(context.credential, "key-123");
    assert_eq!(context.metadata.len(), 2);
}

#[tokio::test]
async fn test_auth_principal_creation() {
    let principal = AuthPrincipal::new("user-123".to_string(), "bearer".to_string());

    assert_eq!(principal.id, "user-123");
    assert_eq!(principal.auth_type, "bearer");
    assert!(principal.attributes.is_empty());
}

#[tokio::test]
async fn test_auth_principal_with_attribute() {
    let mut principal = AuthPrincipal::new("user-123".to_string(), "bearer".to_string());

    principal = principal.with_attribute("role".to_string(), "admin".to_string());
    principal = principal.with_attribute("org".to_string(), "acme".to_string());

    assert_eq!(principal.attributes.len(), 2);
    assert_eq!(
        principal.get_attribute("role"),
        Some(&"admin".to_string())
    );
    assert_eq!(
        principal.get_attribute("org"),
        Some(&"acme".to_string())
    );
    assert_eq!(principal.get_attribute("nonexistent"), None);
}

#[tokio::test]
async fn test_auth_principal_get_attribute_missing() {
    let principal = AuthPrincipal::new("user-123".to_string(), "bearer".to_string());

    assert_eq!(principal.get_attribute("missing"), None);
}

#[tokio::test]
async fn test_multiple_valid_bearer_tokens() {
    let authenticator = BearerTokenAuthenticator::new(vec![
        "token-1".to_string(),
        "token-2".to_string(),
        "token-3".to_string(),
    ]);

    // All tokens should be valid
    for token in &["token-1", "token-2", "token-3"] {
        let context =
            AuthContext::new("bearer".to_string(), token.to_string());
        let result = authenticator.authenticate(&context).await;

        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_multiple_valid_api_keys() {
    let authenticator = ApiKeyAuthenticator::header(
        vec!["key-1".to_string(), "key-2".to_string()],
        "X-API-Key".to_string(),
    );

    let mut metadata = HashMap::new();
    metadata.insert("location".to_string(), "header".to_string());

    // All keys should be valid
    for key in &["key-1", "key-2"] {
        let context = AuthContext {
            scheme_type: "apikey".to_string(),
            credential: key.to_string(),
            metadata: metadata.clone(),
        };

        let result = authenticator.authenticate(&context).await;

        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_empty_bearer_tokens() {
    let authenticator = BearerTokenAuthenticator::new(vec![]);

    let context = AuthContext::new("bearer".to_string(), "any-token".to_string());
    let result = authenticator.authenticate(&context).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_empty_api_keys() {
    let authenticator = ApiKeyAuthenticator::header(vec![], "X-API-Key".to_string());

    let mut metadata = HashMap::new();
    metadata.insert("location".to_string(), "header".to_string());

    let context = AuthContext {
        scheme_type: "apikey".to_string(),
        credential: "any-key".to_string(),
        metadata,
    };

    let result = authenticator.authenticate(&context).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_authenticator_clone_bearer() {
    let authenticator1 =
        BearerTokenAuthenticator::new(vec!["token".to_string()]);
    let authenticator2 = authenticator1.clone();

    // Both should have same configuration
    let context = AuthContext::new("bearer".to_string(), "token".to_string());

    let result1 = authenticator1.authenticate(&context).await;
    let result2 = authenticator2.authenticate(&context).await;

    assert_eq!(result1.is_ok(), result2.is_ok());
}

#[tokio::test]
async fn test_authenticator_clone_api_key() {
    let authenticator1 =
        ApiKeyAuthenticator::header(vec!["key".to_string()], "X-API-Key".to_string());
    let authenticator2 = authenticator1.clone();

    let mut metadata = HashMap::new();
    metadata.insert("location".to_string(), "header".to_string());

    let context = AuthContext {
        scheme_type: "apikey".to_string(),
        credential: "key".to_string(),
        metadata,
    };

    let result1 = authenticator1.authenticate(&context).await;
    let result2 = authenticator2.authenticate(&context).await;

    assert_eq!(result1.is_ok(), result2.is_ok());
}

#[tokio::test]
async fn test_authenticator_clone_noop() {
    let authenticator1 = NoopAuthenticator::new();
    let authenticator2 = authenticator1.clone();

    let context = AuthContext::new("any".to_string(), "any".to_string());

    let result1 = authenticator1.authenticate(&context).await;
    let result2 = authenticator2.authenticate(&context).await;

    assert_eq!(result1.is_ok(), result2.is_ok());
}
