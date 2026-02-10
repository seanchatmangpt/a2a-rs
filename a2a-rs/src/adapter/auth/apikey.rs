//! API Key authentication with bcrypt hashing, key rotation, and rate limiting
//!
//! Features:
//! - Secure key generation (32 bytes, base64-encoded)
//! - Bcrypt hash storage for security
//! - Key rotation with grace period
//! - Per-key rate limiting
//! - Metadata tracking (creation time, last used, etc.)
//!
//! Example:
//! ```no_run
//! use a2a_rs::adapter::auth::ApiKeyManager;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let manager = ApiKeyManager::new();
//!
//! // Generate a new API key
//! let (key_id, api_key) = manager.generate_key("user-123").await?;
//! println!("API Key ID: {}, Key: {}", key_id, api_key);
//!
//! // Rotate a key
//! let (new_key_id, new_api_key) = manager.rotate_key(&key_id, 3600).await?;
//! # Ok(())
//! # }
//! ```

#[cfg(feature = "auth")]
use async_trait::async_trait;
#[cfg(feature = "auth")]
use bcrypt::{hash, verify, DEFAULT_COST};
#[cfg(feature = "auth")]
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
#[cfg(feature = "auth")]
use tokio::sync::RwLock;

use crate::{
    domain::{core::agent::SecurityScheme, A2AError},
    port::authenticator::{AuthContext, AuthPrincipal, Authenticator},
};

/// Metadata associated with an API key
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyMetadata {
    /// Unique identifier for this key
    pub key_id: String,
    /// User or agent ID this key belongs to
    pub owner_id: String,
    /// When the key was created
    pub created_at: SystemTime,
    /// When the key was last used (None if never used)
    pub last_used: Option<SystemTime>,
    /// When the key expires (None for no expiration)
    pub expires_at: Option<SystemTime>,
    /// If this key was rotated from another key, the old key ID
    pub rotated_from: Option<String>,
    /// If this key has been rotated, the new key ID
    pub rotated_to: Option<String>,
    /// Whether the key is active
    pub active: bool,
    /// Custom metadata
    pub custom: HashMap<String, String>,
}

impl ApiKeyMetadata {
    /// Create new metadata for a key
    pub fn new(key_id: String, owner_id: String) -> Self {
        Self {
            key_id,
            owner_id,
            created_at: SystemTime::now(),
            last_used: None,
            expires_at: None,
            rotated_from: None,
            rotated_to: None,
            active: true,
            custom: HashMap::new(),
        }
    }

    /// Check if the key is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            SystemTime::now() > expires_at
        } else {
            false
        }
    }

    /// Mark the key as used
    pub fn mark_used(&mut self) {
        self.last_used = Some(SystemTime::now());
    }

    /// Set expiration time
    pub fn with_expiration(mut self, duration: Duration) -> Self {
        self.expires_at = Some(SystemTime::now() + duration);
        self
    }

    /// Add custom metadata
    pub fn with_custom(mut self, key: String, value: String) -> Self {
        self.custom.insert(key, value);
        self
    }
}

/// Stored API key with bcrypt hash
#[cfg(feature = "auth")]
#[derive(Debug, Clone)]
struct StoredApiKey {
    /// Bcrypt hash of the API key
    hash: String,
    /// Metadata
    metadata: ApiKeyMetadata,
}

/// Rate limit state for a single key
#[cfg(feature = "auth")]
#[derive(Debug, Clone)]
struct RateLimitState {
    /// Number of requests in current window
    count: u32,
    /// When the current window started
    window_start: SystemTime,
    /// Maximum requests per window
    limit: u32,
    /// Window duration in seconds
    window_duration: u64,
}

#[cfg(feature = "auth")]
impl RateLimitState {
    fn new(limit: u32, window_duration: u64) -> Self {
        Self {
            count: 0,
            window_start: SystemTime::now(),
            limit,
            window_duration,
        }
    }

    fn check_and_increment(&mut self) -> Result<(), A2AError> {
        let now = SystemTime::now();
        let elapsed = now
            .duration_since(self.window_start)
            .unwrap_or(Duration::from_secs(0));

        // Reset window if expired
        if elapsed.as_secs() >= self.window_duration {
            self.count = 0;
            self.window_start = now;
        }

        // Check limit
        if self.count >= self.limit {
            let retry_after = Duration::from_secs(self.window_duration) - elapsed;
            return Err(A2AError::RateLimited {
                retry_after: retry_after.as_secs() as u32,
            });
        }

        self.count += 1;
        Ok(())
    }
}

