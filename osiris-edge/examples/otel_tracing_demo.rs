//! OpenTelemetry distributed tracing example
//!
//! Demonstrates:
//! - Trace context creation and propagation
//! - W3C traceparent header parsing
//! - Span management with metrics
//! - Feature-gated OpenTelemetry manager initialization

use osiris_edge::{SpanEvent, SpanHandle, TraceContext, TraceContextInjector, TracingConfig};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example 1: Create and propagate trace context
    println!("\n=== Example 1: Trace Context Creation ===");
    let ctx = TraceContext::new_generated();
    println!("Generated trace ID: {}", ctx.trace_id);
    println!("Generated span ID: {}", ctx.parent_span_id);
    println!("Traceparent header: {}", ctx.to_traceparent());

    // Example 2: Parse W3C traceparent header
    println!("\n=== Example 2: Parse Traceparent Header ===");
    let header = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let parsed = TraceContext::from_traceparent(header)?;
    println!("Parsed trace ID: {}", parsed.trace_id);
    println!("Parsed span ID: {}", parsed.parent_span_id);
    println!("Is sampled: {}", parsed.is_sampled());

    // Example 3: Inject trace context into headers
    println!("\n=== Example 3: Context Injection ===");
    let mut headers = HashMap::new();
    TraceContextInjector::inject(&ctx, &mut headers);
    println!("Injected headers:");
    for (k, v) in &headers {
        println!("  {}: {}", k, v);
    }

    // Example 4: Create tracing configuration
    println!("\n=== Example 4: Tracing Configuration ===");

    // Default configuration
    let default_config = TracingConfig::default();
    println!("Default service: {}", default_config.service_name);
    println!("Default sampling rate: {}", default_config.sampling_rate);

    // Cloud Trace configuration
    let gcloud_config = TracingConfig::gcloud_default("my-project-id");
    println!("GCloud project: {:?}", gcloud_config.gcloud_project_id);

    // Jaeger configuration
    let jaeger_config = TracingConfig::jaeger_default("localhost", 6831);
    println!("Jaeger host: {:?}", jaeger_config.jaeger_host);

    // OTLP configuration with custom sampling
    let otlp_config = TracingConfig::otlp_default("http://localhost:4317")
        .with_sampling_rate(0.5)
        .with_batch_processing(true);
    println!("OTLP endpoint: {:?}", otlp_config.otlp_endpoint);
    println!("Sampling rate: {}", otlp_config.sampling_rate);

    // Example 5: Span events and metrics
    println!("\n=== Example 5: Span Events and Metrics ===");

    let event = SpanEvent::new("user-authenticated")
        .with_attribute("user_id", "12345")
        .with_attribute("auth_method", "oauth2");

    println!("Event name: {}", event.name);
    println!("Event attributes:");
    for (k, v) in &event.attributes {
        println!("  {}: {}", k, v);
    }

    // Example 6: Span handle with metrics
    println!("\n=== Example 6: Span Metrics ===");

    let mut span = SpanHandle {
        name: "request-processing".to_string(),
        start_time: std::time::Instant::now(),
        attributes: Default::default(),
    };

    span.add_attribute("method", "POST");
    span.add_attribute("path", "/api/v1/messages");

    // Simulate some work
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let metrics = span.metrics();
    println!("Span name: {}", metrics.span_name);
    println!("Duration (µs): {}", metrics.duration_micros);
    println!("Status: {}", metrics.status);
    println!("Attributes: {}", metrics.attribute_count);

    // Example 7: Sampling configuration
    println!("\n=== Example 7: Sampling Configuration ===");

    let always_on = TracingConfig::default().with_sampling_rate(1.0);
    let always_off = TracingConfig::default().with_sampling_rate(0.0);
    let half_sampled = TracingConfig::default().with_sampling_rate(0.5);

    println!("Always on rate: {}", always_on.sampling_rate);
    println!("Always off rate: {}", always_off.sampling_rate);
    println!("Half sampled rate: {}", half_sampled.sampling_rate);

    // Example 8: Header conversion
    println!("\n=== Example 8: Header Round-trip ===");

    let original_header = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let ctx = TraceContext::from_traceparent(original_header)?;
    let formatted = ctx.to_traceparent();

    println!("Original: {}", original_header);
    println!("Formatted: {}", formatted);
    println!("Match: {}", original_header == formatted);

    // Example 9: Batch and export configuration
    println!("\n=== Example 9: Batch Processing Config ===");

    let batch_config = TracingConfig::default().with_batch_processing(true);

    println!("Batch enabled: {}", batch_config.batch_spans);
    println!("Batch size: {}", batch_config.batch_size);
    println!("Max queue size: {}", batch_config.max_queue_size);
    println!("Export timeout (ms): {}", batch_config.export_timeout_ms);

    // Example 10: OpenTelemetry manager initialization (when otel feature is enabled)
    #[cfg(feature = "otel")]
    {
        println!("\n=== Example 10: OpenTelemetry Manager ===");

        let config = TracingConfig::default()
            .with_sampling_rate(0.9)
            .with_batch_processing(true);

        match osiris_edge::OpenTelemetryManager::new(config).await {
            Ok(manager) => {
                println!("Service: {}", manager.service_name());
                println!("Version: {}", manager.service_version());

                // Create a span
                let span = manager.create_span("example-operation");
                println!("Created span: {}", span.name());
                println!("Span elapsed: {:?}", span.elapsed());

                // Flush pending spans
                manager.flush().await?;
                println!("Spans flushed successfully");
            }
            Err(e) => {
                println!("Failed to initialize OTel manager: {}", e);
            }
        }
    }

    #[cfg(not(feature = "otel"))]
    {
        println!(
            "\n=== Example 10: OpenTelemetry Manager ===\
             \nNote: Enable 'otel' feature to use OpenTelemetryManager"
        );
    }

    // Example 11: Complex span with multiple attributes
    println!("\n=== Example 11: Complex Span Creation ===");

    let mut complex_span = SpanHandle {
        name: "database-query".to_string(),
        start_time: std::time::Instant::now(),
        attributes: Default::default(),
    };

    complex_span.add_attribute("db_driver", "postgresql");
    complex_span.add_attribute("query_type", "SELECT");
    complex_span.add_attribute("table", "messages");
    complex_span.add_attribute("row_count", "42");

    println!("Complex span attributes: {}", complex_span.attributes.len());
    for (k, v) in &complex_span.attributes {
        println!("  {}: {}", k, v);
    }

    println!("\n=== Examples Complete ===");

    Ok(())
}
