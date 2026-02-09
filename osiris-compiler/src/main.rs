//! Osiris Compiler Service
//!
//! Internal service that provides deterministic compilation μ: O → A.
//! This is the CLM (Constraint Logic Markup) Compiler that produces
//! actions A from operations O.

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use osiris_compiler::{
    adapter::LambdaOrderer,
    domain::{Operation, OrderingError},
    port::DeterministicOrderer,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

/// Application state shared across handlers
#[derive(Clone)]
struct AppState {
    orderer: Arc<LambdaOrderer>,
}

impl AppState {
    fn new() -> Self {
        Self {
            orderer: Arc::new(LambdaOrderer::default()),
        }
    }
}

/// Compilation request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompileRequest {
    operations: Vec<Operation>,
}

/// Compilation response payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompileResponse {
    ordered_operations: Vec<Operation>,
}

/// Error response payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

/// Application error wrapper
enum AppError {
    Ordering(OrderingError),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_response) = match self {
            AppError::Ordering(err) => (
                StatusCode::BAD_REQUEST,
                ErrorResponse {
                    error: "Ordering error".to_string(),
                    details: Some(err.to_string()),
                },
            ),
            AppError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorResponse {
                    error: "Internal error".to_string(),
                    details: Some(msg),
                },
            ),
        };

        (status, Json(error_response)).into_response()
    }
}

impl From<OrderingError> for AppError {
    fn from(err: OrderingError) -> Self {
        AppError::Ordering(err)
    }
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "osiris-compiler",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Compile operations into deterministically ordered actions
async fn compile(
    State(state): State<AppState>,
    Json(request): Json<CompileRequest>,
) -> Result<Json<CompileResponse>, AppError> {
    info!("Compiling {} operations", request.operations.len());

    let ordered_operations = state
        .orderer
        .order(request.operations)
        .map_err(AppError::from)?;

    Ok(Json(CompileResponse { ordered_operations }))
}

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "osiris_compiler=info,tower_http=info".into()),
        )
        .with_target(false)
        .compact()
        .init();

    info!("Starting Osiris Compiler service");

    // Initialize application state
    let state = AppState::new();

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/compile", post(compile))
        .with_state(state);

    // Bind to address
    let addr =
        std::env::var("OSIRIS_COMPILER_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    info!("Listening on {}", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    axum::serve(listener, app).await.expect("Server error");
}
