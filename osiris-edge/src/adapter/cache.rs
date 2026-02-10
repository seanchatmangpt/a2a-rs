//! Redis-based cache adapter
//!
//! Production-ready cache implementation using Redis with:
//! - TTL/expiration support
//! - Pattern-based invalidation
//! - Cache-aside pattern helper
//! - JSON serialization for generic types

#[cfg(feature = "redis")]
use async_trait::async_trait;
#[cfg(feature = "redis")]
use redis::Commands;
#[cfg(feature = "redis")]
use redis::aio::Connection;
#[cfg(feature = "redis")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "redis")]
use std::time::Duration;
#[cfg(feature = "redis")]
use tracing::{debug, warn};

#[cfg(feature = "redis")]
use crate::port::{Cache, CacheConfig, CacheError};

/// Redis cache adapter configuration
#[cfg(feature = "redis")]
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Redis connection URL (e.g., "redis://127.0.0.1:6379")
    pub url: String,
    /// Default TTL for entries (defaults to CacheConfig::default)
    pub config: CacheConfig,
    /// Optional key prefix to namespace cache entries
    pub key_prefix: String,
}

#[cfg(feature = "redis")]
impl RedisConfig {
    /// Create new Redis config with defaults
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            config: CacheConfig::default(),
            key_prefix: String::new(),
        }
    }

    /// Set the key prefix
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.key_prefix = prefix.into();
        self
    }

    /// Set the default TTL
    pub fn with_default_ttl(mut self, ttl_secs: u64) -> Self {
        self.config.default_ttl_secs = ttl_secs;
        self
    }

    /// Build the prefixed key
    fn build_key(&self, key: &str) -> String {
        if self.key_prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}:{}", self.key_prefix, key)
        }
    }
}

/// Redis-based cache implementation
///
/// Provides async generic caching with TTL support and pattern matching.
/// All stored values are JSON-serialized for type flexibility.
#[cfg(feature = "redis")]
pub struct RedisCache {
    client: redis::Client,
    config: RedisConfig,
}

#[cfg(feature = "redis")]
impl RedisCache {
    /// Create a new Redis cache instance
    ///
    /// # Errors
    ///
    /// Returns CacheError if Redis connection fails
    pub async fn new(config: RedisConfig) -> Result<Self, CacheError> {
        let client = redis::Client::open(config.url.as_str()).map_err(|e| {
            CacheError::BackendError(format!("Failed to create Redis client: {}", e))
        })?;

        // Test connection
        let mut conn = client
            .get_async_connection()
            .await
            .map_err(|e| CacheError::BackendError(format!("Failed to connect to Redis: {}", e)))?;

        // Ping to verify connection
        let _: () = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::BackendError(format!("Redis ping failed: {}", e)))?;

        debug!("Successfully connected to Redis at {}", config.url);

        Ok(Self { client, config })
    }

    /// Get a connection from the pool
    async fn get_connection(&self) -> Result<Connection, CacheError> {
        self.client
            .get_async_connection()
            .await
            .map_err(|e| CacheError::BackendError(format!("Failed to get Redis connection: {}", e)))
    }
}

#[cfg(feature = "redis")]
#[async_trait]
impl Cache for RedisCache {
    async fn get<T: for<'de> Deserialize<'de> + Send>(
        &self,
        key: &str,
    ) -> Result<Option<T>, CacheError> {
        let full_key = self.config.build_key(key);
        let mut conn = self.get_connection().await?;

        let json_str: Option<String> = conn
            .get(&full_key)
            .await
            .map_err(|e| CacheError::BackendError(format!("Redis GET failed: {}", e)))?;

        match json_str {
            Some(json) => {
                let value = serde_json::from_str::<T>(&json)
                    .map_err(|e| CacheError::DeserializationError(e.to_string()))?;
                debug!("Cache hit: {}", key);
                Ok(Some(value))
            }
            None => {
                debug!("Cache miss: {}", key);
                Ok(None)
            }
        }
    }

