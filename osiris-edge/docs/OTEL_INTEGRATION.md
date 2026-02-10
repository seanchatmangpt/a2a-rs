# OpenTelemetry Integration Guide

## Overview

osiris-edge provides comprehensive distributed tracing support via OpenTelemetry with:

- **W3C Trace Context** - Standard `traceparent` header parsing and generation
- **Cloud Trace Export** - Native integration with Google Cloud Trace
- **Jaeger Support** - Alternative export to Jaeger APM
- **OTLP Protocol** - Standard OpenTelemetry Protocol endpoint support
- **Configurable Sampling** - Per-trace and policy-based sampling
- **Span Management** - Create, track, and export spans with metrics
- **Context Propagation** - Automatic trace ID injection in HTTP requests

## Features

### Enabled via Feature Flags

```toml
# Minimal OpenTelemetry support (OTLP)
otel = ["opentelemetry", "opentelemetry-otlp", "tracing-opentelemetry", "tonic"]

# Google Cloud Trace export
otel-gcloud = ["otel", "opentelemetry-gcloud-trace"]

# Jaeger export
otel-jaeger = ["otel", "opentelemetry-jaeger-trace"]
```

Enable in your `Cargo.toml`:

```toml
[dependencies]
osiris-edge = { version = "0.1", features = ["otel"] }

# Or with Cloud Trace support
osiris-edge = { version = "0.1", features = ["otel-gcloud"] }

# Or with Jaeger support
osiris-edge = { version = "0.1", features = ["otel-jaeger"] }
```

## Quick Start

### Basic Tracing Without Export

Works without any feature flags:

```rust
use osiris_edge::{TraceContext, SpanHandle};

// Generate a new trace context
let ctx = TraceContext::new_generated();
println!("Trace ID: {}", ctx.trace_id);

// Create a span
let mut span = SpanHandle {
    name: "request-processing".to_string(),
    start_time: std::time::Instant::now(),
    attributes: Default::default(),
};

// Add attributes
span.add_attribute("user_id", "12345");
span.add_attribute("service", "api");

// Get metrics
let metrics = span.metrics();
println!("Duration: {}µs", metrics.duration_micros);
```

### W3C Traceparent Parsing

```rust
use osiris_edge::TraceContext;

// Parse standard W3C traceparent header
let header = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
let ctx = TraceContext::from_traceparent(header)?;

// Check if sampled
assert!(ctx.is_sampled());

// Format back to header
let formatted = ctx.to_traceparent();
assert_eq!(header, formatted);
```

### Context Injection for HTTP Requests

```rust
use osiris_edge::{TraceContext, TraceContextInjector};
use std::collections::HashMap;

let ctx = TraceContext::new_generated();
let mut headers = HashMap::new();

// Inject into regular headers
TraceContextInjector::inject(&ctx, &mut headers);

// Or inject into Axum headers
use axum::http::HeaderMap;
let mut axum_headers = HeaderMap::new();
TraceContextInjector::inject_axum(&ctx, &mut axum_headers)?;
```

### Configuration

```rust
use osiris_edge::TracingConfig;

// Default configuration
let default = TracingConfig::default();

// Google Cloud Trace
let gcloud = TracingConfig::gcloud_default("my-gcp-project");

// Jaeger
let jaeger = TracingConfig::jaeger_default("localhost", 6831);

// OTLP endpoint
let otlp = TracingConfig::otlp_default("http://localhost:4317")
    .with_sampling_rate(0.9)
    .with_batch_processing(true);
```

## Configuration Properties

### `TracingConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `service_name` | String | "osiris-edge" | Service identifier in traces |
| `service_version` | String | Package version | Service version tag |
| `sampling_rate` | f64 | 1.0 | Sampling rate (0.0-1.0) |
| `batch_spans` | bool | true | Enable batch span processing |
| `batch_size` | usize | 512 | Spans per batch before flush |
| `max_queue_size` | usize | 2048 | Max pending spans in queue |
| `export_timeout_ms` | u64 | 30_000 | Export timeout in milliseconds |
| `otlp_endpoint` | Option<String> | None | OTLP collector endpoint |
| `gcloud_project_id` | Option<String> | None | GCP project ID |
| `jaeger_host` | Option<String> | None | Jaeger agent hostname |
| `jaeger_port` | Option<u16> | None | Jaeger agent port (usually 6831) |
| `enable_trace_context` | bool | true | Enable W3C trace context |

