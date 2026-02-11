# HTTP Server Production Features

This document describes the production-ready features available in the a2a-rs HTTP server implementation.

## Overview

The HTTP server adapter includes comprehensive middleware and features for production deployments:

- **CORS (Cross-Origin Resource Sharing)** - Configurable origin handling
- **Rate Limiting** - Request throttling per IP or API key
- **Request Validation** - Content type and size validation
- **Response Compression** - Automatic compression with configurable levels
- **Health Checks** - Liveness, readiness, and detailed health endpoints
- **OpenAPI Specification** - Auto-generated API documentation
- **Graceful Shutdown** - Clean server shutdown on signals
- **Request Timeout** - Configurable request timeout
- **Observability** - Structured logging and tracing

## Middleware Configuration

### CORS

Configure CORS headers for cross-origin requests:

```rust
use a2a_rs::adapter::transport::http::CorsConfig;

// Permissive CORS (development)
let cors = CorsConfig::permissive();

// Restrictive CORS (production)
let cors = CorsConfig::strict()
    .allowed_origins(vec!["https://example.com".to_string()])
    .allow_credentials(true);

server.with_cors(cors)
```

### Rate Limiting

Limit request rate to prevent abuse:

```rust
use a2a_rs::adapter::transport::http::RateLimitConfig;

// Default: 100 requests per minute
let rate_limit = RateLimitConfig::default();

// Restrictive: 10 requests per minute
let rate_limit = RateLimitConfig::restrictive();

// Custom configuration
let rate_limit = RateLimitConfig {
    max_requests: 1000,
    window_seconds: 60,
    use_ip: true,
    use_api_key: false,
    burst_size: 100,
};

server.with_rate_limit(rate_limit)
```

### Request Validation

Validate incoming requests:

```rust
use a2a_rs::adapter::transport::http::ValidationConfig;

// Default validation (10MB max body)
let validation = ValidationConfig::default();

// Strict validation (1MB max body, schema validation)
let validation = ValidationConfig::strict();

server.with_validation(validation)
```

### Compression

Compress responses to reduce bandwidth:

```rust
use a2a_rs::adapter::transport::http::CompressionConfig;

// Fast compression (lower CPU)
let compression = CompressionConfig::fast();

// Maximum compression (smaller responses)
let compression = CompressionConfig::max();

server.with_compression(compression)
```

## Health Checks

The server provides three health endpoints:

- **GET /live** - Liveness probe (always returns 200 if server is running)
- **GET /ready** - Readiness probe (checks if server can accept requests)
- **GET /health** - Detailed health status with component information

Example health check setup:

```rust
use a2a_rs::adapter::transport::http::{HealthChecker, HealthStatus};

let mut health_checker = HealthChecker::new()
    .with_version("1.0.0".to_string());

// Register components
health_checker.register_component("database".to_string(), HealthStatus::Healthy).await;
health_checker.register_component("cache".to_string(), HealthStatus::Healthy).await;

// Update component status
health_checker.update_component("cache".to_string(), HealthStatus::Degraded).await;

server.with_health_checks(health_checker)
```

### Health Response Format

```json
{
  "status": "healthy",
  "timestamp": "2024-01-15T10:30:00Z",
  "version": "1.0.0",
  "uptimeSeconds": 3600.5,
  "components": [
    {
      "name": "database",
      "status": "healthy"
    },
    {
      "name": "cache",
      "status": "healthy"
    }
  ]
}
```

## OpenAPI Specification

The server can automatically generate OpenAPI 3.0 specifications:

```rust
use a2a_rs::adapter::transport::http::OpenApiBuilder;

let openapi = OpenApiBuilder::new()
    .with_title("My A2A Server".to_string())
    .with_version("1.0.0".to_string())
    .with_description("A production-ready A2A server".to_string())
    .add_server(
        "https://api.example.com".to_string(),
        Some("Production".to_string()),
    )
    .include_health(true)
    .include_spec_endpoint(true);

server.with_openapi(openapi)
```

Access the specification at `GET /openapi.json`.

## Graceful Shutdown

The server supports graceful shutdown on SIGINT (Ctrl+C) or SIGTERM:

```rust
use std::time::Duration;

server
    .with_shutdown_timeout(Duration::from_secs(10))
    .with_graceful_shutdown(true);
```

### Shutdown Process

1. Signal received (SIGINT/SIGTERM)
2. Server stops accepting new connections
3. Existing connections complete within timeout
4. Resources are cleaned up
5. Server exits

## Example: Production Server

