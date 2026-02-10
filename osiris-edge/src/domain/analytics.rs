//! WIP analytics domain types
//!
//! Real-time metrics for work-in-progress tracking, Little's Law calculations,
//! percentile latencies, and anomaly detection.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// Individual work item metrics tracked through its lifecycle
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkMetrics {
    /// Work item identifier
    pub id: Uuid,

    /// When work entered the system (arrival time)
    pub arrived_at: DateTime<Utc>,

    /// When work started processing (acquired WIP permit)
    pub started_at: Option<DateTime<Utc>>,

    /// When work completed
    pub completed_at: Option<DateTime<Utc>>,

    /// Work type for categorization
    pub work_type: String,

    /// Lead time: arrival → completion (queue + processing)
    pub lead_time_ms: Option<i64>,

    /// Cycle time: start → completion (processing only)
    pub cycle_time_ms: Option<i64>,

    /// Queue time: arrival → start (waiting time)
    pub queue_time_ms: Option<i64>,
}

impl WorkMetrics {
    /// Create new work metrics when work arrives
    #[must_use]
    pub fn new(id: Uuid, work_type: String) -> Self {
        Self {
            id,
            arrived_at: Utc::now(),
            started_at: None,
            completed_at: None,
            work_type,
            lead_time_ms: None,
            cycle_time_ms: None,
            queue_time_ms: None,
        }
    }

    /// Mark work as started (WIP permit acquired)
    pub fn start(&mut self) {
        let now = Utc::now();
        self.started_at = Some(now);
        self.queue_time_ms = Some((now - self.arrived_at).num_milliseconds());
    }

    /// Mark work as completed and calculate times
    pub fn complete(&mut self) {
        let now = Utc::now();
        self.completed_at = Some(now);
        self.lead_time_ms = Some((now - self.arrived_at).num_milliseconds());

        if let Some(started_at) = self.started_at {
            self.cycle_time_ms = Some((now - started_at).num_milliseconds());
        }
    }
}

/// Point-in-time WIP state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WipSnapshot {
    /// Snapshot timestamp
    pub timestamp: DateTime<Utc>,

    /// Current WIP (occupied slots)
    pub current_wip: usize,

    /// WIP limit (capacity)
    pub wip_limit: usize,

    /// Available slots
    pub available: usize,

    /// Utilization percentage (0-100)
    pub utilization_pct: f64,

    /// Work items currently in progress
    pub in_progress: Vec<Uuid>,
}

impl WipSnapshot {
    /// Create snapshot from gate state
    #[must_use]
    pub fn new(current_wip: usize, wip_limit: usize, in_progress: Vec<Uuid>) -> Self {
        let available = wip_limit.saturating_sub(current_wip);
        let utilization_pct = if wip_limit > 0 {
            (current_wip as f64 / wip_limit as f64) * 100.0
        } else {
            0.0
        };

        Self {
            timestamp: Utc::now(),
            current_wip,
            wip_limit,
            available,
            utilization_pct,
            in_progress,
        }
    }
}

/// Little's Law metrics: WIP = Throughput × Lead Time
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LittlesLawMetrics {
    /// Observed average WIP
    pub avg_wip: f64,

    /// Throughput (items completed per second)
    pub throughput: f64,

    /// Average lead time (seconds)
    pub avg_lead_time_sec: f64,

    /// Calculated WIP from Little's Law: λ × L
    pub calculated_wip: f64,

    /// Difference between observed and calculated WIP
    pub wip_delta: f64,

    /// Window size used for calculation (seconds)
    pub window_size_sec: u64,
}

impl LittlesLawMetrics {
    /// Calculate Little's Law metrics from observations
    #[must_use]
    pub fn calculate(
        avg_wip: f64,
        completed_count: usize,
        total_lead_time_ms: i64,
        window_size_sec: u64,
    ) -> Self {
        let throughput = if window_size_sec > 0 {
            completed_count as f64 / window_size_sec as f64
        } else {
            0.0
        };

        let avg_lead_time_sec = if completed_count > 0 {
            (total_lead_time_ms as f64 / 1000.0) / completed_count as f64
        } else {
            0.0
        };

        let calculated_wip = throughput * avg_lead_time_sec;
        let wip_delta = avg_wip - calculated_wip;

        Self {
            avg_wip,
            throughput,
            avg_lead_time_sec,
            calculated_wip,
            wip_delta,
            window_size_sec,
        }
    }
}

