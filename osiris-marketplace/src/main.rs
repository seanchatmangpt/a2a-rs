//! osiris-marketplace binary - Cloud Run service for Cloud Marketplace integration
//!
//! This service:
//! - Consumes entitlement events from Google Cloud Pub/Sub
//! - Automatically approves account resources via Procurement API
//! - Exposes health check endpoint for Cloud Run

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(all(feature = "pubsub", feature = "procurement-api"))]
use osiris_marketplace::{
    adapter::{ProcurementApiClient, PubSubConsumer},
    application::MarketplaceService,
};

/// Application configuration from environment variables
#[derive(Debug, Clone)]
struct Config {
    /// Google Cloud project ID
    project_id: String,

    /// Pub/Sub subscription ID for entitlement events
    subscription_id: String,

    /// OAuth2 access token for Procurement API (or use Application Default Credentials)
    access_token: Option<String>,

    /// Port to listen on (Cloud Run sets this via PORT env var)
    port: u16,

    /// Whether to auto-approve new entitlements
    auto_approve: bool,
}

impl Config {
    /// Load configuration from environment variables
    fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        dotenvy::dotenv().ok();

        let project_id = std::env::var("GCP_PROJECT_ID")
            .or_else(|_| std::env::var("GOOGLE_CLOUD_PROJECT"))
            .map_err(|_| "GCP_PROJECT_ID or GOOGLE_CLOUD_PROJECT must be set")?;

        let subscription_id = std::env::var("PUBSUB_SUBSCRIPTION_ID")
            .unwrap_or_else(|_| "marketplace-events-sub".to_string());

        let access_token = std::env::var("PROCUREMENT_API_TOKEN").ok();

        let port = std::env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .map_err(|_| "PORT must be a valid u16")?;

        let auto_approve = std::env::var("AUTO_APPROVE")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true);

        Ok(Self {
            project_id,
            subscription_id,
            access_token,
            port,
            auto_approve,
        })
    }
}

/// Application state shared across handlers
#[derive(Clone)]
struct AppState {
    #[allow(dead_code)]
    config: Config,
}

/// Health check response
#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

/// Health check endpoint (required by Cloud Run)
async fn health_check() -> impl IntoResponse {
    Json(HealthResponse {
        status: "healthy",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Readiness check endpoint
async fn readiness_check(State(_state): State<AppState>) -> impl IntoResponse {
    // In production, you might want to check if Pub/Sub connection is alive
    Json(HealthResponse {
        status: "ready",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Root endpoint
async fn root() -> impl IntoResponse {
    (
        StatusCode::OK,
        "osiris-marketplace - Cloud Marketplace integration service",
    )
}

/// Create the Axum router with all endpoints
fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Initialize tracing subscriber
fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,osiris_marketplace=debug"));

    // Use JSON formatting for Cloud Run structured logging
    let json_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(true)
        .with_current_span(true);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(json_layer)
        .init();
}

/// Run the marketplace event processing service
#[cfg(all(feature = "pubsub", feature = "procurement-api"))]
async fn run_marketplace_service(
    config: Config,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!(
        "Initializing marketplace service (project={}, subscription={}, auto_approve={})",
        config.project_id, config.subscription_id, config.auto_approve
    );

    // Create Pub/Sub consumer
    let consumer = Arc::new(
        PubSubConsumer::new(config.project_id.clone(), config.subscription_id.clone()).await?,
    );

    // Create Procurement API client
    // In production, use Application Default Credentials or Workload Identity
    let access_token = config.access_token.clone().ok_or_else(
        || "PROCUREMENT_API_TOKEN must be set (or use Application Default Credentials)",
    )?;

    let approver = Arc::new(ProcurementApiClient::new(
        config.project_id.clone(),
        access_token,
    )?);

    // Create and run the marketplace service
    let service = MarketplaceService::new(consumer, approver, config.auto_approve);

    info!("Starting marketplace event consumer");
    service.run().await?;

    Ok(())
}

/// Fallback for when features are not enabled
#[cfg(not(all(feature = "pubsub", feature = "procurement-api")))]
async fn run_marketplace_service(
    _config: Config,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    warn!("Marketplace service requires 'pubsub' and 'procurement-api' features");
    warn!("Service will only run the health check endpoint");

    // Keep the service alive
    signal::ctrl_c().await?;
    Ok(())
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received SIGTERM signal");
        },
    }

    info!("Shutting down gracefully");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    init_tracing();

    info!("Starting osiris-marketplace v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = Config::from_env()?;
    info!("Configuration loaded: {:?}", config);

    let state = AppState {
        config: config.clone(),
    };

    // Create the Axum server
    let app = create_router(state);

    let addr = format!("0.0.0.0:{}", config.port);
    info!("Starting HTTP server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    // Spawn the marketplace service in a separate task
    let service_handle = tokio::spawn(async move {
        if let Err(e) = run_marketplace_service(config).await {
            error!("Marketplace service error: {}", e);
        }
    });

    // Run the HTTP server with graceful shutdown
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .expect("server error");
    });

    // Wait for both tasks
    tokio::select! {
        _ = service_handle => {
            info!("Marketplace service stopped");
        },
        _ = server_handle => {
            info!("HTTP server stopped");
        },
    }

    info!("Shutdown complete");

    Ok(())
}
