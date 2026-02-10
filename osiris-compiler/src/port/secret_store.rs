//! SecretStore port trait for managing cryptographic secrets with versioning and rotation.
//!
//! Provides a contract for storing, retrieving, and rotating secrets with version management.
//! Implementations can use Google Secret Manager, AWS Secrets Manager, HashiCorp Vault, etc.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error types for secret store operations.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SecretStoreError {
    /// Secret not found.
    #[error("Secret not found: {0}")]
    NotFound(String),

    /// Secret access denied.
    #[error("Access denied to secret: {0}")]
    AccessDenied(String),

    /// Invalid secret name or format.
    #[error("Invalid secret name: {0}")]
    InvalidName(String),

    /// Version not found.
    #[error("Version not found: {0}")]
    VersionNotFound(String),

    /// Secret already exists.
    #[error("Secret already exists: {0}")]
    AlreadyExists(String),

    /// Rotation failed.
    #[error("Rotation failed: {0}")]
    RotationFailed(String),

    /// Backend authentication error.
    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    /// Generic backend error.
    #[error("Backend error: {0}")]
    BackendError(String),

    /// Operation timeout.
    #[error("Operation timed out")]
    TimeoutError,

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    ConfigError(String),
}

/// Metadata about a secret version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretVersion {
    /// Version identifier (e.g., "1", "2", "3").
    pub version_id: String,

    /// When this version was created.
    pub created_at: DateTime<Utc>,

    /// When this version was last rotated.
    pub rotated_at: Option<DateTime<Utc>>,

    /// Current state of the version.
    pub state: VersionState,

    /// Labels/tags associated with this version.
    pub labels: std::collections::HashMap<String, String>,
}

/// State of a secret version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VersionState {
    /// Version is enabled and can be used.
    Enabled,

    /// Version is disabled and cannot be accessed.
    Disabled,

    /// Version is being rotated.
    Rotating,

    /// Version has been destroyed and cannot be recovered.
    Destroyed,
}

/// Configuration for automatic secret rotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotationPolicy {
    /// Rotate every N seconds.
    pub rotation_period_secs: u64,

    /// Time-to-live for each secret version (in seconds).
    pub version_ttl_secs: Option<u64>,

    /// Automatically enable rotation.
    pub auto_rotate: bool,

    /// Custom rotation handler (e.g., lambda function ARN, Cloud Function path).
    pub rotation_handler: Option<String>,
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            rotation_period_secs: 7 * 24 * 60 * 60,    // 7 days
            version_ttl_secs: Some(30 * 24 * 60 * 60), // 30 days
            auto_rotate: false,
            rotation_handler: None,
        }
    }
}

/// Metadata about a secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretMetadata {
    /// Secret name/identifier.
    pub name: String,

    /// Creation timestamp.
    pub created_at: DateTime<Utc>,

    /// Last updated timestamp.
    pub updated_at: DateTime<Utc>,

    /// Current active version.
    pub current_version: String,

    /// All available versions.
    pub versions: Vec<SecretVersion>,

    /// Rotation policy (if any).
    pub rotation_policy: Option<RotationPolicy>,

    /// User-defined labels/tags.
    pub labels: std::collections::HashMap<String, String>,

    /// Whether the secret is replicated across regions.
    pub is_replicated: bool,
}

/// Port trait for managing secrets with versioning and rotation.
///
/// Implementations must:
/// - Support version management (list, get specific versions)
/// - Enforce access control per secret
/// - Support automatic rotation policies
/// - Provide audit logging for all operations
/// - Never return raw secrets in logs or errors
#[async_trait]
pub trait SecretStore: Send + Sync {
    /// Creates a new secret with an initial value.
    ///
    /// # Arguments
    /// * `name` - Unique secret identifier
    /// * `value` - Initial secret value (will be stored securely)
    /// * `labels` - Optional labels for organization
    ///
    /// # Returns
    /// The version ID of the created secret
    ///
    /// # Errors
    /// Returns `SecretStoreError::AlreadyExists` if secret exists
    /// Returns `SecretStoreError::InvalidName` if name is invalid
    async fn create_secret(
        &self,
        name: &str,
        value: &[u8],
        labels: Option<std::collections::HashMap<String, String>>,
    ) -> Result<String, SecretStoreError>;

    /// Retrieves the current version of a secret.
    ///
    /// # Arguments
    /// * `name` - Secret identifier
    ///
    /// # Returns
    /// The secret value (as bytes for maximum compatibility)
    ///
    /// # Errors
    /// Returns `SecretStoreError::NotFound` if secret doesn't exist
    /// Returns `SecretStoreError::AccessDenied` if unauthorized
    async fn get_secret(&self, name: &str) -> Result<Vec<u8>, SecretStoreError>;

    /// Retrieves a specific version of a secret.
    ///
    /// # Arguments
    /// * `name` - Secret identifier
    /// * `version_id` - Version to retrieve
    ///
    /// # Returns
    /// The secret value from the specified version
    ///
    /// # Errors
    /// Returns `SecretStoreError::NotFound` if secret doesn't exist
    /// Returns `SecretStoreError::VersionNotFound` if version doesn't exist
    async fn get_secret_version(
        &self,
        name: &str,
        version_id: &str,
    ) -> Result<Vec<u8>, SecretStoreError>;