/// Percentile latency measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PercentileLatency {
    /// Metric name (lead_time, cycle_time, queue_time)
    pub metric: String,

    /// 50th percentile (median) in milliseconds
    pub p50_ms: i64,

    /// 95th percentile in milliseconds
    pub p95_ms: i64,

    /// 99th percentile in milliseconds
    pub p99_ms: i64,

    /// Minimum value in milliseconds
    pub min_ms: i64,

    /// Maximum value in milliseconds
    pub max_ms: i64,

    /// Sample count
    pub sample_count: usize,
}

impl PercentileLatency {
    /// Calculate percentiles from sorted samples
    #[must_use]
    pub fn from_sorted_samples(metric: String, sorted_samples: &[i64]) -> Self {
        if sorted_samples.is_empty() {
            return Self {
                metric,
                p50_ms: 0,
                p95_ms: 0,
                p99_ms: 0,
                min_ms: 0,
                max_ms: 0,
                sample_count: 0,
            };
        }

        let len = sorted_samples.len();
        let p50_idx = (len as f64 * 0.50) as usize;
        let p95_idx = (len as f64 * 0.95) as usize;
        let p99_idx = (len as f64 * 0.99) as usize;

        Self {
            metric,
            p50_ms: sorted_samples[p50_idx.min(len - 1)],
            p95_ms: sorted_samples[p95_idx.min(len - 1)],
            p99_ms: sorted_samples[p99_idx.min(len - 1)],
            min_ms: sorted_samples[0],
            max_ms: sorted_samples[len - 1],
            sample_count: len,
        }
    }
}

/// Anomaly detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Anomaly {
    /// Anomaly timestamp
    pub timestamp: DateTime<Utc>,

    /// Anomaly type
    pub anomaly_type: AnomalyType,

    /// Severity level
    pub severity: AnomalySeverity,

    /// Observed value
    pub observed_value: f64,

    /// Expected value or threshold
    pub expected_value: f64,

    /// Deviation from expected (absolute or percentage)
    pub deviation: f64,

    /// Human-readable description
    pub description: String,
}

/// Type of anomaly detected
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyType {
    /// WIP utilization spike
    HighUtilization,

    /// Lead time spike
    LeadTimeSpike,

    /// Cycle time spike
    CycleTimeSpike,

    /// Queue time spike
    QueueTimeSpike,

    /// Throughput drop
    ThroughputDrop,

    /// Little's Law violation (WIP ≠ Throughput × Lead Time)
    LittlesLawViolation,
}

/// Anomaly severity level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Bottleneck detection signal
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BottleneckSignal {
    /// Detection timestamp
    pub timestamp: DateTime<Utc>,

    /// Bottleneck type
    pub bottleneck_type: BottleneckType,

    /// Contributing factors
    pub factors: Vec<String>,

    /// Recommended action
    pub recommendation: String,

    /// Confidence score (0-100)
    pub confidence: u8,
}

/// Type of bottleneck detected
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BottleneckType {
    /// WIP limit too low (high rejection rate)
    WipLimitTooLow,

    /// Processing too slow (high cycle time)
    SlowProcessing,

    /// Queue buildup (high queue time vs cycle time)
    QueueBuildup,

    /// Burst traffic (rapid arrival rate)
    BurstTraffic,
}

/// Aggregated analytics snapshot for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSnapshot {
    /// Snapshot timestamp
    pub timestamp: DateTime<Utc>,

    /// Current WIP state
    pub wip_snapshot: WipSnapshot,

    /// Little's Law metrics
    pub littles_law: LittlesLawMetrics,

    /// Lead time percentiles
    pub lead_time_percentiles: PercentileLatency,

    /// Cycle time percentiles
    pub cycle_time_percentiles: PercentileLatency,

    /// Queue time percentiles
    pub queue_time_percentiles: PercentileLatency,

    /// Total arrivals in window
    pub total_arrivals: usize,

    /// Total completions in window
    pub total_completions: usize,

    /// Total rejections (WIP limit reached)
    pub total_rejections: usize,

    /// Detected anomalies
    pub anomalies: Vec<Anomaly>,

    /// Detected bottlenecks
    pub bottlenecks: Vec<BottleneckSignal>,
}

/// Time-series data point for charting
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSeriesPoint {
    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Metric value
    pub value: f64,
}

/// Circular buffer for efficient time-windowed metrics
#[derive(Debug, Clone)]
pub struct TimeWindowBuffer<T> {
    buffer: VecDeque<(DateTime<Utc>, T)>,
    window: Duration,
}

impl<T: Clone> TimeWindowBuffer<T> {
    /// Create new time window buffer
    #[must_use]
    pub fn new(window: Duration) -> Self {
        Self {
            buffer: VecDeque::new(),
            window,
        }
    }

