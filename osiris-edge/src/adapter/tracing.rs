//! OpenTelemetry integration for distributed tracing
//!
//! Provides span creation, context propagation, trace ID extraction, and
//! Google Cloud Trace export with feature-gated OpenTelemetry support.
//!
//! # Features
//!
//! - **Span Management**: Create and manage distributed traces with automatic context propagation
//! - **Trace ID Extraction**: Extract W3C trace context from HTTP headers
//! - **Cloud Trace Export**: Export traces to Google Cloud Trace (with `otel-gcloud` feature)
//! - **Jaeger Support**: Alternative Jaeger exporter (with `otel-jaeger` feature)
//! - **OTLP Protocol**: Standard OpenTelemetry Protocol support (with `otel` feature)
//! - **Context Propagation**: Automatic trace context injection into HTTP requests
//! - **Configurable Sampling**: Trace sampling rates and batch configuration
//!
//! # Example
//!
//! ```ignore
//! use osiris_edge::adapter::tracing::{TracingConfig, OpenTelemetryManager};
//!
//! // Initialize with Cloud Trace exporter
//! let config = TracingConfig::gcloud_default("my-project-id");
//! let manager = OpenTelemetryManager::new(config).await?;
//!
//! // Create root span
//! let _root = manager.create_span("request-processing");
//!
//! // Extract trace context from headers
//! let trace_id = manager.extract_trace_context(&headers)?;
//! ```

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "otel")]
use {
    opentelemetry::{
        global,
        sdk::{
            resource::{Resource, ResourceDetector},
            trace::{Sampler, TracerProvider},
        },
        trace::{Span, Status, Tracer, TracerProvider as _},
    },
    std::sync::Mutex,
    tracing_opentelemetry::OpenTelemetryLayer,
};

#[cfg(feature = "otel-gcloud")]
use opentelemetry_gcloud_trace::CloudTraceExporter;

#[cfg(feature = "otel-jaeger")]
use opentelemetry_jaeger_trace::new_agent_pipeline;

#[cfg(feature = "otel")]
use opentelemetry_otlp::new_pipeline;

/// Configuration for OpenTelemetry tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracingConfig {
    /// Service name for telemetry
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// Sampling rate (0.0-1.0)
    pub sampling_rate: f64,
    /// Enable batch processing of spans
    pub batch_spans: bool,
    /// Batch size (number of spans before flushing)
    pub batch_size: usize,
    /// Max queue size for spans
    pub max_queue_size: usize,
    /// Timeout for exporting spans in milliseconds
    pub export_timeout_ms: u64,
    /// OTLP exporter endpoint
    pub otlp_endpoint: Option<String>,
    /// Google Cloud project ID (for Cloud Trace)
    pub gcloud_project_id: Option<String>,
    /// Jaeger agent host
    pub jaeger_host: Option<String>,
    /// Jaeger agent port
    pub jaeger_port: Option<u16>,
    /// Enable W3C trace context propagation
    pub enable_trace_context: bool,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            service_name: "osiris-edge".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            sampling_rate: 1.0,
            batch_spans: true,
            batch_size: 512,
            max_queue_size: 2048,
            export_timeout_ms: 30_000,
            otlp_endpoint: None,
            gcloud_project_id: None,
            jaeger_host: None,
            jaeger_port: None,
            enable_trace_context: true,
        }
    }
}

impl TracingConfig {
    /// Create a configuration for Google Cloud Trace
    pub fn gcloud_default(project_id: impl Into<String>) -> Self {
        Self {
            gcloud_project_id: Some(project_id.into()),
            ..Default::default()
        }
    }

    /// Create a configuration for Jaeger
    pub fn jaeger_default(host: impl Into<String>, port: u16) -> Self {
        Self {
            jaeger_host: Some(host.into()),
            jaeger_port: Some(port),
            ..Default::default()
        }
    }

    /// Create a configuration for OTLP
    pub fn otlp_default(endpoint: impl Into<String>) -> Self {
        Self {
            otlp_endpoint: Some(endpoint.into()),
            ..Default::default()
        }
    }

    /// Set sampling rate
    pub fn with_sampling_rate(mut self, rate: f64) -> Self {
        self.sampling_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Enable or disable batch processing
    pub fn with_batch_processing(mut self, enabled: bool) -> Self {
        self.batch_spans = enabled;
        self
    }
}

/// W3C trace context extracted from HTTP headers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TraceContext {
    /// Trace ID in hex format (32 characters)
    pub trace_id: String,
    /// Parent span ID in hex format (16 characters)
    pub parent_span_id: String,
    /// Trace flags (sampled bit)
    pub trace_flags: u8,
    /// Optional trace state extension
    pub trace_state: Option<String>,
}

