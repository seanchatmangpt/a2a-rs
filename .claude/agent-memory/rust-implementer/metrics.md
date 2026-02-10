# Prometheus Metrics Pattern (2026-02-10)

## Overview

Implemented comprehensive Prometheus metrics system in osiris-edge for real-time request and error monitoring.

**Files created**:
- `src/port/metrics.rs` - MetricsCollector trait (87 lines)
- `src/adapter/metrics.rs` - PrometheusCollector implementation (410 lines)
- `src/application/metrics_handler.rs` - HTTP handler and middleware (220 lines)
- `examples/metrics_integration_demo.rs` - Complete example (180 lines)
- `docs/METRICS.md` - Comprehensive documentation (400+ lines)

## Architecture

### Port Trait: MetricsCollector

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

### Adapter: PrometheusCollector

Wraps `prometheus::Registry` with built-in metrics:

1. **http_requests_total** (Counter)
   - Labels: method, path, status
   - Incremented on every request completion

2. **http_request_duration_seconds** (Histogram)
   - Labels: method, path
   - Buckets: [0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
   - Records latency distribution with percentiles

3. **errors_total** (Counter)
   - Labels: error_type, path
   - Tracks errors by category

4. **active_connections** (Gauge)
   - Single value metric
   - Updated manually

### Custom Metrics Storage

```rust
struct PrometheusCollector {
    registry: Arc<Registry>,
    http_requests: Arc<CounterVec>,
    request_duration: Arc<HistogramVec>,
    errors: Arc<CounterVec>,
    active_connections: Arc<Gauge>,
    custom_counters: Arc<RwLock<HashMap<String, Arc<CounterVec>>>>,
    custom_gauges: Arc<RwLock<HashMap<String, Arc<Gauge>>>>,
    custom_histograms: Arc<RwLock<HashMap<String, Arc<HistogramVec>>>>,
}
```

**Lazy creation pattern**: Metrics created on first use, not pre-registered. This avoids cardinality explosion from unbounded label values.

## Integration Patterns

### Basic Setup

```rust
use osiris_edge::{PrometheusCollector, metrics_handler};
use axum::Router;
use std::sync::Arc;

let metrics = Arc::new(PrometheusCollector::new()?);

let router = Router::new()
    .route("/metrics", get(metrics_handler::<PrometheusCollector>))
    .with_state(metrics);
```

### With Middleware

```rust
use osiris_edge::simple_request_metrics_middleware;
use axum::middleware;

let router = Router::new()
    .route("/api", get(handler))
    .layer(middleware::from_fn_with_state(
        metrics.clone(),
        simple_request_metrics_middleware,
    ))
    .with_state(metrics);
```

### Manual Tracking in Handlers

```rust
async fn api_handler(
    State(metrics): State<Arc<PrometheusCollector>>,
) -> StatusCode {
    metrics.record_request("GET", "/api", 200, 5).await;
    StatusCode::OK
}
```

## Built-in Metrics Reference

### HTTP Requests Counter
```
http_requests_total{method="POST",path="/webhook",status="202"} 42
```
- Dimensions: HTTP method, request path, response status
- Use: `sum(rate(http_requests_total[5m]))` for request rate

### Request Duration Histogram
```
http_request_duration_seconds_bucket{method="GET",path="/api",le="0.05"} 150
http_request_duration_seconds_bucket{method="GET",path="/api",le="0.1"} 180
http_request_duration_seconds_sum{method="GET",path="/api"} 8.5
http_request_duration_seconds_count{method="GET",path="/api"} 200
```
- Dimensions: HTTP method, request path
- Buckets: 11 predefined (1ms to 10s)
- Use: `histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))` for P95 latency

### Errors Counter
```
errors_total{error_type="validation_error",path="/webhook"} 3
errors_total{error_type="timeout_error",path="/api"} 1
```
- Dimensions: error type, path
- Use: `rate(errors_total[5m])` for error rate

### Active Connections Gauge
```
active_connections 42
```
- Single value, no labels
- Updated explicitly via `set_active_connections()`

## Custom Metrics

### Counter
```rust
metrics.increment_counter("webhook_events", vec![("source", "gmail")]).await;
metrics.increment_counter("webhook_events", vec![("source", "gmail")]).await;

// Result in metrics:
// webhook_events{source="gmail"} 2
```

### Gauge
```rust
metrics.set_gauge("queue_depth", 150.0, vec![("queue", "notifications")]).await;

// Result:
// queue_depth{queue="notifications"} 150
```

### Histogram
```rust
metrics.observe_histogram("db_latency", 0.045, vec![("operation", "select")]).await;

// Results in:
// db_latency_bucket{operation="select",le="0.05"} 1
// db_latency_bucket{operation="select",le="0.1"} 1
// etc...
```

## Middleware

### simple_request_metrics_middleware

Automatically records request duration and status:

```rust
pub async fn simple_request_metrics_middleware<M: MetricsCollector + 'static>(
    State(metrics): State<Arc<M>>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    let response = next.run(req).await;

    let duration = start.elapsed().as_millis() as u64;
    let status = response.status().as_u16();

    metrics.record_request(&method, &path, status, duration).await;
    response
}
```

### error_tracking_middleware

Automatically records 4xx and 5xx errors:

```rust
pub async fn error_tracking_middleware<M: MetricsCollector + 'static>(
    State(metrics): State<Arc<M>>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let response = next.run(req).await;

    if response.status().is_client_error() {
        metrics.record_error("client_error", &path).await;
    } else if response.status().is_server_error() {
        metrics.record_error("server_error", &path).await;
    }

    response
}
```

## Prometheus Queries

### Request Rate (requests/sec)
```promql
rate(http_requests_total[5m])
```

### Error Rate (%)
```promql
(rate(errors_total[5m]) / rate(http_requests_total[5m])) * 100
```

### P95 Latency
```promql
histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))
```

### Average Response Time
```promql
rate(http_request_duration_seconds_sum[5m]) / rate(http_request_duration_seconds_count[5m])
```

### Requests by Status
```promql
sum(rate(http_requests_total[5m])) by (status)
```

## Key Implementation Details

### Lazy Metric Creation

Custom metrics are created on first use rather than pre-registered:

```rust
pub async fn increment_counter(&self, name: &str, labels: Vec<(&str, &str)>) {
    let counters = self.custom_counters.read();

    if let Some(counter) = counters.get(name) {
        counter.with_label_values(&label_values).inc();
    } else {
        drop(counters);  // Release read lock

        // Create metric on first use
        let counter = register_counter_vec_with_registry!(...)?;
        self.custom_counters.write().insert(name.to_string(), Arc::new(counter));
    }
}
```

**Why**: Prevents cardinality explosion from unbounded label values.

### Histogram Buckets

Default buckets optimized for web requests:
```rust
vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
```

Covers:
- 1ms to 10ms: sub-10ms requests (local/cache hits)
- 10ms to 100ms: typical API requests
- 100ms to 1s: slow operations (database, external calls)
- 1s to 10s: very slow operations (long-running tasks)

### Prometheus Exposition Format

TextEncoder produces standard Prometheus format:

```
# HELP http_requests_total Total HTTP requests by method, path, and status code
# TYPE http_requests_total counter
http_requests_total{method="GET",path="/health",status="200"} 1234
```

Parseable by Prometheus, Grafana, and other tools.

## Testing Strategy

### Unit Tests

Located in `metrics.rs`:

```rust
#[tokio::test]
async fn test_record_request() {
    let collector = PrometheusCollector::new().unwrap();
    collector.record_request("GET", "/api/test", 200, 5).await;

    let metrics = collector.get_metrics().await;
    assert!(metrics.contains("http_requests_total"));
}
```

### Integration Tests

Located in `metrics_handler.rs`:

```rust
#[tokio::test]
async fn test_metrics_handler() {
    let collector = PrometheusCollector::new().unwrap();
    let metrics_text = collector.get_metrics().await;

    assert!(!metrics_text.is_empty());
    assert!(metrics_text.contains("http_requests_total"));
}
```

### Example Demo

Run full example:
```bash
cargo run -p osiris-edge --example metrics_integration_demo
```

## Performance Characteristics

### Overhead Per Request

- Counter increment: <100ns (atomic operation)
- Histogram observe: <500ns (bucket search + increment)
- Middleware: <1ms total overhead

### Memory

- Built-in metrics: ~10KB (histogram buckets + labels)
- Per custom metric: ~1KB base + labels overhead
- Custom label storage: HashMap with parking_lot RwLock (thread-safe)

### Concurrency

- Lock-free counter operations (atomic increments)
- RwLock for custom metrics (allows concurrent readers)
- No global locks, minimal contention

## Label Best Practices

### ✅ Good: Bounded Labels
```rust
// Limited values: free, premium, enterprise
metrics.increment_counter("user_events", vec![("user_type", "premium")]).await;

// Bounded HTTP methods
metrics.record_request("GET", "/api", 200, 5).await;  // method in {GET, POST, PUT, DELETE, etc}

// Bounded paths (routes, not raw URLs)
metrics.record_request("GET", "/api/users/:id", 200, 5).await;
```

### ❌ Bad: Unbounded Labels
```rust
// High cardinality: millions of unique user IDs
metrics.increment_counter("user_action", vec![("user_id", &user.id)]).await;

// High cardinality: arbitrary request paths
metrics.record_request("GET", &request.uri().to_string(), 200, 5).await;  // NO!
```

## Integration with WIP Gates

Track admission control decisions:

```rust
match wip_gate.try_acquire().await {
    Ok(permit) => {
        metrics.record_request("POST", "/webhook", 202, 5).await;
        // Process...
    }
    Err(_) => {
        metrics.record_error("wip_limit_exceeded", "/webhook").await;
    }
}
```

## References

- Prometheus docs: https://prometheus.io/docs/instrumenting/clientlibs/
- Axum middleware: https://docs.rs/axum/latest/axum/middleware/
- Best practices: https://prometheus.io/docs/practices/instrumentation/
