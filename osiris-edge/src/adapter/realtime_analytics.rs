//! Real-time WIP analytics engine implementation
//!
//! Provides live metrics tracking with:
//! - Work lifecycle tracking (arrival → start → completion)
//! - Little's Law calculations (WIP = Throughput × Lead Time)
//! - Percentile latencies (p50, p95, p99)
//! - Anomaly detection (spikes, drops, violations)
//! - Bottleneck identification
//! - SSE streaming for dashboards

use async_trait::async_trait;
use chrono::{Duration, Utc};
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tokio::time::{Duration as TokioDuration, interval};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

use crate::domain::{
    AnalyticsSnapshot, Anomaly, AnomalySeverity, AnomalyType, BottleneckSignal, BottleneckType,
    LittlesLawMetrics, PercentileLatency, TimeWindowBuffer, WipSnapshot, WorkMetrics,
};
use crate::port::{AnalyticsConfig, AnalyticsEngine};

/// Real-time analytics engine with in-memory metrics tracking
#[derive(Clone)]
pub struct RealtimeAnalyticsEngine {
    state: Arc<RwLock<AnalyticsState>>,
    config: AnalyticsConfig,
    snapshot_tx: broadcast::Sender<AnalyticsSnapshot>,
}

/// Internal analytics state
struct AnalyticsState {
    /// Active work items being tracked (id → metrics)
    work_items: HashMap<Uuid, WorkMetrics>,

    /// Completed work items (time-windowed)
    completed_buffer: TimeWindowBuffer<WorkMetrics>,

    /// Current WIP snapshot
    current_wip: Option<WipSnapshot>,

    /// Arrival timestamps (for throughput calculation)
    arrival_buffer: TimeWindowBuffer<()>,

    /// Rejection timestamps (for rejection rate)
    rejection_buffer: TimeWindowBuffer<String>,

    /// WIP samples over time (for average WIP)
    wip_samples: TimeWindowBuffer<usize>,

    /// Historical throughput (items/sec)
    throughput_history: TimeWindowBuffer<f64>,

    /// Last calculated metrics (for anomaly detection)
    last_snapshot: Option<AnalyticsSnapshot>,
}

impl RealtimeAnalyticsEngine {
    /// Create a new real-time analytics engine
    ///
    /// # Arguments
    /// * `config` - Analytics configuration
    pub fn new(config: AnalyticsConfig) -> Self {
        let window = Duration::seconds(config.window_size_sec as i64);

        let state = AnalyticsState {
            work_items: HashMap::new(),
            completed_buffer: TimeWindowBuffer::new(window),
            current_wip: None,
            arrival_buffer: TimeWindowBuffer::new(window),
            rejection_buffer: TimeWindowBuffer::new(window),
            wip_samples: TimeWindowBuffer::new(window),
            throughput_history: TimeWindowBuffer::new(window),
            last_snapshot: None,
        };

        let (snapshot_tx, _) = broadcast::channel(1000);

        let engine = Self {
            state: Arc::new(RwLock::new(state)),
            config,
            snapshot_tx,
        };

        // Start background snapshot broadcaster
        engine.start_broadcaster();

        engine
    }