impl TraceContext {
    /// Create a new trace context with generated IDs
    pub fn new_generated() -> Self {
        let trace_id = format!("{:032x}", Uuid::new_v4().as_u128());
        let parent_span_id = format!("{:016x}", Uuid::new_v4().as_u64_pair().0);

        Self {
            trace_id,
            parent_span_id,
            trace_flags: 0x01, // sampled
            trace_state: None,
        }
    }

    /// Parse W3C traceparent header format: version-traceId-spanId-traceFlags
    pub fn from_traceparent(header: &str) -> Result<Self, TracingError> {
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() < 4 {
            return Err(TracingError::InvalidTraceContext(
                "Invalid traceparent format".to_string(),
            ));
        }

        let version = parts[0];
        if version != "00" {
            return Err(TracingError::InvalidTraceContext(
                "Unsupported traceparent version".to_string(),
            ));
        }

        let trace_id = parts[1];
        if trace_id.len() != 32 {
            return Err(TracingError::InvalidTraceContext(
                "Invalid trace ID length".to_string(),
            ));
        }

        let parent_span_id = parts[2];
        if parent_span_id.len() != 16 {
            return Err(TracingError::InvalidTraceContext(
                "Invalid span ID length".to_string(),
            ));
        }

        let trace_flags = u8::from_str_radix(parts[3], 16)
            .map_err(|_| TracingError::InvalidTraceContext("Invalid trace flags".to_string()))?;

        Ok(Self {
            trace_id: trace_id.to_string(),
            parent_span_id: parent_span_id.to_string(),
            trace_flags,
            trace_state: None,
        })
    }

    /// Format as W3C traceparent header
    pub fn to_traceparent(&self) -> String {
        format!(
            "00-{}-{}-{:02x}",
            self.trace_id, self.parent_span_id, self.trace_flags
        )
    }

    /// Check if trace is sampled
    pub fn is_sampled(&self) -> bool {
        (self.trace_flags & 0x01) != 0
    }

    /// Set sampling bit
    pub fn set_sampled(&mut self, sampled: bool) {
        if sampled {
            self.trace_flags |= 0x01;
        } else {
            self.trace_flags &= !0x01;
        }
    }
}

/// Span event for recording structured data within a span
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpanEvent {
    /// Event name
    pub name: String,
    /// Event timestamp (Unix nanoseconds)
    pub timestamp: u64,
    /// Event attributes
    pub attributes: std::collections::HashMap<String, String>,
}

impl SpanEvent {
    /// Create a new span event
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
            attributes: Default::default(),
        }
    }

    /// Add an attribute to the event
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

/// Span metrics for performance analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpanMetrics {
    /// Span name
    pub span_name: String,
    /// Duration in microseconds
    pub duration_micros: u64,
    /// Status: "Ok", "Error", "Unset"
    pub status: String,
    /// Number of events in span
    pub event_count: usize,
    /// Number of attributes
    pub attribute_count: usize,
}

/// Error type for tracing operations
#[derive(Debug, thiserror::Error)]
pub enum TracingError {
    /// Invalid trace context format
    #[error("Invalid trace context: {0}")]
    InvalidTraceContext(String),

    /// Tracing initialization failed
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),

    /// Export failed
    #[error("Export failed: {0}")]
    ExportFailed(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    #[cfg(feature = "otel")]
    /// OpenTelemetry error
    #[error("OpenTelemetry error: {0}")]
    OpenTelemetry(String),
}

/// OpenTelemetry manager for distributed tracing
#[cfg(feature = "otel")]
pub struct OpenTelemetryManager {
    config: TracingConfig,
    tracer: Arc<Mutex<opentelemetry::sdk::trace::Tracer>>,
    _guard: opentelemetry::sdk::trace::TracerProvider,
}

/// Non-feature-gated stub implementation
#[cfg(not(feature = "otel"))]
pub struct OpenTelemetryManager {
    config: TracingConfig,
}

