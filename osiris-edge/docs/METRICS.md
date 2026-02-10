# Prometheus Metrics Collection

Complete guide to osiris-edge metrics collection using Prometheus.

## Overview

The metrics system provides:
- **HTTP Request Tracking**: Counters and histograms for all requests
- **Error Rate Monitoring**: Automated error tracking by type and endpoint
- **Connection Monitoring**: Active connection gauges
- **Custom Metrics**: Application-level business metrics
- **Prometheus Export**: `/metrics` endpoint for scraping

## Architecture

### Port Trait: `MetricsCollector`

Located in `src/port/metrics.rs`, defines the interface:

```rust
#[async_trait]
pub trait MetricsCollector: Send + Sync {
    async fn record_request(&self, method: &str, path: &str, status_code: u16, duration_ms: u64);
    async fn record_error(&self, error_type: &str, path: &str);
    async fn set_active_connections(&self, count: usize);
    async fn increment_counter(&self, name: &str, labels: Vec<(&str, &str)>);
    async fn set_gauge(&self, name: &str, value: f64, labels: Vec<(&str, &str)>);
    async fn observe_histogram(&self, name: &str, value: f64, labels: Vec<(&str, &str)>);
    async fn get_metrics(&self) -> String;
    async fn reset(&self);
}
```

### Adapter: `PrometheusCollector`

Located in `src/adapter/metrics.rs`, provides Prometheus implementation.

## Quick Start

### 1. Create a Collector

```rust
use osiris_edge::adapter::PrometheusCollector;
use std::sync::Arc;

let metrics = Arc::new(PrometheusCollector::new()?);
```

### 2. Add to Router

```rust
use axiom::Router;
use axiom::routing::get;
use osiris_edge::application::metrics_handler;

let router = Router::new()
    .route("/metrics", get(metrics_handler::<PrometheusCollector>))
    .with_state(metrics);
```

### 3. Track Metrics

```rust
// HTTP request
metrics.record_request("GET", "/api/endpoint", 200, 45).await;

// Error
metrics.record_error("validation_error", "/api/webhook").await;

// Custom metric
metrics.set_gauge("queue_depth", 123.0, vec![]).await;
```

### 4. Scrape Metrics

```bash
curl http://localhost:3000/metrics
```

## Metrics

### Built-in Metrics

#### HTTP Requests Counter
- **Name**: `http_requests_total`
- **Type**: Counter
- **Labels**: `method`, `path`, `status`
- **Example**: `http_requests_total{method="GET",path="/api/test",status="200"} 42`
- **Tracks**: Total requests by HTTP method, path, and status code

#### Request Duration Histogram
- **Name**: `http_request_duration_seconds`
- **Type**: Histogram
- **Labels**: `method`, `path`
- **Buckets**: 0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0 seconds
- **Tracks**: Request latency distribution with percentiles

#### Errors Counter
- **Name**: `errors_total`
- **Type**: Counter
- **Labels**: `error_type`, `path`
- **Example**: `errors_total{error_type="validation_error",path="/webhook"} 3`
- **Tracks**: Error counts by type and endpoint

#### Active Connections Gauge
- **Name**: `active_connections`
- **Type**: Gauge
- **Example**: `active_connections 42`
- **Tracks**: Current number of active HTTP connections

### Custom Metrics

Track application-specific metrics:

```rust
// Counter
metrics.increment_counter("custom_events", vec![("event_type", "webhook")]).await;

// Gauge
metrics.set_gauge("cache_size", 5000.0, vec![("cache", "requests")]).await;

// Histogram
metrics.observe_histogram("db_query_time", 0.125, vec![("operation", "select")]).await;
```

## Middleware Integration

### Simple Request Duration Tracking

```rust
use osiris_edge::application::simple_request_metrics_middleware;
use axum::middleware;

let router = Router::new()
    .route("/api", get(handler))
    .layer(middleware::from_fn_with_state(
        metrics.clone(),
        simple_request_metrics_middleware,
    ))
    .with_state(metrics);
```

### Error Tracking

```rust
use osiris_edge::application::error_tracking_middleware;

let router = Router::new()
    .route("/api", get(handler))
    .layer(middleware::from_fn_with_state(
        metrics.clone(),
        error_tracking_middleware,
    ))
    .with_state(metrics);
```

## Prometheus Queries

### Example Queries for Grafana/Prometheus

