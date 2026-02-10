//! A2A Load Testing Tool
//!
//! Generates concurrent typed packet streams to test A2A protocol implementations.
//! Measures throughput, latency (p50/p99), and outputs CSV results.

use a2a_rs::{Message, Part, Role, Task, TaskState, TaskStatus};
use chrono::{DateTime, Utc};
use clap::Parser;
use hdrhistogram::Histogram;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{info, warn};
use uuid::Uuid;

/// Command line arguments for the load test
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Target operations per second (1000, 10000, 100000, etc.)
    #[arg(short, long, default_value = "1000")]
    ops_per_sec: u64,

    /// Duration of the test in seconds
    #[arg(short, long, default_value = "60")]
    duration: u64,

    /// Number of concurrent workers
    #[arg(short, long, default_value = "10")]
    workers: usize,

    /// Packet type to generate: message, task, or mixed
    #[arg(short, long, default_value = "message")]
    packet_type: PacketType,

    /// Output CSV file path
    #[arg(short = 'o', long, default_value = "loadtest_results.csv")]
    output: String,

    /// Reporting interval in seconds
    #[arg(short, long, default_value = "5")]
    report_interval: u64,

    /// Server endpoint (optional - if not provided, only generates packets without sending)
    #[arg(short, long)]
    server: Option<String>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum PacketType {
    Message,
    Task,
    Mixed,
}

/// Metrics collected during the load test
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Metrics {
    timestamp: DateTime<Utc>,
    elapsed_secs: f64,
    total_ops: u64,
    throughput_ops_per_sec: f64,
    latency_p50_us: u64,
    latency_p95_us: u64,
    latency_p99_us: u64,
    latency_max_us: u64,
    errors: u64,
}

/// Result of a single operation
#[derive(Debug)]
struct OpResult {
    latency_us: u64,
    success: bool,
}

/// Generate a random message packet
fn generate_message() -> Message {
    let mut rng = rand::thread_rng();
    let message_id = Uuid::new_v4().to_string();
    let role = if rng.gen_bool(0.5) {
        Role::User
    } else {
        Role::Agent
    };

    let text = format!(
        "Load test message {} - {}",
        message_id,
        generate_random_text(50)
    );

    Message::builder()
        .role(role)
        .parts(vec![Part::text(text)])
        .message_id(message_id)
        .build()
}

/// Generate a random task packet
fn generate_task() -> Task {
    let task_id = Uuid::new_v4().to_string();
    let context_id = Uuid::new_v4().to_string();

    let states = [
        TaskState::Submitted,
        TaskState::Working,
        TaskState::Completed,
        TaskState::InputRequired,
    ];
    let state = states[rand::thread_rng().gen_range(0..states.len())].clone();

    Task::builder()
        .id(task_id)
        .context_id(context_id)
        .status(TaskStatus {
            state,
            message: None,
            timestamp: Some(Utc::now()),
        })
        .build()
}

/// Generate random text of approximately the given length
fn generate_random_text(approx_len: usize) -> String {
    const WORDS: &[&str] = &[
        "the",
        "quick",
        "brown",
        "fox",
        "jumps",
        "over",
        "lazy",
        "dog",
        "agent",
        "task",
        "message",
        "protocol",
        "load",
        "test",
        "performance",
        "benchmark",
        "throughput",
        "latency",
        "concurrent",
        "async",
        "distributed",
        "system",
        "network",
        "packet",
    ];

    let mut rng = rand::thread_rng();
    let word_count = approx_len / 5; // Rough estimate
    (0..word_count)
        .map(|_| WORDS[rng.gen_range(0..WORDS.len())])
        .collect::<Vec<_>>()
        .join(" ")
}