/// API Key manager with bcrypt hashing, rotation, and rate limiting
#[cfg(feature = "auth")]
#[derive(Clone)]
pub struct ApiKeyManager {
    /// Stored API keys (key_id -> StoredApiKey)
    keys: Arc<RwLock<HashMap<String, StoredApiKey>>>,
    /// Rate limit state per key
    rate_limits: Arc<RwLock<HashMap<String, RateLimitState>>>,
    /// Default rate limit (requests per minute)
    default_rate_limit: u32,
    /// Rate limit window duration in seconds
    rate_limit_window: u64,
    /// Security scheme configuration
    scheme: SecurityScheme,
    /// Bcrypt cost (default: 12)
    bcrypt_cost: u32,
}

#[cfg(feature = "auth")]
impl ApiKeyManager {
    /// Create a new API key manager with default settings
    pub fn new() -> Self {
        Self {
            keys: Arc::new(RwLock::new(HashMap::new())),
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
            default_rate_limit: 100, // 100 requests per minute
            rate_limit_window: 60,   // 60 seconds
            scheme: SecurityScheme::ApiKey {
                location: "header".to_string(),
                name: "X-API-Key".to_string(),
                description: Some("API Key authentication with bcrypt hashing".to_string()),
            },
            bcrypt_cost: DEFAULT_COST,
        }
    }

    /// Create with custom rate limit
    pub fn with_rate_limit(mut self, requests_per_minute: u32) -> Self {
        self.default_rate_limit = requests_per_minute;
        self
    }

    /// Create with custom rate limit window
    pub fn with_rate_limit_window(mut self, window_seconds: u64) -> Self {
        self.rate_limit_window = window_seconds;
        self
    }

    /// Create with custom bcrypt cost
    pub fn with_bcrypt_cost(mut self, cost: u32) -> Self {
        self.bcrypt_cost = cost;
        self
    }

    /// Generate a secure API key (32 bytes, base64-encoded)
    pub fn generate_secure_key() -> String {
        let mut rng = rand::thread_rng();
        let key_bytes: [u8; 32] = rng.gen();
        base64::encode(&key_bytes)
    }

    /// Generate a new API key for an owner
    ///
    /// Returns (key_id, api_key) where api_key is the raw key to give to the user
    pub async fn generate_key(&self, owner_id: &str) -> Result<(String, String), A2AError> {
        let key_id = uuid::Uuid::new_v4().to_string();
        let api_key = Self::generate_secure_key();

        // Hash the key with bcrypt
        let hash = hash(&api_key, self.bcrypt_cost)
            .map_err(|e| A2AError::Internal(format!("Failed to hash API key: {}", e)))?;

        let metadata = ApiKeyMetadata::new(key_id.clone(), owner_id.to_string());

        let stored_key = StoredApiKey { hash, metadata };

        // Store the key
        let mut keys = self.keys.write().await;
        keys.insert(key_id.clone(), stored_key);

        // Initialize rate limit state
        let mut rate_limits = self.rate_limits.write().await;
        rate_limits.insert(
            key_id.clone(),
            RateLimitState::new(self.default_rate_limit, self.rate_limit_window),
        );

        Ok((key_id, api_key))
    }

    /// Generate a key with custom expiration
    pub async fn generate_key_with_expiration(
        &self,
        owner_id: &str,
        expires_in: Duration,
    ) -> Result<(String, String), A2AError> {
        let (key_id, api_key) = self.generate_key(owner_id).await?;

        // Update metadata with expiration
        let mut keys = self.keys.write().await;
        if let Some(stored_key) = keys.get_mut(&key_id) {
            stored_key.metadata.expires_at = Some(SystemTime::now() + expires_in);
        }

        Ok((key_id, api_key))
    }

    /// Rotate an API key
    ///
    /// Creates a new key and marks the old one as rotated. The old key remains
    /// valid for the grace period.
    ///
    /// Returns (new_key_id, new_api_key)
    pub async fn rotate_key(
        &self,
        old_key_id: &str,
        grace_period_seconds: u64,
    ) -> Result<(String, String), A2AError> {
        // Get the old key's owner
        let owner_id = {
            let keys = self.keys.read().await;
            let old_key = keys
                .get(old_key_id)
                .ok_or_else(|| A2AError::Internal("API key not found".to_string()))?;
            old_key.metadata.owner_id.clone()
        };

        // Generate new key
        let (new_key_id, new_api_key) = self.generate_key(&owner_id).await?;

        // Update old key metadata
        let mut keys = self.keys.write().await;
        if let Some(old_key) = keys.get_mut(old_key_id) {
            old_key.metadata.rotated_to = Some(new_key_id.clone());
            old_key.metadata.expires_at = Some(
                SystemTime::now() + Duration::from_secs(grace_period_seconds),
            );
        }

        // Update new key metadata
        if let Some(new_key) = keys.get_mut(&new_key_id) {
            new_key.metadata.rotated_from = Some(old_key_id.to_string());
        }

        Ok((new_key_id, new_api_key))
    }

