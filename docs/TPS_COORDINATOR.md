# TPS Coordinator - Autonomous Agent Coordination with Lean Manufacturing Principles

## Overview

The TPS (Toyota Production System) Coordinator implements lean manufacturing principles for autonomous agent task coordination in the A2A protocol ecosystem. It provides pull-based scheduling, real-time monitoring, and automatic quality control.

## Core Principles

### 1. Kanban Board
- **Concept**: Visual workflow management with WIP (Work-In-Progress) limits
- **Implementation**: Each station has a configurable WIP limit
- **Benefit**: Prevents system overload and identifies bottlenecks

### 2. Pull-Based Scheduling
- **Concept**: Work is pulled when capacity is available, not pushed
- **Implementation**: `pull_task()` method checks WIP limits before returning work
- **Benefit**: Natural load balancing and backpressure handling

### 3. Andon System
- **Concept**: Real-time status visualization (factory floor lights)
- **Implementation**: Three-level status (GREEN/YELLOW/RED) based on utilization
- **Benefit**: Immediate visibility into system health

### 4. Jidoka (Autonomation)
- **Concept**: Automatic stopping when quality issues detected
- **Implementation**: System halts when defect rate exceeds threshold
- **Benefit**: Prevents defect propagation, forces root cause analysis

### 5. Heijunka (Level Loading)
- **Concept**: Smooth production over time to avoid spikes
- **Implementation**: Target throughput per period with acceptance limiting
- **Benefit**: Predictable resource utilization

### 6. Takt Time
- **Concept**: Rhythm of production aligned with customer demand
- **Implementation**: `TaktTime` calculator based on available time and demand
- **Benefit**: Ensures production matches demand pace

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    TPS Coordinator                           │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                 Coordinator State                      │  │
│  │  • Station Queues (pull-based)                        │  │
│  │  • WIP Tracking                                       │  │
│  │  • Timing Metrics                                     │  │
│  │  • Andon Status                                       │  │
│  │  • Jidoka Gate                                        │  │
│  │  • Heijunka Scheduler                                 │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │  Metrics   │  │   Heijunka   │  │  Andon Monitor  │   │
│  │ Collector  │  │  Scheduler   │  │                  │   │
│  └────────────┘  └──────────────┘  └──────────────────┘   │
│      ↓ tokio::spawn background tasks ↓                     │
└─────────────────────────────────────────────────────────────┘
                            ↓
                    AsyncTaskManager
                     (Port Trait)
                            ↓
              ┌─────────────┴─────────────┐
              │                           │
        InMemoryTaskStorage      SQLxTaskStorage
         (Adapter)                 (Adapter)
```

## Configuration

```rust
use a2a_rs::services::coordinator::{CoordinatorConfig, Station};

let config = CoordinatorConfig::builder()
    // Define the kanban pipeline
    .stations(vec![
        Station::new("intake", 10),
        Station::new("processing", 5),
        Station::new("review", 2),
    ])

    // Andon thresholds
    .andon_yellow_threshold(0.7)  // 70% utilization
    .andon_red_threshold(0.9)     // 90% utilization

    // Takt time (seconds per task)
    .takt_time_seconds(60.0)

    // Jidoka settings
    .enable_jidoka(true)
    .jidoka_defect_threshold(0.1)  // 10% defect rate

    // Heijunka (level loading)
    .heijunka_period_seconds(300.0)  // 5 minute periods
    .heijunka_target_throughput(20)  // 20 tasks per period

    // Queue limits
    .max_queue_size(1000)

    // Metrics collection
    .metrics_interval_seconds(30.0)

    .build();
```

## Usage

### Basic Setup

```rust
use a2a_rs::{
    services::coordinator::TpsCoordinator,
    port::AsyncTaskManager,
};

// Create with your task manager implementation
let task_manager = MyTaskManager::new();
let coordinator = TpsCoordinator::new(config, task_manager);

// Start background monitoring
coordinator.start().await?;
```

### Submitting Tasks

```rust
// Submit task to first station with priority
let task = coordinator.submit_task(
    "task-123",
    "context-456",
    10,  // priority (higher = more urgent)
).await?;
```

### Pull-Based Processing

```rust
// Worker pulls task when ready (respects WIP limits)
if let Some(task) = coordinator.pull_task("processing").await? {
    // Process the task
    let result = process_task(&task).await;

    // Complete at station (moves to next station or completes)
    coordinator.complete_task_at_station(
        &task.id,
        "processing",
        result.is_ok(),
    ).await?;
}
```

### Monitoring

```rust
// Check Andon status
let status = coordinator.get_andon_status().await?;
match status {
    AndonStatus::Green => println!("✅ Normal operation"),
    AndonStatus::Yellow => println!("⚠️ Approaching capacity"),
    AndonStatus::Red => println!("🚨 At capacity"),
}

