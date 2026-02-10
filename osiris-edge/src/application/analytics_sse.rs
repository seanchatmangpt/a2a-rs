//! SSE streaming endpoint for real-time WIP analytics
//!
//! Provides Server-Sent Events stream for live dashboard updates.

use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::StreamExt;

use crate::adapter::RealtimeAnalyticsEngine;
use crate::domain::AnalyticsSnapshot;
use crate::port::AnalyticsEngine;

/// SSE handler for analytics snapshots
///
/// Streams live analytics data to clients for dashboard visualization.
/// Each event contains a complete analytics snapshot with:
/// - Current WIP state
/// - Little's Law metrics
/// - Percentile latencies
/// - Detected anomalies
/// - Detected bottlenecks
///
/// # Example
/// ```no_run
/// use axum::{Router, routing::get};
/// use osiris_edge::adapter::RealtimeAnalyticsEngine;
/// use osiris_edge::port::AnalyticsConfig;
/// use osiris_edge::application::analytics_sse_handler;
/// use std::sync::Arc;
///
/// # async fn example() {
/// let analytics = Arc::new(RealtimeAnalyticsEngine::new(AnalyticsConfig::default()));
///
/// let app = Router::new()
///     .route("/analytics/stream", get(analytics_sse_handler))
///     .with_state(analytics);
/// # }
/// ```
pub async fn analytics_sse_handler(
    analytics: Arc<RealtimeAnalyticsEngine>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = analytics
        .subscribe()
        .map(|snapshot| Ok(snapshot_to_event(snapshot)));

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Convert analytics snapshot to SSE event
fn snapshot_to_event(snapshot: AnalyticsSnapshot) -> Event {
    let data = serde_json::to_string(&snapshot)
        .unwrap_or_else(|e| format!(r#"{{"error":"Failed to serialize snapshot: {}"}}"#, e));

    Event::default()
        .event("analytics")
        .id(snapshot.timestamp.timestamp_millis().to_string())
        .data(data)
}

/// Metrics snapshot handler (GET /analytics/snapshot)
///
/// Returns the current analytics snapshot as JSON (non-streaming).
pub async fn analytics_snapshot_handler(
    analytics: Arc<RealtimeAnalyticsEngine>,
) -> impl IntoResponse {
    let snapshot = analytics.get_snapshot().await;
    axum::Json(snapshot)
}

/// Time series handler (GET /analytics/timeseries?metric=wip&window=300)
///
/// Returns time-series data for a specific metric.
pub async fn analytics_timeseries_handler(
    analytics: Arc<RealtimeAnalyticsEngine>,
    metric: String,
    window_sec: Option<u64>,
) -> impl IntoResponse {
    let window = window_sec.unwrap_or(300); // Default 5 minutes
    let data = analytics.get_time_series(&metric, window).await;

    axum::Json(serde_json::json!({
        "metric": metric,
        "window_sec": window,
        "data": data
    }))
}

/// Health check handler (GET /analytics/health)
///
/// Returns basic health information about the analytics engine.
pub async fn analytics_health_handler(
    analytics: Arc<RealtimeAnalyticsEngine>,
) -> impl IntoResponse {
    let snapshot = analytics.get_snapshot().await;

    axum::Json(serde_json::json!({
        "status": "healthy",
        "timestamp": snapshot.timestamp,
        "current_wip": snapshot.wip_snapshot.current_wip,
        "wip_limit": snapshot.wip_snapshot.wip_limit,
        "utilization_pct": snapshot.wip_snapshot.utilization_pct,
        "total_completions": snapshot.total_completions,
        "total_rejections": snapshot.total_rejections,
        "anomaly_count": snapshot.anomalies.len(),
        "bottleneck_count": snapshot.bottlenecks.len()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port::AnalyticsConfig;

    #[tokio::test]
    async fn test_snapshot_handler() {
        let analytics = Arc::new(RealtimeAnalyticsEngine::new(AnalyticsConfig::default()));

        let response = analytics_snapshot_handler(analytics).await.into_response();

        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn test_health_handler() {
        let analytics = Arc::new(RealtimeAnalyticsEngine::new(AnalyticsConfig::default()));

        let response = analytics_health_handler(analytics).await.into_response();

        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn test_timeseries_handler() {
        let analytics = Arc::new(RealtimeAnalyticsEngine::new(AnalyticsConfig::default()));

        let response = analytics_timeseries_handler(analytics, "wip".to_string(), Some(60))
            .await
            .into_response();

        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn test_snapshot_to_event() {
        use crate::domain::{LittlesLawMetrics, PercentileLatency, WipSnapshot};
        use chrono::Utc;
        use uuid::Uuid;

        let snapshot = AnalyticsSnapshot {
            timestamp: Utc::now(),
            wip_snapshot: WipSnapshot::new(2, 5, vec![Uuid::new_v4()]),
            littles_law: LittlesLawMetrics::calculate(2.0, 10, 20000, 60),
            lead_time_percentiles: PercentileLatency::from_sorted_samples(
                "lead_time".to_string(),
                &[100, 200, 300],
            ),
            cycle_time_percentiles: PercentileLatency::from_sorted_samples(
                "cycle_time".to_string(),
                &[50, 100, 150],
            ),
            queue_time_percentiles: PercentileLatency::from_sorted_samples(
                "queue_time".to_string(),
                &[50, 100, 150],
            ),
            total_arrivals: 10,
            total_completions: 10,
            total_rejections: 0,
            anomalies: vec![],
            bottlenecks: vec![],
        };

        let event = snapshot_to_event(snapshot);

        assert!(event.data.is_some());
        assert_eq!(event.event.as_deref(), Some("analytics"));
    }
}
