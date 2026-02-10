//! Redis cache demonstration
//!
//! This example shows how to use the Redis cache adapter with:
//! - Basic get/set operations
//! - TTL configuration and validation
//! - Cache-aside pattern for expensive computations
//! - Pattern-based cache invalidation
//! - Batch operations

use osiris_edge::{Cache, CacheConfig, RedisCache, RedisConfig};
use serde_json::json;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("Redis Cache Demo\n");

    // Create cache with custom configuration
    let cache_config = CacheConfig {
        default_ttl_secs: 300,
        max_ttl_secs: 3600,
        max_pattern_results: 1000,
    };

    let redis_config = RedisConfig::new("redis://127.0.0.1:6379")
        .with_prefix("demo")
        .with_default_ttl(300);

    // Connect to Redis
    println!("1. Connecting to Redis...");
    let cache = RedisCache::new(redis_config).await?;
    println!("   ✓ Connected successfully\n");

    // Basic operations
    println!("2. Basic set/get operations:");
    let user_data = json!({
        "id": 123,
        "name": "Alice",
        "email": "alice@example.com",
        "roles": ["admin", "user"]
    });

    cache
        .set("user:123", &user_data, Duration::from_secs(600))
        .await?;
    println!("   ✓ Set user:123");

    if let Ok(Some(retrieved)) = cache.get::<serde_json::Value>("user:123").await {
        println!("   ✓ Retrieved: {}\n", retrieved);
    }

    // Check TTL
    println!("3. TTL operations:");
    if let Ok(Some(ttl)) = cache.ttl("user:123").await {
        println!("   ✓ TTL for user:123: {}s\n", ttl);
    }

    // Cache-aside pattern (expensive computation)
    println!("4. Cache-aside pattern (lazy loading):");
    let expensive_key = "expensive:computation:42";
    let result: serde_json::Value = cache
        .get_or_load(
            expensive_key,
            || async {
                println!("   → Computing expensive value...");
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(json!({
                    "result": 42,
                    "computed_at": chrono::Utc::now().to_rfc3339()
                }))
            },
            Duration::from_secs(600),
        )
        .await?;
    println!("   ✓ First call (computed): {}", result);

    // Second call should be cached
    println!("   → Calling again (should be cached)...");
    let cached_result: serde_json::Value = cache
        .get_or_load(
            expensive_key,
            || async {
                println!("   ✗ This should not print!");
                Ok(json!({"should": "not appear"}))
            },
            Duration::from_secs(600),
        )
        .await?;
    println!("   ✓ Second call (cached): {}\n", cached_result);

    // Pattern-based operations
    println!("5. Pattern-based cache invalidation:");
    // Set multiple session keys
    for i in 0..5 {
        let session_key = format!("session:user:{}", i);
        let session_data = json!({
            "user_id": i,
            "session_token": format!("token_{}", i),
            "expires_at": chrono::Utc::now().to_rfc3339()
        });
        cache
            .set(&session_key, &session_data, Duration::from_secs(600))
            .await?;
    }
    println!("   ✓ Created 5 session keys");

    // Count keys matching pattern
    let count = cache.count_pattern("session:user:*").await?;
    println!("   ✓ Found {} keys matching 'session:user:*'", count);

    // Invalidate all sessions
    let invalidated = cache.invalidate_pattern("session:user:*").await?;
    println!("   ✓ Invalidated {} session keys\n", invalidated);

    // Verify they're gone
    let count_after = cache.count_pattern("session:user:*").await?;
    println!("   ✓ Remaining keys: {}\n", count_after);

    // Batch operations
    println!("6. Batch operations:");
    let batch_items = vec![
        (
            "product:1",
            json!({"id": 1, "name": "Widget", "price": 9.99}),
        ),
        (
            "product:2",
            json!({"id": 2, "name": "Gadget", "price": 19.99}),
        ),
        (
            "product:3",
            json!({"id": 3, "name": "Doohickey", "price": 14.99}),
        ),
    ];

    // Build references for batch set
    let refs: Vec<_> = batch_items
        .iter()
        .map(|(k, v)| (k.as_str(), v, Duration::from_secs(600)))
        .collect();

    cache.mset(&refs).await?;
    println!("   ✓ Batch set 3 products");

    // Batch get
    let keys = vec!["product:1", "product:2", "product:3"];
    let results: Vec<Option<serde_json::Value>> = cache.mget(&keys).await?;
    for (key, result) in keys.iter().zip(results.iter()) {
        if let Some(value) = result {
            println!("   ✓ {}: {}", key, value);
        }
    }
    println!();

    // Delete operations
    println!("7. Delete operations:");
    cache.delete("user:123").await?;
    println!("   ✓ Deleted user:123");

    let exists = cache.exists("user:123").await?;
    println!("   ✓ user:123 exists: {}\n", exists);

    // Prefix isolation example
    println!("8. Prefix isolation (different cache instances):");
    let cache_app1 =
        RedisCache::new(RedisConfig::new("redis://127.0.0.1:6379").with_prefix("app1")).await?;

    let cache_app2 =
        RedisCache::new(RedisConfig::new("redis://127.0.0.1:6379").with_prefix("app2")).await?;

    let shared_key = "config:version";
    let app1_config = json!({"version": 1, "app": "app1"});
    let app2_config = json!({"version": 2, "app": "app2"});

    cache_app1
        .set(shared_key, &app1_config, Duration::from_secs(600))
        .await?;
    cache_app2
        .set(shared_key, &app2_config, Duration::from_secs(600))
        .await?;

    let retrieved_app1: Option<serde_json::Value> = cache_app1.get(shared_key).await?;
    let retrieved_app2: Option<serde_json::Value> = cache_app2.get(shared_key).await?;

    println!("   ✓ app1 config: {}", retrieved_app1.unwrap_or_default());
    println!("   ✓ app2 config: {}", retrieved_app2.unwrap_or_default());
    println!("   ✓ Prefixes kept data isolated\n");

    // Error handling example
    println!("9. Error handling:");

    // Invalid TTL (too large)
    match cache
        .set("test", &json!({}), Duration::from_secs(100000))
        .await
    {
        Err(e) => println!("   ✓ Caught invalid TTL error: {}", e),
        Ok(_) => println!("   ✗ Should have failed"),
    }

    // Zero TTL
    match cache.set("test", &json!({}), Duration::from_secs(0)).await {
        Err(e) => println!("   ✓ Caught zero TTL error: {}\n", e),
        Ok(_) => println!("   ✗ Should have failed"),
    }

    // Performance characteristics
    println!("10. Performance characteristics:");
    println!("    - O(1) get/set/delete/exists");
    println!("    - O(N) pattern matching (uses SCAN cursor)");
    println!("    - Atomic set with TTL (SET EX)");
    println!("    - JSON serialization for type flexibility");
    println!("    - Connection pooling via redis client");

    println!("\n✓ All examples completed successfully!");

    Ok(())
}