```promql
# Request rate (requests per second)
rate(http_requests_total[5m])

# Error rate percentage
(rate(errors_total[5m]) / rate(http_requests_total[5m])) * 100

# P95 request latency
histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))

# Average request duration
rate(http_request_duration_seconds_sum[5m]) / rate(http_request_duration_seconds_count[5m])

# Requests by status code
sum(rate(http_requests_total[5m])) by (status)

# Active connections
active_connections

# Request latency heatmap
rate(http_request_duration_seconds_bucket[5m])
```

## Integration Examples

### Basic HTTP Server

```rust
use axum::Router;
use osiris_edge::adapter::PrometheusCollector;
use osiris_edge::application::metrics_handler;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let metrics = Arc::new(PrometheusCollector::new()?);

    let router = Router::new()
        .route("/metrics", get(metrics_handler::<PrometheusCollector>))
        .with_state(metrics);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, router).await?;
    Ok(())
}
```

### With Custom Handler

```rust
use axum::extract::State;
use axiom::http::StatusCode;
use std::sync::Arc;
use std::time::Instant;

async fn custom_handler(
    State(metrics): State<Arc<PrometheusCollector>>,
) -> StatusCode {
    let start = Instant::now();

    // Do work...

    let duration = start.elapsed().as_millis() as u64;
    metrics.record_request("POST", "/api/process", 200, duration).await;

    StatusCode::OK
}
```

### With WIP Gate Integration

```rust
use osiris_edge::adapter::{PrometheusCollector, KanbanWipGate};

let wip_gate = Arc::new(KanbanWipGate::new(100));
let metrics = Arc::new(PrometheusCollector::new()?);

// When processing requests
match wip_gate.try_acquire().await {
    Ok(permit) => {
        metrics.record_request("GET", "/webhook", 202, 5).await;
        // Process...
    }
    Err(_) => {
        metrics.record_error("wip_limit_exceeded", "/webhook").await;
    }
}
```

## Testing

### Unit Tests

```bash
cargo test -p osiris-edge --lib metrics --lib
```

### Example Demo

```bash
cargo run -p osiris-edge --example metrics_integration_demo
```

Then test metrics:
```bash
curl http://127.0.0.1:3001/metrics
curl -X POST http://127.0.0.1:3001/webhook
curl http://127.0.0.1:3001/api/demo
```

## Performance Considerations

1. **Async-first**: All metric operations are async (non-blocking)
2. **Lock-free counters**: Prometheus counters use atomic operations
3. **Bounded storage**: Custom metrics stored in `HashMap` with parking_lot RwLock
4. **Minimal overhead**: Request tracking adds <1ms per request
5. **Memory safe**: No unwrap()/expect() in metrics code

## Label Guidelines

When using labels in custom metrics:

```rust
// Good: Consistent label names and values
metrics.increment_counter("api_calls", vec![
    ("service", "auth"),
    ("method", "login")
]).await;

// Avoid: Unbounded label values (can cause cardinality explosion)
// metrics.increment_counter("user_events", vec![
//     ("user_id", &request.user_id)  // WRONG: unbounded cardinality
// ]).await;

// Better: Categorize values
metrics.increment_counter("user_events", vec![
    ("user_type", "premium")  // bounded: free, premium, enterprise
]).await;
```

## Troubleshooting

### Metrics endpoint returns empty

1. Check metrics were recorded:
   ```rust
   let text = metrics.get_metrics().await;
   println!("{}", text);
   ```

2. Verify Prometheus format:
   ```bash
   curl http://localhost:3000/metrics | head -20
   ```

### High cardinality metrics

Use bounded label values to avoid memory issues:

```rust
// Bad: unbounded user IDs
fn record_user_event(id: &str) {
    metrics.increment_counter("user_events", vec![("user_id", id)]).await;
}

// Good: categorize
fn record_user_event(user_type: &str) {
    metrics.increment_counter("user_events", vec![("user_type", user_type)]).await;
}
```

### Custom metrics not appearing

1. Ensure counter/gauge/histogram names are valid Prometheus identifiers
   - Alphanumeric, underscore, colon
   - Start with letter or underscore

2. Check labels are consistent across calls
   - Same metric name requires same label names
   - Different label order works (alphabetically normalized)

## References

- [Prometheus Client Libraries](https://prometheus.io/docs/instrumenting/clientlibs/)
- [Metrics Best Practices](https://prometheus.io/docs/practices/instrumentation/)
- [Axum Middleware](https://docs.rs/axum/latest/axum/middleware/)
- [Tokio Best Practices](https://tokio.rs/tokio/tutorial)

## See Also

- `/metrics` endpoint implementation: `src/application/metrics_handler.rs`
- Prometheus adapter: `src/adapter/metrics.rs`
- Integration example: `examples/metrics_integration_demo.rs`