```rust
use a2a_rs::{
    adapter::transport::http::{
        CorsConfig, CompressionConfig, HttpServer,
        RateLimitConfig, ValidationConfig,
        HealthChecker, HealthStatus, OpenApiBuilder,
    },
    // ... other imports
};

// Configure health checks
let mut health_checker = HealthChecker::new();
health_checker.register_component("database".to_string(), HealthStatus::Healthy).await;

// Configure OpenAPI
let openapi = OpenApiBuilder::new()
    .with_title("Production Server".to_string())
    .add_server("https://api.example.com".to_string(), None);

// Build server
let server = HttpServer::new(processor, agent_info, "0.0.0.0:8080".to_string())
    .with_cors(CorsConfig::strict())
    .with_rate_limit(RateLimitConfig::default())
    .with_validation(ValidationConfig::strict())
    .with_compression(CompressionConfig::fast())
    .with_health_checks(health_checker)
    .with_openapi(openapi)
    .with_request_timeout(Duration::from_secs(30))
    .with_shutdown_timeout(Duration::from_secs(10))
    .with_graceful_shutdown(true);

// Start server
server.start().await?;
```

## Available Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/` | POST | JSON-RPC 2.0 endpoint |
| `/.well-known/agent-card.json` | GET | Agent card (RFC 8615) |
| `/agent-card` | GET | Agent card (legacy) |
| `/skills` | GET | List all skills |
| `/skills/{id}` | GET | Get skill by ID |
| `/health` | GET | Detailed health status |
| `/ready` | GET | Readiness probe |
| `/live` | GET | Liveness probe |
| `/openapi.json` | GET | OpenAPI specification |

## Configuration Best Practices

### Development

```rust
CorsConfig::permissive()
RateLimitConfig::permissive()
ValidationConfig::default()
CompressionConfig::fast()
```

### Production

```rust
CorsConfig::strict()
    .allowed_origins(vec!["https://yourdomain.com".to_string()])
RateLimitConfig::default()  // 100 req/min
ValidationConfig::strict()
CompressionConfig::max()
```

## Observability

All middleware includes structured logging when the `tracing` feature is enabled:

```rust
// Enable tracing in Cargo.toml
[features]
tracing = ["dep:tracing", "dep:tracing-subscriber"]

// Initialize in main()
tracing_subscriber::fmt()
    .with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info".into()),
    )
    .init();
```

### Logging Output

```
INFO http_request{request_id="abc123", method="POST", uri="/"}: Starting request processing
INFO http_request: Request completed successfully duration_ms=45
```

## Security Considerations

1. **CORS**: Configure appropriate origins for production
2. **Rate Limiting**: Enable to prevent abuse
3. **Validation**: Enable strict validation for production
4. **Timeouts**: Set appropriate request timeout
5. **Authentication**: Use with authentication middleware

## Performance Tuning

### Compression Levels

- **Fast** (level 1-3): Lower CPU, less compression
- **Medium** (level 4-6): Balanced
- **High** (level 7-9): Higher CPU, more compression

### Rate Limiting

- Choose window size based on typical request patterns
- Set burst size for temporary traffic spikes
- Use API keys for per-client limits

### Timeouts

- Set request timeout based on longest operation
- Set shutdown timeout to allow in-flight requests to complete
- Monitor for timeout errors and adjust as needed

## Testing

```bash
# Health checks
curl http://localhost:8080/health
curl http://localhost:8080/ready
curl http://localhost:8080/live

# OpenAPI spec
curl http://localhost:8080/openapi.json

# Agent card
curl http://localhost:8080/.well-known/agent-card.json

# JSON-RPC request
curl -X POST http://localhost:8080/ \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "tasks/send",
    "params": {"id": "task-123", "message": {"role": "user", "parts": [{"content": "Hello"}]}},
    "id": 1
  }'
```

## Troubleshooting

### CORS Errors

Check browser console for CORS errors. Verify:
- Origin is in `allowed_origins`
- Methods are in `allowed_methods`
- Headers are in `allowed_headers`
- Credentials setting matches frontend

### Rate Limiting

If seeing rate limit errors:
- Increase `max_requests` or `window_seconds`
- Check `burst_size` for spike handling
- Verify client is sending API key if configured

### Health Check Failures

- Check component status in `/health` response
- Verify components are registered
- Check component update logic
- Review logs for component errors

## Further Reading

- [A2A Protocol Specification](https://w3.org/TR/a2a/)
- [OpenAPI Specification](https://spec.openapis.org/oas/v3.0.0)
- [HTTP Server Documentation](../src/adapter/transport/http/)