    /// Push a value with timestamp
    pub fn push(&mut self, value: T) {
        let now = Utc::now();
        self.buffer.push_back((now, value));
        self.evict_old();
    }

    /// Get all values within window
    pub fn values(&self) -> Vec<&T> {
        self.buffer.iter().map(|(_, v)| v).collect()
    }

    /// Count of values in window
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Evict values older than window
    fn evict_old(&mut self) {
        let cutoff = Utc::now() - self.window;
        while let Some((ts, _)) = self.buffer.front() {
            if *ts < cutoff {
                self.buffer.pop_front();
            } else {
                break;
            }
        }
    }

    /// Clear all values
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_metrics_lifecycle() {
        let mut metrics = WorkMetrics::new(Uuid::new_v4(), "email".to_string());

        assert!(metrics.started_at.is_none());
        assert!(metrics.completed_at.is_none());

        metrics.start();
        assert!(metrics.started_at.is_some());
        assert!(metrics.queue_time_ms.is_some());
        assert!(metrics.queue_time_ms.unwrap() >= 0);

        metrics.complete();
        assert!(metrics.completed_at.is_some());
        assert!(metrics.lead_time_ms.is_some());
        assert!(metrics.cycle_time_ms.is_some());
    }

    #[test]
    fn test_wip_snapshot() {
        let in_progress = vec![Uuid::new_v4(), Uuid::new_v4()];
        let snapshot = WipSnapshot::new(2, 5, in_progress);

        assert_eq!(snapshot.current_wip, 2);
        assert_eq!(snapshot.wip_limit, 5);
        assert_eq!(snapshot.available, 3);
        assert_eq!(snapshot.utilization_pct, 40.0);
    }

    #[test]
    fn test_littles_law_calculation() {
        // avg_wip=2, completed=10 items, total_lead_time=20000ms, window=60s
        let metrics = LittlesLawMetrics::calculate(2.0, 10, 20000, 60);

        assert_eq!(metrics.avg_wip, 2.0);
        assert!((metrics.throughput - 10.0 / 60.0).abs() < 0.001); // ~0.166 items/sec
        assert!((metrics.avg_lead_time_sec - 2.0).abs() < 0.001); // 2000ms avg per item
        // WIP = λ × L = 0.166 × 2.0 ≈ 0.333
        assert!((metrics.calculated_wip - 0.333).abs() < 0.01);
    }

    #[test]
    fn test_percentile_latency() {
        let samples = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let percentiles = PercentileLatency::from_sorted_samples("test".to_string(), &samples);

        assert_eq!(percentiles.sample_count, 10);
        assert_eq!(percentiles.min_ms, 10);
        assert_eq!(percentiles.max_ms, 100);
        assert_eq!(percentiles.p50_ms, 50); // median
        assert!(percentiles.p95_ms >= 90);
        assert!(percentiles.p99_ms >= 90);
    }

    #[test]
    fn test_percentile_latency_empty() {
        let percentiles = PercentileLatency::from_sorted_samples("test".to_string(), &[]);

        assert_eq!(percentiles.sample_count, 0);
        assert_eq!(percentiles.min_ms, 0);
        assert_eq!(percentiles.max_ms, 0);
    }

    #[test]
    fn test_time_window_buffer() {
        let mut buffer = TimeWindowBuffer::new(Duration::seconds(1));

        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        assert_eq!(buffer.len(), 3);

        // Wait for window to expire
        std::thread::sleep(std::time::Duration::from_millis(1100));
        buffer.push(4); // Triggers eviction

        assert!(buffer.len() <= 1); // Old values evicted
    }

    #[test]
    fn test_anomaly_creation() {
        let anomaly = Anomaly {
            timestamp: Utc::now(),
            anomaly_type: AnomalyType::HighUtilization,
            severity: AnomalySeverity::High,
            observed_value: 95.0,
            expected_value: 70.0,
            deviation: 25.0,
            description: "WIP utilization at 95%".to_string(),
        };

        assert_eq!(anomaly.severity, AnomalySeverity::High);
        assert_eq!(anomaly.deviation, 25.0);
    }

    #[test]
    fn test_bottleneck_signal() {
        let bottleneck = BottleneckSignal {
            timestamp: Utc::now(),
            bottleneck_type: BottleneckType::WipLimitTooLow,
            factors: vec!["High rejection rate: 45%".to_string()],
            recommendation: "Increase WIP limit from 5 to 8".to_string(),
            confidence: 85,
        };

        assert_eq!(bottleneck.confidence, 85);
        assert_eq!(bottleneck.factors.len(), 1);
    }
}