    /// Start background task to broadcast periodic snapshots
    fn start_broadcaster(&self) {
        let state = Arc::clone(&self.state);
        let snapshot_tx = self.snapshot_tx.clone();
        let interval_sec = self.config.snapshot_interval_sec;
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut ticker = interval(TokioDuration::from_secs(interval_sec));

            loop {
                ticker.tick().await;

                // Calculate snapshot
                let snapshot = Self::calculate_snapshot(&state, &config).await;

                // Update last snapshot in state
                {
                    let mut state_lock = state.write().await;
                    state_lock.last_snapshot = Some(snapshot.clone());
                }

                // Broadcast to subscribers (ignore errors if no receivers)
                let _ = snapshot_tx.send(snapshot);
            }
        });
    }

    /// Calculate analytics snapshot from current state
    async fn calculate_snapshot(
        state: &Arc<RwLock<AnalyticsState>>,
        config: &AnalyticsConfig,
    ) -> AnalyticsSnapshot {
        let state_lock = state.read().await;

        // Get current WIP state
        let wip_snapshot = state_lock
            .current_wip
            .clone()
            .unwrap_or_else(|| WipSnapshot::new(0, 0, vec![]));

        // Collect completed work metrics
        let completed: Vec<&WorkMetrics> = state_lock.completed_buffer.values();

        // Calculate Little's Law metrics
        let avg_wip = if !state_lock.wip_samples.is_empty() {
            state_lock
                .wip_samples
                .values()
                .into_iter()
                .map(|&w| w as f64)
                .sum::<f64>()
                / state_lock.wip_samples.len() as f64
        } else {
            wip_snapshot.current_wip as f64
        };

        let total_lead_time_ms: i64 = completed.iter().filter_map(|m| m.lead_time_ms).sum();

        let littles_law = LittlesLawMetrics::calculate(
            avg_wip,
            completed.len(),
            total_lead_time_ms,
            config.window_size_sec,
        );

        // Calculate percentile latencies
        let lead_time_percentiles =
            Self::calculate_percentiles("lead_time", &completed, |m| m.lead_time_ms);
        let cycle_time_percentiles =
            Self::calculate_percentiles("cycle_time", &completed, |m| m.cycle_time_ms);
        let queue_time_percentiles =
            Self::calculate_percentiles("queue_time", &completed, |m| m.queue_time_ms);

        // Detect anomalies
        let anomalies = Self::detect_anomalies(
            config,
            &wip_snapshot,
            &littles_law,
            &lead_time_percentiles,
            &cycle_time_percentiles,
            &queue_time_percentiles,
            state_lock.last_snapshot.as_ref(),
        );

        // Detect bottlenecks
        let bottlenecks = Self::detect_bottlenecks(
            config,
            &wip_snapshot,
            &littles_law,
            &cycle_time_percentiles,
            &queue_time_percentiles,
            state_lock.rejection_buffer.len(),
            state_lock.arrival_buffer.len(),
        );

        AnalyticsSnapshot {
            timestamp: Utc::now(),
            wip_snapshot,
            littles_law,
            lead_time_percentiles,
            cycle_time_percentiles,
            queue_time_percentiles,
            total_arrivals: state_lock.arrival_buffer.len(),
            total_completions: completed.len(),
            total_rejections: state_lock.rejection_buffer.len(),
            anomalies,
            bottlenecks,
        }
    }

    /// Calculate percentile latencies from completed work
    fn calculate_percentiles<F>(
        metric: &str,
        completed: &[&WorkMetrics],
        extractor: F,
    ) -> PercentileLatency
    where
        F: Fn(&WorkMetrics) -> Option<i64>,
    {
        let mut samples: Vec<i64> = completed.iter().filter_map(|m| extractor(m)).collect();

        if samples.is_empty() {
            return PercentileLatency::from_sorted_samples(metric.to_string(), &[]);
        }

        samples.sort_unstable();
        PercentileLatency::from_sorted_samples(metric.to_string(), &samples)
    }

    /// Detect anomalies in current metrics
    #[allow(clippy::too_many_arguments)]
    fn detect_anomalies(
        config: &AnalyticsConfig,
        wip_snapshot: &WipSnapshot,
        littles_law: &LittlesLawMetrics,
        lead_time_percentiles: &PercentileLatency,
        cycle_time_percentiles: &PercentileLatency,
        queue_time_percentiles: &PercentileLatency,
        last_snapshot: Option<&AnalyticsSnapshot>,
    ) -> Vec<Anomaly> {
        let mut anomalies = Vec::new();
        let now = Utc::now();

        // High utilization
        if wip_snapshot.utilization_pct > config.high_utilization_threshold {
            anomalies.push(Anomaly {
                timestamp: now,
                anomaly_type: AnomalyType::HighUtilization,
                severity: if wip_snapshot.utilization_pct > 95.0 {
                    AnomalySeverity::Critical
                } else if wip_snapshot.utilization_pct > 90.0 {
                    AnomalySeverity::High
                } else {
                    AnomalySeverity::Medium
                },
                observed_value: wip_snapshot.utilization_pct,
                expected_value: config.high_utilization_threshold,
                deviation: wip_snapshot.utilization_pct - config.high_utilization_threshold,
                description: format!(
                    "WIP utilization at {:.1}% (threshold: {:.1}%)",
                    wip_snapshot.utilization_pct, config.high_utilization_threshold
                ),
            });
        }

        // Lead time spike (p95 > threshold × p50)
        if lead_time_percentiles.sample_count > 10 && lead_time_percentiles.p50_ms > 0 {
            let spike_threshold =
                (lead_time_percentiles.p50_ms as f64 * config.lead_time_spike_threshold) as i64;
            if lead_time_percentiles.p95_ms > spike_threshold {
                anomalies.push(Anomaly {
                    timestamp: now,
                    anomaly_type: AnomalyType::LeadTimeSpike,
                    severity: AnomalySeverity::High,
                    observed_value: lead_time_percentiles.p95_ms as f64,
                    expected_value: spike_threshold as f64,
                    deviation: (lead_time_percentiles.p95_ms - spike_threshold) as f64,
                    description: format!(
                        "Lead time p95 spike: {}ms ({}x median of {}ms)",
                        lead_time_percentiles.p95_ms,
                        lead_time_percentiles.p95_ms / lead_time_percentiles.p50_ms.max(1),
                        lead_time_percentiles.p50_ms
                    ),
                });
            }
        }

        // Cycle time spike
        if cycle_time_percentiles.sample_count > 10 && cycle_time_percentiles.p50_ms > 0 {
            let spike_threshold =
                (cycle_time_percentiles.p50_ms as f64 * config.cycle_time_spike_threshold) as i64;
            if cycle_time_percentiles.p95_ms > spike_threshold {
                anomalies.push(Anomaly {
                    timestamp: now,
                    anomaly_type: AnomalyType::CycleTimeSpike,
                    severity: AnomalySeverity::High,
                    observed_value: cycle_time_percentiles.p95_ms as f64,
                    expected_value: spike_threshold as f64,
                    deviation: (cycle_time_percentiles.p95_ms - spike_threshold) as f64,
                    description: format!(
                        "Cycle time p95 spike: {}ms ({}x median of {}ms)",
                        cycle_time_percentiles.p95_ms,
                        cycle_time_percentiles.p95_ms / cycle_time_percentiles.p50_ms.max(1),
                        cycle_time_percentiles.p50_ms
                    ),
                });
            }
        }

        // Queue time spike
        if queue_time_percentiles.sample_count > 10 && queue_time_percentiles.p50_ms > 0 {
            let spike_threshold = (queue_time_percentiles.p50_ms as f64 * 3.0) as i64;
            if queue_time_percentiles.p95_ms > spike_threshold {
                anomalies.push(Anomaly {
                    timestamp: now,
                    anomaly_type: AnomalyType::QueueTimeSpike,
                    severity: AnomalySeverity::High,
                    observed_value: queue_time_percentiles.p95_ms as f64,
                    expected_value: spike_threshold as f64,
                    deviation: (queue_time_percentiles.p95_ms - spike_threshold) as f64,
                    description: format!(
                        "Queue time p95 spike: {}ms ({}x median of {}ms)",
                        queue_time_percentiles.p95_ms,
                        queue_time_percentiles.p95_ms / queue_time_percentiles.p50_ms.max(1),
                        queue_time_percentiles.p50_ms
                    ),
                });
            }
        }

        // Throughput drop
        if let Some(last) = last_snapshot {
            let current_throughput = littles_law.throughput;
            let last_throughput = last.littles_law.throughput;

            if last_throughput > 0.0 {
                let drop_pct = ((last_throughput - current_throughput) / last_throughput) * 100.0;
                if drop_pct > config.throughput_drop_threshold {
                    anomalies.push(Anomaly {
                        timestamp: now,
                        anomaly_type: AnomalyType::ThroughputDrop,
                        severity: if drop_pct > 75.0 {
                            AnomalySeverity::Critical
                        } else {
                            AnomalySeverity::High
                        },
                        observed_value: current_throughput,
                        expected_value: last_throughput,
                        deviation: drop_pct,
                        description: format!(
                            "Throughput dropped {:.1}%: {:.3} → {:.3} items/sec",
                            drop_pct, last_throughput, current_throughput
                        ),
                    });
                }
            }
        }

        // Little's Law violation (WIP ≠ Throughput × Lead Time)
        if littles_law.throughput > 0.01 && littles_law.avg_lead_time_sec > 0.01 {
            let deviation_pct =
                (littles_law.wip_delta.abs() / littles_law.avg_wip.max(0.01)) * 100.0;
            if deviation_pct > config.littles_law_violation_threshold {
                anomalies.push(Anomaly {
                    timestamp: now,
                    anomaly_type: AnomalyType::LittlesLawViolation,
                    severity: AnomalySeverity::Medium,
                    observed_value: littles_law.avg_wip,
                    expected_value: littles_law.calculated_wip,
                    deviation: deviation_pct,
                    description: format!(
                        "Little's Law deviation {:.1}%: observed WIP={:.2}, calculated WIP={:.2}",
                        deviation_pct, littles_law.avg_wip, littles_law.calculated_wip
                    ),
                });
            }
        }

        anomalies
    }

    /// Detect bottlenecks from metrics
    #[allow(clippy::too_many_arguments)]
    fn detect_bottlenecks(
        config: &AnalyticsConfig,
        wip_snapshot: &WipSnapshot,
        littles_law: &LittlesLawMetrics,
        cycle_time_percentiles: &PercentileLatency,
        queue_time_percentiles: &PercentileLatency,
        rejection_count: usize,
        arrival_count: usize,
    ) -> Vec<BottleneckSignal> {
        let mut bottlenecks = Vec::new();
        let now = Utc::now();

        // High rejection rate indicates WIP limit too low
        if arrival_count > 0 {
            let rejection_rate = (rejection_count as f64 / arrival_count as f64) * 100.0;
            if rejection_rate > 20.0 {
                let confidence = if rejection_rate > 50.0 { 95 } else { 75 };
                bottlenecks.push(BottleneckSignal {
                    timestamp: now,
                    bottleneck_type: BottleneckType::WipLimitTooLow,
                    factors: vec![
                        format!("Rejection rate: {:.1}%", rejection_rate),
                        format!("Utilization: {:.1}%", wip_snapshot.utilization_pct),
                    ],
                    recommendation: format!(
                        "Consider increasing WIP limit from {} to {}",
                        wip_snapshot.wip_limit,
                        wip_snapshot.wip_limit + (wip_snapshot.wip_limit / 2).max(1)
                    ),
                    confidence,
                });
            }
        }

        // High cycle time indicates slow processing
        if cycle_time_percentiles.sample_count > 10 {
            let cycle_median_sec = cycle_time_percentiles.p50_ms as f64 / 1000.0;
            let cycle_p95_sec = cycle_time_percentiles.p95_ms as f64 / 1000.0;

            if cycle_median_sec > config.window_size_sec as f64 * 0.1 {
                bottlenecks.push(BottleneckSignal {
                    timestamp: now,
                    bottleneck_type: BottleneckType::SlowProcessing,
                    factors: vec![
                        format!("Median cycle time: {:.1}s", cycle_median_sec),
                        format!("P95 cycle time: {:.1}s", cycle_p95_sec),
                    ],
                    recommendation: "Investigate processing bottlenecks or optimize work execution"
                        .to_string(),
                    confidence: 80,
                });
            }
        }

        // Queue time >> cycle time indicates queue buildup
        if queue_time_percentiles.sample_count > 10 && cycle_time_percentiles.sample_count > 10 {
            let queue_median = queue_time_percentiles.p50_ms;
            let cycle_median = cycle_time_percentiles.p50_ms;

            if queue_median > cycle_median * 2 {
                bottlenecks.push(BottleneckSignal {
                    timestamp: now,
                    bottleneck_type: BottleneckType::QueueBuildup,
                    factors: vec![
                        format!("Queue time median: {}ms", queue_median),
                        format!("Cycle time median: {}ms", cycle_median),
                        format!(
                            "Queue/Cycle ratio: {:.1}x",
                            queue_median as f64 / cycle_median.max(1) as f64
                        ),
                    ],
                    recommendation: "Increase WIP limit or reduce arrival rate to clear queue"
                        .to_string(),
                    confidence: 85,
                });
            }
        }

        // High throughput variance indicates burst traffic
        if littles_law.throughput > 0.0 && wip_snapshot.utilization_pct > 80.0 {
            bottlenecks.push(BottleneckSignal {
                timestamp: now,
                bottleneck_type: BottleneckType::BurstTraffic,
                factors: vec![
                    format!(
                        "Current throughput: {:.2} items/sec",
                        littles_law.throughput
                    ),
                    format!("Utilization: {:.1}%", wip_snapshot.utilization_pct),
                ],
                recommendation: "Implement request buffering or rate limiting for burst protection"
                    .to_string(),
                confidence: 70,
            });
        }

        bottlenecks
    }

    /// Evict old work items to prevent memory growth
    async fn evict_old_items(&self) {
        let mut state = self.state.write().await;

        if state.work_items.len() > self.config.max_tracked_items {
            // Remove oldest items
            let cutoff = Utc::now() - Duration::hours(1);
            state
                .work_items
                .retain(|_, m| m.arrived_at > cutoff || m.completed_at.is_none());
        }
    }
}