## W3C Traceparent Header

Format: `version-traceId-spanId-traceFlags`

Example: `00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01`

- **version**: Protocol version (always `00` for current)
- **traceId**: 128-bit trace ID (32 hex chars)
- **spanId**: 64-bit parent span ID (16 hex chars)
- **traceFlags**: Single byte (02 hex chars)
  - Bit 0: Sampled flag (1 = sampled, 0 = not sampled)

## Span Metrics

Each span tracks:

```rust
pub struct SpanMetrics {
    pub span_name: String,
    pub duration_micros: u64,
    pub status: String,              // "Ok", "Error", "Unset"
    pub event_count: usize,
    pub attribute_count: usize,
}
```

Access via `span.metrics()`:

```rust
let span = manager.create_span("operation");
// ... do work ...
let metrics = span.metrics();
println!("Took {}µs", metrics.duration_micros);
```

## Span Events

Record structured events within a span:

```rust
use osiris_edge::SpanEvent;

let event = SpanEvent::new("database-query")
    .with_attribute("query_type", "SELECT")
    .with_attribute("table", "users")
    .with_attribute("row_count", "42");

manager.record_event(&span, event);
```

## OpenTelemetry Manager (with `otel` feature)

### Initialization

```rust
#[cfg(feature = "otel")]
async fn setup_tracing() -> Result<(), Box<dyn std::error::Error>> {
    use osiris_edge::{OpenTelemetryManager, TracingConfig};

    let config = TracingConfig::default()
        .with_sampling_rate(0.9);

    let manager = OpenTelemetryManager::new(config).await?;

    // Use manager for span creation and context extraction
    let span = manager.create_span("request");

    // Flush pending spans before shutdown
    manager.flush().await?;

    Ok(())
}
```

### Cloud Trace Export (with `otel-gcloud` feature)

```rust
#[cfg(feature = "otel-gcloud")]
async fn setup_cloud_trace() -> Result<(), Box<dyn std::error::Error>> {
    use osiris_edge::{OpenTelemetryManager, TracingConfig};

    let config = TracingConfig::gcloud_default("my-project-id")
        .with_sampling_rate(1.0)
        .with_batch_processing(true);

    let manager = OpenTelemetryManager::new(config).await?;

    // Traces are automatically exported to Cloud Trace
    let span = manager.create_span("operation");

    manager.flush().await?;
    Ok(())
}
```

### Jaeger Export (with `otel-jaeger` feature)

```rust
#[cfg(feature = "otel-jaeger")]
async fn setup_jaeger() -> Result<(), Box<dyn std::error::Error>> {
    use osiris_edge::{OpenTelemetryManager, TracingConfig};

    // Jaeger agent typically on localhost:6831
    let config = TracingConfig::jaeger_default("localhost", 6831)
        .with_sampling_rate(0.5);

    let manager = OpenTelemetryManager::new(config).await?;

    let span = manager.create_span("service-call");
    manager.flush().await?;
    Ok(())
}
```

### OTLP Endpoint

```rust
#[cfg(feature = "otel")]
async fn setup_otlp() -> Result<(), Box<dyn std::error::Error>> {
    use osiris_edge::{OpenTelemetryManager, TracingConfig};

    let config = TracingConfig::otlp_default("http://localhost:4317")
        .with_batch_size(256)
        .with_batch_processing(true);

    let manager = OpenTelemetryManager::new(config).await?;

    // Send traces to OTLP collector
    let span = manager.create_span("traced-operation");
    manager.flush().await?;
    Ok(())
}
```

## Sampling Strategies

### Always Sample

```rust
let config = TracingConfig::default().with_sampling_rate(1.0);
```

### Never Sample (Debugging Mode)

```rust
let config = TracingConfig::default().with_sampling_rate(0.0);
```

### 50% Sampling