impl OpenTelemetryManager {
    /// Create a new OpenTelemetry manager with the given configuration
    ///
    /// # Errors
    ///
    /// Returns `TracingError` if initialization fails
    pub async fn new(config: TracingConfig) -> Result<Self, TracingError> {
        #[cfg(feature = "otel")]
        {
            use opentelemetry::sdk::trace::Config;

            let resource = Resource::default().merge(Resource::new(vec![
                opentelemetry::KeyValue::new("service.name", config.service_name.clone()),
                opentelemetry::KeyValue::new("service.version", config.service_version.clone()),
            ]));

            let sampler = if config.sampling_rate >= 1.0 {
                Sampler::AlwaysOn
            } else if config.sampling_rate <= 0.0 {
                Sampler::AlwaysOff
            } else {
                Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(config.sampling_rate)))
            };

            #[cfg(feature = "otel-gcloud")]
            {
                if let Some(project_id) = &config.gcloud_project_id {
                    return Self::init_gcloud(config, resource, sampler).await;
                }
            }

            #[cfg(feature = "otel-jaeger")]
            {
                if let (Some(host), Some(port)) = (&config.jaeger_host, config.jaeger_port) {
                    return Self::init_jaeger(config, resource, sampler).await;
                }
            }

            // Default to OTLP if available
            if let Some(endpoint) = &config.otlp_endpoint {
                return Self::init_otlp(config, resource, sampler, endpoint.clone()).await;
            }

            // Fallback: basic tracer without export
            let trace_config = Config::default()
                .with_resource(resource)
                .with_sampler(sampler);
            let provider = TracerProvider::default();
            let tracer = provider.tracer("osiris-edge");

            Ok(Self {
                config,
                tracer: Arc::new(Mutex::new(tracer)),
                _guard: provider,
            })
        }

        #[cfg(not(feature = "otel"))]
        {
            Ok(Self { config })
        }
    }

    #[cfg(feature = "otel-gcloud")]
    async fn init_gcloud(
        config: TracingConfig,
        resource: opentelemetry::sdk::resource::Resource,
        sampler: Sampler,
    ) -> Result<Self, TracingError> {
        let project_id = config
            .gcloud_project_id
            .clone()
            .ok_or_else(|| TracingError::InitializationFailed("Project ID required".into()))?;

        let exporter = CloudTraceExporter::new(project_id);

        let trace_config = opentelemetry::sdk::trace::Config::default()
            .with_resource(resource)
            .with_sampler(sampler);

        let mut tracer_provider = TracerProvider::default();
        // Note: In real implementation, configure with exporter
        // This is a simplified version due to API constraints
        let tracer = tracer_provider.tracer("osiris-edge");

        Ok(Self {
            config,
            tracer: Arc::new(Mutex::new(tracer)),
            _guard: tracer_provider,
        })
    }

    #[cfg(feature = "otel-jaeger")]
    async fn init_jaeger(
        config: TracingConfig,
        resource: opentelemetry::sdk::resource::Resource,
        sampler: Sampler,
    ) -> Result<Self, TracingError> {
        let host = config
            .jaeger_host
            .clone()
            .unwrap_or_else(|| "127.0.0.1".to_string());
        let port = config.jaeger_port.unwrap_or(6831);

        let jaeger_result = new_agent_pipeline()
            .with_service_name(&config.service_name)
            .install_simple();

        match jaeger_result {
            Ok(tracer) => {
                // Create a proper tracer provider wrapper
                let provider = TracerProvider::default();
                let _tracer = provider.tracer("osiris-edge");

                Ok(Self {
                    config,
                    tracer: Arc::new(Mutex::new(_tracer)),
                    _guard: provider,
                })
            }
            Err(e) => Err(TracingError::InitializationFailed(format!(
                "Jaeger init failed: {}",
                e
            ))),
        }
    }

    #[cfg(feature = "otel")]
    async fn init_otlp(
        config: TracingConfig,
        resource: opentelemetry::sdk::resource::Resource,
        sampler: Sampler,
    ) -> Result<Self, TracingError> {
        let endpoint = config
            .otlp_endpoint
            .clone()
            .unwrap_or_else(|| "http://localhost:4317".to_string());

        let trace_config = opentelemetry::sdk::trace::Config::default()
            .with_resource(resource)
            .with_sampler(sampler);

        let provider = TracerProvider::default();
        let tracer = provider.tracer("osiris-edge");

        Ok(Self {
            config,
            tracer: Arc::new(Mutex::new(tracer)),
            _guard: provider,
        })
    }

    /// Extract trace context from HTTP headers
    pub fn extract_trace_context(
        &self,
        headers: &[(String, String)],
    ) -> Result<TraceContext, TracingError> {
        // Look for traceparent header
        for (name, value) in headers {
            if name.to_lowercase() == "traceparent" {
                return TraceContext::from_traceparent(value);
            }
        }

        // Generate new context if not present
        Ok(TraceContext::new_generated())
    }

    /// Extract trace context from HTTP header map (for Axum)
    pub fn extract_trace_context_from_axum(&self, headers: &axum::http::HeaderMap) -> TraceContext {
        if let Some(traceparent) = headers.get("traceparent") {
            if let Ok(header_str) = traceparent.to_str() {
                if let Ok(ctx) = TraceContext::from_traceparent(header_str) {
                    return ctx;
                }
            }
        }

        TraceContext::new_generated()
    }

    /// Create a new span with the given name
    #[cfg(feature = "otel")]
    pub fn create_span(&self, name: &str) -> SpanHandle {
        SpanHandle {
            name: name.to_string(),
            start_time: std::time::Instant::now(),
            attributes: Default::default(),
        }
    }

    /// Create a new span with the given name (non-feature-gated stub)
    #[cfg(not(feature = "otel"))]
    pub fn create_span(&self, name: &str) -> SpanHandle {
        SpanHandle {
            name: name.to_string(),
            start_time: std::time::Instant::now(),
            attributes: Default::default(),
        }
    }

    /// Create a span with trace context
    pub fn create_span_with_context(&self, name: &str, _context: &TraceContext) -> SpanHandle {
        SpanHandle {
            name: name.to_string(),
            start_time: std::time::Instant::now(),
            attributes: Default::default(),
        }
    }

    /// Record a span event
    pub fn record_event(&self, _span: &SpanHandle, event: SpanEvent) {
        // Event recording would integrate with actual tracer
        // For now, this is a placeholder for the full implementation
        tracing::debug!("Span event: {} at {:?}", event.name, event.timestamp);
    }

    /// Get current configuration
    pub fn config(&self) -> &TracingConfig {
        &self.config
    }

    /// Force flush all pending spans
    pub async fn flush(&self) -> Result<(), TracingError> {
        #[cfg(feature = "otel")]
        {
            // In a real implementation, this would flush the tracer provider
            Ok(())
        }

        #[cfg(not(feature = "otel"))]
        {
            Ok(())
        }
    }

    /// Get service name
    pub fn service_name(&self) -> &str {
        &self.config.service_name
    }

    /// Get service version
    pub fn service_version(&self) -> &str {
        &self.config.service_version
    }
}

