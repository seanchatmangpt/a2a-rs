//! WIP Analytics Demo
//!
//! Demonstrates real-time WIP analytics with:
//! - Instrumented WIP gate tracking work lifecycle
//! - Live metrics streaming via SSE
//! - Little's Law calculations
//! - Percentile latencies
//! - Anomaly detection
//! - Bottleneck identification
//!
//! Run with: cargo run --example wip_analytics_demo
//!
//! Then open http://localhost:3000/analytics/stream in a browser or:
//! curl -N http://localhost:3000/analytics/stream

use axum::{Router, extract::State, routing::get};
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use uuid::Uuid;

use osiris_edge::{
    AnalyticsConfig, AnalyticsEngine, AsyncWipGate, InstrumentedWipGate, KanbanWipGate,
    RealtimeAnalyticsEngine,
};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info,osiris_edge=debug")
        .init();

    tracing::info!("Starting WIP Analytics Demo");

    // Create WIP gate with limit of 5
    let gate = KanbanWipGate::new(5);

    // Create analytics engine with custom config
    let analytics_config = AnalyticsConfig {
        window_size_sec: 60,      // 1 minute window
        snapshot_interval_sec: 2, // Update every 2 seconds
        high_utilization_threshold: 80.0,
        lead_time_spike_threshold: 3.0,
        cycle_time_spike_threshold: 3.0,
        throughput_drop_threshold: 50.0,
        littles_law_violation_threshold: 25.0,
        max_tracked_items: 1000,
    };

    let analytics = RealtimeAnalyticsEngine::new(analytics_config);

    // Create instrumented gate (combines WIP limiting + analytics)
    let instrumented_gate = Arc::new(InstrumentedWipGate::new(gate, analytics.clone()));
    let analytics_arc = Arc::new(analytics);

    // Start simulated workload
    let gate_clone = Arc::clone(&instrumented_gate);
    tokio::spawn(async move {
        simulate_workload(gate_clone).await;
    });

    // Print metrics to console
    let analytics_clone = Arc::clone(&analytics_arc);
    tokio::spawn(async move {
        print_metrics(analytics_clone).await;
    });

    // Build web server with analytics endpoints
    let app = Router::new()
        .route("/", get(index_handler))
        .route(
            "/analytics/stream",
            get(
                |State(analytics): State<Arc<RealtimeAnalyticsEngine>>| async move {
                    osiris_edge::analytics_sse_handler(analytics).await
                },
            ),
        )
        .route(
            "/analytics/snapshot",
            get(
                |State(analytics): State<Arc<RealtimeAnalyticsEngine>>| async move {
                    osiris_edge::analytics_snapshot_handler(analytics).await
                },
            ),
        )
        .route(
            "/analytics/health",
            get(
                |State(analytics): State<Arc<RealtimeAnalyticsEngine>>| async move {
                    osiris_edge::analytics_health_handler(analytics).await
                },
            ),
        )
        .with_state(analytics_arc);

    tracing::info!("Analytics dashboard available at http://localhost:3000");
    tracing::info!("  - SSE stream: http://localhost:3000/analytics/stream");
    tracing::info!("  - Snapshot: http://localhost:3000/analytics/snapshot");
    tracing::info!("  - Health: http://localhost:3000/analytics/health");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

/// Simulate realistic workload with varying arrival rates and processing times
async fn simulate_workload(gate: Arc<InstrumentedWipGate>) {
    tracing::info!("Starting workload simulation");

    let mut iteration = 0u64;

    loop {
        iteration += 1;

        // Vary arrival rate: burst every 30 seconds
        let arrival_delay = if iteration % 15 == 0 {
            Duration::from_millis(10) // Burst: 100 req/sec
        } else {
            Duration::from_millis(100) // Normal: 10 req/sec
        };

        sleep(arrival_delay).await;

        let work_id = Uuid::new_v4();
        let work_type = match iteration % 4 {
            0 => "email",
            1 => "calendar",
            2 => "drive_file",
            _ => "drive_folder",
        };

        let gate_clone = Arc::clone(&gate);

        // Spawn work execution
        tokio::spawn(async move {
            match gate_clone.try_acquire_with_id(work_id, work_type).await {
                Ok(_permit) => {
                    // Simulate work processing
                    let process_time = Duration::from_millis(50 + (rand::random::<u64>() % 200));
                    sleep(process_time).await;
                    // Permit auto-released on drop
                }
                Err(e) => {
                    tracing::debug!("Work rejected: {}", e);
                }
            }
        });
    }
}

