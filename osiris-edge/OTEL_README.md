# OpenTelemetry Tracing Implementation - osiris-edge

## Overview

This document describes the complete OpenTelemetry distributed tracing integration for osiris-edge, including span creation, W3C trace context propagation, and Cloud Trace/Jaeger/OTLP export capabilities.

## Files Created

### Core Implementation

1. **`src/adapter/tracing.rs`** (880 lines)
   - Complete OpenTelemetry adapter with all tracing functionality
   - W3C traceparent format support
   - Feature-gated manager initialization
   - Comprehensive error handling and tests

### Documentation

2. **`docs/OTEL_INTEGRATION.md`** (450+ lines)
   - Complete integration guide
   - Configuration examples for Cloud Trace, Jaeger, OTLP
   - Performance tuning recommendations
   - Troubleshooting guide

### Examples

3. **`examples/otel_tracing_demo.rs`** (180+ lines)
   - 11 working examples covering all major features
   - W3C header parsing and generation
   - Context injection patterns
   - OpenTelemetry manager usage

### Dependencies Updated

4. **`Cargo.toml`** - Added OpenTelemetry dependencies with feature gates:
   - `opentelemetry` (0.23)
   - `opentelemetry-otlp` (0.16) with grpc-tonic
   - `opentelemetry-jaeger-trace` (0.22)
   - `opentelemetry-gcloud-trace` (0.24)
   - `tracing-opentelemetry` (0.24)
   - `tonic` (0.11)

### Module Exports

5. **`src/adapter/mod.rs`** - Added tracing module and public exports:
   ```rust
   pub mod tracing;
   pub use tracing::{
       OpenTelemetryManager, SpanEvent, SpanHandle, SpanMetrics,
       TraceContext, TraceContextInjector, TracingConfig, TracingError,
   };
   ```

6. **`src/lib.rs`** - Public API exports for all tracing types

## Features

### Three-Tier Feature Flags

```toml
# Basic OpenTelemetry (OTLP protocol)
otel = ["opentelemetry", "opentelemetry-otlp", "tracing-opentelemetry", "tonic"]

# Google Cloud Trace export
otel-gcloud = ["otel", "opentelemetry-gcloud-trace"]

# Jaeger APM export
otel-jaeger = ["otel", "opentelemetry-jaeger-trace"]
```

## Quick Start Examples

### Without Feature Flags (Always Works)

```rust
use osiris_edge::TraceContext;

// Generate new trace
let ctx = TraceContext::new_generated();

// Parse W3C header
let header = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
let ctx = TraceContext::from_traceparent(header)?;

// Check sampling
println!("Sampled: {}", ctx.is_sampled());

// Format back
println!("Header: {}", ctx.to_traceparent());
```

### W3C Header Propagation

```rust
use osiris_edge::{TraceContext, TraceContextInjector};

let ctx = TraceContext::new_generated();
let mut headers = HashMap::new();

// Inject for HTTP requests
TraceContextInjector::inject(&ctx, &mut headers);

// Or for Axum
use axum::http::HeaderMap;
let mut axum_headers = HeaderMap::new();
TraceContextInjector::inject_axum(&ctx, &mut axum_headers)?;
```

### With OpenTelemetry (Requires `otel` Feature)

```rust
#[cfg(feature = "otel")]
async fn setup_tracing() -> Result<(), Box<dyn std::error::Error>> {
    use osiris_edge::{OpenTelemetryManager, TracingConfig};

    // Initialize with OTLP
    let config = TracingConfig::otlp_default("http://localhost:4317")
        .with_sampling_rate(0.9);

    let manager = OpenTelemetryManager::new(config).await?;

    // Create and track spans
    let span = manager.create_span("request");

    // Flush before shutdown
    manager.flush().await?;

    Ok(())
}
```

### Google Cloud Trace (Requires `otel-gcloud` Feature)