    /// Updates a secret with a new value (creates a new version).
    ///
    /// # Arguments
    /// * `name` - Secret identifier
    /// * `value` - New secret value
    ///
    /// # Returns
    /// The new version ID
    ///
    /// # Errors
    /// Returns `SecretStoreError::NotFound` if secret doesn't exist
    async fn update_secret(&self, name: &str, value: &[u8]) -> Result<String, SecretStoreError>;

    /// Deletes a secret and all its versions.
    ///
    /// # Arguments
    /// * `name` - Secret identifier
    ///
    /// # Errors
    /// Returns `SecretStoreError::NotFound` if secret doesn't exist
    async fn delete_secret(&self, name: &str) -> Result<(), SecretStoreError>;

    /// Lists all versions of a secret.
    ///
    /// # Arguments
    /// * `name` - Secret identifier
    ///
    /// # Returns
    /// Vector of version metadata
    ///
    /// # Errors
    /// Returns `SecretStoreError::NotFound` if secret doesn't exist
    async fn list_versions(&self, name: &str) -> Result<Vec<SecretVersion>, SecretStoreError>;

    /// Disables a specific version.
    ///
    /// # Arguments
    /// * `name` - Secret identifier
    /// * `version_id` - Version to disable
    ///
    /// # Errors
    /// Returns `SecretStoreError::VersionNotFound` if version doesn't exist
    async fn disable_version(&self, name: &str, version_id: &str) -> Result<(), SecretStoreError>;

    /// Enables a previously disabled version.
    ///
    /// # Arguments
    /// * `name` - Secret identifier
    /// * `version_id` - Version to enable
    ///
    /// # Errors
    /// Returns `SecretStoreError::VersionNotFound` if version doesn't exist
    async fn enable_version(&self, name: &str, version_id: &str) -> Result<(), SecretStoreError>;

    /// Destroys a secret version permanently.
    ///
    /// # Arguments
    /// * `name` - Secret identifier
    /// * `version_id` - Version to destroy
    ///
    /// # Errors
    /// Returns `SecretStoreError::VersionNotFound` if version doesn't exist
    async fn destroy_version(&self, name: &str, version_id: &str) -> Result<(), SecretStoreError>;

    /// Rotates a secret by creating a new version.
    ///
    /// If a rotation handler is configured, it will be called to generate the new value.
    /// Otherwise, the operation returns success but no automatic generation occurs.
    ///
    /// # Arguments
    /// * `name` - Secret identifier
    /// * `new_value` - New secret value (optional, only used if no handler)
    ///
    /// # Returns
    /// The new version ID
    ///
    /// # Errors
    /// Returns `SecretStoreError::NotFound` if secret doesn't exist
    /// Returns `SecretStoreError::RotationFailed` if rotation handler fails
    async fn rotate_secret(
        &self,
        name: &str,
        new_value: Option<&[u8]>,
    ) -> Result<String, SecretStoreError>;

    /// Sets or updates the rotation policy for a secret.
    ///
    /// # Arguments
    /// * `name` - Secret identifier
    /// * `policy` - Rotation policy configuration
    ///
    /// # Errors
    /// Returns `SecretStoreError::NotFound` if secret doesn't exist
    async fn set_rotation_policy(
        &self,
        name: &str,
        policy: RotationPolicy,
    ) -> Result<(), SecretStoreError>;

    /// Gets the rotation policy for a secret.
    ///
    /// # Arguments
    /// * `name` - Secret identifier
    ///
    /// # Returns
    /// The rotation policy if configured
    ///
    /// # Errors
    /// Returns `SecretStoreError::NotFound` if secret doesn't exist
    async fn get_rotation_policy(
        &self,
        name: &str,
    ) -> Result<Option<RotationPolicy>, SecretStoreError>;

    /// Retrieves metadata about a secret.
    ///
    /// # Arguments
    /// * `name` - Secret identifier
    ///
    /// # Returns
    /// Complete metadata including all versions and policies
    ///
    /// # Errors
    /// Returns `SecretStoreError::NotFound` if secret doesn't exist
    async fn get_metadata(&self, name: &str) -> Result<SecretMetadata, SecretStoreError>;

    /// Performs a health check on the secret store backend.
    ///
    /// # Returns
    /// `Ok(())` if the backend is healthy
    async fn health_check(&self) -> Result<(), SecretStoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_state_serialization() {
        let state = VersionState::Enabled;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"ENABLED\"");

        let state = VersionState::Rotating;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"ROTATING\"");
    }

    #[test]
    fn test_rotation_policy_defaults() {
        let policy = RotationPolicy::default();
        assert_eq!(policy.rotation_period_secs, 7 * 24 * 60 * 60);
        assert_eq!(policy.version_ttl_secs, Some(30 * 24 * 60 * 60));
        assert!(!policy.auto_rotate);
        assert!(policy.rotation_handler.is_none());
    }

    #[test]
    fn test_secret_version_creation() {
        let now = Utc::now();
        let mut labels = std::collections::HashMap::new();
        labels.insert("env".to_string(), "prod".to_string());

        let version = SecretVersion {
            version_id: "1".to_string(),
            created_at: now,
            rotated_at: None,
            state: VersionState::Enabled,
            labels,
        };

        assert_eq!(version.version_id, "1");
        assert_eq!(version.state, VersionState::Enabled);
    }

    #[test]
    fn test_secret_error_serialization() {
        let error = SecretStoreError::NotFound("api_key".to_string());
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("notFound"));
        assert!(json.contains("api_key"));
    }
}