/// Print metrics to console periodically
async fn print_metrics(analytics: Arc<RealtimeAnalyticsEngine>) {
    loop {
        sleep(Duration::from_secs(5)).await;

        let snapshot = analytics.get_snapshot().await;

        tracing::info!("=== WIP Analytics ===");
        tracing::info!(
            "WIP: {}/{} ({:.1}% utilization)",
            snapshot.wip_snapshot.current_wip,
            snapshot.wip_snapshot.wip_limit,
            snapshot.wip_snapshot.utilization_pct
        );

        tracing::info!(
            "Throughput: {:.3} items/sec",
            snapshot.littles_law.throughput
        );

        tracing::info!(
            "Lead time: p50={} ms, p95={} ms, p99={} ms",
            snapshot.lead_time_percentiles.p50_ms,
            snapshot.lead_time_percentiles.p95_ms,
            snapshot.lead_time_percentiles.p99_ms
        );

        tracing::info!(
            "Cycle time: p50={} ms, p95={} ms, p99={} ms",
            snapshot.cycle_time_percentiles.p50_ms,
            snapshot.cycle_time_percentiles.p95_ms,
            snapshot.cycle_time_percentiles.p99_ms
        );

        tracing::info!(
            "Queue time: p50={} ms, p95={} ms, p99={} ms",
            snapshot.queue_time_percentiles.p50_ms,
            snapshot.queue_time_percentiles.p95_ms,
            snapshot.queue_time_percentiles.p99_ms
        );

        tracing::info!(
            "Little's Law: WIP={:.2} ≈ λ={:.3} × L={:.2}s (calculated={:.2})",
            snapshot.littles_law.avg_wip,
            snapshot.littles_law.throughput,
            snapshot.littles_law.avg_lead_time_sec,
            snapshot.littles_law.calculated_wip
        );

        tracing::info!(
            "Totals: arrivals={}, completions={}, rejections={}",
            snapshot.total_arrivals,
            snapshot.total_completions,
            snapshot.total_rejections
        );

        if !snapshot.anomalies.is_empty() {
            tracing::warn!("Anomalies detected: {}", snapshot.anomalies.len());
            for anomaly in &snapshot.anomalies {
                tracing::warn!("  - {:?}: {}", anomaly.anomaly_type, anomaly.description);
            }
        }

        if !snapshot.bottlenecks.is_empty() {
            tracing::warn!("Bottlenecks detected: {}", snapshot.bottlenecks.len());
            for bottleneck in &snapshot.bottlenecks {
                tracing::warn!(
                    "  - {:?} ({}% confidence): {}",
                    bottleneck.bottleneck_type,
                    bottleneck.confidence,
                    bottleneck.recommendation
                );
            }
        }

        tracing::info!("=====================\n");
    }
}

/// Index page with links to analytics endpoints
async fn index_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>WIP Analytics Demo</title>
    <style>
        body { font-family: monospace; padding: 20px; background: #1a1a1a; color: #00ff00; }
        h1 { color: #00ff00; }
        a { color: #00aaff; }
        .metrics { margin: 20px 0; padding: 10px; background: #2a2a2a; border: 1px solid #00ff00; }
        pre { background: #0a0a0a; padding: 10px; overflow-x: auto; }
    </style>
</head>
<body>
    <h1>WIP Analytics Demo</h1>

    <div class="metrics">
        <h2>Endpoints:</h2>
        <ul>
            <li><a href="/analytics/stream">SSE Stream</a> - Live metrics updates</li>
            <li><a href="/analytics/snapshot">Current Snapshot</a> - JSON snapshot</li>
            <li><a href="/analytics/health">Health Check</a> - System health</li>
        </ul>
    </div>

    <div class="metrics">
        <h2>Try it:</h2>
        <pre>curl -N http://localhost:3000/analytics/stream</pre>
        <pre>curl http://localhost:3000/analytics/snapshot | jq</pre>
        <pre>curl http://localhost:3000/analytics/health | jq</pre>
    </div>

    <div class="metrics">
        <h2>Metrics:</h2>
        <ul>
            <li><strong>WIP</strong>: Current work-in-progress vs limit</li>
            <li><strong>Throughput</strong>: Completed items per second</li>
            <li><strong>Lead Time</strong>: Arrival → Completion (queue + processing)</li>
            <li><strong>Cycle Time</strong>: Start → Completion (processing only)</li>
            <li><strong>Queue Time</strong>: Arrival → Start (waiting time)</li>
            <li><strong>Little's Law</strong>: WIP = Throughput × Lead Time</li>
        </ul>
    </div>

    <div class="metrics">
        <h2>Anomaly Detection:</h2>
        <ul>
            <li>High utilization (>85%)</li>
            <li>Lead time spikes (>3x median)</li>
            <li>Cycle time spikes (>3x median)</li>
            <li>Throughput drops (>50%)</li>
            <li>Little's Law violations (>25% deviation)</li>
        </ul>
    </div>

    <div class="metrics">
        <h2>Bottleneck Detection:</h2>
        <ul>
            <li>WIP limit too low (high rejection rate)</li>
            <li>Slow processing (high cycle time)</li>
            <li>Queue buildup (queue time >> cycle time)</li>
            <li>Burst traffic (high throughput variance)</li>
        </ul>
    </div>

    <p>Check console output for live metrics display.</p>
</body>
</html>
"#,
    )
}
