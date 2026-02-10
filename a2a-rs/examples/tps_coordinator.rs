//! Example demonstrating the TPS (Toyota Production System) Coordinator
//!
//! This example shows how to use the autonomous agent coordinator with
//! Kanban, pull-based scheduling, Andon system, Jidoka, and Heijunka.
//!
//! Run with:
//! ```bash
//! cargo run -p a2a-rs --example tps_coordinator --features server
//! ```

use std::time::Duration;

use a2a_rs::{
    Task, TaskState,
    domain::A2AError,
    port::AsyncTaskManager,
    services::coordinator::{CoordinatorConfig, Station, TpsCoordinator},
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Simple in-memory task manager for demonstration
#[derive(Clone)]
struct SimpleTaskManager {
    tasks: Arc<Mutex<HashMap<String, Task>>>,
}

impl SimpleTaskManager {
    fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl AsyncTaskManager for SimpleTaskManager {
    async fn create_task<'a>(
        &self,
        task_id: &'a str,
        context_id: &'a str,
    ) -> Result<Task, A2AError> {
        let task = Task::new(task_id.to_string(), context_id.to_string());
        self.tasks
            .lock()
            .unwrap()
            .insert(task_id.to_string(), task.clone());
        Ok(task)
    }

    async fn get_task<'a>(
        &self,
        task_id: &'a str,
        _history_length: Option<u32>,
    ) -> Result<Task, A2AError> {
        self.tasks
            .lock()
            .unwrap()
            .get(task_id)
            .cloned()
            .ok_or_else(|| A2AError::TaskNotFound(task_id.to_string()))
    }

    async fn update_task_status<'a>(
        &self,
        task_id: &'a str,
        state: TaskState,
        message: Option<a2a_rs::Message>,
    ) -> Result<Task, A2AError> {
        let mut tasks = self.tasks.lock().unwrap();
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| A2AError::TaskNotFound(task_id.to_string()))?;

        task.update_status(state, message);
        Ok(task.clone())
    }

    async fn cancel_task<'a>(&self, task_id: &'a str) -> Result<Task, A2AError> {
        self.update_task_status(task_id, TaskState::Canceled, None)
            .await
    }

    async fn task_exists<'a>(&self, task_id: &'a str) -> Result<bool, A2AError> {
        Ok(self.tasks.lock().unwrap().contains_key(task_id))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for observability
    #[cfg(feature = "tracing")]
    {
        use tracing_subscriber::{EnvFilter, fmt, prelude::*};
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(
                EnvFilter::from_default_env()
                    .add_directive("tps_coordinator=info".parse()?)
                    .add_directive("a2a_rs=info".parse()?),
            )
            .init();
    }

    println!("🏭 TPS Coordinator Example");
    println!("═══════════════════════════════════════════════════");
    println!();

    // Configure the coordinator with TPS principles
    let config = CoordinatorConfig::builder()
        .stations(vec![
            Station::new("intake", 10),    // High capacity intake
            Station::new("processing", 5), // Medium capacity processing
            Station::new("validation", 3), // Low capacity validation
            Station::new("completion", 2), // Final station
        ])
        .andon_yellow_threshold(0.7) // Yellow at 70% utilization
        .andon_red_threshold(0.9) // Red at 90% utilization
        .takt_time_seconds(30.0) // 30 second takt time
        .enable_jidoka(true) // Enable automatic stopping
        .jidoka_defect_threshold(0.15) // Halt at 15% defect rate
        .heijunka_period_seconds(300.0) // 5 minute level loading period
        .heijunka_target_throughput(20) // 20 tasks per period
        .max_queue_size(100) // Maximum queue capacity
        .metrics_interval_seconds(10.0) // Collect metrics every 10s
        .build();

    println!("📋 Configuration:");
    println!("   Stations: {}", config.stations.len());
    println!(
        "   Andon thresholds: YELLOW={:.0}%, RED={:.0}%",
        config.andon_yellow_threshold * 100.0,
        config.andon_red_threshold * 100.0
    );
    println!("   Takt time: {:.0}s", config.takt_time_seconds);
    println!(
        "   Heijunka: {} tasks per {:.0}s period",
        config.heijunka_target_throughput, config.heijunka_period_seconds
    );
    println!();

    // Create task manager and coordinator
    let task_manager = SimpleTaskManager::new();
    let coordinator = TpsCoordinator::new(config, task_manager);

    // Start the coordinator
    println!("🚀 Starting coordinator...");
    coordinator.start().await?;
    println!("✓ Coordinator started");
    println!();

    // Simulate task submission
    println!("📥 Submitting tasks to coordinator...");
    let mut task_ids = Vec::new();

    for i in 1..=10 {
        let task_id = format!("task-{:03}", i);
        let context_id = format!("context-{}", (i - 1) / 3 + 1); // Group into contexts
        let priority = if i <= 3 { 10 } else { 5 }; // First 3 have high priority

        match coordinator
            .submit_task(&task_id, &context_id, priority)
            .await
        {
            Ok(_task) => {
                println!(
                    "   ✓ Submitted {}: context={}, priority={}",
                    task_id, context_id, priority
                );
                task_ids.push(task_id);
            }
            Err(e) => {
                println!("   ✗ Failed to submit {}: {}", task_id, e);
            }
        }
    }
    println!();

    // Simulate pull-based processing from first station
    println!("🔄 Pulling tasks from 'intake' station...");
    tokio::time::sleep(Duration::from_secs(1)).await;

    for _ in 0..5 {
        match coordinator.pull_task("intake").await? {
            Some(task) => {
                println!("   ← Pulled task: {}", task.id);

                // Simulate processing
                tokio::time::sleep(Duration::from_millis(100)).await;

                // Complete at station (move to next)
                coordinator
                    .complete_task_at_station(&task.id, "intake", true)
                    .await?;
                println!("   → Task {} moved to next station", task.id);
            }
            None => {
                println!("   ○ No tasks available at intake");
                break;
            }
        }
    }
    println!();

    // Check Andon status
    println!("🚦 Andon System Status:");
    let andon_status = coordinator.get_andon_status().await?;
    let status_emoji = match andon_status {
        a2a_rs::services::coordinator::AndonStatus::Green => "🟢",
        a2a_rs::services::coordinator::AndonStatus::Yellow => "🟡",
        a2a_rs::services::coordinator::AndonStatus::Red => "🔴",
    };
    println!("   Status: {} {:?}", status_emoji, andon_status);
    println!();

    // Check Jidoka gate
    println!("🛑 Jidoka Gate Status:");
    let jidoka = coordinator.get_jidoka_gate().await?;
    if jidoka.is_halted {
        println!(
            "   ⚠️  HALTED: {}",
            jidoka.halt_reason.unwrap_or_else(|| "unknown".to_string())
        );
        println!("   Blocked tasks: {}", jidoka.blocked_count);
    } else {
        println!("   ✓ Running normally");
    }
    println!();

    // Display metrics
    println!("📊 Coordinator Metrics:");
    let metrics = coordinator.get_metrics().await?;
    println!("   Total processed: {}", metrics.total_processed);
    println!("   Total failed: {}", metrics.total_failed);
    println!("   Total canceled: {}", metrics.total_canceled);
    println!("   Current WIP: {}", metrics.current_wip);
    println!("   Defect rate: {:.2}%", metrics.defect_rate * 100.0);
    println!(
        "   Throughput: {:.2} tasks/min",
        metrics.throughput_per_minute
    );
    println!("   Andon incidents: {}", metrics.andon_incidents);
    println!("   Jidoka halts: {}", metrics.jidoka_halts);
    println!();

    println!("📈 Station Metrics:");
    for (station_name, station_metrics) in &metrics.station_metrics {
        println!(
            "   {} ({}/{} = {:.0}% utilization):",
            station_name,
            station_metrics.current_wip,
            station_metrics.wip_limit,
            station_metrics.utilization * 100.0
        );
        println!("      Tasks processed: {}", station_metrics.tasks_processed);
        println!(
            "      Avg processing time: {:.2}s",
            station_metrics.avg_processing_time_seconds
        );
        println!("      Blocked: {}", station_metrics.tasks_blocked);
    }
    println!();

    // Demonstrate Jidoka halt
    println!("⚠️  Testing manual Jidoka halt...");
    coordinator
        .trigger_jidoka_halt("Quality issue detected".to_string())
        .await?;
    println!("   ✓ Jidoka triggered");

    // Try to submit task (should fail)
    match coordinator
        .submit_task("task-blocked", "context-test", 5)
        .await
    {
        Ok(_) => println!("   ✗ Unexpected: task accepted during halt"),
        Err(e) => println!("   ✓ Expected: task rejected - {}", e),
    }
    println!();

    // Resume from halt
    println!("▶️  Resuming from Jidoka halt...");
    coordinator.resume_from_jidoka().await?;
    println!("   ✓ System resumed");
    println!();

    // Stop the coordinator
    println!("🛑 Stopping coordinator...");
    coordinator.stop().await?;
    println!("✓ Coordinator stopped");
    println!();

    println!("═══════════════════════════════════════════════════");
    println!("✅ Example completed successfully!");
    println!();
    println!("Key TPS Principles Demonstrated:");
    println!("  • Kanban: WIP limits enforced per station");
    println!("  • Pull: Tasks pulled when capacity available");
    println!("  • Andon: Real-time status monitoring");
    println!("  • Jidoka: Automatic halt on quality issues");
    println!("  • Heijunka: Level loading prevents overload");
    println!("  • Metrics: Continuous improvement tracking");

    Ok(())
}
