//! Unit tests for token service

#[cfg(test)]
#[cfg(feature = "auth")]
mod tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use crate::adapter::auth::token_service::{
        TokenService, TokenRefreshRequest, UserInfo,
        AuthorizationUrlGenerator,
    };
    use crate::port::authenticator::AuthPrincipal;

    #[test]
    fn test_token_service_generate() {
        let secret = b"test_secret";
        let token_service = TokenService::new_with_secret(secret)
            .with_expiration(3600)
            .with_issuer("test_issuer".to_string())
            .with_audience("test_audience".to_string());

        let principal = AuthPrincipal::new("user_123".to_string(), "test".to_string())
            .with_attribute("name".to_string(), "Test User".to_string())
            .with_attribute("email".to_string(), "test@example.com".to_string());

        let response = token_service
            .generate_token(&principal)
            .expect("Token generation should succeed");

        assert_eq!(response.token_type, "Bearer");
        assert_eq!(response.expires_in, 3600);
        assert!(response.access_token.len() > 0);
        assert!(response.refresh_token.is_none());
    }

    #[test]
    fn test_token_service_with_refresh() {
        let secret = b"test_secret_refresh";
        let token_service = TokenService::new_with_secret(secret);

        let principal = AuthPrincipal::new("user_456".to_string(), "test".to_string());

        let response = token_service
            .generate_token_with_refresh(&principal)
            .expect("Token generation with refresh should succeed");

        assert!(response.refresh_token.is_some());
        assert!(response.access_token.len() > 0);
    }

    #[test]
    fn test_token_service_validate() {
        let secret = b"test_secret_validation";
        let token_service = TokenService::new_with_secret(secret);

        let principal = AuthPrincipal::new("user_789".to_string(), "test".to_string())
            .with_attribute("role".to_string(), "admin".to_string());

        let response = token_service
            .generate_token(&principal)
            .expect("Token generation should succeed");

        let validated = token_service
            .validate_token(&response.access_token)
            .expect("Token validation should succeed");

        assert_eq!(validated.id, "user_789");
        // The attribute may be JSON-encoded, so check for admin
        let role = validated.attributes.get("role").unwrap();
        assert!(role.contains("admin"));
    }

    #[test]
    fn test_token_service_invalid_token() {
        let secret = b"test_secret_invalid";
        let token_service = TokenService::new_with_secret(secret);

        let result = token_service.validate_token("invalid_token");
        assert!(result.is_err());
    }

    #[test]
    fn test_token_service_refresh() {
        let secret = b"test_secret_refresh_flow";
        let token_service = TokenService::new_with_secret(secret);

        let principal = AuthPrincipal::new("user_refresh".to_string(), "test".to_string());

        let response = token_service
            .generate_token_with_refresh(&principal)
            .expect("Token generation should succeed");

        let refresh_token = response
            .refresh_token
            .expect("Should have refresh token");

        let refreshed = token_service
            .refresh_token(&refresh_token)
            .expect("Token refresh should succeed");

        assert!(refreshed.access_token.len() > 0);
        assert!(refreshed.refresh_token.is_some());
    }

    #[test]
    fn test_token_service_user_info() {
        let secret = b"test_secret_userinfo";
        let token_service = TokenService::new_with_secret(secret);

        let principal = AuthPrincipal::new("user_info".to_string(), "test".to_string())
            .with_attribute("name".to_string(), "Jane Doe".to_string())
            .with_attribute("email".to_string(), "jane@example.com".to_string())
            .with_attribute("email_verified".to_string(), "true".to_string())
            .with_attribute("picture".to_string(), "https://example.com/jane.jpg".to_string());

        let response = token_service
            .generate_token(&principal)
            .expect("Token generation should succeed");

        let user_info = token_service
            .get_user_info(&response.access_token)
            .expect("Get user info should succeed");

        assert_eq!(user_info.sub, "user_info");
        // Name and email may be JSON-encoded
        assert!(user_info.name.as_ref().unwrap().contains("Jane"));
        assert!(user_info.email.as_ref().unwrap().contains("jane@example.com"));
        assert_eq!(user_info.email_verified.unwrap(), true);
        assert!(user_info.picture.as_ref().unwrap().contains("example.com"));
    }

    #[test]
    fn test_authorization_url_generator() {
        let generator = AuthorizationUrlGenerator::new(
            "https://example.com/oauth/authorize".to_string(),
            "my_client_id".to_string(),
            "https://myapp.com/callback".to_string(),
            vec!["openid".to_string(), "profile".to_string(), "email".to_string()],
        );

        let response = generator
            .generate()
            .expect("Should generate authorization URL");

        assert!(response.url.contains("https://example.com/oauth/authorize"));
        assert!(response.url.contains("client_id=my_client_id"));
        // redirect_uri may be URL-encoded
        assert!(response.url.contains("myapp.com"));
        assert!(response.url.contains("scope"));
        assert!(!response.csrf_token.is_empty());
    }
}