```rust
#[cfg(feature = "otel-gcloud")]
async fn setup_cloud_trace() -> Result<(), Box<dyn std::error::Error>> {
    use osiris_edge::{OpenTelemetryManager, TracingConfig};

    let config = TracingConfig::gcloud_default("my-gcp-project")
        .with_sampling_rate(1.0)
        .with_batch_processing(true);

    let manager = OpenTelemetryManager::new(config).await?;

    let span = manager.create_span("operation");
    manager.flush().await?;

    Ok(())
}
```

### Jaeger APM (Requires `otel-jaeger` Feature)

```rust
#[cfg(feature = "otel-jaeger")]
async fn setup_jaeger() -> Result<(), Box<dyn std::error::Error>> {
    use osiris_edge::{OpenTelemetryManager, TracingConfig};

    let config = TracingConfig::jaeger_default("localhost", 6831)
        .with_sampling_rate(0.5);

    let manager = OpenTelemetryManager::new(config).await?;

    let span = manager.create_span("service-call");
    manager.flush().await?;

    Ok(())
}
```

## Core Types

### TraceContext

Represents W3C trace context with span IDs and sampling flags.

```rust
pub struct TraceContext {
    pub trace_id: String,        // 32 hex chars (128-bit)
    pub parent_span_id: String,  // 16 hex chars (64-bit)
    pub trace_flags: u8,         // Sampled bit + reserved
    pub trace_state: Option<String>,
}
```

Methods:
- `new_generated()` - Create with random IDs
- `from_traceparent(header)` - Parse W3C header
- `to_traceparent()` - Format as W3C header
- `is_sampled()` / `set_sampled()` - Check/set sampling bit

### SpanHandle

Represents an active span with attributes and metrics.

```rust
pub struct SpanHandle {
    name: String,
    start_time: Instant,
    attributes: HashMap<String, String>,
}
```

Methods:
- `add_attribute(key, value)` - Add attribute
- `elapsed()` - Get duration since creation
- `metrics()` - Get SpanMetrics snapshot

### SpanMetrics

Encapsulates collected span data.

```rust
pub struct SpanMetrics {
    pub span_name: String,
    pub duration_micros: u64,
    pub status: String,           // "Ok", "Error", "Unset"
    pub event_count: usize,
    pub attribute_count: usize,
}
```

### TracingConfig

Configuration for initialization.

```rust
pub struct TracingConfig {
    pub service_name: String,
    pub service_version: String,
    pub sampling_rate: f64,       // 0.0-1.0
    pub batch_spans: bool,
    pub batch_size: usize,
    pub max_queue_size: usize,
    pub export_timeout_ms: u64,
    pub otlp_endpoint: Option<String>,
    pub gcloud_project_id: Option<String>,
    pub jaeger_host: Option<String>,
    pub jaeger_port: Option<u16>,
    pub enable_trace_context: bool,
}
```

Convenience builders:
- `TracingConfig::gcloud_default(project_id)`
- `TracingConfig::jaeger_default(host, port)`
- `TracingConfig::otlp_default(endpoint)`
- `with_sampling_rate(f64)` - Configure sampling
- `with_batch_processing(bool)` - Enable/disable batching

### OpenTelemetryManager (Feature-gated)

Main entry point for OpenTelemetry integration.

```rust
#[cfg(feature = "otel")]
pub struct OpenTelemetryManager { ... }
```

Methods:
- `new(config)` - Initialize with config
- `create_span(name)` - Create new span
- `create_span_with_context(name, context)` - With explicit context
- `extract_trace_context(headers)` - Parse from headers
- `extract_trace_context_from_axum(headers)` - For Axum
- `record_event(span, event)` - Record span event
- `flush()` - Flush pending spans
- `config()` / `service_name()` / `service_version()` - Accessors

### TracingError

Error type for all tracing operations.

```rust
pub enum TracingError {
    InvalidTraceContext(String),
    InitializationFailed(String),
    ExportFailed(String),
    Internal(String),
    #[cfg(feature = "otel")]
    OpenTelemetry(String),
}
```

## W3C Traceparent Format

Standard format: `version-traceId-spanId-traceFlags`

Example: `00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01`