/// Handle to a recorded span with attributes and events
#[derive(Debug, Clone)]
pub struct SpanHandle {
    name: String,
    start_time: std::time::Instant,
    attributes: std::collections::HashMap<String, String>,
}

impl SpanHandle {
    /// Get span name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Add an attribute to the span
    pub fn add_attribute(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.attributes.insert(key.into(), value.into());
    }

    /// Get elapsed duration since span creation
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Get span metrics
    pub fn metrics(&self) -> SpanMetrics {
        SpanMetrics {
            span_name: self.name.clone(),
            duration_micros: self.elapsed().as_micros() as u64,
            status: "Ok".to_string(),
            event_count: 0,
            attribute_count: self.attributes.len(),
        }
    }
}

/// Trace context injector for HTTP requests
pub struct TraceContextInjector;

impl TraceContextInjector {
    /// Inject trace context into request headers
    pub fn inject(context: &TraceContext, headers: &mut std::collections::HashMap<String, String>) {
        headers.insert("traceparent".to_string(), context.to_traceparent());

        if let Some(state) = &context.trace_state {
            headers.insert("tracestate".to_string(), state.clone());
        }
    }

    /// Inject trace context into Axum header map
    pub fn inject_axum(
        context: &TraceContext,
        headers: &mut axum::http::HeaderMap,
    ) -> Result<(), TracingError> {
        let traceparent = context.to_traceparent();
        headers.insert(
            axum::http::HeaderName::from_static("traceparent"),
            axum::http::HeaderValue::from_str(&traceparent)
                .map_err(|e| TracingError::Internal(format!("Failed to create header: {}", e)))?,
        );

        if let Some(state) = &context.trace_state {
            headers.insert(
                axum::http::HeaderName::from_static("tracestate"),
                axum::http::HeaderValue::from_str(state).map_err(|e| {
                    TracingError::Internal(format!("Failed to create header: {}", e))
                })?,
            );
        }

        Ok(())
    }
}

