//! Metrics integration demo - Prometheus metrics collection and /metrics endpoint
//!
//! Demonstrates:
//! - Creating a PrometheusCollector
//! - Setting up /metrics endpoint
//! - Tracking request duration and error rates
//! - Custom business metrics
//! - Real-time metrics exposition

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use osiris_edge::{
    adapter::PrometheusCollector, application::metrics_handler, port::MetricsCollector,
};
use std::sync::Arc;
use tokio::time::Duration;
use tracing_subscriber;

/// Example custom request handler that tracks metrics
async fn api_endpoint(State(metrics): State<Arc<PrometheusCollector>>) -> impl IntoResponse {
    // Simulate some work
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Track a custom metric
    metrics
        .increment_counter("api_endpoint_calls", vec![("endpoint", "demo")])
        .await;

    StatusCode::OK
}

/// Webhook handler with error tracking
async fn webhook_endpoint(State(metrics): State<Arc<PrometheusCollector>>) -> impl IntoResponse {
    // Simulate random success/failure
    let should_fail = rand::random::<bool>();

    if should_fail {
        metrics
            .record_error("webhook_validation_failed", "/webhook")
            .await;
        (StatusCode::BAD_REQUEST, "Invalid webhook")
    } else {
        (StatusCode::ACCEPTED, "Webhook received")
    }
}

/// Health check endpoint
async fn health_endpoint() -> impl IntoResponse {
    StatusCode::OK
}

/// Create a router with metrics integration
fn create_metrics_router(metrics: Arc<PrometheusCollector>) -> Router {
    Router::new()
        // Health check (no metrics)
        .route("/health", get(health_endpoint))
        // API endpoints
        .route("/api/demo", get(api_endpoint))
        .route("/webhook", post(webhook_endpoint))
        // Metrics endpoint
        .route("/metrics", get(metrics_handler::<PrometheusCollector>))
        // Router state
        .with_state(metrics)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();

    // Create Prometheus collector
    let metrics = Arc::new(PrometheusCollector::new()?);

    println!("Creating metrics-enabled server...");

    // Create router with metrics
    let app = create_metrics_router(metrics.clone());

    // Bind to port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001").await?;
    let local_addr = listener.local_addr()?;

    println!("\nMetrics server listening on http://{}", local_addr);
    println!("\nEndpoints:");
    println!("  GET  http://{}/health     - Health check", local_addr);
    println!(
        "  GET  http://{}/api/demo    - Example API endpoint",
        local_addr
    );
    println!(
        "  POST http://{}/webhook     - Webhook endpoint",
        local_addr
    );
    println!(
        "  GET  http://{}/metrics     - Prometheus metrics",
        local_addr
    );

    println!("\nExample requests:");
    println!("  curl http://{}/metrics", local_addr);
    println!("  curl -X POST http://{}/webhook -d '{{}}'", local_addr);

    // Spawn a task to generate some metrics
    let metrics_clone = metrics.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;

            // Simulate metrics collection
            metrics_clone
                .record_request("GET", "/api/background", 200, 5)
                .await;
            metrics_clone
                .set_gauge("background_jobs", 42.0, vec![])
                .await;

            println!("\n[Background] Simulated metrics recorded");
        }
    });

    // Spawn a task to print metrics every 10 seconds
    let metrics_print = metrics.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;

            let metrics_text = metrics_print.get_metrics().await;
            let lines: Vec<&str> = metrics_text
                .lines()
                .filter(|line| !line.starts_with('#') && !line.is_empty())
                .collect();

            println!("\n[Metrics Summary] {} metrics recorded:", lines.len());
            for line in lines.iter().take(5) {
                println!("  {}", line);
            }
            if lines.len() > 5 {
                println!("  ... and {} more", lines.len() - 5);
            }
        }
    });

    // Run server
    axum::serve(listener, app).await?;

    Ok(())
}