// Check Jidoka gate
let jidoka = coordinator.get_jidoka_gate().await?;
if jidoka.is_halted {
    println!("🛑 System halted: {}", jidoka.halt_reason.unwrap());
}

// Get comprehensive metrics
let metrics = coordinator.get_metrics().await?;
println!("Total processed: {}", metrics.total_processed);
println!("Defect rate: {:.2}%", metrics.defect_rate * 100.0);
println!("Throughput: {:.2}/min", metrics.throughput_per_minute);
```

## Integration with A2A Transport

### HTTP Server Integration

```rust
use a2a_rs::{
    HttpServer,
    adapter::DefaultRequestProcessor,
    services::coordinator::TpsCoordinator,
};

// Create coordinator
let coordinator = TpsCoordinator::new(config, task_manager.clone());
coordinator.start().await?;

// Create HTTP endpoints for monitoring
let app = axum::Router::new()
    .route("/metrics", axum::routing::get({
        let coordinator = coordinator.clone();
        move || async move {
            let metrics = coordinator.get_metrics().await?;
            Ok::<_, A2AError>(axum::Json(metrics))
        }
    }))
    .route("/andon", axum::routing::get({
        let coordinator = coordinator.clone();
        move || async move {
            let status = coordinator.get_andon_status().await?;
            Ok::<_, A2AError>(axum::Json(status))
        }
    }));

// Standard A2A server continues to work
let server = HttpServer::new(processor, agent_info, "127.0.0.1:8080");
server.start().await?;
```

### WebSocket Real-Time Updates

```rust
use tokio::sync::broadcast;

// Create broadcast channel for andon signals
let (tx, _) = broadcast::channel(100);

// Spawn andon monitoring task
let coordinator_clone = coordinator.clone();
let tx_clone = tx.clone();
tokio::spawn(async move {
    let mut last_status = AndonStatus::Green;
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let status = coordinator_clone.get_andon_status().await.unwrap();

        if status != last_status {
            let signal = AndonSignal {
                status,
                reason: "Utilization changed".to_string(),
                station: None,
                timestamp: chrono::Utc::now(),
                metadata: None,
            };
            let _ = tx_clone.send(signal);
            last_status = status;
        }
    }
});

// WebSocket endpoint broadcasts signals
let ws = warp::ws()
    .map(move |ws: warp::ws::Ws| {
        let mut rx = tx.subscribe();
        ws.on_upgrade(move |socket| async move {
            while let Ok(signal) = rx.recv().await {
                let json = serde_json::to_string(&signal).unwrap();
                let _ = socket.send(warp::ws::Message::text(json)).await;
            }
        })
    });
```

## Metrics and Observability

### Key Metrics

| Metric | Description | Interpretation |
|--------|-------------|----------------|
| `total_processed` | Total tasks completed | Cumulative throughput |
| `total_failed` | Total tasks failed | Quality indicator |
| `defect_rate` | failed / processed | Quality percentage |
| `current_wip` | Tasks in progress | Load indicator |
| `throughput_per_minute` | Tasks/minute | Performance indicator |
| `avg_cycle_time_seconds` | End-to-end time | Efficiency indicator |
| `andon_incidents` | Yellow/Red events | Capacity indicator |
| `jidoka_halts` | Automatic stops | Quality control events |

### Station-Specific Metrics

```rust
for (name, station_metrics) in &metrics.station_metrics {
    println!("{} station:", name);
    println!("  WIP: {}/{}",
        station_metrics.current_wip,
        station_metrics.wip_limit);
    println!("  Utilization: {:.0}%",
        station_metrics.utilization * 100.0);
    println!("  Avg processing: {:.2}s",
        station_metrics.avg_processing_time_seconds);
}
```

### Tracing Integration

Enable comprehensive telemetry with the `tracing` feature:

```rust
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

tracing_subscriber::registry()
    .with(fmt::layer())
    .with(EnvFilter::from_default_env())
    .init();
