//! Analytics engine port definitions
//!
//! Defines the interface for real-time WIP analytics with metrics collection,
//! Little's Law calculations, percentile latencies, anomaly detection, and SSE streaming.

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use uuid::Uuid;

use crate::domain::{AnalyticsSnapshot, WorkMetrics};

/// Real-time WIP analytics engine
///
/// Tracks work metrics through the system lifecycle and provides:
/// - Live WIP state monitoring
/// - Little's Law calculations (WIP = Throughput × Lead Time)
/// - Percentile latency measurements (p50, p95, p99)
/// - Anomaly detection (spikes, drops, violations)
/// - Bottleneck identification
/// - SSE streaming for dashboards
#[async_trait]
pub trait AnalyticsEngine: Send + Sync {
    /// Record work arrival (entering the system)
    ///
    /// # Arguments
    /// * `id` - Work item identifier
    /// * `work_type` - Type of work for categorization
    async fn record_arrival(&self, id: Uuid, work_type: String);

    /// Record work start (WIP permit acquired)
    ///
    /// # Arguments
    /// * `id` - Work item identifier
    async fn record_start(&self, id: Uuid);

    /// Record work completion (WIP permit released)
    ///
    /// # Arguments
    /// * `id` - Work item identifier
    async fn record_completion(&self, id: Uuid);

    /// Record work rejection (WIP limit reached)
    ///
    /// # Arguments
    /// * `work_type` - Type of work that was rejected
    async fn record_rejection(&self, work_type: String);

    /// Update current WIP state from gate
    ///
    /// # Arguments
    /// * `current_wip` - Current WIP count
    /// * `wip_limit` - WIP limit
    /// * `in_progress` - List of work item IDs currently in progress
    async fn update_wip_state(&self, current_wip: usize, wip_limit: usize, in_progress: Vec<Uuid>);

    /// Get current analytics snapshot
    ///
    /// Returns aggregated metrics including:
    /// - WIP state
    /// - Little's Law metrics
    /// - Percentile latencies
    /// - Anomalies
    /// - Bottlenecks
    async fn get_snapshot(&self) -> AnalyticsSnapshot;

    /// Get work metrics for a specific item
    ///
    /// # Arguments
    /// * `id` - Work item identifier
    async fn get_work_metrics(&self, id: &Uuid) -> Option<WorkMetrics>;

    /// Subscribe to analytics snapshot stream (SSE)
    ///
    /// Returns a stream that emits periodic analytics snapshots.
    /// The stream continues until dropped or the engine shuts down.
    fn subscribe(&self) -> Pin<Box<dyn Stream<Item = AnalyticsSnapshot> + Send>>;

    /// Get time-series data for a specific metric
    ///
    /// # Arguments
    /// * `metric` - Metric name (wip, throughput, lead_time, cycle_time, queue_time)
    /// * `window_sec` - Time window in seconds (max 3600)
    ///
    /// Returns time-series points for charting
    async fn get_time_series(&self, metric: &str, window_sec: u64) -> Vec<(i64, f64)>;

    /// Reset all metrics (for testing)
    async fn reset(&self);
}

/// Analytics engine configuration
#[derive(Debug, Clone)]
pub struct AnalyticsConfig {
    /// Time window for metrics calculation (seconds)
    pub window_size_sec: u64,

    /// Snapshot broadcast interval (seconds)
    pub snapshot_interval_sec: u64,

    /// High utilization threshold (percentage, 0-100)
    pub high_utilization_threshold: f64,

    /// Lead time spike threshold (multiple of median)
    pub lead_time_spike_threshold: f64,

    /// Cycle time spike threshold (multiple of median)
    pub cycle_time_spike_threshold: f64,

    /// Throughput drop threshold (percentage, 0-100)
    pub throughput_drop_threshold: f64,

    /// Little's Law violation threshold (percentage deviation, 0-100)
    pub littles_law_violation_threshold: f64,

    /// Maximum work items to track (prevents memory growth)
    pub max_tracked_items: usize,
}