```rust
let config = TracingConfig::default().with_sampling_rate(0.5);
```

## Error Handling

```rust
use osiris_edge::TracingError;

match TraceContext::from_traceparent(header) {
    Ok(ctx) => {
        println!("Trace ID: {}", ctx.trace_id);
    }
    Err(TracingError::InvalidTraceContext(msg)) => {
        eprintln!("Invalid header: {}", msg);
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

Error types:

- `InvalidTraceContext` - W3C header parsing failed
- `InitializationFailed` - Manager setup failed
- `ExportFailed` - Exporting traces to backend failed
- `Internal` - Internal error (usually header injection)
- `OpenTelemetry` - OpenTelemetry SDK error (feature-gated)

## Integration with Axum

### Extract Trace Context from Request Headers

```rust
use axum::http::HeaderMap;
use osiris_edge::OpenTelemetryManager;

async fn handler(headers: &HeaderMap) -> String {
    let manager = /* get from state */;
    let ctx = manager.extract_trace_context_from_axum(headers);
    format!("Trace: {}", ctx.trace_id)
}
```

### Inject Trace Context into Response Headers

```rust
use axiom::http::HeaderMap;
use osiris_edge::TraceContextInjector;

let ctx = TraceContext::new_generated();
let mut response_headers = HeaderMap::new();
TraceContextInjector::inject_axum(&ctx, &mut response_headers)?;
```

## Batch Processing Configuration

For high-throughput scenarios, configure batching:

```rust
let config = TracingConfig::default()
    .with_batch_processing(true);

// Customize batch settings
let config = TracingConfig {
    batch_spans: true,
    batch_size: 1024,          // Flush every 1024 spans
    max_queue_size: 4096,      // Max 4096 pending spans
    export_timeout_ms: 60_000, // 60s timeout
    ..Default::default()
};
```

## Testing

The implementation includes comprehensive tests:

```bash
# Run all tracing tests
cargo test -p osiris-edge --lib adapter::tracing

# Run examples
cargo run -p osiris-edge --example otel_tracing_demo
```

Key test coverage:

- W3C traceparent format validation
- Header round-trip consistency
- Trace context generation
- Sampling flag management
- Configuration presets
- Span metrics calculation
- Context injection

## Performance Considerations

1. **Sampling**: Use < 100% sampling for high-traffic services
   - 10% sampling: 90% reduction in exported traces
   - 1% sampling: 99% reduction

2. **Batch Size**: Larger batches reduce export overhead
   - Default: 512 spans per batch
   - High throughput: 1024-2048 spans

3. **Queue Size**: Prevents OOM from span accumulation
   - Default: 2048 max pending
   - Set based on expected latency × throughput

4. **Memory**: Spans stored in memory before export
   - Each span: ~200-500 bytes (including attributes)
   - Max memory: batch_size × 500 bytes (conservative)

## Environment Variables

Configure via environment:

```bash
# Sampling rate
export OSIRIS_SAMPLING_RATE=0.5

# Cloud Trace project
export OSIRIS_GCLOUD_PROJECT_ID=my-project

# OTLP endpoint
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317

# Jaeger agent
export OSIRIS_JAEGER_HOST=localhost
export OSIRIS_JAEGER_PORT=6831
```

## Troubleshooting

### No traces appearing in backend

1. Check sampling rate: `config.sampling_rate > 0`
2. Verify export endpoint is accessible
3. Ensure credentials/auth are configured
4. Check `manager.flush()` is called before shutdown

### High memory usage

1. Reduce `batch_size` or `max_queue_size`
2. Increase sampling rate to reduce span volume
3. Check for slow export or network issues

### Incomplete traces

1. Ensure `manager.flush().await` before shutdown
2. Verify `export_timeout_ms` is sufficient
3. Check for span context propagation in async boundaries

## See Also

- [W3C Trace Context](https://w3c.github.io/trace-context/)
- [OpenTelemetry Spec](https://opentelemetry.io/docs/specs/otel/)
- [OTLP Protocol](https://opentelemetry.io/docs/specs/otlp/)
- Example: `examples/otel_tracing_demo.rs`
