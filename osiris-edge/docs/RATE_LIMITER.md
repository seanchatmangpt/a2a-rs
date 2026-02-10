# Rate Limiter Implementation

## Overview

The Osiris-Edge rate limiter provides token bucket-based rate limiting with support for:
- **Per-IP address limits** - Different limits for each source IP
- **Per-tenant limits** - Different limits for each tenant ID
- **Global gateway limits** - Single limit for the entire gateway
- **Axum middleware integration** - Ready-to-use HTTP middleware

## Architecture

### Port-Adapter Pattern

```
port/rate_limiter.rs
    └─ RateLimiter trait (async, non-blocking, three-level limits)

adapter/rate_limiter.rs
    └─ TokenBucketRateLimiter (concrete implementation)

application/rate_limit_middleware.rs
    └─ Axum middleware layer for HTTP requests
```

### Token Bucket Algorithm

The token bucket algorithm refills at a constant rate and allows bursting up to the bucket capacity:

1. **Tokens available**: Start with capacity equal to refill rate
2. **Request arrives**: Consume N tokens (default N=1)
3. **Refill**: Tokens continuously refill at the configured rate
4. **Rate control**: When tokens run out, requests are rejected with retry-after time

### Three-Level Rate Limiting

The rate limiter enforces limits at three independent levels:

```
Request
  ├─ Check Global Limit (gateway-wide)
  ├─ Check IP Limit (per IP address)
  └─ Check Tenant Limit (per tenant ID)
```

All three limits must pass for a request to succeed.

## Port Interface

The `RateLimiter` trait defines the port:

```rust
#[async_trait]
pub trait RateLimiter: Send + Sync {
    // Check all applicable limits
    async fn check_rate_limit(&self, key: &str, tokens: u32) -> RateLimitResult;

    // Check specific limit levels
    async fn check_ip_limit(&self, ip: &str, tokens: u32) -> RateLimitResult;
    async fn check_tenant_limit(&self, tenant_id: &str, tokens: u32) -> RateLimitResult;
    async fn check_global_limit(&self, tokens: u32) -> RateLimitResult;

    // Query current state
    async fn get_rate(&self, key: &str) -> u32;
    async fn get_global_rate(&self) -> u32;
    async fn get_limit(&self, key: &str) -> u32;

    // Reset all limits (for testing)
    async fn reset(&self);

    // Configuration access
    fn config(&self) -> RateLimitConfig;
}
```

## Configuration

The `RateLimitConfig` struct configures the three limits:

```rust
pub struct RateLimitConfig {
    pub per_ip_rps: u32,         // Requests per second per IP
    pub per_tenant_rps: u32,     // Requests per second per tenant
    pub global_rps: u32,         // Requests per second globally
    pub window_secs: u64,        // Time window (typically 1 second)
}
```

### Predefined Configurations

```rust
// Default: 1000 req/s per IP, 5000 req/s per tenant, 10000 req/s global
let config = RateLimitConfig::default();

// Strict: 10 req/s per IP, 50 req/s per tenant, 100 req/s global (for testing)
let config = RateLimitConfig::strict();

// Custom
let config = RateLimitConfig::new(
    1000,  // per_ip_rps
    5000,  // per_tenant_rps
    10000, // global_rps
    1,     // window_secs
);
```

## Adapter Implementation

The `TokenBucketRateLimiter` is the concrete implementation:

```rust
pub struct TokenBucketRateLimiter {
    config: RateLimitConfig,
    ip_limiters: Arc<RwLock<HashMap<String, TokenBucketState>>>,
    tenant_limiters: Arc<RwLock<HashMap<String, TokenBucketState>>>,
    global_limiter: Arc<RwLock<TokenBucketState>>,
}
```

### Key Features

1. **Non-blocking**: All operations are async and non-blocking
2. **Thread-safe**: Uses `Arc<RwLock<T>>` for shared state
3. **Per-key isolation**: Each IP and tenant has independent state
4. **Automatic refill**: Tokens refill continuously based on elapsed time
5. **Efficient**: O(1) lookup for per-IP and per-tenant limiters

## Axum Middleware

The rate limit middleware integrates with Axum HTTP server:

```rust
pub struct RateLimitMiddlewareConfig {
    pub check_ip: bool,      // Check per-IP limits
    pub check_tenant: bool,  // Check per-tenant limits
    pub check_global: bool,  // Check global limits
}
```

### Usage

```rust
use osiris_edge::{TokenBucketRateLimiter, RateLimitMiddlewareConfig, rate_limit_layer};
use axum::Router;
use std::sync::Arc;

// Create limiter
let limiter = Arc::new(TokenBucketRateLimiter::default());

// Configure middleware
let config = RateLimitMiddlewareConfig::all();

// Apply to router
let app = Router::new()
    .route("/api/endpoint", post(handler))
    .layer(rate_limit_layer(limiter, config));
```

### Request Processing

The middleware:

1. **Extracts IP**: From X-Forwarded-For → X-Real-IP → socket address
2. **Extracts Tenant ID**: From X-Tenant-ID header (optional)
3. **Checks limits**: In order: global → IP → tenant
4. **Rejects**: Returns HTTP 429 with Retry-After header if any limit exceeded
5. **Allows**: Continues to next handler if all checks pass

## Error Handling

The `RateLimitError` enum covers three error cases:

```rust
pub enum RateLimitError {
    RateLimitExceeded {
        key: String,
        current_rate: u32,
        limit: u32,
        retry_after_secs: u64,
    },
    ConfigurationError(String),
    InvalidKey(String),
}
```

HTTP responses use HTTP 429 (Too Many Requests) with:
- `Retry-After` header (seconds to wait)
- JSON body with error details

## Usage Examples

### Basic Rate Limiting

```rust
let config = RateLimitConfig::new(100, 500, 1000, 1);
let limiter = TokenBucketRateLimiter::new(config);

match limiter.check_ip_limit("192.168.1.1", 1).await {
    Ok(()) => println!("Request allowed"),
    Err(e) => println!("Request rejected: {}", e),
}
```

### Per-Tenant Rate Limiting

```rust
match limiter.check_tenant_limit("tenant-123", 1).await {
    Ok(()) => println!("Request allowed"),
    Err(e) => println!("Request rejected: {}", e),
}
```

### Global Rate Limiting

```rust
match limiter.check_global_limit(1).await {
    Ok(()) => println!("Request allowed"),
    Err(e) => println!("Request rejected: {}", e),
}
```

### Querying Current State

```rust
// Get current rate (requests consumed in window)
let rate = limiter.get_rate("192.168.1.1").await;

// Get configured limit
let limit = limiter.get_limit("192.168.1.1").await;

// Get global rate
let global_rate = limiter.get_global_rate().await;
```

## Testing

The implementation includes comprehensive tests:

```bash
# Run rate limiter tests
cargo test -p osiris-edge rate_limiter

# Run middleware tests
cargo test -p osiris-edge rate_limit_middleware
```

Test coverage includes:
- Basic rate limiting (allow/reject)
- Per-IP isolation
- Per-tenant isolation
- Global limits
- Token bucket refill
- Error responses
- Configuration variants

## Performance Characteristics

### Time Complexity
- `check_*_limit()`: O(1) average, O(n) worst case (HashMap resize)
- `get_rate()`: O(1)
- `reset()`: O(n) where n = number of tracked keys

### Space Complexity
- Per-IP limiters: O(n) where n = unique IPs
- Per-tenant limiters: O(m) where m = unique tenants
- Global limiter: O(1)

### Recommendations

1. **Per-IP limits**: Good for API gateways with diverse clients
2. **Per-tenant limits**: Good for multi-tenant SaaS platforms
3. **Global limits**: Good for database/backend capacity management
4. **Combination**: All three levels for defense-in-depth

## Integration Points

### With WIP Gate

The rate limiter complements the WIP gate:

```
Rate Limiter (HTTP layer)
    ↓
    └─→ Enforces request rate (tokens per second)

WIP Gate (application layer)
    ↓
    └─→ Enforces concurrent work (bounded concurrency)
```

### With Authentication

Use tenant ID from authenticated token:

```rust
// Extract from JWT claim
let tenant_id = token.claims().tenant_id;

// Check tenant limit
limiter.check_tenant_limit(&tenant_id, 1).await?;
```

### With Observability

Rate limiter logs rejections:

```
warn!(ip, current_rate, limit, "IP rate limit exceeded")
warn!(tenant_id, current_rate, limit, "Tenant rate limit exceeded")
warn!(current_rate, limit, "Global rate limit exceeded")
```

## Configuration Best Practices

1. **Development**: Use `RateLimitConfig::strict()` (10/50/100 req/s)
2. **Testing**: Adjust window_secs to 1 for fast test cycles
3. **Production**:
   - Per-IP: 1000+ req/s (depends on client profile)
   - Per-tenant: 5000+ req/s (sum of tenant's IPs)
   - Global: 10000+ req/s (total gateway capacity)

4. **Monitoring**:
   - Track rejection rate per IP
   - Track rejection rate per tenant
   - Alert on sustained high rejection rate

## Limitations and Future Work

### Current Limitations

1. **No persistence**: State is in-memory only (resets on restart)
2. **Single-instance**: No distributed state across multiple servers
3. **No sliding window**: Fixed time windows (can cause cliff effects)
4. **IP spoofing**: Relies on proxy headers (validate X-Forwarded-For)

### Future Enhancements

1. **Distributed rate limiting**: Redis-backed state for multi-instance deployments
2. **Sliding window**: Smoother rate limiting without cliff effects
3. **Hierarchical limits**: Parent tenant → sub-tenant limits
4. **Cost-based limiting**: Different costs for different operations
5. **Adaptive limits**: Automatically adjust based on load

## References

- Token Bucket Algorithm: https://en.wikipedia.org/wiki/Token_bucket
- HTTP 429 Status: https://tools.ietf.org/html/rfc6585#section-4
- Retry-After Header: https://tools.ietf.org/html/rfc7231#section-7.1.3