    /// Revoke an API key immediately
    pub async fn revoke_key(&self, key_id: &str) -> Result<(), A2AError> {
        let mut keys = self.keys.write().await;
        let key = keys
            .get_mut(key_id)
            .ok_or_else(|| A2AError::Internal("API key not found".to_string()))?;

        key.metadata.active = false;
        Ok(())
    }

    /// Verify an API key and return the key ID if valid
    async fn verify_key(&self, api_key: &str) -> Result<String, A2AError> {
        let keys = self.keys.read().await;

        // Try to verify against all stored keys
        for (key_id, stored_key) in keys.iter() {
            // Skip inactive keys
            if !stored_key.metadata.active {
                continue;
            }

            // Skip expired keys
            if stored_key.metadata.is_expired() {
                continue;
            }

            // Verify the hash
            match verify(api_key, &stored_key.hash) {
                Ok(true) => return Ok(key_id.clone()),
                Ok(false) => continue,
                Err(_) => continue,
            }
        }

        Err(A2AError::Internal("Invalid API key".to_string()))
    }

    /// Check rate limit for a key
    async fn check_rate_limit(&self, key_id: &str) -> Result<(), A2AError> {
        let mut rate_limits = self.rate_limits.write().await;

        let rate_limit = rate_limits
            .entry(key_id.to_string())
            .or_insert_with(|| RateLimitState::new(self.default_rate_limit, self.rate_limit_window));

        rate_limit.check_and_increment()
    }

    /// Update last used timestamp for a key
    async fn mark_key_used(&self, key_id: &str) {
        let mut keys = self.keys.write().await;
        if let Some(stored_key) = keys.get_mut(key_id) {
            stored_key.metadata.mark_used();
        }
    }

    /// Get metadata for a key
    pub async fn get_metadata(&self, key_id: &str) -> Option<ApiKeyMetadata> {
        let keys = self.keys.read().await;
        keys.get(key_id).map(|k| k.metadata.clone())
    }

    /// List all keys for an owner
    pub async fn list_keys(&self, owner_id: &str) -> Vec<ApiKeyMetadata> {
        let keys = self.keys.read().await;
        keys.values()
            .filter(|k| k.metadata.owner_id == owner_id)
            .map(|k| k.metadata.clone())
            .collect()
    }

    /// Get rate limit statistics for a key
    pub async fn get_rate_limit_stats(&self, key_id: &str) -> Option<(u32, u32)> {
        let rate_limits = self.rate_limits.read().await;
        rate_limits.get(key_id).map(|rl| (rl.count, rl.limit))
    }
}

#[cfg(feature = "auth")]
impl Default for ApiKeyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "auth")]
#[async_trait]
impl Authenticator for ApiKeyManager {
    async fn authenticate(&self, context: &AuthContext) -> Result<AuthPrincipal, A2AError> {
        self.validate_context(context)?;

        let api_key = &context.credential;

        // Verify the key
        let key_id = self.verify_key(api_key).await?;

        // Check rate limit
        self.check_rate_limit(&key_id).await?;

        // Mark key as used
        self.mark_key_used(&key_id).await;

        // Get metadata
        let metadata = self
            .get_metadata(&key_id)
            .await
            .ok_or_else(|| A2AError::Internal("Key metadata not found".to_string()))?;

        // Build principal
        let mut principal = AuthPrincipal::new(metadata.owner_id.clone(), "apikey".to_string());

        principal = principal
            .with_attribute("key_id".to_string(), key_id.clone())
            .with_attribute(
                "created_at".to_string(),
                format!(
                    "{:?}",
                    metadata
                        .created_at
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                ),
            );

        // Add custom metadata as attributes
        for (key, value) in metadata.custom {
            principal = principal.with_attribute(key, value);
        }

        Ok(principal)
    }

    fn security_scheme(&self) -> &SecurityScheme {
        &self.scheme
    }

    fn validate_context(&self, context: &AuthContext) -> Result<(), A2AError> {
        if context.scheme_type != "apikey" {
            return Err(A2AError::Internal(format!(
                "Invalid authentication scheme: expected 'apikey', got '{}'",
                context.scheme_type
            )));
        }
        Ok(())
    }
}

#[cfg(not(feature = "auth"))]
/// Placeholder when auth feature is not enabled
pub struct ApiKeyManager;

#[cfg(not(feature = "auth"))]
impl ApiKeyManager {
    pub fn new() -> Self {
        compile_error!("API Key authentication requires the 'auth' feature");
    }
}

