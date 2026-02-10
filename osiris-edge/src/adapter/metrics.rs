//! Prometheus metrics collector adapter
//!
//! Implements metrics collection using the Prometheus client library.
//! Provides counters, gauges, and histograms for request tracking,
//! error monitoring, and custom business metrics.

use async_trait::async_trait;
use prometheus::{
    CounterVec, Encoder, Gauge, HistogramVec, Registry, TextEncoder,
    register_counter_vec_with_registry, register_gauge_with_registry,
    register_histogram_vec_with_registry,
};
use std::sync::Arc;
use tracing::error;

use crate::port::{MetricsCollector, MetricsError};

/// Prometheus metrics collector
///
/// Tracks:
/// - HTTP request counters (total requests by method/path/status)
/// - Request duration histograms (latency distribution)
/// - Error rates and types
/// - Active connection gauges
/// - Custom business metrics
#[derive(Clone)]
pub struct PrometheusCollector {
    /// Prometheus registry holding all metrics
    registry: Arc<Registry>,

    /// HTTP requests counter (labels: method, path, status)
    http_requests: Arc<CounterVec>,

    /// Request duration histogram in seconds (labels: method, path)
    request_duration: Arc<HistogramVec>,

    /// Error counter (labels: error_type, path)
    errors: Arc<CounterVec>,

    /// Active connections gauge
    active_connections: Arc<Gauge>,

    /// Custom counter metrics storage
    custom_counters: Arc<parking_lot::RwLock<std::collections::HashMap<String, Arc<CounterVec>>>>,

    /// Custom gauge metrics storage
    custom_gauges: Arc<parking_lot::RwLock<std::collections::HashMap<String, Arc<Gauge>>>>,

    /// Custom histogram metrics storage
    custom_histograms:
        Arc<parking_lot::RwLock<std::collections::HashMap<String, Arc<HistogramVec>>>>,
}

impl PrometheusCollector {
    /// Create a new Prometheus metrics collector with default registry
    pub fn new() -> Result<Self, MetricsError> {
        let registry = Registry::new();
        Self::with_registry(registry)
    }

