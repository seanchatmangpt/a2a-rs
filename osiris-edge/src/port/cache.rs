//! Cache port definitions
//!
//! Defines the interface for caching with TTL and invalidation support.
//! The cache supports generic serializable types via JSON serialization.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Cache-related errors
#[derive(Debug, Clone)]
pub enum CacheError {
    /// Serialization error
    SerializationError(String),
    /// Deserialization error
    DeserializationError(String),
    /// Cache backend error (Redis, etc.)
    BackendError(String),
    /// Key not found
    KeyNotFound(String),
    /// Invalid TTL
    InvalidTtl(String),
    /// Pattern matching error
    PatternError(String),
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            CacheError::DeserializationError(msg) => write!(f, "Deserialization error: {}", msg),
            CacheError::BackendError(msg) => write!(f, "Cache backend error: {}", msg),
            CacheError::KeyNotFound(key) => write!(f, "Cache key not found: {}", key),
            CacheError::InvalidTtl(msg) => write!(f, "Invalid TTL: {}", msg),
            CacheError::PatternError(msg) => write!(f, "Pattern error: {}", msg),
        }
    }
}

impl std::error::Error for CacheError {}

impl From<CacheError> for crate::domain::EdgeError {
    fn from(err: CacheError) -> Self {
        crate::domain::EdgeError::Internal(err.to_string())
    }
}

/// Configuration for cache operations
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Default TTL for cache entries (in seconds)
    pub default_ttl_secs: u64,
    /// Maximum TTL for cache entries (in seconds)
    pub max_ttl_secs: u64,
    /// Maximum number of keys to return in pattern matching
    pub max_pattern_results: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            default_ttl_secs: 3600, // 1 hour
            max_ttl_secs: 86400,    // 24 hours
            max_pattern_results: 10000,
        }
    }
}

impl CacheConfig {
    /// Validate and normalize a TTL
    pub fn validate_ttl(&self, ttl: Duration) -> Result<u64, CacheError> {
        let secs = ttl.as_secs();
        if secs == 0 {
            return Err(CacheError::InvalidTtl(
                "TTL must be greater than zero".to_string(),
            ));
        }
        if secs > self.max_ttl_secs {
            return Err(CacheError::InvalidTtl(format!(
                "TTL exceeds maximum of {} seconds",
                self.max_ttl_secs
            )));
        }
        Ok(secs)
    }
}

/// Asynchronous generic cache interface
///
/// Supports caching of any Serialize + Deserialize types with TTL and pattern-based invalidation.
/// Implements the cache-aside pattern for lazy loading of expensive computations.
#[async_trait]
pub trait Cache: Send + Sync {
    /// Get a value from the cache
    ///
    /// Returns None if the key doesn't exist or has expired
    async fn get<T: for<'de> Deserialize<'de> + Send>(
        &self,
        key: &str,
    ) -> Result<Option<T>, CacheError>;

    /// Set a value in the cache with TTL
    ///
    /// Overwrites existing values
    async fn set<T: Serialize + Send>(
        &self,
        key: &str,
        value: &T,
        ttl: Duration,
    ) -> Result<(), CacheError>;

    /// Delete a key from the cache
    ///
    /// Returns Ok even if the key doesn't exist
    async fn delete(&self, key: &str) -> Result<(), CacheError>;

    /// Check if a key exists
    async fn exists(&self, key: &str) -> Result<bool, CacheError>;

    /// Get the TTL of a key in seconds
    ///
    /// Returns None if key doesn't exist or has no expiration
    async fn ttl(&self, key: &str) -> Result<Option<u64>, CacheError>;

    /// Delete all keys matching a pattern (glob-style)
    ///
    /// Pattern examples: "user:*", "session:*:data", "*:recent"
    async fn invalidate_pattern(&self, pattern: &str) -> Result<usize, CacheError>;

    /// Clear the entire cache
    ///
    /// Warning: This is destructive for all cached data
    async fn clear(&self) -> Result<(), CacheError>;

    /// Cache-aside pattern: get or load
    ///
    /// Atomically gets from cache or calls loader function if miss.
    /// The loader result is cached with the specified TTL.
    /// Loader errors are NOT cached.
    async fn get_or_load<T, F, Fut>(
        &self,
        key: &str,
        loader: F,
        ttl: Duration,
    ) -> Result<T, CacheError>
    where
        T: Serialize + for<'de> Deserialize<'de> + Send,
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<T, String>> + Send,
    {
        // Try to get from cache
        if let Ok(Some(cached)) = self.get::<T>(key).await {
            return Ok(cached);
        }

        // Cache miss: call loader
        let value = loader().await.map_err(|e| CacheError::BackendError(e))?;

        // Store result in cache (ignore errors, we'll return value anyway)
        let _ = self.set(key, &value, ttl).await;

        Ok(value)
    }

    /// Batch get multiple keys
    ///
    /// Errors are returned as None in the result vector
    async fn mget<T: for<'de> Deserialize<'de> + Send>(
        &self,
        keys: &[&str],
    ) -> Result<Vec<Option<T>>, CacheError> {
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            results.push(self.get::<T>(key).await.ok().flatten());
        }
        Ok(results)
    }

    /// Batch set multiple keys
    async fn mset<T: Serialize + Send + Sync>(
        &self,
        items: &[(&str, &T, Duration)],
    ) -> Result<(), CacheError> {
        for (key, value, ttl) in items {
            self.set(key, value, *ttl).await?;
        }
        Ok(())
    }

    /// Get count of keys matching pattern
    async fn count_pattern(&self, pattern: &str) -> Result<usize, CacheError>;
}
