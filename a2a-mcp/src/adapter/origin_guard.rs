//! Origin guard adapter - DNS rebinding defense implementation

use crate::error::{Error, Result};
use crate::port::OriginValidator;

/// Origin guard implementation for DNS rebinding defense
///
/// Validates the Origin header against a configurable allowlist
/// to prevent DNS rebinding attacks. Returns deterministic 403 Forbidden
/// for invalid origins.
///
/// # Example
///
/// ```rust
/// use a2a_mcp::adapter::OriginGuard;
/// use a2a_mcp::port::OriginValidator;
///
/// let allowed_origins = vec![
///     "http://localhost:3000".to_string(),
///     "https://example.com".to_string(),
/// ];
///
/// let guard = OriginGuard::new(allowed_origins);
///
/// // Valid origin
/// assert!(guard.validate_origin(Some("http://localhost:3000")).is_ok());
///
/// // Invalid origin
/// assert!(guard.validate_origin(Some("https://evil.com")).is_err());
///
/// // Missing origin (treated as invalid)
/// assert!(guard.validate_origin(None).is_err());
/// ```
#[derive(Debug, Clone)]
pub struct OriginGuard {
    /// List of allowed origins (e.g., "http://localhost:3000", "https://example.com")
    allowed_origins: Vec<String>,
}

impl OriginGuard {
    /// Create a new origin guard with the specified allowed origins
    ///
    /// # Arguments
    ///
    /// * `allowed_origins` - List of origin strings that are permitted
    ///
    /// # Example
    ///
    /// ```rust
    /// use a2a_mcp::adapter::OriginGuard;
    ///
    /// let guard = OriginGuard::new(vec![
    ///     "http://localhost:3000".to_string(),
    ///     "https://example.com".to_string(),
    /// ]);
    /// ```
    pub fn new(allowed_origins: Vec<String>) -> Self {
        Self { allowed_origins }
    }

    /// Create a new origin guard that allows only localhost origins
    ///
    /// Allows common localhost origins with various ports.
    ///
    /// # Example
    ///
    /// ```rust
    /// use a2a_mcp::adapter::OriginGuard;
    ///
    /// let guard = OriginGuard::localhost_only();
    /// ```
    pub fn localhost_only() -> Self {
        Self::new(vec![
            "http://localhost".to_string(),
            "http://localhost:3000".to_string(),
            "http://localhost:8080".to_string(),
            "http://127.0.0.1".to_string(),
            "http://127.0.0.1:3000".to_string(),
            "http://127.0.0.1:8080".to_string(),
        ])
    }

    /// Create a new origin guard that allows all origins (UNSAFE)
    ///
    /// This is useful for testing but should never be used in production.
    ///
    /// # Security Warning
    ///
    /// This completely disables DNS rebinding protection and should
    /// only be used in development/testing environments.
    ///
    /// # Example
    ///
    /// ```rust
    /// use a2a_mcp::adapter::OriginGuard;
    ///
    /// // WARNING: Only use in tests!
    /// let guard = OriginGuard::allow_all();
    /// ```
    #[cfg(test)]
    pub fn allow_all() -> Self {
        Self::new(vec!["*".to_string()])
    }
}

impl OriginValidator for OriginGuard {
    fn validate_origin(&self, origin: Option<&str>) -> Result<()> {
        match origin {
            None => {
                // Missing Origin header - reject for security
                Err(Error::OriginForbidden("Missing Origin header".to_string()))
            }
            Some(origin_value) => {
                if self.is_origin_allowed(origin_value) {
                    Ok(())
                } else {
                    Err(Error::OriginForbidden(format!(
                        "Origin '{}' is not in the allowlist",
                        origin_value
                    )))
                }
            }
        }
    }

    fn allowed_origins(&self) -> &[String] {
        &self.allowed_origins
    }

    fn is_origin_allowed(&self, origin: &str) -> bool {
        // Special case: wildcard allows everything
        if self.allowed_origins.contains(&"*".to_string()) {
            return true;
        }

        // Check if the origin is in the allowlist
        // We do exact string matching for security
        self.allowed_origins.iter().any(|allowed| {
            // Exact match
            allowed == origin
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_origin() {
        let guard = OriginGuard::new(vec![
            "http://localhost:3000".to_string(),
            "https://example.com".to_string(),
        ]);

        assert!(guard.validate_origin(Some("http://localhost:3000")).is_ok());
        assert!(guard.validate_origin(Some("https://example.com")).is_ok());
    }

    #[test]
    fn test_invalid_origin() {
        let guard = OriginGuard::new(vec!["http://localhost:3000".to_string()]);

        let result = guard.validate_origin(Some("https://evil.com"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::OriginForbidden(_)));
    }

    #[test]
    fn test_missing_origin() {
        let guard = OriginGuard::new(vec!["http://localhost:3000".to_string()]);

        let result = guard.validate_origin(None);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::OriginForbidden(_)));
    }

    #[test]
    fn test_localhost_only() {
        let guard = OriginGuard::localhost_only();

        assert!(guard.validate_origin(Some("http://localhost:3000")).is_ok());
        assert!(guard.validate_origin(Some("http://127.0.0.1:8080")).is_ok());
        assert!(guard.validate_origin(Some("https://evil.com")).is_err());
    }

    #[test]
    fn test_allow_all() {
        let guard = OriginGuard::allow_all();

        assert!(guard.validate_origin(Some("http://localhost:3000")).is_ok());
        assert!(guard.validate_origin(Some("https://evil.com")).is_ok());
        assert!(guard.validate_origin(Some("https://anything.com")).is_ok());
    }

    #[test]
    fn test_is_origin_allowed() {
        let guard = OriginGuard::new(vec![
            "http://localhost:3000".to_string(),
            "https://example.com".to_string(),
        ]);

        assert!(guard.is_origin_allowed("http://localhost:3000"));
        assert!(guard.is_origin_allowed("https://example.com"));
        assert!(!guard.is_origin_allowed("https://evil.com"));
    }

    #[test]
    fn test_allowed_origins() {
        let allowed = vec![
            "http://localhost:3000".to_string(),
            "https://example.com".to_string(),
        ];
        let guard = OriginGuard::new(allowed.clone());

        assert_eq!(guard.allowed_origins(), &allowed);
    }

    #[test]
    fn test_case_sensitivity() {
        let guard = OriginGuard::new(vec!["http://localhost:3000".to_string()]);

        // Origins should be case-sensitive for security
        assert!(guard.validate_origin(Some("http://localhost:3000")).is_ok());
        assert!(
            guard
                .validate_origin(Some("HTTP://LOCALHOST:3000"))
                .is_err()
        );
    }

    #[test]
    fn test_exact_matching() {
        let guard = OriginGuard::new(vec!["http://localhost:3000".to_string()]);

        // Should not match substrings or prefixes
        assert!(guard.validate_origin(Some("http://localhost:3000")).is_ok());
        assert!(
            guard
                .validate_origin(Some("http://localhost:3001"))
                .is_err()
        );
        assert!(
            guard
                .validate_origin(Some("http://localhost:30000"))
                .is_err()
        );
        assert!(
            guard
                .validate_origin(Some("http://localhost:3000/path"))
                .is_err()
        );
    }
}
