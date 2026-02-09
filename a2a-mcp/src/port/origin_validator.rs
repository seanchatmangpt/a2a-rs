//! Origin validation port - defines the interface for DNS rebinding defense

use crate::error::Result;

/// Port interface for origin validation
///
/// Provides DNS rebinding defense by validating the Origin header
/// against an allowlist of permitted origins.
pub trait OriginValidator: Send + Sync {
    /// Validate the origin header against the allowlist
    ///
    /// # Arguments
    ///
    /// * `origin` - The Origin header value to validate
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the origin is valid
    /// * `Err(Error::OriginForbidden)` if the origin is not in the allowlist
    fn validate_origin(&self, origin: Option<&str>) -> Result<()>;

    /// Get the list of allowed origins
    fn allowed_origins(&self) -> &[String];

    /// Check if an origin is allowed
    fn is_origin_allowed(&self, origin: &str) -> bool;
}
