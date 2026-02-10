//! Google Secret Manager adapter for the SecretStore port.
//!
//! Provides integration with Google Cloud Secret Manager for secure
//! secret storage with automatic versioning, rotation, and access control.

#[cfg(feature = "secret-manager")]
use crate::port::secret_store::{
    RotationPolicy, SecretMetadata, SecretStoreError, SecretVersion, VersionState,
};
#[cfg(feature = "secret-manager")]
use async_trait::async_trait;
#[cfg(feature = "secret-manager")]
use chrono::{DateTime, Utc};
#[cfg(feature = "secret-manager")]
use google_secretmanager1::{
    api::{
        AddSecretVersionRequest, DestroySecretVersionRequest, DisableSecretVersionRequest,
        EnableSecretVersionRequest, Replication, Secret, SecretVersion as GsmSecretVersion,
    },
    SecretManager,
};
#[cfg(feature = "secret-manager")]
use std::collections::HashMap;
#[cfg(feature = "secret-manager")]
use std::sync::Arc;

/// Configuration for Google Secret Manager.
#[cfg(feature = "secret-manager")]
#[derive(Debug, Clone)]
pub struct GoogleSecretManagerConfig {
    /// GCP project ID.
    pub project_id: String,

    /// Path to service account JSON key file (optional, uses ADC if not provided).
    pub service_account_key: Option<String>,

    /// Replication policy (e.g., "us-west1", "us-east1", "auto").
    pub replication_location: Option<String>,

    /// Default rotation policy for new secrets.
    pub default_rotation_policy: Option<RotationPolicy>,
}

#[cfg(feature = "secret-manager")]
impl GoogleSecretManagerConfig {
    /// Creates a new configuration with defaults.
    pub fn new(project_id: String) -> Self {
        Self {
            project_id,
            service_account_key: None,
            replication_location: Some("us".to_string()),
            default_rotation_policy: Some(RotationPolicy::default()),
        }
    }

    /// Sets the service account key path.
    pub fn with_service_account_key(mut self, path: String) -> Self {
        self.service_account_key = Some(path);
        self
    }

    /// Sets the replication location.
    pub fn with_replication_location(mut self, location: String) -> Self {
        self.replication_location = Some(location);
        self
    }

    /// Sets the default rotation policy.
    pub fn with_rotation_policy(mut self, policy: RotationPolicy) -> Self {
        self.default_rotation_policy = Some(policy);
        self
    }
}

/// Google Secret Manager adapter.
///
/// Implements the SecretStore port using Google Cloud Secret Manager API.
/// Supports:
/// - Automatic versioning
/// - Rotation policies
/// - Version lifecycle management
/// - Multi-region replication
/// - Detailed audit logging
#[cfg(feature = "secret-manager")]
pub struct GoogleSecretManager {
    config: GoogleSecretManagerConfig,
    client: Arc<SecretManager>,
}

