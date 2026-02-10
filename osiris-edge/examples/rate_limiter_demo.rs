//! Rate limiter demonstration
//!
//! This example shows how to use the TokenBucketRateLimiter with both
//! per-IP and per-tenant limits, and how to integrate it with Axum.
//!
//! Run with:
//! ```
//! cargo run -p osiris-edge --example rate_limiter_demo
//! ```

use osiris_edge::{
    RateLimitConfig, RateLimitMiddlewareConfig, RateLimiter, TokenBucketRateLimiter,
};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() {
    println!("=== Osiris-Edge Rate Limiter Demo ===\n");

    // Example 1: Basic rate limiting
    example_basic_rate_limiting().await;

    // Example 2: Per-IP rate limiting
    example_per_ip_rate_limiting().await;

    // Example 3: Per-tenant rate limiting
    example_per_tenant_rate_limiting().await;

    // Example 4: Token bucket refill
    example_token_bucket_refill().await;

    // Example 5: Middleware configuration
    example_middleware_configuration();

    println!("\n=== Demo completed successfully ===");
}

/// Demonstrates basic rate limiting
async fn example_basic_rate_limiting() {
    println!("Example 1: Basic Rate Limiting");
    println!("------------------------------");

    let config = RateLimitConfig::new(5, 10, 20, 1); // 5 req/s per IP
    let limiter = TokenBucketRateLimiter::new(config);

    // Try to make requests within the limit
    for i in 1..=5 {
        match limiter.check_ip_limit("192.168.1.1", 1).await {
            Ok(()) => println!("  Request {}: ALLOWED", i),
            Err(e) => println!("  Request {}: REJECTED - {}", i, e),
        }
    }

    // This request should be rejected
    println!("  Attempting 6th request (should fail):");
    match limiter.check_ip_limit("192.168.1.1", 1).await {
        Ok(()) => println!("    ALLOWED (unexpected)"),
        Err(e) => println!("    REJECTED - {}", e),
    }

    println!();
}

/// Demonstrates per-IP rate limiting isolation
async fn example_per_ip_rate_limiting() {
    println!("Example 2: Per-IP Rate Limiting");
    println!("-------------------------------");

    let config = RateLimitConfig::new(3, 50, 100, 1); // 3 req/s per IP
    let limiter = TokenBucketRateLimiter::new(config);

    // IP 1 makes 3 requests (at limit)
    println!("  IP 192.168.1.1: Making 3 requests");
    for i in 1..=3 {
        let result = limiter.check_ip_limit("192.168.1.1", 1).await;
        println!(
            "    Request {}: {}",
            i,
            if result.is_ok() { "OK" } else { "REJECTED" }
        );
    }

    // IP 2 can still make requests (isolated from IP 1)
    println!("  IP 192.168.1.2: Making 3 requests");
    for i in 1..=3 {
        let result = limiter.check_ip_limit("192.168.1.2", 1).await;
        println!(
            "    Request {}: {}",
            i,
            if result.is_ok() { "OK" } else { "REJECTED" }
        );
    }

    // Verify IP 1 is at limit
    println!("  IP 192.168.1.1: Attempting 4th request");
    match limiter.check_ip_limit("192.168.1.1", 1).await {
        Ok(()) => println!("    ALLOWED (unexpected)"),
        Err(_) => println!("    REJECTED (as expected - at limit)"),
    }

    println!();
}

