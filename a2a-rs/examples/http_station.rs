//! HTTP API Station Example
//!
//! Demonstrates using the Station trait with axum to create a simple
//! HTTP API server that handles SendMessage and GetTask requests.
//!
//! The Station pattern provides:
//! - Type-safe packet processing (no serde_json::Value at boundaries)
//! - Deterministic state transitions via OntologyState
//! - Admission control before processing
//! - Typed refusal receipts for errors
//!
//! Run with:
//! ```bash
//! cargo run --example http_station --features http-server
//! ```
//!
//! Test with:
//! ```bash
//! # Send a message (creates a task)
//! curl -X POST http://localhost:8080/jsonrpc \
//!   -H "Content-Type: application/json" \
//!   -d '{
//!     "jsonrpc": "2.0",
//!     "id": "1",
//!     "method": "message/send",
//!     "params": {
//!       "message": {
//!         "role": "user",
//!         "parts": [{"text": "Hello from curl!"}],
//!         "messageId": "msg-123"
//!       }
//!     }
//!   }'
//!
//! # Get the task (use task ID from previous response)
//! curl -X POST http://localhost:8080/jsonrpc \
//!   -H "Content-Type: application/json" \
//!   -d '{
//!     "jsonrpc": "2.0",
//!     "id": "2",
//!     "method": "tasks/get",
//!     "params": {
//!       "id": "task-xxxxx"
//!     }
//!   }'
//! ```

use std::sync::Arc;
use tokio::sync::RwLock;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};

use a2a_rs::construct::{
    station::{Ontology, RefusalReceipt, StationRegistry},
    types::JsonRpcId,
};

/// Shared application state
#[derive(Clone)]
struct AppState {
    /// Station registry for method dispatch
    registry: Arc<RwLock<StationRegistry>>,
    /// Ontology state (protocol state model)
    ontology: Arc<RwLock<Ontology>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(StationRegistry::new())),
            ontology: Arc::new(RwLock::new(Ontology::new())),
        }
    }
}

/// JSON-RPC request envelope
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<JsonRpcId>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

/// JSON-RPC response envelope
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<JsonRpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC error object
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

impl From<RefusalReceipt> for JsonRpcError {
    fn from(receipt: RefusalReceipt) -> Self {
        JsonRpcError {
            code: receipt.code,
            message: receipt.reason,
            data: receipt.data,
        }
    }
}

/// JSON-RPC handler using Station registry
async fn handle_jsonrpc(
    State(state): State<AppState>,
    Json(request): Json<JsonRpcRequest>,
) -> Response {
    // Validate JSON-RPC version
    if request.jsonrpc != "2.0" {
        return error_response(
            request.id,
            -32600,
            "Invalid JSON-RPC version".to_string(),
            None,
        );
    }

    // Dispatch to station registry
    let mut registry = state.registry.write().await;
    let mut ontology = state.ontology.write().await;

    match registry.dispatch(
        &request.method,
        &mut ontology,
        request.params,
        request.id.clone(),
    ) {
        Ok(result) => success_response(request.id, result),
        Err(refusal) => {
            let error = JsonRpcError::from(refusal);
            error_response(request.id, error.code, error.message, error.data)
        }
    }
}

/// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

/// Statistics endpoint - shows ontology state
async fn stats(State(state): State<AppState>) -> impl IntoResponse {
    let ontology = state.ontology.read().await;
    let stats = ontology.stats();

    Json(serde_json::json!({
        "tasks": stats.task_count,
        "messages": stats.total_messages,
        "agents": stats.agent_count,
        "notificationConfigs": stats.notification_config_count,
    }))
}

/// Create success JSON-RPC response
fn success_response(id: Option<JsonRpcId>, result: serde_json::Value) -> Response {
    let response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(result),
        error: None,
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// Create error JSON-RPC response
fn error_response(
    id: Option<JsonRpcId>,
    code: i32,
    message: String,
    data: Option<serde_json::Value>,
) -> Response {
    let response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message,
            data,
        }),
    };

    (StatusCode::OK, Json(response)).into_response()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== HTTP Station Example ===\n");
    println!("Starting HTTP API station on 0.0.0.0:8080");
    println!("\nSupported methods:");
    println!("  - message/send    : Send a message (creates or updates task)");
    println!("  - tasks/get       : Get task by ID");
    println!("  - tasks/cancel    : Cancel a task");
    println!("  - tasks/list      : List tasks with filters");
    println!("\nAdditional endpoints:");
    println!("  - GET  /health    : Health check");
    println!("  - GET  /stats     : Ontology statistics");
    println!("\nPress Ctrl+C to stop\n");

    // Create application state
    let state = AppState::new();

    // Build router
    let app = Router::new()
        .route("/jsonrpc", post(handle_jsonrpc))
        .route("/health", axum::routing::get(health_check))
        .route("/stats", axum::routing::get(stats))
        .with_state(state);

    // Create listener
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    println!("Listening on http://0.0.0.0:8080");

    // Run server
    axum::serve(listener, app).await?;

    Ok(())
}