    async fn set<T: Serialize + Send>(
        &self,
        key: &str,
        value: &T,
        ttl: Duration,
    ) -> Result<(), CacheError> {
        let full_key = self.config.build_key(key);
        let secs = self.config.validate_ttl(ttl)?;
        let mut conn = self.get_connection().await?;

        let json_str = serde_json::to_string(value)
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;

        conn.set_ex(&full_key, json_str, secs as usize)
            .await
            .map_err(|e| CacheError::BackendError(format!("Redis SET failed: {}", e)))?;

        debug!("Cache set: {} (TTL: {}s)", key, secs);
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        let full_key = self.config.build_key(key);
        let mut conn = self.get_connection().await?;

        conn.del(&full_key)
            .await
            .map_err(|e| CacheError::BackendError(format!("Redis DEL failed: {}", e)))?;

        debug!("Cache deleted: {}", key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        let full_key = self.config.build_key(key);
        let mut conn = self.get_connection().await?;

        let exists: bool = conn
            .exists(&full_key)
            .await
            .map_err(|e| CacheError::BackendError(format!("Redis EXISTS failed: {}", e)))?;

        Ok(exists)
    }

    async fn ttl(&self, key: &str) -> Result<Option<u64>, CacheError> {
        let full_key = self.config.build_key(key);
        let mut conn = self.get_connection().await?;

        let ttl_secs: i64 = conn
            .ttl(&full_key)
            .await
            .map_err(|e| CacheError::BackendError(format!("Redis TTL failed: {}", e)))?;

        match ttl_secs {
            -2 => Ok(None), // Key doesn't exist
            -1 => Ok(None), // Key exists but has no expiration
            n if n >= 0 => Ok(Some(n as u64)),
            _ => Err(CacheError::BackendError(format!(
                "Unexpected TTL response: {}",
                ttl_secs
            ))),
        }
    }

    async fn invalidate_pattern(&self, pattern: &str) -> Result<usize, CacheError> {
        let full_pattern = self.config.build_key(pattern);
        let mut conn = self.get_connection().await?;

        // Use SCAN to find matching keys (safer than KEYS for large datasets)
        let mut cursor = 0u64;
        let mut deleted_count = 0usize;
        let mut keys_to_delete = Vec::new();

        loop {
            let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&full_pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
                .map_err(|e| CacheError::PatternError(format!("SCAN failed: {}", e)))?;

            keys_to_delete.extend(keys);
            cursor = new_cursor;

            if cursor == 0 {
                break;
            }
        }

        if !keys_to_delete.is_empty() {
            // Limit pattern results
            if keys_to_delete.len() > self.config.config.max_pattern_results {
                warn!(
                    "Pattern {} matches {} keys, limiting to {}",
                    pattern,
                    keys_to_delete.len(),
                    self.config.config.max_pattern_results
                );
                keys_to_delete.truncate(self.config.config.max_pattern_results);
            }

            deleted_count = keys_to_delete.len();
            conn.del(keys_to_delete)
                .await
                .map_err(|e| CacheError::BackendError(format!("Redis DEL failed: {}", e)))?;
        }

        debug!(
            "Pattern invalidation: {} matched {} keys",
            pattern, deleted_count
        );
        Ok(deleted_count)
    }

    async fn clear(&self) -> Result<(), CacheError> {
        let mut conn = self.get_connection().await?;

        if self.config.key_prefix.is_empty() {
            // Only clear if no prefix - safer to require explicit pattern
            warn!("Clearing entire Redis cache (no prefix)");
            redis::cmd("FLUSHDB")
                .query_async(&mut conn)
                .await
                .map_err(|e| CacheError::BackendError(format!("Redis FLUSHDB failed: {}", e)))?;
        } else {
            // Use pattern matching for prefixed caches
            self.invalidate_pattern("*").await?;
        }

        Ok(())
    }

    async fn count_pattern(&self, pattern: &str) -> Result<usize, CacheError> {
        let full_pattern = self.config.build_key(pattern);
        let mut conn = self.get_connection().await?;

        let mut cursor = 0u64;
        let mut count = 0usize;

        loop {
            let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&full_pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
                .map_err(|e| CacheError::PatternError(format!("SCAN failed: {}", e)))?;

            count += keys.len();
            cursor = new_cursor;

            if cursor == 0 {
                break;
            }
        }

        Ok(count)
    }
}

#[cfg(all(test, feature = "redis"))]
mod tests {
    use super::*;