/// Demonstrates per-tenant rate limiting
async fn example_per_tenant_rate_limiting() {
    println!("Example 3: Per-Tenant Rate Limiting");
    println!("-----------------------------------");

    let config = RateLimitConfig::new(20, 5, 50, 1); // 5 req/s per tenant
    let limiter = TokenBucketRateLimiter::new(config);

    // Tenant A makes requests
    println!("  Tenant A: Making 5 requests");
    for i in 1..=5 {
        let result = limiter.check_tenant_limit("tenant-a", 1).await;
        println!(
            "    Request {}: {}",
            i,
            if result.is_ok() { "OK" } else { "REJECTED" }
        );
    }

    // Tenant B can make requests independently
    println!("  Tenant B: Making 5 requests");
    for i in 1..=5 {
        let result = limiter.check_tenant_limit("tenant-b", 1).await;
        println!(
            "    Request {}: {}",
            i,
            if result.is_ok() { "OK" } else { "REJECTED" }
        );
    }

    // Check rates
    let rate_a = limiter.get_rate("tenant-a").await;
    let rate_b = limiter.get_rate("tenant-b").await;
    println!("  Tenant A rate: {} requests in window", rate_a);
    println!("  Tenant B rate: {} requests in window", rate_b);

    println!();
}

/// Demonstrates token bucket refilling over time
async fn example_token_bucket_refill() {
    println!("Example 4: Token Bucket Refill");
    println!("------------------------------");

    let config = RateLimitConfig::new(2, 10, 20, 1); // 2 req/s per IP
    let limiter = TokenBucketRateLimiter::new(config);

    let ip = "192.168.1.100";

    // Consume all tokens
    println!("  Consuming all tokens:");
    for i in 1..=2 {
        let result = limiter.check_ip_limit(ip, 1).await;
        println!(
            "    Token {}: {}",
            i,
            if result.is_ok() { "acquired" } else { "failed" }
        );
    }

    // Try to make another request (should fail)
    println!("  Attempting request after consuming all tokens:");
    match limiter.check_ip_limit(ip, 1).await {
        Ok(()) => println!("    ALLOWED (unexpected)"),
        Err(e) => {
            if let osiris_edge::RateLimitError::RateLimitExceeded {
                retry_after_secs, ..
            } = e
            {
                println!("    REJECTED - Retry after {} seconds", retry_after_secs);
            }
        }
    }

    // Wait for tokens to refill
    println!("  Waiting 1.1 seconds for tokens to refill...");
    sleep(Duration::from_millis(1100)).await;

    // Try again (should succeed now)
    println!("  Attempting request after refill:");
    match limiter.check_ip_limit(ip, 1).await {
        Ok(()) => println!("    ALLOWED - Tokens have refilled!"),
        Err(_) => println!("    REJECTED (unexpected)"),
    }

    println!();
}

/// Demonstrates middleware configuration options
fn example_middleware_configuration() {
    println!("Example 5: Middleware Configuration");
    println!("----------------------------------");

    let config_all = RateLimitMiddlewareConfig::all();
    println!("  Config::all():");
    println!("    check_ip: {}", config_all.check_ip);
    println!("    check_tenant: {}", config_all.check_tenant);
    println!("    check_global: {}", config_all.check_global);

    let config_global = RateLimitMiddlewareConfig::global_only();
    println!("  Config::global_only():");
    println!("    check_ip: {}", config_global.check_ip);
    println!("    check_tenant: {}", config_global.check_tenant);
    println!("    check_global: {}", config_global.check_global);

    let config_ip_global = RateLimitMiddlewareConfig::ip_and_global();
    println!("  Config::ip_and_global():");
    println!("    check_ip: {}", config_ip_global.check_ip);
    println!("    check_tenant: {}", config_ip_global.check_tenant);
    println!("    check_global: {}", config_ip_global.check_global);

    println!();
}

// Example of how to use with Axum (pseudo-code - not runnable directly)
#[allow(dead_code)]
async fn example_axum_integration() {
    use axum::Router;

    // Create rate limiter
    let limiter = Arc::new(TokenBucketRateLimiter::default());

    // Create router with rate limiter middleware
    let config = RateLimitMiddlewareConfig::all();
    let middleware = osiris_edge::rate_limit_layer(limiter, config);

    // Apply middleware to router
    let _app = Router::new().layer(middleware);

    // Now all requests to this router will be rate limited!
}