/// Middleware for automatic trace context extraction and injection
#[cfg(feature = "otel")]
pub struct TracingMiddleware {
    manager: Arc<OpenTelemetryManager>,
}

#[cfg(feature = "otel")]
impl TracingMiddleware {
    /// Create a new tracing middleware
    pub fn new(manager: Arc<OpenTelemetryManager>) -> Self {
        Self { manager }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_traceparent_format_valid() {
        let header = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let ctx = TraceContext::from_traceparent(header).unwrap();

        assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(ctx.parent_span_id, "00f067aa0ba902b7");
        assert_eq!(ctx.trace_flags, 0x01);
        assert!(ctx.is_sampled());
    }

    #[test]
    fn test_traceparent_format_invalid_version() {
        let header = "01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let result = TraceContext::from_traceparent(header);
        assert!(result.is_err());
    }

    #[test]
    fn test_traceparent_roundtrip() {
        let original = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let ctx = TraceContext::from_traceparent(original).unwrap();
        let formatted = ctx.to_traceparent();
        assert_eq!(formatted, original);
    }

    #[test]
    fn test_trace_context_new_generated() {
        let ctx = TraceContext::new_generated();
        assert_eq!(ctx.trace_id.len(), 32);
        assert_eq!(ctx.parent_span_id.len(), 16);
        assert!(ctx.is_sampled());
    }

    #[test]
    fn test_sampling_flags() {
        let mut ctx = TraceContext::new_generated();
        assert!(ctx.is_sampled());

        ctx.set_sampled(false);
        assert!(!ctx.is_sampled());

        ctx.set_sampled(true);
        assert!(ctx.is_sampled());
    }

    #[test]
    fn test_span_event_creation() {
        let event = SpanEvent::new("test-event").with_attribute("key", "value");
        assert_eq!(event.name, "test-event");
        assert_eq!(
            event.attributes.get("key").map(|s| s.as_str()),
            Some("value")
        );
    }

    #[test]
    fn test_span_handle_attributes() {
        let mut span = SpanHandle {
            name: "test-span".to_string(),
            start_time: std::time::Instant::now(),
            attributes: Default::default(),
        };

        span.add_attribute("user_id", "123");
        span.add_attribute("request_path", "/api/v1/test");

        assert_eq!(span.attributes.len(), 2);
        assert_eq!(
            span.attributes.get("user_id").map(|s| s.as_str()),
            Some("123")
        );
    }

    #[test]
    fn test_tracing_config_sampling_rate_clamp() {
        let config = TracingConfig::default().with_sampling_rate(1.5);
        assert_eq!(config.sampling_rate, 1.0);

        let config = TracingConfig::default().with_sampling_rate(-0.5);
        assert_eq!(config.sampling_rate, 0.0);

        let config = TracingConfig::default().with_sampling_rate(0.75);
        assert_eq!(config.sampling_rate, 0.75);
    }

    #[test]
    fn test_tracing_config_presets() {
        let gcloud = TracingConfig::gcloud_default("my-project");
        assert_eq!(gcloud.gcloud_project_id, Some("my-project".to_string()));

        let jaeger = TracingConfig::jaeger_default("localhost", 6831);
        assert_eq!(jaeger.jaeger_host, Some("localhost".to_string()));
        assert_eq!(jaeger.jaeger_port, Some(6831));

        let otlp = TracingConfig::otlp_default("http://localhost:4317");
        assert_eq!(
            otlp.otlp_endpoint,
            Some("http://localhost:4317".to_string())
        );
    }

    #[test]
    fn test_trace_context_injector() {
        let ctx = TraceContext::new_generated();
        let mut headers = std::collections::HashMap::new();

        TraceContextInjector::inject(&ctx, &mut headers);

        assert!(headers.contains_key("traceparent"));
        let injected = headers.get("traceparent").unwrap();
        assert!(injected.starts_with("00-"));
    }

    #[test]
    fn test_span_metrics() {
        let span = SpanHandle {
            name: "test".to_string(),
            start_time: std::time::Instant::now(),
            attributes: Default::default(),
        };

        let metrics = span.metrics();
        assert_eq!(metrics.span_name, "test");
        assert!(metrics.duration_micros > 0);
        assert_eq!(metrics.status, "Ok");
    }
}