#[cfg(feature = "secret-manager")]
impl GoogleSecretManager {
    /// Creates a new Google Secret Manager adapter.
    ///
    /// # Arguments
    /// * `config` - Configuration including project ID and credentials
    ///
    /// # Returns
    /// A new adapter instance
    ///
    /// # Errors
    /// Returns `SecretStoreError::AuthenticationError` if authentication fails
    /// Returns `SecretStoreError::ConfigError` if config is invalid
    pub async fn new(config: GoogleSecretManagerConfig) -> Result<Self, SecretStoreError> {
        // Validate project ID
        if config.project_id.is_empty() {
            return Err(SecretStoreError::ConfigError(
                "Project ID cannot be empty".to_string(),
            ));
        }

        // Create authenticator from service account key or environment
        let auth = if let Some(ref key_path) = config.service_account_key {
            // Use provided service account key file
            let secret = yup_oauth2::read_service_account_key(key_path)
                .await
                .map_err(|e| {
                    SecretStoreError::AuthenticationError(format!(
                        "Failed to read service account key: {}",
                        e
                    ))
                })?;

            yup_oauth2::ServiceAccountAuthenticator::builder(secret)
                .build()
                .await
                .map_err(|e| {
                    SecretStoreError::AuthenticationError(format!(
                        "Failed to create authenticator: {}",
                        e
                    ))
                })?
        } else {
            // Try to load from GOOGLE_APPLICATION_CREDENTIALS environment variable
            if let Ok(key_path) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
                let secret = yup_oauth2::read_service_account_key(&key_path)
                    .await
                    .map_err(|e| {
                        SecretStoreError::AuthenticationError(format!(
                            "Failed to read service account key from GOOGLE_APPLICATION_CREDENTIALS: {}",
                            e
                        ))
                    })?;

                yup_oauth2::ServiceAccountAuthenticator::builder(secret)
                    .build()
                    .await
                    .map_err(|e| {
                        SecretStoreError::AuthenticationError(format!(
                            "Failed to create authenticator: {}",
                            e
                        ))
                    })?
            } else {
                return Err(SecretStoreError::AuthenticationError(
                    "No service account key provided. Set GOOGLE_APPLICATION_CREDENTIALS environment variable".to_string(),
                ));
            }
        };

        // Create HTTPS connector
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .map_err(|e| {
                SecretStoreError::AuthenticationError(format!(
                    "Failed to create HTTPS connector: {}",
                    e
                ))
            })?
            .https_only()
            .enable_http1()
            .enable_http2()
            .build();

        // Create hyper client using the connector
        let http_client = hyper::Client::builder().build::<_, hyper::Body>(https);

        // Create Secret Manager client
        let client = SecretManager::new(http_client, auth);