```

All coordinator operations emit structured logs with:
- Task IDs and context IDs
- Station names
- Queue sizes
- Timing information
- Status changes

## Production Deployment

### Capacity Planning

1. **Determine Takt Time**:
   ```
   Takt Time = Available Time / Customer Demand
   Example: 3600s / 60 tasks = 60s per task
   ```

2. **Set WIP Limits**:
   ```
   WIP Limit = Takt Time / Avg Processing Time
   Example: 60s / 10s = 6 tasks per station
   ```

3. **Configure Heijunka**:
   ```
   Period = Takt Time × Target Throughput
   Example: 60s × 10 tasks = 600s = 10 minutes
   ```

### High Availability

```rust
// Graceful shutdown
tokio::select! {
    _ = tokio::signal::ctrl_c() => {
        coordinator.stop().await?;
        // Wait for in-flight tasks
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
```

### Scaling

The coordinator is designed for single-node coordination. For multi-node:

1. **Horizontal**: Run multiple coordinators with different task manager backends
2. **Vertical**: Increase WIP limits and adjust thresholds
3. **Hybrid**: Partition by context_id across coordinators

## Best Practices

### 1. Station Design
- Keep stations focused (single responsibility)
- Balance WIP limits (identify bottlenecks)
- Monitor utilization (aim for 70-80%)

### 2. Priority Management
- Use priorities sparingly (too many = none)
- Reserve high priority for urgent work
- Default priority = 5

### 3. Jidoka Configuration
- Set defect threshold based on acceptable quality
- Investigate every halt (root cause analysis)
- Resume only after fix implemented

### 4. Metrics Review
- Review metrics regularly (daily/weekly)
- Track trends over time
- Use for capacity planning

### 5. Andon Response
- **Green**: Normal operation, optimize
- **Yellow**: Monitor closely, prepare to scale
- **Red**: Immediate action, investigate bottleneck

## Troubleshooting

### High WIP
- **Symptom**: Current WIP consistently high
- **Causes**: Slow processing, high input rate
- **Solutions**: Increase station capacity, add workers

### Frequent Jidoka Halts
- **Symptom**: System halts repeatedly
- **Causes**: Systemic quality issues
- **Solutions**: Fix root cause, adjust threshold

### Low Throughput
- **Symptom**: throughput_per_minute below target
- **Causes**: Bottleneck station, inefficient processing
- **Solutions**: Balance WIP limits, optimize hot paths

### Andon Always Red
- **Symptom**: Constant red status
- **Causes**: WIP limits too low, demand exceeds capacity
- **Solutions**: Increase capacity, adjust thresholds

## Example: Production Setup

```rust
use a2a_rs::services::coordinator::{CoordinatorConfig, Station, TpsCoordinator};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Production configuration
    let config = CoordinatorConfig::builder()
        .stations(vec![
            Station::new("intake", 20),
            Station::new("enrichment", 10),
            Station::new("processing", 15),
            Station::new("validation", 8),
            Station::new("delivery", 5),
        ])
        .andon_yellow_threshold(0.75)
        .andon_red_threshold(0.90)
        .takt_time_seconds(30.0)
        .enable_jidoka(true)
        .jidoka_defect_threshold(0.05)  // 5% defect rate
        .heijunka_period_seconds(600.0)  // 10 minutes
        .heijunka_target_throughput(50)
        .max_queue_size(5000)
        .metrics_interval_seconds(60.0)  // 1 minute
        .build();

    // Initialize
    let task_manager = create_production_task_manager().await?;
    let coordinator = TpsCoordinator::new(config, task_manager);
    coordinator.start().await?;

    // Spawn monitoring
    spawn_metrics_exporter(&coordinator);
    spawn_alert_handler(&coordinator);

    // Run until shutdown
    tokio::signal::ctrl_c().await?;
    coordinator.stop().await?;

    Ok(())
}
```

## References

- [Toyota Production System](https://en.wikipedia.org/wiki/Toyota_Production_System)
- [Kanban](https://en.wikipedia.org/wiki/Kanban)
- [Andon](https://en.wikipedia.org/wiki/Andon_(manufacturing))
- [Jidoka](https://en.wikipedia.org/wiki/Autonomation)
- [Heijunka](https://en.wikipedia.org/wiki/Heijunka)
- [A2A Protocol Specification](../spec/)

## License

Same as a2a-rs project (MIT or Apache-2.0).