impl Default for AnalyticsConfig {
    fn default() -> Self {
        Self {
            window_size_sec: 300,     // 5 minutes
            snapshot_interval_sec: 5, // 5 seconds
            high_utilization_threshold: 85.0,
            lead_time_spike_threshold: 3.0,        // 3x median
            cycle_time_spike_threshold: 3.0,       // 3x median
            throughput_drop_threshold: 50.0,       // 50% drop
            littles_law_violation_threshold: 25.0, // 25% deviation
            max_tracked_items: 10000,
        }
    }
}

impl AnalyticsConfig {
    /// Create a new analytics configuration with custom settings
    pub fn builder() -> AnalyticsConfigBuilder {
        AnalyticsConfigBuilder::default()
    }
}

/// Builder for AnalyticsConfig
#[derive(Debug, Default)]
pub struct AnalyticsConfigBuilder {
    window_size_sec: Option<u64>,
    snapshot_interval_sec: Option<u64>,
    high_utilization_threshold: Option<f64>,
    lead_time_spike_threshold: Option<f64>,
    cycle_time_spike_threshold: Option<f64>,
    throughput_drop_threshold: Option<f64>,
    littles_law_violation_threshold: Option<f64>,
    max_tracked_items: Option<usize>,
}

impl AnalyticsConfigBuilder {
    /// Set window size in seconds
    #[must_use]
    pub fn window_size_sec(mut self, value: u64) -> Self {
        self.window_size_sec = Some(value);
        self
    }

    /// Set snapshot interval in seconds
    #[must_use]
    pub fn snapshot_interval_sec(mut self, value: u64) -> Self {
        self.snapshot_interval_sec = Some(value);
        self
    }

    /// Set high utilization threshold (0-100)
    #[must_use]
    pub fn high_utilization_threshold(mut self, value: f64) -> Self {
        self.high_utilization_threshold = Some(value);
        self
    }

    /// Set lead time spike threshold (multiple of median)
    #[must_use]
    pub fn lead_time_spike_threshold(mut self, value: f64) -> Self {
        self.lead_time_spike_threshold = Some(value);
        self
    }

    /// Set cycle time spike threshold (multiple of median)
    #[must_use]
    pub fn cycle_time_spike_threshold(mut self, value: f64) -> Self {
        self.cycle_time_spike_threshold = Some(value);
        self
    }

    /// Set throughput drop threshold (0-100)
    #[must_use]
    pub fn throughput_drop_threshold(mut self, value: f64) -> Self {
        self.throughput_drop_threshold = Some(value);
        self
    }

    /// Set Little's Law violation threshold (0-100)
    #[must_use]
    pub fn littles_law_violation_threshold(mut self, value: f64) -> Self {
        self.littles_law_violation_threshold = Some(value);
        self
    }

    /// Set max tracked items
    #[must_use]
    pub fn max_tracked_items(mut self, value: usize) -> Self {
        self.max_tracked_items = Some(value);
        self
    }

    /// Build the configuration
    #[must_use]
    pub fn build(self) -> AnalyticsConfig {
        let default = AnalyticsConfig::default();
        AnalyticsConfig {
            window_size_sec: self.window_size_sec.unwrap_or(default.window_size_sec),
            snapshot_interval_sec: self
                .snapshot_interval_sec
                .unwrap_or(default.snapshot_interval_sec),
            high_utilization_threshold: self
                .high_utilization_threshold
                .unwrap_or(default.high_utilization_threshold),
            lead_time_spike_threshold: self
                .lead_time_spike_threshold
                .unwrap_or(default.lead_time_spike_threshold),
            cycle_time_spike_threshold: self
                .cycle_time_spike_threshold
                .unwrap_or(default.cycle_time_spike_threshold),
            throughput_drop_threshold: self
                .throughput_drop_threshold
                .unwrap_or(default.throughput_drop_threshold),
            littles_law_violation_threshold: self
                .littles_law_violation_threshold
                .unwrap_or(default.littles_law_violation_threshold),
            max_tracked_items: self.max_tracked_items.unwrap_or(default.max_tracked_items),
        }
    }
}