    // Note: These tests require a running Redis instance on localhost:6379
    // Run with: cargo test --features redis -- --test-threads=1
    //
    // To start Redis in Docker:
    // docker run -d -p 6379:6379 redis:alpine

    async fn setup_cache() -> RedisCache {
        RedisCache::new(
            RedisConfig::new("redis://127.0.0.1:6379")
                .with_prefix("test")
                .with_default_ttl(60),
        )
        .await
        .expect("Failed to connect to Redis")
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_set_and_get() {
        let cache = setup_cache().await;
        let key = "test_key";
        let value = serde_json::json!({"id": 42, "name": "test"});

        cache
            .set(key, &value, Duration::from_secs(60))
            .await
            .expect("Set failed");

        let retrieved: Option<serde_json::Value> = cache.get(key).await.expect("Get failed");

        assert_eq!(retrieved, Some(value));

        cache.delete(key).await.expect("Delete failed");
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_ttl_validation() {
        let cache = setup_cache().await;
        let key = "ttl_test";
        let value = serde_json::json!({"test": true});

        // Test valid TTL
        cache
            .set(key, &value, Duration::from_secs(60))
            .await
            .expect("Set with valid TTL failed");

        // Test zero TTL
        let result = cache.set(key, &value, Duration::from_secs(0)).await;
        assert!(result.is_err());

        cache.delete(key).await.expect("Delete failed");
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_pattern_invalidation() {
        let cache = setup_cache().await;
        let value = serde_json::json!({"test": true});

        // Set multiple keys
        for i in 0..5 {
            let key = format!("user:{}", i);
            cache
                .set(&key, &value, Duration::from_secs(60))
                .await
                .expect("Set failed");
        }

        // Invalidate pattern
        let deleted = cache
            .invalidate_pattern("user:*")
            .await
            .expect("Pattern invalidation failed");

        assert_eq!(deleted, 5);

        // Verify they're gone
        for i in 0..5 {
            let key = format!("user:{}", i);
            let exists = cache.exists(&key).await.expect("Exists check failed");
            assert!(!exists);
        }
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_cache_aside() {
        let cache = setup_cache().await;
        let key = "expensive_compute";
        let mut load_count = 0;

        // First call should invoke loader
        let value1: serde_json::Value = cache
            .get_or_load(
                key,
                || async {
                    load_count += 1;
                    Ok(serde_json::json!({"computed": 42}))
                },
                Duration::from_secs(60),
            )
            .await
            .expect("get_or_load failed");

        // Second call should be from cache (loader not called)
        let value2: serde_json::Value = cache
            .get_or_load(
                key,
                || async {
                    load_count += 1;
                    Ok(serde_json::json!({"computed": 999}))
                },
                Duration::from_secs(60),
            )
            .await
            .expect("get_or_load failed");

        // Both values should be identical (cached)
        assert_eq!(value1, value2);

        cache.delete(key).await.expect("Delete failed");
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_prefix_isolation() {
        let cache1 = RedisCache::new(
            RedisConfig::new("redis://127.0.0.1:6379")
                .with_prefix("app1")
                .with_default_ttl(60),
        )
        .await
        .expect("Failed to connect to Redis");

        let cache2 = RedisCache::new(
            RedisConfig::new("redis://127.0.0.1:6379")
                .with_prefix("app2")
                .with_default_ttl(60),
        )
        .await
        .expect("Failed to connect to Redis");

        let key = "shared_key";
        let value1 = serde_json::json!({"app": "app1"});
        let value2 = serde_json::json!({"app": "app2"});

        // Set different values with same key in different prefixes
        cache1
            .set(key, &value1, Duration::from_secs(60))
            .await
            .expect("Set cache1 failed");

        cache2
            .set(key, &value2, Duration::from_secs(60))
            .await
            .expect("Set cache2 failed");

        // Verify isolation
        let retrieved1: Option<serde_json::Value> =
            cache1.get(key).await.expect("Get cache1 failed");
        let retrieved2: Option<serde_json::Value> =
            cache2.get(key).await.expect("Get cache2 failed");

        assert_eq!(retrieved1, Some(value1));
        assert_eq!(retrieved2, Some(value2));

        cache1.delete(key).await.expect("Delete cache1 failed");
        cache2.delete(key).await.expect("Delete cache2 failed");
    }
}