    /// Create a new Prometheus metrics collector with a specific registry
    pub fn with_registry(registry: Registry) -> Result<Self, MetricsError> {
        let registry = Arc::new(registry);

        // Create HTTP requests counter
        let http_requests = register_counter_vec_with_registry!(
            prometheus::opts!(
                "http_requests_total",
                "Total HTTP requests by method, path, and status code"
            ),
            &["method", "path", "status"],
            (*registry).clone()
        )
        .map_err(|e| {
            MetricsError::MetricCreationFailed(format!("Failed to create http_requests: {}", e))
        })?;

        // Create request duration histogram (in seconds, bucketed)
        let request_duration = register_histogram_vec_with_registry!(
            prometheus::histogram_opts!(
                "http_request_duration_seconds",
                "HTTP request duration in seconds",
                vec![
                    0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0
                ]
            ),
            &["method", "path"],
            (*registry).clone()
        )
        .map_err(|e| {
            MetricsError::MetricCreationFailed(format!("Failed to create request_duration: {}", e))
        })?;

        // Create error counter
        let errors = register_counter_vec_with_registry!(
            prometheus::opts!("errors_total", "Total errors by type and path"),
            &["error_type", "path"],
            (*registry).clone()
        )
        .map_err(|e| {
            MetricsError::MetricCreationFailed(format!("Failed to create errors: {}", e))
        })?;

        // Create active connections gauge
        let active_connections = register_gauge_with_registry!(
            prometheus::opts!("active_connections", "Current number of active connections"),
            (*registry).clone()
        )
        .map_err(|e| {
            MetricsError::MetricCreationFailed(format!(
                "Failed to create active_connections: {}",
                e
            ))
        })?;

        Ok(Self {
            registry,
            http_requests: Arc::new(http_requests),
            request_duration: Arc::new(request_duration),
            errors: Arc::new(errors),
            active_connections: Arc::new(active_connections),
            custom_counters: Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
            custom_gauges: Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
            custom_histograms: Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// Get the internal Prometheus registry
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}

impl Default for PrometheusCollector {
    fn default() -> Self {
        Self::new().expect("Failed to create PrometheusCollector")
    }
}

#[async_trait]
impl MetricsCollector for PrometheusCollector {
    /// Record an HTTP request with method, path, status code, and duration
    async fn record_request(&self, method: &str, path: &str, status_code: u16, duration_ms: u64) {
        let status_str = status_code.to_string();
        let duration_sec = duration_ms as f64 / 1000.0;

        // Record request counter
        self.http_requests
            .with_label_values(&[method, path, &status_str])
            .inc();

        // Record duration histogram
        self.request_duration
            .with_label_values(&[method, path])
            .observe(duration_sec);
    }

    /// Record an error by type and path
    async fn record_error(&self, error_type: &str, path: &str) {
        self.errors.with_label_values(&[error_type, path]).inc();
    }

    /// Set the current number of active connections
    async fn set_active_connections(&self, count: usize) {
        self.active_connections.set(count as f64);
    }

    /// Increment a custom counter metric
    async fn increment_counter(&self, name: &str, labels: Vec<(&str, &str)>) {
        let counters = self.custom_counters.read();

        if let Some(counter) = counters.get(name) {
            let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
            counter.with_label_values(&label_values).inc();
        } else {
            drop(counters);

            // Create counter if it doesn't exist
            let label_names: Vec<&str> = labels.iter().map(|(k, _)| *k).collect();
            match register_counter_vec_with_registry!(
                prometheus::opts!(name, "Custom counter metric"),
                &label_names,
                (*self.registry).clone()
            ) {
                Ok(counter) => {
                    let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
                    counter.with_label_values(&label_values).inc();
                    self.custom_counters
                        .write()
                        .insert(name.to_string(), Arc::new(counter));
                }
                Err(e) => {
                    error!("Failed to create custom counter {}: {}", name, e);
                }
            }
        }
    }

    /// Set a custom gauge metric
    async fn set_gauge(&self, name: &str, value: f64, _labels: Vec<(&str, &str)>) {
        let gauges = self.custom_gauges.read();

        if let Some(gauge) = gauges.get(name) {
            gauge.set(value);
        } else {
            drop(gauges);

            // Create gauge if it doesn't exist
            match register_gauge_with_registry!(
                prometheus::opts!(name, "Custom gauge metric"),
                (*self.registry).clone()
            ) {
                Ok(gauge) => {
                    gauge.set(value);
                    self.custom_gauges
                        .write()
                        .insert(name.to_string(), Arc::new(gauge));
                }
                Err(e) => {
                    error!("Failed to create custom gauge {}: {}", name, e);
                }
            }
        }
    }

    /// Record a histogram observation
    async fn observe_histogram(&self, name: &str, value: f64, labels: Vec<(&str, &str)>) {
        let histograms = self.custom_histograms.read();

        if let Some(histogram) = histograms.get(name) {
            let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
            histogram.with_label_values(&label_values).observe(value);
        } else {
            drop(histograms);

            // Create histogram if it doesn't exist
            let label_names: Vec<&str> = labels.iter().map(|(k, _)| *k).collect();
            match register_histogram_vec_with_registry!(
                prometheus::histogram_opts!(name, "Custom histogram metric"),
                &label_names,
                (*self.registry).clone()
            ) {
                Ok(histogram) => {
                    let label_values: Vec<&str> = labels.iter().map(|(_, v)| *v).collect();
                    histogram.with_label_values(&label_values).observe(value);
                    self.custom_histograms
                        .write()
                        .insert(name.to_string(), Arc::new(histogram));
                }
                Err(e) => {
                    error!("Failed to create custom histogram {}: {}", name, e);
                }
            }
        }
    }

    /// Get Prometheus text format metrics
    ///
    /// Returns all collected metrics in Prometheus exposition format.
    async fn get_metrics(&self) -> String {
        let mut buffer = vec![];
        let encoder = TextEncoder::new();

        match encoder.encode(&self.registry.gather(), &mut buffer) {
            Ok(()) => String::from_utf8_lossy(&buffer).to_string(),
            Err(e) => {
                error!("Failed to encode metrics: {}", e);
                format!("Error encoding metrics: {}", e)
            }
        }
    }

    /// Reset all metrics (for testing)
    async fn reset(&self) {
        // Prometheus registry doesn't support reset, but we can log it
        tracing::debug!("Metrics reset requested (Prometheus registry cannot be reset)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_prometheus_collector_creation() {
        let collector = PrometheusCollector::new();
        assert!(collector.is_ok());
    }

    #[tokio::test]
    async fn test_record_request() {
        let collector = PrometheusCollector::new().unwrap();

        collector.record_request("GET", "/api/health", 200, 5).await;

        let metrics = collector.get_metrics().await;
        assert!(metrics.contains("http_requests_total"));
        assert!(metrics.contains("http_request_duration_seconds"));
    }

    #[tokio::test]
    async fn test_record_multiple_requests() {
        let collector = PrometheusCollector::new().unwrap();

        for i in 0..5 {
            collector
                .record_request("POST", "/api/process", 202, 10 + i * 2)
                .await;
        }

        let metrics = collector.get_metrics().await;
        assert!(metrics.contains("http_requests_total"));
        assert!(metrics.contains("method=\"POST\""));
    }

    #[tokio::test]
    async fn test_record_error() {
        let collector = PrometheusCollector::new().unwrap();

        collector
            .record_error("validation_error", "/api/webhook")
            .await;
        collector
            .record_error("validation_error", "/api/webhook")
            .await;

        let metrics = collector.get_metrics().await;
        assert!(metrics.contains("errors_total"));
        assert!(metrics.contains("error_type=\"validation_error\""));
    }

    #[tokio::test]
    async fn test_active_connections() {
        let collector = PrometheusCollector::new().unwrap();

        collector.set_active_connections(42).await;

        let metrics = collector.get_metrics().await;
        assert!(metrics.contains("active_connections 42"));
    }

    #[tokio::test]
    async fn test_custom_counter() {
        let collector = PrometheusCollector::new().unwrap();

        collector
            .increment_counter("custom_events", vec![("event_type", "webhook")])
            .await;
        collector
            .increment_counter("custom_events", vec![("event_type", "webhook")])
            .await;

        let metrics = collector.get_metrics().await;
        assert!(metrics.contains("custom_events"));
    }

    #[tokio::test]
    async fn test_custom_gauge() {
        let collector = PrometheusCollector::new().unwrap();

        collector
            .set_gauge("queue_depth", 123.0, vec![("queue", "notifications")])
            .await;

        let metrics = collector.get_metrics().await;
        assert!(metrics.contains("queue_depth"));
        assert!(metrics.contains("123"));
    }

    #[tokio::test]
    async fn test_custom_histogram() {
        let collector = PrometheusCollector::new().unwrap();

        collector
            .observe_histogram("processing_time", 0.5, vec![("operation", "compute")])
            .await;

        let metrics = collector.get_metrics().await;
        assert!(metrics.contains("processing_time"));
    }

    #[tokio::test]
    async fn test_different_status_codes() {
        let collector = PrometheusCollector::new().unwrap();

        collector.record_request("GET", "/api/test", 200, 5).await;
        collector.record_request("GET", "/api/test", 404, 3).await;
        collector.record_request("GET", "/api/test", 500, 15).await;

        let metrics = collector.get_metrics().await;
        assert!(metrics.contains("status=\"200\""));
        assert!(metrics.contains("status=\"404\""));
        assert!(metrics.contains("status=\"500\""));
    }

    #[tokio::test]
    async fn test_metrics_exposition_format() {
        let collector = PrometheusCollector::new().unwrap();

        collector.record_request("GET", "/metrics", 200, 2).await;

        let metrics = collector.get_metrics().await;

        // Check for Prometheus format characteristics
        assert!(metrics.contains("# HELP") || metrics.contains("http_requests_total"));
        assert!(metrics.contains("http_requests_total"));

        // Ensure it's valid UTF-8
        assert!(!metrics.is_empty());
    }
}
