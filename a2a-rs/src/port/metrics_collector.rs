//! Metrics collection port for observability
//!
//! This port defines the interface for collecting and exporting metrics about
//! the A2A protocol implementation, including message throughput, task lifecycle,
//! and connection statistics.

use async_trait::async_trait;

/// Interface for collecting metrics about A2A protocol operations
///
/// Implementations of this trait provide metrics collection for monitoring
/// and observability systems such as Prometheus, StatsD, or custom backends.
#[async_trait]
pub trait MetricsCollector: Send + Sync {
    /// Record that a message was sent
    ///
    /// # Arguments
    /// * `labels` - Optional labels for categorizing the metric (e.g., agent_id, message_type)
    fn record_message_sent(&self, labels: &[(&str, &str)]);

    /// Record that a message was received
    ///
    /// # Arguments
    /// * `labels` - Optional labels for categorizing the metric (e.g., agent_id, message_type)
    fn record_message_received(&self, labels: &[(&str, &str)]);

    /// Record that a task was created
    ///
    /// # Arguments
    /// * `labels` - Optional labels for categorizing the metric (e.g., task_type, priority)
    fn record_task_created(&self, labels: &[(&str, &str)]);

    /// Record the latency of message processing
    ///
    /// # Arguments
    /// * `duration_ms` - Duration in milliseconds
    /// * `labels` - Optional labels for categorizing the metric
    fn record_message_latency(&self, duration_ms: f64, labels: &[(&str, &str)]);

    /// Record the duration of task execution
    ///
    /// # Arguments
    /// * `duration_ms` - Duration in milliseconds
    /// * `labels` - Optional labels for categorizing the metric
    fn record_task_duration(&self, duration_ms: f64, labels: &[(&str, &str)]);

    /// Set the current number of active connections
    ///
    /// # Arguments
    /// * `count` - Number of active connections
    /// * `labels` - Optional labels for categorizing the metric (e.g., protocol, transport)
    fn set_active_connections(&self, count: i64, labels: &[(&str, &str)]);

    /// Set the current queue depth
    ///
    /// # Arguments
    /// * `depth` - Number of items in queue
    /// * `labels` - Optional labels for categorizing the metric (e.g., queue_name)
    fn set_queue_depth(&self, depth: i64, labels: &[(&str, &str)]);

    /// Export metrics in the format expected by the implementation
    ///
    /// Returns the metrics data as a string suitable for scraping/export.
    /// For Prometheus, this would be the text exposition format.
    async fn export_metrics(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;
}

/// A no-op metrics collector that discards all metrics
///
/// Useful for testing or when metrics are not desired.
#[derive(Debug, Clone, Default)]
pub struct NoopMetricsCollector;

#[async_trait]
impl MetricsCollector for NoopMetricsCollector {
    fn record_message_sent(&self, _labels: &[(&str, &str)]) {}
    fn record_message_received(&self, _labels: &[(&str, &str)]) {}
    fn record_task_created(&self, _labels: &[(&str, &str)]) {}
    fn record_message_latency(&self, _duration_ms: f64, _labels: &[(&str, &str)]) {}
    fn record_task_duration(&self, _duration_ms: f64, _labels: &[(&str, &str)]) {}
    fn set_active_connections(&self, _count: i64, _labels: &[(&str, &str)]) {}
    fn set_queue_depth(&self, _depth: i64, _labels: &[(&str, &str)]) {}

    async fn export_metrics(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok(String::new())
    }
}