#[async_trait]
impl AnalyticsEngine for RealtimeAnalyticsEngine {
    async fn record_arrival(&self, id: Uuid, work_type: String) {
        let mut state = self.state.write().await;
        state.work_items.insert(id, WorkMetrics::new(id, work_type));
        state.arrival_buffer.push(());
    }

    async fn record_start(&self, id: Uuid) {
        let mut state = self.state.write().await;
        if let Some(metrics) = state.work_items.get_mut(&id) {
            metrics.start();
        }
    }

    async fn record_completion(&self, id: Uuid) {
        let mut state = self.state.write().await;
        if let Some(metrics) = state.work_items.get_mut(&id) {
            metrics.complete();
            // Clone before dropping the borrow
            let completed_metrics = metrics.clone();
            drop(metrics); // Explicitly drop the mutable borrow
            // Move to completed buffer
            state.completed_buffer.push(completed_metrics);
        }
        // Keep in work_items for queryability (will be evicted later)
    }

    async fn record_rejection(&self, work_type: String) {
        let mut state = self.state.write().await;
        state.rejection_buffer.push(work_type);
    }

    async fn update_wip_state(&self, current_wip: usize, wip_limit: usize, in_progress: Vec<Uuid>) {
        let mut state = self.state.write().await;
        state.current_wip = Some(WipSnapshot::new(current_wip, wip_limit, in_progress));
        state.wip_samples.push(current_wip);
    }

