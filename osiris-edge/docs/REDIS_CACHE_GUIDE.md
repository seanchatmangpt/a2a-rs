# Redis Cache Guide for osiris-edge

## Overview

The Redis cache adapter provides a production-ready, async-first caching layer with:

- **Generic type support**: Cache any `Serialize + Deserialize` type via JSON
- **TTL management**: Automatic expiration with validation
- **Pattern-based invalidation**: Efficient bulk cache invalidation using glob-style patterns
- **Cache-aside pattern**: Built-in helper for lazy loading expensive computations
- **Batch operations**: `mget` and `mset` for efficient bulk operations
- **Key prefixing**: Namespace isolation for multi-tenant/multi-app scenarios
- **SCAN-based safety**: Pattern matching uses cursor-based SCAN instead of blocking KEYS

## Architecture

```
┌─────────────────────────────────────┐
│  Application Layer                  │
│  (HTTP handlers, business logic)    │
└──────────────┬──────────────────────┘
               │ uses trait
               ▼
┌─────────────────────────────────────┐
│  Cache Port (port/cache.rs)         │
│  - Async trait definition            │
│  - Generic type bounds               │
│  - Error types                       │
└──────────────┬──────────────────────┘
               │ implements
               ▼
┌─────────────────────────────────────┐
│  Redis Adapter (adapter/cache.rs)   │
│  - Redis 0.26 backend                │
│  - JSON serialization                │
│  - Connection pooling                │
│  - Error conversion                  │
└──────────────┬──────────────────────┘
               │ connects to
               ▼
            Redis
```

## Feature Flag

Enable Redis support in `Cargo.toml`:

```toml
[dependencies]
osiris-edge = { path = ".", features = ["redis"] }
```

## Configuration

### RedisConfig

The `RedisConfig` builder pattern provides flexible configuration:

```rust
use osiris_edge::RedisConfig;

let config = RedisConfig::new("redis://127.0.0.1:6379")
    .with_prefix("myapp")           // Namespace prefix
    .with_default_ttl(3600);         // Default TTL in seconds
```

Configuration options:

| Option | Default | Purpose |
|--------|---------|---------|
| `url` | Required | Redis connection URL (supports AUTH: `redis://:password@host:port`) |
| `key_prefix` | Empty | Prefix for all keys (enables multi-tenant isolation) |
| `default_ttl_secs` | 3600 | Default TTL for entries (1 hour) |
| `max_ttl_secs` | 86400 | Maximum allowed TTL (24 hours) |
| `max_pattern_results` | 10000 | Maximum keys returned in pattern matching |

## Core Operations

### Basic Operations

```rust
use osiris_edge::{Cache, RedisCache, RedisConfig};
use serde_json::json;
use std::time::Duration;

let cache = RedisCache::new(
    RedisConfig::new("redis://127.0.0.1:6379")
).await?;

// Set a value with TTL
let data = json!({"user": "alice", "role": "admin"});
cache.set("user:alice", &data, Duration::from_secs(3600)).await?;

// Get a value
let retrieved: Option<serde_json::Value> = cache.get("user:alice").await?;

// Check existence
let exists: bool = cache.exists("user:alice").await?;

// Get TTL
let ttl: Option<u64> = cache.ttl("user:alice").await?;  // Returns seconds

// Delete
cache.delete("user:alice").await?;
```

### Cache-Aside Pattern

The cache-aside pattern is essential for wrapping expensive operations:

```rust
// Generic async function that computes expensive value
async fn fetch_user_from_database(id: u32) -> Result<User, String> {
    // Simulated expensive database query
    tokio::time::sleep(Duration::from_millis(100)).await;
    Ok(User { id, name: format!("User{}", id) })
}

// Use cache-aside for automatic caching
let user: User = cache.get_or_load(
    &format!("user:{}", user_id),
    || fetch_user_from_database(user_id),
    Duration::from_secs(3600)
).await?;
```

**Semantics**:
- First call: Cache miss → Calls loader → Stores result
- Subsequent calls: Cache hit → Returns immediately
- Loader errors are **not cached** (only successful results)
- Works with any `Serialize + Deserialize + Send` type

### Pattern-Based Operations

Efficiently invalidate multiple keys matching a pattern:

```rust
// Set multiple related keys
for session_id in 0..1000 {
    cache.set(
        &format!("session:{}:data", session_id),
        &session_data,
        Duration::from_secs(600)
    ).await?;
}

// Invalidate all sessions matching pattern
let deleted = cache.invalidate_pattern("session:*:data").await?;
println!("Deleted {} sessions", deleted);

// Count matching keys
let count = cache.count_pattern("session:*:data").await?;
println!("Remaining: {} keys", count);

// Patterns use glob-style syntax:
// - "user:*"         matches all user keys
// - "cache:*:data"   matches cache keys with data suffix
// - "*:temp:*"       matches temp keys anywhere
```

**Implementation**:
- Uses Redis SCAN cursor for safe iteration (doesn't block)
- Respects `max_pattern_results` limit
- Logs warnings if limit exceeded
- O(N) complexity in key count, not dataset size

### Batch Operations

Process multiple keys efficiently:

```rust
// Batch get
let keys = vec!["user:1", "user:2", "user:3"];
let values: Vec<Option<User>> = cache.mget(&keys).await?;

// Batch set
let items = vec![
    ("user:1", &user1, Duration::from_secs(3600)),
    ("user:2", &user2, Duration::from_secs(3600)),
    ("user:3", &user3, Duration::from_secs(3600)),
];
cache.mset(&items).await?;
```

## Multi-Tenant/Multi-App Pattern

Use key prefixing to isolate cache between tenants or applications:

```rust
// Tenant A
let cache_tenant_a = RedisCache::new(
    RedisConfig::new("redis://127.0.0.1:6379")
        .with_prefix("tenant_a")
).await?;

// Tenant B
let cache_tenant_b = RedisCache::new(
    RedisConfig::new("redis://127.0.0.1:6379")
        .with_prefix("tenant_b")
).await?;

// Both use key "config:version" but data is isolated
cache_tenant_a.set("config:version", &{"version": 1}, ...).await?;
cache_tenant_b.set("config:version", &{"version": 2}, ...).await?;

// Each tenant retrieves their own data
let a_version = cache_tenant_a.get("config:version").await?;  // 1
let b_version = cache_tenant_b.get("config:version").await?;  // 2
```

**Internal implementation**: Prefixes are applied as `prefix:key` at the Redis level.

## Error Handling

The `CacheError` enum covers all failure scenarios:

```rust
use osiris_edge::CacheError;

match cache.set("key", &value, ttl).await {
    Ok(_) => println!("Success"),
    Err(CacheError::SerializationError(e)) => {
        // Value failed to serialize to JSON
        eprintln!("Failed to serialize: {}", e);
    }
    Err(CacheError::DeserializationError(e)) => {
        // Cached value failed to deserialize
        eprintln!("Failed to deserialize: {}", e);
    }
    Err(CacheError::BackendError(e)) => {
        // Redis connection or operation failed
        eprintln!("Redis error: {}", e);
    }
    Err(CacheError::InvalidTtl(e)) => {
        // TTL outside valid range or zero
        eprintln!("Invalid TTL: {}", e);
    }
    Err(CacheError::KeyNotFound(key)) => {
        // Key doesn't exist (not always an error)
    }
    Err(CacheError::PatternError(e)) => {
        // Pattern matching failed
        eprintln!("Pattern error: {}", e);
    }
}
```

**Convert to EdgeError**:

```rust
use osiris_edge::EdgeError;

let result: Result<_, EdgeError> = cache.set("key", &value, ttl)
    .await
    .map_err(|e| EdgeError::from(e));
```

## TTL Validation

TTLs are validated against `CacheConfig` limits:

```rust
use std::time::Duration;

let config = CacheConfig {
    default_ttl_secs: 3600,
    max_ttl_secs: 86400,  // 24 hours
    max_pattern_results: 10000,
};

// Valid
cache.set("key", &value, Duration::from_secs(3600)).await?;  // OK

// Error: Zero TTL
cache.set("key", &value, Duration::from_secs(0)).await
    .expect_err("Should reject zero TTL");

// Error: Exceeds max
cache.set("key", &value, Duration::from_secs(100000)).await
    .expect_err("Should reject TTL > max_ttl_secs");
```

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| `get()` | O(1) | Redis GET command |
| `set()` | O(1) | Redis SET EX (atomic) |
| `delete()` | O(1) | Redis DEL command |
| `exists()` | O(1) | Redis EXISTS command |
| `ttl()` | O(1) | Redis TTL command |
| `invalidate_pattern()` | O(N) | N = number of matching keys; uses SCAN |
| `count_pattern()` | O(N) | Counts matches via SCAN iteration |
| `mget()` | O(K) | K = number of keys in request |
| `mset()` | O(K) | K = number of items |

## Connection Management

The `RedisCache` uses async connection pooling:

```rust
let cache = RedisCache::new(config).await?;

// Each operation gets a connection from the pool
cache.get("key1").await?;     // Pool connection 1
cache.get("key2").await?;     // Pool connection 2
cache.set("key3", ...).await?;  // Pool connection 3

// Connections auto-returned to pool after operation
```

The redis crate handles connection pooling transparently. Concurrent operations can safely share the same `RedisCache` instance (it's `Send + Sync`).

## Testing

Run tests against a local Redis instance:

```bash
# Start Redis in Docker
docker run -d -p 6379:6379 redis:alpine

# Run cache tests
cargo test --features redis -- --test-threads=1 --ignored
```

Included tests:
- `test_set_and_get` - Basic operations
- `test_ttl_validation` - TTL bounds checking
- `test_pattern_invalidation` - Pattern matching
- `test_cache_aside` - Cache-aside pattern
- `test_prefix_isolation` - Prefix namespacing

## Example Usage in HTTP Handler

```rust
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};

#[derive(Clone)]
pub struct AppState {
    cache: Arc<RedisCache>,
}

pub async fn get_user_handler(
    State(state): State<AppState>,
    axum::extract::Path(user_id): axum::extract::Path<u32>,
) -> impl IntoResponse {
    let cache = &state.cache;

    match cache.get_or_load::<User, _, _>(
        &format!("user:{}", user_id),
        || async {
            // Expensive database query
            load_user_from_db(user_id).await.map_err(|e| e.to_string())
        },
        Duration::from_secs(3600),
    ).await {
        Ok(user) => (StatusCode::OK, Json(user)).into_response(),
        Err(e) => {
            tracing::error!("Cache error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal error"}))
            ).into_response()
        }
    }
}
```

## Debugging

Enable tracing for cache operations:

```rust
use tracing::Level;

tracing_subscriber::fmt()
    .with_max_level(Level::DEBUG)
    .init();

// Cache debug output:
// DEBUG osiris_edge::adapter::cache: Successfully connected to Redis at redis://127.0.0.1:6379
// DEBUG osiris_edge::adapter::cache: Cache hit: user:alice
// DEBUG osiris_edge::adapter::cache: Cache set: user:bob (TTL: 3600s)
// DEBUG osiris_edge::adapter::cache: Cache deleted: user:charlie
// DEBUG osiris_edge::adapter::cache: Pattern invalidation: session:* matched 42 keys
```

## Troubleshooting

### Connection errors

```
CacheError::BackendError("Failed to connect to Redis: ...")
```

- Verify Redis is running: `redis-cli ping` should return `PONG`
- Check connection URL format
- Verify network connectivity and firewall rules

### Serialization errors

```
CacheError::SerializationError("...")
CacheError::DeserializationError("...")
```

- Ensure types implement `Serialize + Deserialize`
- Check for recursive structures that serde can't handle
- Use `serde(skip)` for fields you don't want to cache

### TTL errors

```
CacheError::InvalidTtl("...")
```

- TTL must be > 0 seconds
- TTL must be ≤ `CacheConfig::max_ttl_secs`
- Adjust `CacheConfig` if you need longer TTLs

### Performance degradation

- Monitor `invalidate_pattern()` operations - they iterate all matching keys
- Set `max_pattern_results` appropriately for your use case
- Consider using specific key patterns instead of wildcards
- Use batch operations for bulk work

## Feature Flags

The Redis support is optional:

```toml
# In Cargo.toml
[dependencies]
osiris-edge = { path = "osiris-edge", features = ["redis"] }

# Or conditionally in code
#[cfg(feature = "redis")]
let cache = RedisCache::new(config).await?;
```

## Next Steps

- See `examples/redis_cache_demo.rs` for a complete working example
- Run the demo: `cargo run --example redis_cache_demo --features redis`
- Integrate into HTTP handlers for request caching
- Use cache-aside pattern for database query wrapping
- Implement cache warming strategies for critical data
