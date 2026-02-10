# Redis Cache Implementation (osiris-edge, 2026-02-10)

## Summary

Production-ready async Redis cache with TTL, pattern-based invalidation, and cache-aside pattern support.

## Architecture

```
Application Layer (HTTP handlers, business logic)
    ↓
Cache Port (port/cache.rs)
    - Async trait with generic bounds
    - Error types
    - Config validation
    ↓
RedisCache Adapter (adapter/cache.rs)
    - JSON serialization (serde_json)
    - Connection pooling (redis client)
    - SCAN-based pattern matching
    ↓
Redis Server
```

## Port Trait Design

**File**: `src/port/cache.rs` (200 lines)

### Key Methods

```rust
#[async_trait]
pub trait Cache: Send + Sync {
    // Core operations
    async fn get<T: Deserialize>(key: &str) -> Result<Option<T>>;
    async fn set<T: Serialize>(key: &str, value: &T, ttl: Duration) -> Result<()>;
    async fn delete(key: &str) -> Result<()>;
    async fn exists(key: &str) -> Result<bool>;
    async fn ttl(key: &str) -> Result<Option<u64>>;  // Returns seconds

    // Pattern matching
    async fn invalidate_pattern(pattern: &str) -> Result<usize>;  // Returns count deleted
    async fn count_pattern(pattern: &str) -> Result<usize>;
    async fn clear() -> Result<()>;

    // Cache-aside pattern
    async fn get_or_load<T, F, Fut>(
        key: &str,
        loader: F,  // async fn returning Result<T, String>
        ttl: Duration
    ) -> Result<T> { ... }

    // Batch operations
    async fn mget<T>(keys: &[&str]) -> Result<Vec<Option<T>>>;
    async fn mset<T>(items: &[(&str, &T, Duration)]) -> Result<()>;
}
```

### Error Types

```rust
pub enum CacheError {
    SerializationError(String),
    DeserializationError(String),
    BackendError(String),
    KeyNotFound(String),
    InvalidTtl(String),
    PatternError(String),
}

// Converts to EdgeError
impl From<CacheError> for EdgeError { ... }
```

### Configuration

```rust
pub struct CacheConfig {
    pub default_ttl_secs: u64,        // 1 hour default
    pub max_ttl_secs: u64,             // 24 hours max
    pub max_pattern_results: usize,    // 10000 limit
}

impl CacheConfig {
    pub fn validate_ttl(&self, ttl: Duration) -> Result<u64, CacheError> {
        // Rejects zero and exceeds max
    }
}
```

## Adapter Implementation

**File**: `src/adapter/cache.rs` (500+ lines)

### Configuration

```rust
pub struct RedisConfig {
    pub url: String,           // e.g., "redis://127.0.0.1:6379"
    pub config: CacheConfig,
    pub key_prefix: String,    // "" for no prefix
}

impl RedisConfig {
    pub fn new(url: impl Into<String>) -> Self;
    pub fn with_prefix(self, prefix: impl Into<String>) -> Self;
    pub fn with_default_ttl(self, ttl_secs: u64) -> Self;

    fn build_key(&self, key: &str) -> String {
        if self.key_prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}:{}", self.key_prefix, key)
        }
    }
}
```

### Connection Management

```rust
pub struct RedisCache {
    client: redis::Client,
    config: RedisConfig,
}

impl RedisCache {
    pub async fn new(config: RedisConfig) -> Result<Self, CacheError> {
        // Create client
        let client = redis::Client::open(...)?;

        // Test connection with PING
        let mut conn = client.get_async_connection().await?;
        redis::cmd("PING").query_async(&mut conn).await?;

        Ok(Self { client, config })
    }

    async fn get_connection(&self) -> Result<Connection, CacheError> {
        self.client.get_async_connection().await
    }
}
```

### Implementation Details

**Serialization**:
- Use `serde_json::to_string()` for SET
- Use `serde_json::from_str()` for GET
- Allows any `Serialize + Deserialize` type
- JSON provides cross-language compatibility

**TTL Handling**:
- Use Redis `SET key value EX seconds` (atomic)
- Validate before operations
- `validate_ttl()` checks bounds and rejects zero

**Pattern Matching**:
- Use SCAN cursor instead of KEYS (non-blocking)
- Cursor loop until cursor returns 0
- Collect matching keys, then DEL batch
- Respect `max_pattern_results` limit
- Log warning if truncated

**Implementation**:
```rust
async fn invalidate_pattern(&self, pattern: &str) -> Result<usize, CacheError> {
    let full_pattern = self.config.build_key(pattern);
    let mut conn = self.get_connection().await?;

    let mut cursor = 0u64;
    let mut keys_to_delete = Vec::new();

    loop {
        let (new_cursor, keys): (u64, Vec<String>) =
            redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH").arg(&full_pattern)
                .arg("COUNT").arg(100)
                .query_async(&mut conn)
                .await?;

        keys_to_delete.extend(keys);
        cursor = new_cursor;
        if cursor == 0 { break; }
    }

    if keys_to_delete.len() > max_pattern_results {
        keys_to_delete.truncate(max_pattern_results);
    }

    let count = keys_to_delete.len();
    if count > 0 {
        conn.del(keys_to_delete).await?;
    }

    Ok(count)
}
```

**Clear Operation**:
```rust
async fn clear(&self) -> Result<(), CacheError> {
    if self.config.key_prefix.is_empty() {
        // Direct FLUSHDB only if no prefix (safer)
        redis::cmd("FLUSHDB")
            .query_async(&mut conn)
            .await?;
    } else {
        // Use pattern matching for prefixed caches
        self.invalidate_pattern("*").await?;
    }
    Ok(())
}
```

## Cache-Aside Pattern