    async fn get_snapshot(&self) -> AnalyticsSnapshot {
        Self::calculate_snapshot(&self.state, &self.config).await
    }

    async fn get_work_metrics(&self, id: &Uuid) -> Option<WorkMetrics> {
        let state = self.state.read().await;
        state.work_items.get(id).cloned()
    }

    fn subscribe(&self) -> Pin<Box<dyn Stream<Item = AnalyticsSnapshot> + Send>> {
        let rx = self.snapshot_tx.subscribe();
        Box::pin(BroadcastStream::new(rx).filter_map(|result| result.ok()))
    }

    async fn get_time_series(&self, metric: &str, window_sec: u64) -> Vec<(i64, f64)> {
        let state = self.state.read().await;
        let window_sec = window_sec.min(3600); // Cap at 1 hour

        match metric {
            "wip" => state
                .wip_samples
                .values()
                .into_iter()
                .enumerate()
                .map(|(i, &v)| {
                    let ts = Utc::now().timestamp() - (window_sec as i64) + (i as i64);
                    (ts, v as f64)
                })
                .collect(),
            "throughput" => state
                .throughput_history
                .values()
                .into_iter()
                .enumerate()
                .map(|(i, &v)| {
                    let ts = Utc::now().timestamp() - (window_sec as i64) + (i as i64);
                    (ts, v)
                })
                .collect(),
            _ => vec![],
        }
    }

