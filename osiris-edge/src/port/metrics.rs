//! Metrics collector port definitions
//!
//! Defines the interface for collecting and exposing observability metrics
//! including request duration, error rates, and business metrics.

use async_trait::async_trait;

/// Metrics collector for tracking request and system metrics
///
/// Provides interfaces for recording:
/// - Request counters (total, by status, by endpoint)
/// - Request duration histograms
/// - Error rates and types
/// - Active connection gauges
/// - Custom business metrics
#[async_trait]
pub trait MetricsCollector: Send + Sync {
    /// Record an HTTP request
    ///
    /// # Arguments
    /// * `method` - HTTP method (GET, POST, etc.)
    /// * `path` - Request path
    /// * `status_code` - Response HTTP status code
    /// * `duration_ms` - Request duration in milliseconds
    async fn record_request(&self, method: &str, path: &str, status_code: u16, duration_ms: u64);

    /// Record an error
    ///
    /// # Arguments
    /// * `error_type` - Type/category of error
    /// * `path` - Request path where error occurred
    async fn record_error(&self, error_type: &str, path: &str);

    /// Record active connections gauge
    ///
    /// # Arguments
    /// * `count` - Current number of active connections
    async fn set_active_connections(&self, count: usize);

    /// Increment counter for a custom metric
    ///
    /// # Arguments
    /// * `name` - Metric name
    /// * `labels` - Optional labels as (key, value) pairs
    async fn increment_counter(&self, name: &str, labels: Vec<(&str, &str)>);

    /// Set gauge value for a custom metric
    ///
    /// # Arguments
    /// * `name` - Metric name
    /// * `value` - Gauge value
    /// * `labels` - Optional labels as (key, value) pairs
    async fn set_gauge(&self, name: &str, value: f64, labels: Vec<(&str, &str)>);

    /// Record histogram observation
    ///
    /// # Arguments
    /// * `name` - Metric name
    /// * `value` - Observation value
    /// * `labels` - Optional labels as (key, value) pairs
    async fn observe_histogram(&self, name: &str, value: f64, labels: Vec<(&str, &str)>);

    /// Get Prometheus text format metrics
    ///
    /// Returns all collected metrics in Prometheus exposition format (text/plain)
    async fn get_metrics(&self) -> String;

    /// Reset all metrics (for testing)
    async fn reset(&self);
}

/// Metrics error types
#[derive(Debug, Clone)]
pub enum MetricsError {
    /// Failed to create metric
    MetricCreationFailed(String),
    /// Failed to record value
    RecordFailed(String),
    /// Invalid metric name
    InvalidMetricName(String),
}

impl std::fmt::Display for MetricsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetricsError::MetricCreationFailed(msg) => write!(f, "Metric creation failed: {}", msg),
            MetricsError::RecordFailed(msg) => write!(f, "Record failed: {}", msg),
            MetricsError::InvalidMetricName(msg) => write!(f, "Invalid metric name: {}", msg),
        }
    }
}

impl std::error::Error for MetricsError {}