**Design**:
- Loader called ONLY on cache miss
- Loader errors are NOT cached
- Success results are cached with TTL

**Implementation**:
```rust
async fn get_or_load<T, F, Fut>(
    &self,
    key: &str,
    loader: F,
    ttl: Duration,
) -> Result<T, CacheError>
where
    T: Serialize + Deserialize + Send,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, String>>,
{
    // Try cache first
    if let Ok(Some(cached)) = self.get::<T>(key).await {
        return Ok(cached);
    }

    // Cache miss: call loader
    let value = loader()
        .await
        .map_err(|e| CacheError::BackendError(e))?;

    // Store result (ignore cache errors)
    let _ = self.set(key, &value, ttl).await;

    Ok(value)
}
```

**Semantics**:
- Non-blocking: loader called directly if miss
- Fault-tolerant: cache failures don't prevent work
- Type-safe: T must be Serialize + Deserialize

## Multi-Tenant Pattern

Use prefix isolation to separate tenants/applications:

```rust
// Tenant A - all keys prefixed with "tenant_a:"
let cache_a = RedisCache::new(
    RedisConfig::new("redis://127.0.0.1:6379")
        .with_prefix("tenant_a")
).await?;

// Tenant B - all keys prefixed with "tenant_b:"
let cache_b = RedisCache::new(
    RedisConfig::new("redis://127.0.0.1:6379")
        .with_prefix("tenant_b")
).await?;

// Same logical key, different physical keys
cache_a.set("config", &{"v": 1}, ...).await?;
cache_b.set("config", &{"v": 2}, ...).await?;

// Each retrieves their own data
let a = cache_a.get("config").await?;  // {"v": 1}
let b = cache_b.get("config").await?;  // {"v": 2}
```

## Feature Gating

**Cargo.toml**:
```toml
[dependencies]
redis = { version = "0.26", features = ["aio", "tokio-comp"], optional = true }

[features]
redis = ["dep:redis"]
```

**Code**:
```rust
#[cfg(feature = "redis")]
use async_trait::async_trait;

#[cfg(feature = "redis")]
pub struct RedisCache { ... }

#[cfg(feature = "redis")]
#[async_trait]
impl Cache for RedisCache { ... }
```

**Module Exports**:
```rust
// adapter/mod.rs
#[cfg(feature = "redis")]
pub use cache::RedisCache;
pub use cache::RedisConfig;

// lib.rs
#[cfg(feature = "redis")]
pub use adapter::RedisCache;
```

## Testing

Tests are in `adapter/cache.rs` with `#[ignore]` attribute:

```bash
# Requires Redis running
docker run -d -p 6379:6379 redis:alpine

# Run tests
cargo test --features redis -- --test-threads=1 --ignored
```

**Test Coverage**:
- `test_set_and_get` - Basic operations
- `test_ttl_validation` - Bounds checking
- `test_pattern_invalidation` - SCAN-based matching
- `test_cache_aside` - Loader calling semantics
- `test_prefix_isolation` - Multi-tenant isolation

## Common Patterns

### HTTP Handler Integration

```rust
#[derive(Clone)]
pub struct AppState {
    cache: Arc<RedisCache>,
}

async fn get_user_handler(
    State(state): State<AppState>,
    Path(user_id): Path<u32>,
) -> impl IntoResponse {
    match state.cache.get_or_load(
        &format!("user:{}", user_id),
        || async { fetch_user(user_id).await.map_err(|e| e.to_string()) },
        Duration::from_secs(3600),
    ).await {
        Ok(user) => Json(user),
        Err(e) => {
            error!("Cache error: {}", e);
            // Handle error
        }
    }
}
```

### Batch Operations

```rust
// Multiple gets
let user_ids = vec!["user:1", "user:2", "user:3"];
let users: Vec<Option<User>> = cache.mget(&user_ids).await?;

// Multiple sets
let items = vec![
    ("user:1", &user1, Duration::from_secs(3600)),
    ("user:2", &user2, Duration::from_secs(3600)),
];
cache.mset(&items).await?;
```

### Session Invalidation

```rust
// Invalidate all sessions for a user
let deleted = cache.invalidate_pattern(&format!("session:{}:*", user_id)).await?;
println!("Invalidated {} sessions", deleted);
```

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| get/set/delete | O(1) | Redis native |
| pattern match | O(N) | N = matching keys, SCAN cursor used |
| count_pattern | O(N) | Counts via cursor iteration |
| batch mget/mset | O(K) | K = number of items |
| Pattern with 10k keys | ~50ms | SCAN with COUNT=100 batches |

## Error Handling

**Conversion Chain**:
```
CacheError::BackendError(e)
    ↓
EdgeError::Internal(msg)
    ↓ (in HTTP handler)
StatusCode::INTERNAL_SERVER_ERROR
```

**Graceful Degradation**:
```rust
// Cache failure doesn't prevent work
match cache.get_or_load(key, loader, ttl).await {
    Ok(value) => respond_with(value),
    Err(CacheError::BackendError(_)) => {
        // Call loader anyway
        let value = loader().await?;
        respond_with(value)  // Uncompensated latency
    }
}
```

## Known Limitations

1. **Pattern matching is slow with large key counts**: Use specific patterns, avoid wildcards
2. **No WATCH/transaction support**: Single operations only (redis-rs limitation)
3. **No eviction policy in adapter**: Relies on Redis server maxmemory policy
4. **JSON overhead**: Not suitable for high-frequency microsecond operations
5. **No compression**: Large values pay serialization cost

## Next Steps

- Integrate into HTTP handlers for request caching
- Warm cache on startup for critical data
- Monitor cache hit rates with metrics
- Add cache purge strategies
- Consider compression for large values (gzip + feature gate)