    async fn reset(&self) {
        let mut state = self.state.write().await;
        state.work_items.clear();
        state.completed_buffer.clear();
        state.current_wip = None;
        state.arrival_buffer.clear();
        state.rejection_buffer.clear();
        state.wip_samples.clear();
        state.throughput_history.clear();
        state.last_snapshot = None;
    }
}

impl Drop for RealtimeAnalyticsEngine {
    fn drop(&mut self) {
        // Background broadcaster task will naturally terminate when all clones are dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_work_lifecycle() {
        let config = AnalyticsConfig::default();
        let engine = RealtimeAnalyticsEngine::new(config);

        let work_id = Uuid::new_v4();

        // Record arrival
        engine
            .record_arrival(work_id, "test_work".to_string())
            .await;

        let metrics = engine.get_work_metrics(&work_id).await.unwrap();
        assert!(metrics.started_at.is_none());

        // Record start
        engine.record_start(work_id).await;

        let metrics = engine.get_work_metrics(&work_id).await.unwrap();
        assert!(metrics.started_at.is_some());
        assert!(metrics.queue_time_ms.is_some());

        // Record completion
        engine.record_completion(work_id).await;

        let metrics = engine.get_work_metrics(&work_id).await.unwrap();
        assert!(metrics.completed_at.is_some());
        assert!(metrics.lead_time_ms.is_some());
        assert!(metrics.cycle_time_ms.is_some());
    }