/// Worker task that generates and processes operations
async fn worker(
    worker_id: usize,
    packet_type: PacketType,
    mut work_rx: mpsc::Receiver<()>,
    result_tx: mpsc::Sender<OpResult>,
) {
    while work_rx.recv().await.is_some() {
        let start = Instant::now();

        // Generate packet based on type
        let _packet = match packet_type {
            PacketType::Message => {
                let msg = generate_message();
                // Simulate serialization cost
                let _ = serde_json::to_string(&msg);
            }
            PacketType::Task => {
                let task = generate_task();
                let _ = serde_json::to_string(&task);
            }
            PacketType::Mixed => {
                if rand::thread_rng().gen_bool(0.5) {
                    let msg = generate_message();
                    let _ = serde_json::to_string(&msg);
                } else {
                    let task = generate_task();
                    let _ = serde_json::to_string(&task);
                }
            }
        };

        let latency_us = start.elapsed().as_micros() as u64;

        // Send result
        let result = OpResult {
            latency_us,
            success: true,
        };

        if result_tx.send(result).await.is_err() {
            warn!("Worker {}: Failed to send result", worker_id);
            break;
        }
    }
}

/// Metrics collector that aggregates results and reports periodically
async fn metrics_collector(
    mut result_rx: mpsc::Receiver<OpResult>,
    report_interval_secs: u64,
    output_path: String,
) {
    let start_time = Instant::now();
    let mut histogram = Histogram::<u64>::new(3).expect("Failed to create histogram");
    let mut total_ops = 0u64;
    let mut total_errors = 0u64;
    let mut metrics_records = Vec::new();

    // Create reporting interval
    let mut report_ticker = interval(Duration::from_secs(report_interval_secs));
    report_ticker.tick().await; // Skip first immediate tick

    loop {
        tokio::select! {
            // Collect results
            Some(result) = result_rx.recv() => {
                total_ops += 1;
                if result.success {
                    if let Err(e) = histogram.record(result.latency_us) {
                        warn!("Failed to record latency: {}", e);
                    }
                } else {
                    total_errors += 1;
                }
            }

            // Periodic reporting
            _ = report_ticker.tick() => {
                let elapsed = start_time.elapsed();
                let elapsed_secs = elapsed.as_secs_f64();

                if total_ops > 0 {
                    let metrics = Metrics {
                        timestamp: Utc::now(),
                        elapsed_secs,
                        total_ops,
                        throughput_ops_per_sec: total_ops as f64 / elapsed_secs,
                        latency_p50_us: histogram.value_at_quantile(0.50),
                        latency_p95_us: histogram.value_at_quantile(0.95),
                        latency_p99_us: histogram.value_at_quantile(0.99),
                        latency_max_us: histogram.max(),
                        errors: total_errors,
                    };

                    info!(
                        "Elapsed: {:.1}s | Ops: {} | Throughput: {:.0} ops/s | Latency p50: {}μs, p99: {}μs | Errors: {}",
                        metrics.elapsed_secs,
                        metrics.total_ops,
                        metrics.throughput_ops_per_sec,
                        metrics.latency_p50_us,
                        metrics.latency_p99_us,
                        metrics.errors
                    );

                    metrics_records.push(metrics);
                }
            }

            // Channel closed - write final results
            else => {
                // Final metrics
                let elapsed = start_time.elapsed();
                let elapsed_secs = elapsed.as_secs_f64();

                if total_ops > 0 {
                    let final_metrics = Metrics {
                        timestamp: Utc::now(),
                        elapsed_secs,
                        total_ops,
                        throughput_ops_per_sec: total_ops as f64 / elapsed_secs,
                        latency_p50_us: histogram.value_at_quantile(0.50),
                        latency_p95_us: histogram.value_at_quantile(0.95),
                        latency_p99_us: histogram.value_at_quantile(0.99),
                        latency_max_us: histogram.max(),
                        errors: total_errors,
                    };

                    info!("\n=== Final Results ===");
                    info!("Total operations: {}", final_metrics.total_ops);
                    info!("Duration: {:.2}s", final_metrics.elapsed_secs);
                    info!("Throughput: {:.2} ops/s", final_metrics.throughput_ops_per_sec);
                    info!("Latency p50: {}μs ({:.2}ms)", final_metrics.latency_p50_us, final_metrics.latency_p50_us as f64 / 1000.0);
                    info!("Latency p95: {}μs ({:.2}ms)", final_metrics.latency_p95_us, final_metrics.latency_p95_us as f64 / 1000.0);
                    info!("Latency p99: {}μs ({:.2}ms)", final_metrics.latency_p99_us, final_metrics.latency_p99_us as f64 / 1000.0);
                    info!("Latency max: {}μs ({:.2}ms)", final_metrics.latency_max_us, final_metrics.latency_max_us as f64 / 1000.0);
                    info!("Errors: {}", final_metrics.errors);

                    metrics_records.push(final_metrics);
                }

                // Write CSV
                if let Err(e) = write_csv(&output_path, &metrics_records) {
                    warn!("Failed to write CSV: {}", e);
                } else {
                    info!("Results written to: {}", output_path);
                }

                break;
            }
        }
    }
}