- **version**: `00` (current version)
- **traceId**: 32 hex characters (128-bit value)
- **spanId**: 16 hex characters (64-bit value)
- **traceFlags**: 2 hex characters
  - Bit 0: Sampled flag (1 = sampled, 0 = not sampled)
  - Bits 1-7: Reserved (must be zero)

## Test Coverage

Run examples and tests:

```bash
# Run example
cargo run -p osiris-edge --example otel_tracing_demo

# Run adapter tests
cargo test -p osiris-edge --lib adapter::tracing

# Run all osiris-edge tests
cargo test -p osiris-edge
```

Tests include:
- W3C header format validation
- Traceparent round-trip consistency
- Trace context generation
- Sampling flag management
- Configuration presets
- Span metrics calculation
- Context injection patterns

## Integration Patterns

### Axum HTTP Handler Integration

```rust
use axum::extract::State;
use osiris_edge::OpenTelemetryManager;

#[derive(Clone)]
pub struct AppState {
    tracing_manager: Arc<OpenTelemetryManager>,
}

async fn handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> String {
    // Extract trace context from incoming request
    let ctx = state.tracing_manager.extract_trace_context_from_axum(&headers);

    // Create span for this operation
    let span = state.tracing_manager.create_span_with_context("handler", &ctx);

    // ... handle request ...

    format!("Trace ID: {}", ctx.trace_id)
}
```

### Request Propagation

```rust
use osiris_edge::TraceContextInjector;

async fn call_downstream_service(ctx: &TraceContext) -> Result<()> {
    let mut headers = HashMap::new();

    // Inject context for propagation
    TraceContextInjector::inject(ctx, &mut headers);

    // Make request with propagated context
    let client = reqwest::Client::new();
    client.get("http://downstream/api")
        .headers(/* headers */)
        .send()
        .await?;

    Ok(())
}
```

## Performance Tuning

### Sampling Strategies

```rust
// High-traffic service: 1% sampling
let config = TracingConfig::default().with_sampling_rate(0.01);

// Development: 100% sampling
let config = TracingConfig::default().with_sampling_rate(1.0);

// Production: 10% sampling
let config = TracingConfig::default().with_sampling_rate(0.1);
```

### Batch Configuration

```rust
let config = TracingConfig {
    batch_spans: true,
    batch_size: 1024,           // Larger = fewer exports
    max_queue_size: 4096,       // Prevent OOM
    export_timeout_ms: 60_000,  // Give export time
    ..Default::default()
};
```

### Memory Estimation

- Per span: ~200-500 bytes (including attributes)
- Max memory: `batch_size × 500` bytes (conservative)
- Example: 512 batch size = ~250KB max before export

## Troubleshooting

### No Traces in Backend

1. Verify `sampling_rate > 0`: `config.sampling_rate = 0.1` means 10% of traces
2. Check backend connectivity: Can reach export endpoint?
3. Ensure credentials configured: Cloud Trace, Jaeger auth
4. Call `manager.flush().await` before shutdown

### High Memory Usage

1. Reduce `batch_size`: Smaller batches export more frequently
2. Reduce `max_queue_size`: Prevent unbounded accumulation
3. Increase sampling rate: Fewer spans = less memory

### Incomplete Traces

1. Ensure `manager.flush()` completes before shutdown
2. Check `export_timeout_ms`: May be too short for slow networks
3. Verify context propagation: Are trace IDs consistent across services?

## Next Steps

1. **Integrate with main server**: Wire into Axum handlers
2. **Add middleware**: Automatic span creation per request
3. **Dashboard**: Visualization of traces in Cloud Trace UI
4. **Alerting**: Configure alerts on latency/error rates
5. **Custom events**: Record application-specific events in spans

## See Also

- `docs/OTEL_INTEGRATION.md` - Detailed configuration guide
- `examples/otel_tracing_demo.rs` - Working code examples
- [W3C Trace Context](https://w3c.github.io/trace-context/)
- [OpenTelemetry Documentation](https://opentelemetry.io/docs/)