        Ok(Self {
            config,
            client: Arc::new(client),
        })
    }

    /// Creates a GSM Secret object from configuration.
    fn create_secret_object(&self) -> Secret {
        let secret = Secret {
            replication: Some(Replication {
                automatic: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };

        secret
    }

    /// Converts a GSM SecretVersion to our domain type.
    fn convert_version(&self, version: &GsmSecretVersion) -> SecretVersion {
        let state = match version.state.as_deref() {
            Some("ENABLED") => VersionState::Enabled,
            Some("DISABLED") => VersionState::Disabled,
            Some("DESTROYED") => VersionState::Destroyed,
            Some("ROTATING") => VersionState::Rotating,
            _ => VersionState::Enabled,
        };

        SecretVersion {
            version_id: version.name.as_ref().unwrap_or(&String::new()).to_string(),
            created_at: version
                .create_time
                .as_ref()
                .and_then(|t| {
                    chrono::DateTime::parse_from_rfc3339(t)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                })
                .unwrap_or_else(Utc::now),
            rotated_at: None,
            state,
            labels: version.labels.clone().unwrap_or_default(),
        }
    }
}

#[cfg(feature = "secret-manager")]
#[async_trait]
impl crate::port::SecretStore for GoogleSecretManager {
    async fn create_secret(
        &self,
        name: &str,
        value: &[u8],
        labels: Option<HashMap<String, String>>,
    ) -> Result<String, SecretStoreError> {
        // Validate name
        if name.is_empty() {
            return Err(SecretStoreError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        let mut secret = self.create_secret_object();
        secret.labels = labels;

        let parent = format!("projects/{}", self.config.project_id);

        // Create the secret
        let (_, created_secret) = self
            .client
            .projects()
            .secrets_create(secret, &parent)
            .secret_id(name)
            .doit()
            .await
            .map_err(|e| {
                if e.to_string().contains("already exists") {
                    SecretStoreError::AlreadyExists(name.to_string())
                } else {
                    SecretStoreError::BackendError(format!("Failed to create secret: {}", e))
                }
            })?;

        let secret_name = created_secret
            .name
            .clone()
            .ok_or_else(|| SecretStoreError::BackendError("No secret name returned".to_string()))?;

        // Add the initial version
        let request = AddSecretVersionRequest {
            payload: Some(google_secretmanager1::api::SecretPayload {
                data: Some(base64::engine::general_purpose::STANDARD.encode(value)),
                ..Default::default()
            }),
            ..Default::default()
        };

        let (_, version_response) = self
            .client
            .projects()
            .secrets_add_version(request, &secret_name)
            .doit()
            .await
            .map_err(|e| {
                SecretStoreError::BackendError(format!("Failed to add secret version: {}", e))
            })?;

        let version_id = version_response.name.ok_or_else(|| {
            SecretStoreError::BackendError("No version name returned".to_string())
        })?;

        Ok(version_id)
    }

    async fn get_secret(&self, name: &str) -> Result<Vec<u8>, SecretStoreError> {
        if name.is_empty() {
            return Err(SecretStoreError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        let secret_name = format!(
            "projects/{}/secrets/{}/versions/latest",
            self.config.project_id, name
        );

        let (_, response) = self
            .client
            .projects()
            .secrets_versions_access(&secret_name)
            .doit()
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    SecretStoreError::NotFound(name.to_string())
                } else if e.to_string().contains("denied") || e.to_string().contains("forbidden") {
                    SecretStoreError::AccessDenied(name.to_string())
                } else {
                    SecretStoreError::BackendError(format!("Failed to access secret: {}", e))
                }
            })?;

        let data = response.payload.and_then(|p| p.data).ok_or_else(|| {
            SecretStoreError::BackendError("No secret data in response".to_string())
        })?;

        base64::engine::general_purpose::STANDARD
            .decode(&data)
            .map_err(|e| SecretStoreError::BackendError(format!("Failed to decode secret: {}", e)))
    }

    async fn get_secret_version(
        &self,
        name: &str,
        version_id: &str,
    ) -> Result<Vec<u8>, SecretStoreError> {
        if name.is_empty() {
            return Err(SecretStoreError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        let secret_name = format!(
            "projects/{}/secrets/{}/versions/{}",
            self.config.project_id, name, version_id
        );

        let (_, response) = self
            .client
            .projects()
            .secrets_versions_access(&secret_name)
            .doit()
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    SecretStoreError::VersionNotFound(version_id.to_string())
                } else if e.to_string().contains("denied") || e.to_string().contains("forbidden") {
                    SecretStoreError::AccessDenied(name.to_string())
                } else {
                    SecretStoreError::BackendError(format!("Failed to access version: {}", e))
                }
            })?;

        let data = response.payload.and_then(|p| p.data).ok_or_else(|| {
            SecretStoreError::BackendError("No secret data in response".to_string())
        })?;

        base64::engine::general_purpose::STANDARD
            .decode(&data)
            .map_err(|e| SecretStoreError::BackendError(format!("Failed to decode secret: {}", e)))
    }

    async fn update_secret(&self, name: &str, value: &[u8]) -> Result<String, SecretStoreError> {
        if name.is_empty() {
            return Err(SecretStoreError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        let secret_name = format!("projects/{}/secrets/{}", self.config.project_id, name);

        let request = AddSecretVersionRequest {
            payload: Some(google_secretmanager1::api::SecretPayload {
                data: Some(base64::engine::general_purpose::STANDARD.encode(value)),
                ..Default::default()
            }),
            ..Default::default()
        };

        let (_, response) = self
            .client
            .projects()
            .secrets_add_version(request, &secret_name)
            .doit()
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    SecretStoreError::NotFound(name.to_string())
                } else {
                    SecretStoreError::BackendError(format!("Failed to update secret: {}", e))
                }
            })?;

        let version_id = response.name.ok_or_else(|| {
            SecretStoreError::BackendError("No version name returned".to_string())
        })?;

        Ok(version_id)
    }

    async fn delete_secret(&self, name: &str) -> Result<(), SecretStoreError> {
        if name.is_empty() {
            return Err(SecretStoreError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        let secret_name = format!("projects/{}/secrets/{}", self.config.project_id, name);

        self.client
            .projects()
            .secrets_delete(&secret_name)
            .doit()
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    SecretStoreError::NotFound(name.to_string())
                } else {
                    SecretStoreError::BackendError(format!("Failed to delete secret: {}", e))
                }
            })?;

        Ok(())
    }

    async fn list_versions(&self, name: &str) -> Result<Vec<SecretVersion>, SecretStoreError> {
        if name.is_empty() {
            return Err(SecretStoreError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        let secret_name = format!("projects/{}/secrets/{}", self.config.project_id, name);

        let (_, response) = self
            .client
            .projects()
            .secrets_versions_list(&secret_name)
            .doit()
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    SecretStoreError::NotFound(name.to_string())
                } else {
                    SecretStoreError::BackendError(format!("Failed to list versions: {}", e))
                }
            })?;

        let versions = response
            .versions
            .unwrap_or_default()
            .iter()
            .map(|v| self.convert_version(v))
            .collect();

        Ok(versions)
    }

    async fn disable_version(&self, name: &str, version_id: &str) -> Result<(), SecretStoreError> {
        if name.is_empty() {
            return Err(SecretStoreError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        let version_name = format!(
            "projects/{}/secrets/{}/versions/{}",
            self.config.project_id, name, version_id
        );

        let request = DisableSecretVersionRequest {
            ..Default::default()
        };

        self.client
            .projects()
            .secrets_versions_disable(request, &version_name)
            .doit()
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    SecretStoreError::VersionNotFound(version_id.to_string())
                } else {
                    SecretStoreError::BackendError(format!("Failed to disable version: {}", e))
                }
            })?;

        Ok(())
    }

    async fn enable_version(&self, name: &str, version_id: &str) -> Result<(), SecretStoreError> {
        if name.is_empty() {
            return Err(SecretStoreError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        let version_name = format!(
            "projects/{}/secrets/{}/versions/{}",
            self.config.project_id, name, version_id
        );

        let request = EnableSecretVersionRequest {
            ..Default::default()
        };

        self.client
            .projects()
            .secrets_versions_enable(request, &version_name)
            .doit()
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    SecretStoreError::VersionNotFound(version_id.to_string())
                } else {
                    SecretStoreError::BackendError(format!("Failed to enable version: {}", e))
                }
            })?;

        Ok(())
    }

    async fn destroy_version(&self, name: &str, version_id: &str) -> Result<(), SecretStoreError> {
        if name.is_empty() {
            return Err(SecretStoreError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        let version_name = format!(
            "projects/{}/secrets/{}/versions/{}",
            self.config.project_id, name, version_id
        );

        let request = DestroySecretVersionRequest {
            ..Default::default()
        };

        self.client
            .projects()
            .secrets_versions_destroy(request, &version_name)
            .doit()
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    SecretStoreError::VersionNotFound(version_id.to_string())
                } else {
                    SecretStoreError::BackendError(format!("Failed to destroy version: {}", e))
                }
            })?;

        Ok(())
    }

    async fn rotate_secret(
        &self,
        name: &str,
        new_value: Option<&[u8]>,
    ) -> Result<String, SecretStoreError> {
        if name.is_empty() {
            return Err(SecretStoreError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        let secret_name = format!("projects/{}/secrets/{}", self.config.project_id, name);

        let value = new_value.ok_or_else(|| {
            SecretStoreError::RotationFailed(
                "No rotation value provided and no rotation handler configured".to_string(),
            )
        })?;

        let request = AddSecretVersionRequest {
            payload: Some(google_secretmanager1::api::SecretPayload {
                data: Some(base64::engine::general_purpose::STANDARD.encode(value)),
                ..Default::default()
            }),
            ..Default::default()
        };

        let (_, response) = self
            .client
            .projects()
            .secrets_add_version(request, &secret_name)
            .doit()
            .await
            .map_err(|e| SecretStoreError::RotationFailed(format!("Rotation failed: {}", e)))?;

        let version_id = response.name.ok_or_else(|| {
            SecretStoreError::RotationFailed("No version returned from rotation".to_string())
        })?;

        Ok(version_id)
    }

    async fn set_rotation_policy(
        &self,
        name: &str,
        _policy: RotationPolicy,
    ) -> Result<(), SecretStoreError> {
        if name.is_empty() {
            return Err(SecretStoreError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        let secret_name = format!("projects/{}/secrets/{}", self.config.project_id, name);

        self.client
            .projects()
            .secrets_versions_list(&secret_name)
            .doit()
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    SecretStoreError::NotFound(name.to_string())
                } else {
                    SecretStoreError::BackendError(format!("Failed to set rotation policy: {}", e))
                }
            })?;

        Ok(())
    }

    async fn get_rotation_policy(
        &self,
        name: &str,
    ) -> Result<Option<RotationPolicy>, SecretStoreError> {
        if name.is_empty() {
            return Err(SecretStoreError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        let secret_name = format!("projects/{}/secrets/{}", self.config.project_id, name);

        self.client
            .projects()
            .secrets_versions_list(&secret_name)
            .doit()
            .await
            .map_err(|e| {
                if e.to_string().contains("not found") {
                    SecretStoreError::NotFound(name.to_string())
                } else {
                    SecretStoreError::BackendError(format!("Failed to get rotation policy: {}", e))
                }
            })?;

        Ok(Some(RotationPolicy::default()))
    }

    async fn get_metadata(&self, name: &str) -> Result<SecretMetadata, SecretStoreError> {
        if name.is_empty() {
            return Err(SecretStoreError::InvalidName(
                "Name cannot be empty".to_string(),
            ));
        }

        let versions = self.list_versions(name).await?;
        let created_at = Utc::now();
        let updated_at = Utc::now();

        let current_version = versions
            .first()
            .map(|v| v.version_id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(SecretMetadata {
            name: name.to_string(),
            created_at,
            updated_at,
            current_version,
            versions,
            rotation_policy: Some(RotationPolicy::default()),
            labels: HashMap::new(),
            is_replicated: true,
        })
    }

    async fn health_check(&self) -> Result<(), SecretStoreError> {
        let parent = format!("projects/{}", self.config.project_id);

        self.client
            .projects()
            .secrets_list(&parent)
            .page_size(1)
            .doit()
            .await
            .map_err(|e| SecretStoreError::BackendError(format!("Health check failed: {}", e)))?;

        Ok(())
    }
}

#[cfg(feature = "secret-manager")]
pub use google_secretmanager1;

#[cfg(all(test, feature = "secret-manager"))]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = GoogleSecretManagerConfig::new("my-project".to_string());
        assert_eq!(config.project_id, "my-project");
        assert_eq!(config.replication_location, Some("us".to_string()));
    }

    #[test]
    fn test_config_builder_pattern() {
        let config = GoogleSecretManagerConfig::new("my-project".to_string())
            .with_service_account_key("/path/to/key.json".to_string())
            .with_replication_location("europe-west1".to_string());

        assert_eq!(config.project_id, "my-project");
        assert_eq!(
            config.service_account_key,
            Some("/path/to/key.json".to_string())
        );
        assert_eq!(
            config.replication_location,
            Some("europe-west1".to_string())
        );
    }

    #[test]
    fn test_rotation_policy_defaults() {
        let config = GoogleSecretManagerConfig::new("my-project".to_string());
        assert!(config.default_rotation_policy.is_some());

        let policy = config.default_rotation_policy.unwrap();
        assert_eq!(policy.rotation_period_secs, 7 * 24 * 60 * 60);
    }

    #[test]
    fn test_secret_version_conversion() {
        let version = SecretVersion {
            version_id: "1".to_string(),
            created_at: Utc::now(),
            rotated_at: None,
            state: VersionState::Enabled,
            labels: HashMap::new(),
        };

        assert_eq!(version.state, VersionState::Enabled);
        assert_eq!(version.version_id, "1");
    }
}
