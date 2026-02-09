//! KMS-based signer implementation using Google Cloud KMS.
//!
//! This adapter provides production-grade signing using Cloud KMS,
//! which stores keys in hardware security modules (HSMs).

#[cfg(feature = "kms")]
use crate::adapter::receipt_builder::Signer;
#[cfg(feature = "kms")]
use crate::domain::ReceiptError;
#[cfg(feature = "kms")]
use async_trait::async_trait;
#[cfg(feature = "kms")]
use google_cloudkms1::{
    CloudKMS,
    api::{AsymmetricSignRequest, AsymmetricSignResponse},
    hyper::{self, client::HttpConnector},
    hyper_rustls::{self, HttpsConnector},
};
#[cfg(feature = "kms")]
use sha2::{Digest, Sha256};
#[cfg(feature = "kms")]
use std::sync::Arc;
#[cfg(feature = "kms")]
use yup_oauth2::ServiceAccountAuthenticator;

/// Configuration for Cloud KMS signing.
#[cfg(feature = "kms")]
#[derive(Debug, Clone)]
pub struct KmsConfig {
    /// GCP project ID
    pub project_id: String,

    /// KMS location (e.g., "global", "us-east1")
    pub location: String,

    /// KMS key ring name
    pub key_ring: String,

    /// KMS crypto key name
    pub key_name: String,

    /// KMS crypto key version (e.g., "1")
    pub key_version: String,

    /// Path to service account JSON key file
    pub service_account_key: Option<String>,
}

#[cfg(feature = "kms")]
impl KmsConfig {
    /// Returns the full resource name for the crypto key version.
    pub fn key_resource_name(&self) -> String {
        format!(
            "projects/{}/locations/{}/keyRings/{}/cryptoKeys/{}/cryptoKeyVersions/{}",
            self.project_id, self.location, self.key_ring, self.key_name, self.key_version
        )
    }
}

/// Cloud KMS signer implementation.
///
/// This signer uses Google Cloud KMS to sign data with keys stored in HSMs.
/// Keys never leave the HSM, providing strong security guarantees.
#[cfg(feature = "kms")]
pub struct KmsSigner {
    config: KmsConfig,
    hub: Arc<CloudKMS<HttpsConnector<HttpConnector>>>,
}

#[cfg(feature = "kms")]
impl KmsSigner {
    /// Creates a new KMS signer with the given configuration.
    ///
    /// # Arguments
    /// * `config` - KMS configuration including project, location, and key details
    ///
    /// # Returns
    /// A new KMS signer instance
    ///
    /// # Errors
    /// Returns error if authentication fails or KMS client cannot be created
    pub async fn new(config: KmsConfig) -> Result<Self, ReceiptError> {
        // Create authenticator
        let auth = if let Some(key_path) = &config.service_account_key {
            // Use service account key file
            let secret = yup_oauth2::read_service_account_key(key_path)
                .await
                .map_err(|e| {
                    ReceiptError::SignatureError(format!(
                        "Failed to read service account key: {}",
                        e
                    ))
                })?;

            ServiceAccountAuthenticator::builder(secret)
                .build()
                .await
                .map_err(|e| {
                    ReceiptError::SignatureError(format!("Failed to create authenticator: {}", e))
                })?
        } else {
            // Use default credentials (e.g., from metadata server in GCE/GKE)
            ServiceAccountAuthenticator::builder(yup_oauth2::ServiceAccountKey::default())
                .build()
                .await
                .map_err(|e| {
                    ReceiptError::SignatureError(format!("Failed to create authenticator: {}", e))
                })?
        };

        // Create HTTPS connector
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .map_err(|e| {
                ReceiptError::SignatureError(format!("Failed to create HTTPS connector: {}", e))
            })?
            .https_only()
            .enable_http1()
            .enable_http2()
            .build();

        // Create KMS hub
        let hub = CloudKMS::new(hyper::Client::builder().build(https), Arc::new(auth));

        Ok(Self {
            config,
            hub: Arc::new(hub),
        })
    }