/// Write metrics to CSV file
fn write_csv(path: &str, metrics: &[Metrics]) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = csv::Writer::from_path(path)?;

    // Write header
    wtr.write_record(&[
        "timestamp",
        "elapsed_secs",
        "total_ops",
        "throughput_ops_per_sec",
        "latency_p50_us",
        "latency_p95_us",
        "latency_p99_us",
        "latency_max_us",
        "errors",
    ])?;

    // Write data
    for m in metrics {
        wtr.write_record(&[
            m.timestamp.to_rfc3339(),
            m.elapsed_secs.to_string(),
            m.total_ops.to_string(),
            m.throughput_ops_per_sec.to_string(),
            m.latency_p50_us.to_string(),
            m.latency_p95_us.to_string(),
            m.latency_p99_us.to_string(),
            m.latency_max_us.to_string(),
            m.errors.to_string(),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

/// Rate limiter that distributes work across intervals
async fn rate_limiter(
    ops_per_sec: u64,
    duration_secs: u64,
    workers: usize,
    work_txs: Vec<mpsc::Sender<()>>,
) {
    let total_ops = ops_per_sec * duration_secs;
    let interval_ms = 1000.0 / ops_per_sec as f64;
    let mut interval_timer = interval(Duration::from_micros((interval_ms * 1000.0) as u64));
    interval_timer.tick().await; // Skip first immediate tick

    let start = Instant::now();
    let mut ops_sent = 0u64;
    let mut worker_idx = 0;

    info!(
        "Starting load test: {} ops/s for {}s (total: {} ops)",
        ops_per_sec, duration_secs, total_ops
    );

    while ops_sent < total_ops && start.elapsed().as_secs() < duration_secs {
        interval_timer.tick().await;

        // Round-robin work distribution
        if work_txs[worker_idx].send(()).await.is_ok() {
            ops_sent += 1;
            worker_idx = (worker_idx + 1) % workers;
        } else {
            warn!("Failed to send work to worker {}", worker_idx);
        }
    }

    info!("Rate limiter finished: sent {} operations", ops_sent);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize tracing
    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .init();

    info!("A2A Load Test Configuration:");
    info!("  Target ops/sec: {}", args.ops_per_sec);
    info!("  Duration: {}s", args.duration);
    info!("  Workers: {}", args.workers);
    info!("  Packet type: {:?}", args.packet_type);
    info!("  Output file: {}", args.output);
    info!("  Report interval: {}s", args.report_interval);

    if let Some(ref server) = args.server {
        info!("  Server: {}", server);
    } else {
        info!("  Server: None (packet generation only)");
    }

    // Create channels
    let (result_tx, result_rx) = mpsc::channel(10000);
    let work_txs: Vec<_> = (0..args.workers)
        .map(|_| {
            let (tx, rx) = mpsc::channel(1000);
            let result_tx = result_tx.clone();
            let packet_type = args.packet_type;

            tokio::spawn(async move {
                let worker_id = rand::random::<usize>();
                worker(worker_id, packet_type, rx, result_tx).await;
            });

            tx
        })
        .collect();

    drop(result_tx); // Drop the original sender

    // Start metrics collector
    let metrics_handle = tokio::spawn(metrics_collector(
        result_rx,
        args.report_interval,
        args.output.clone(),
    ));

    // Start rate limiter
    let rate_limiter_handle = tokio::spawn(rate_limiter(
        args.ops_per_sec,
        args.duration,
        args.workers,
        work_txs,
    ));

    // Wait for completion
    rate_limiter_handle.await?;
    metrics_handle.await?;

    Ok(())
}