#[cfg(all(test, feature = "auth"))]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let key = ApiKeyManager::generate_secure_key();
        assert!(!key.is_empty());

        // Base64-encoded 32 bytes should be 44 characters
        assert_eq!(key.len(), 44);
    }

    #[tokio::test]
    async fn test_generate_and_verify_key() {
        let manager = ApiKeyManager::new();

        // Generate a key
        let (key_id, api_key) = manager
            .generate_key("user-123")
            .await
            .expect("Failed to generate key");

        assert!(!key_id.is_empty());
        assert!(!api_key.is_empty());

        // Verify it
        let verified_key_id = manager
            .verify_key(&api_key)
            .await
            .expect("Failed to verify key");
        assert_eq!(verified_key_id, key_id);
    }

    #[tokio::test]
    async fn test_verify_invalid_key() {
        let manager = ApiKeyManager::new();

        // Try to verify a key that doesn't exist
        let result = manager.verify_key("invalid-key").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_key_rotation() {
        let manager = ApiKeyManager::new();

        // Generate initial key
        let (old_key_id, old_api_key) = manager
            .generate_key("user-123")
            .await
            .expect("Failed to generate key");

        // Rotate it
        let (new_key_id, new_api_key) = manager
            .rotate_key(&old_key_id, 3600)
            .await
            .expect("Failed to rotate key");

        assert_ne!(old_key_id, new_key_id);
        assert_ne!(old_api_key, new_api_key);

        // Both keys should work during grace period
        assert!(manager.verify_key(&old_api_key).await.is_ok());
        assert!(manager.verify_key(&new_api_key).await.is_ok());

        // Check metadata linkage
        let new_metadata = manager.get_metadata(&new_key_id).await.unwrap();
        assert_eq!(new_metadata.rotated_from, Some(old_key_id.clone()));

        let old_metadata = manager.get_metadata(&old_key_id).await.unwrap();
        assert_eq!(old_metadata.rotated_to, Some(new_key_id));
    }

    #[tokio::test]
    async fn test_key_revocation() {
        let manager = ApiKeyManager::new();

        let (key_id, api_key) = manager
            .generate_key("user-123")
            .await
            .expect("Failed to generate key");

        // Key should work before revocation
        assert!(manager.verify_key(&api_key).await.is_ok());

        // Revoke the key
        manager.revoke_key(&key_id).await.expect("Failed to revoke");

        // Key should not work after revocation
        assert!(manager.verify_key(&api_key).await.is_err());
    }

    #[tokio::test]
    async fn test_key_expiration() {
        let manager = ApiKeyManager::new();

        // Generate key with 1 second expiration
        let (_key_id, api_key) = manager
            .generate_key_with_expiration("user-123", Duration::from_secs(1))
            .await
            .expect("Failed to generate key");

        // Should work immediately
        assert!(manager.verify_key(&api_key).await.is_ok());

        // Wait for expiration
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should not work after expiration
        assert!(manager.verify_key(&api_key).await.is_err());
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let manager = ApiKeyManager::new().with_rate_limit(5).with_rate_limit_window(60);

        let (key_id, _api_key) = manager
            .generate_key("user-123")
            .await
            .expect("Failed to generate key");

        // First 5 requests should succeed
        for _ in 0..5 {
            assert!(manager.check_rate_limit(&key_id).await.is_ok());
        }

        // 6th request should fail
        let result = manager.check_rate_limit(&key_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_authentication() {
        let manager = ApiKeyManager::new();

        let (_key_id, api_key) = manager
            .generate_key("user-123")
            .await
            .expect("Failed to generate key");

        let context = AuthContext::new("apikey".to_string(), api_key);

        let principal = manager
            .authenticate(&context)
            .await
            .expect("Authentication failed");

        assert_eq!(principal.id, "user-123");
        assert_eq!(principal.scheme, "apikey");
    }

    #[tokio::test]
    async fn test_list_keys() {
        let manager = ApiKeyManager::new();

        // Generate multiple keys for the same owner
        manager
            .generate_key("user-123")
            .await
            .expect("Failed to generate key");
        manager
            .generate_key("user-123")
            .await
            .expect("Failed to generate key");
        manager
            .generate_key("user-456")
            .await
            .expect("Failed to generate key");

        let user123_keys = manager.list_keys("user-123").await;
        assert_eq!(user123_keys.len(), 2);

        let user456_keys = manager.list_keys("user-456").await;
        assert_eq!(user456_keys.len(), 1);
    }

    #[tokio::test]
    async fn test_metadata_tracking() {
        let manager = ApiKeyManager::new();

        let (key_id, api_key) = manager
            .generate_key("user-123")
            .await
            .expect("Failed to generate key");

        // Initially, last_used should be None
        let metadata = manager.get_metadata(&key_id).await.unwrap();
        assert!(metadata.last_used.is_none());

        // Verify the key (which marks it as used)
        manager.verify_key(&api_key).await.expect("Failed to verify");

        // After verification, last_used should be Some
        let metadata = manager.get_metadata(&key_id).await.unwrap();
        assert!(metadata.last_used.is_some());
    }
}