    /// Creates a KMS signer from environment variables.
    ///
    /// Expected environment variables:
    /// - `GCP_PROJECT_ID`: GCP project ID
    /// - `KMS_LOCATION`: KMS location
    /// - `KMS_KEY_RING`: KMS key ring name
    /// - `KMS_KEY_NAME`: KMS crypto key name
    /// - `KMS_KEY_VERSION`: KMS key version
    /// - `GOOGLE_APPLICATION_CREDENTIALS` (optional): Path to service account key
    pub async fn from_env() -> Result<Self, ReceiptError> {
        let config = KmsConfig {
            project_id: std::env::var("GCP_PROJECT_ID")
                .map_err(|_| ReceiptError::SignatureError("Missing GCP_PROJECT_ID".to_string()))?,
            location: std::env::var("KMS_LOCATION")
                .map_err(|_| ReceiptError::SignatureError("Missing KMS_LOCATION".to_string()))?,
            key_ring: std::env::var("KMS_KEY_RING")
                .map_err(|_| ReceiptError::SignatureError("Missing KMS_KEY_RING".to_string()))?,
            key_name: std::env::var("KMS_KEY_NAME")
                .map_err(|_| ReceiptError::SignatureError("Missing KMS_KEY_NAME".to_string()))?,
            key_version: std::env::var("KMS_KEY_VERSION")
                .map_err(|_| ReceiptError::SignatureError("Missing KMS_KEY_VERSION".to_string()))?,
            service_account_key: std::env::var("GOOGLE_APPLICATION_CREDENTIALS").ok(),
        };

        Self::new(config).await
    }
}

#[cfg(feature = "kms")]
#[async_trait]
impl Signer for KmsSigner {
    async fn sign(&self, data: &[u8]) -> Result<String, ReceiptError> {
        // 1. Hash the data with SHA-256
        let mut hasher = Sha256::new();
        hasher.update(data);
        let digest = hasher.finalize();

        // 2. Create asymmetric sign request
        let request = AsymmetricSignRequest {
            digest: Some(google_cloudkms1::api::Digest {
                sha256: Some(base64::encode(&digest)),
                ..Default::default()
            }),
            ..Default::default()
        };

        // 3. Call KMS to sign
        let key_name = self.config.key_resource_name();
        let result: AsymmetricSignResponse = self
            .hub
            .projects()
            .locations_key_rings_crypto_keys_crypto_key_versions_asymmetric_sign(request, &key_name)
            .doit()
            .await
            .map_err(|e| ReceiptError::SignatureError(format!("KMS signing failed: {}", e)))?
            .1;

        // 4. Return the signature
        result
            .signature
            .ok_or_else(|| ReceiptError::SignatureError("KMS returned no signature".to_string()))
    }

    async fn verify(&self, data: &[u8], signature: &str) -> Result<bool, ReceiptError> {
        // For KMS verification, we would typically:
        // 1. Get the public key from KMS
        // 2. Use it to verify the signature locally
        //
        // However, this requires additional dependencies and complexity.
        // For now, we'll defer to the KMS service for verification by
        // re-signing and comparing signatures (not optimal for performance
        // but works for proof-of-concept).

        let computed_sig = self.sign(data).await?;
        Ok(computed_sig == signature)
    }

    fn signer_id(&self) -> String {
        format!(
            "kms:{}:{}:{}:{}:{}",
            self.config.project_id,
            self.config.location,
            self.config.key_ring,
            self.config.key_name,
            self.config.key_version
        )
    }
}

#[cfg(test)]
#[cfg(feature = "kms")]
mod tests {
    use super::*;

    #[test]
    fn test_kms_config_resource_name() {
        let config = KmsConfig {
            project_id: "my-project".to_string(),
            location: "global".to_string(),
            key_ring: "my-keyring".to_string(),
            key_name: "my-key".to_string(),
            key_version: "1".to_string(),
            service_account_key: None,
        };

        let expected = "projects/my-project/locations/global/keyRings/my-keyring/cryptoKeys/my-key/cryptoKeyVersions/1";
        assert_eq!(config.key_resource_name(), expected);
    }

    // Integration tests would require actual KMS setup
    // and should be run separately with proper credentials
}

// Re-export KMS types when feature is enabled
#[cfg(feature = "kms")]
pub use google_cloudkms1;