    #[tokio::test]
    async fn test_wip_state_update() {
        let config = AnalyticsConfig::default();
        let engine = RealtimeAnalyticsEngine::new(config);

        let in_progress = vec![Uuid::new_v4(), Uuid::new_v4()];
        engine.update_wip_state(2, 5, in_progress.clone()).await;

        let snapshot = engine.get_snapshot().await;
        assert_eq!(snapshot.wip_snapshot.current_wip, 2);
        assert_eq!(snapshot.wip_snapshot.wip_limit, 5);
        assert_eq!(snapshot.wip_snapshot.available, 3);
        assert_eq!(snapshot.wip_snapshot.utilization_pct, 40.0);
    }

    #[tokio::test]
    async fn test_snapshot_calculation() {
        let config = AnalyticsConfig::default();
        let engine = RealtimeAnalyticsEngine::new(config);

        // Simulate some work
        for i in 0..5 {
            let work_id = Uuid::new_v4();
            engine.record_arrival(work_id, format!("work_{}", i)).await;
            engine.record_start(work_id).await;
            tokio::time::sleep(TokioDuration::from_millis(10)).await;
            engine.record_completion(work_id).await;
        }

        let snapshot = engine.get_snapshot().await;
        assert_eq!(snapshot.total_arrivals, 5);
        assert_eq!(snapshot.total_completions, 5);
        assert_eq!(snapshot.total_rejections, 0);
        assert!(snapshot.littles_law.throughput >= 0.0);
    }

    #[tokio::test]
    async fn test_rejection_tracking() {
        let config = AnalyticsConfig::default();
        let engine = RealtimeAnalyticsEngine::new(config);

        engine.record_rejection("email".to_string()).await;
        engine.record_rejection("calendar".to_string()).await;

        let snapshot = engine.get_snapshot().await;
        assert_eq!(snapshot.total_rejections, 2);
    }

    #[tokio::test]
    async fn test_subscribe_stream() {
        let config = AnalyticsConfig {
            snapshot_interval_sec: 1,
            ..Default::default()
        };
        let engine = RealtimeAnalyticsEngine::new(config);

        let mut stream = engine.subscribe();

        // Wait for at least one snapshot
        tokio::time::sleep(TokioDuration::from_secs(2)).await;

        // Should receive snapshot
        let snapshot = tokio::time::timeout(TokioDuration::from_secs(3), stream.next())
            .await
            .expect("Timeout waiting for snapshot")
            .expect("Stream ended");

        assert!(snapshot.timestamp <= Utc::now());
    }

    #[tokio::test]
    async fn test_anomaly_detection_high_utilization() {
        let config = AnalyticsConfig {
            high_utilization_threshold: 80.0,
            ..Default::default()
        };
        let engine = RealtimeAnalyticsEngine::new(config);

        // Simulate high utilization
        engine.update_wip_state(9, 10, vec![]).await;

        let snapshot = engine.get_snapshot().await;
        let has_high_util_anomaly = snapshot
            .anomalies
            .iter()
            .any(|a| matches!(a.anomaly_type, AnomalyType::HighUtilization));

        assert!(has_high_util_anomaly);
    }

    #[tokio::test]
    async fn test_reset() {
        let config = AnalyticsConfig::default();
        let engine = RealtimeAnalyticsEngine::new(config);

        // Add some data
        engine
            .record_arrival(Uuid::new_v4(), "test".to_string())
            .await;
        engine.record_rejection("test".to_string()).await;

        let snapshot = engine.get_snapshot().await;
        assert!(snapshot.total_arrivals > 0 || snapshot.total_rejections > 0);

        // Reset
        engine.reset().await;

        let snapshot = engine.get_snapshot().await;
        assert_eq!(snapshot.total_arrivals, 0);
        assert_eq!(snapshot.total_rejections, 0);
    }
}
