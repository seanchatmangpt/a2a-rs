//! HTTP server with MCP-Session-Id header handling
//!
//! This example demonstrates how to integrate the SessionManager with an Axum HTTP server
//! to handle MCP-Session-Id headers in actual HTTP requests.

use a2a_mcp::{InMemorySessionManager, SessionManager};
use axum::{
    Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Shared application state
#[derive(Clone)]
struct AppState {
    session_manager: Arc<InMemorySessionManager>,
}

/// Extract or generate session ID from MCP-Session-Id header
fn get_or_create_session_id(headers: &HeaderMap) -> String {
    headers
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

/// Request to store data in session
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoreDataRequest {
    key: String,
    value: serde_json::Value,
}

/// Response with session information
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionResponse {
    session_id: String,
    created: bool,
    data: Option<serde_json::Value>,
}

/// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

/// Get or create session and return session info
async fn session_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SessionResponse>, StatusCode> {
    let session_id = get_or_create_session_id(&headers);

    let (session, created) = state
        .session_manager
        .get_or_create_session(session_id.clone())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(SessionResponse {
        session_id: session.id,
        created,
        data: session.state,
    }))
}

/// Store data in the session
async fn store_data(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<StoreDataRequest>,
) -> Result<Json<SessionResponse>, StatusCode> {
    let session_id = get_or_create_session_id(&headers);

    let (mut session, created) = state
        .session_manager
        .get_or_create_session(session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get existing state or create new
    let mut state_map = if let Some(state_value) = session.state {
        state_value.as_object().cloned().unwrap_or_default()
    } else {
        serde_json::Map::new()
    };

    // Store the new key-value pair
    state_map.insert(payload.key, payload.value);
    session.state = Some(serde_json::Value::Object(state_map));

    // Update session
    state
        .session_manager
        .update_session(session.clone())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(SessionResponse {
        session_id: session.id,
        created,
        data: session.state,
    }))
}

/// Retrieve data from session
async fn get_data(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SessionResponse>, StatusCode> {
    let session_id = get_or_create_session_id(&headers);

    match state.session_manager.get_session(&session_id).await {
        Ok(Some(session)) => {
            // Touch the session to update last accessed time
            let _ = state.session_manager.touch_session(&session_id).await;

            Ok(Json(SessionResponse {
                session_id: session.id,
                created: false,
                data: session.state,
            }))
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Delete a session
async fn delete_session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let session_id = get_or_create_session_id(&headers);

    match state.session_manager.delete_session(&session_id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// List all active sessions
async fn list_sessions(State(state): State<AppState>) -> Result<Json<Vec<String>>, StatusCode> {
    state
        .session_manager
        .list_sessions()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create shared state with session manager
    let state = AppState {
        session_manager: Arc::new(InMemorySessionManager::new()),
    };

    // Background task to periodically clean up expired sessions
    let cleanup_manager = state.session_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Ok(count) = cleanup_manager.cleanup_expired_sessions().await {
                if count > 0 {
                    tracing::info!("Cleaned up {} expired sessions", count);
                }
            }
        }
    });

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/session", get(session_info))
        .route("/session/data", post(store_data))
        .route("/session/data", get(get_data))
        .route("/session", axum::routing::delete(delete_session))
        .route("/sessions", get(list_sessions))
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));
    println!("=== MCP Session Server ===");
    println!("Listening on http://{}", addr);
    println!("\nExample requests:");
    println!(
        "  Get session info:  curl -H 'MCP-Session-Id: my-session' http://{}/session",
        addr
    );
    println!(
        "  Store data:        curl -X POST -H 'MCP-Session-Id: my-session' -H 'Content-Type: application/json' -d '{{\"key\":\"foo\",\"value\":\"bar\"}}' http://{}/session/data",
        addr
    );
    println!(
        "  Get data:          curl -H 'MCP-Session-Id: my-session' http://{}/session/data",
        addr
    );
    println!(
        "  Delete session:    curl -X DELETE -H 'MCP-Session-Id: my-session' http://{}/session",
        addr
    );
    println!("  List sessions:     curl http://{}/sessions", addr);
    println!();

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
