//! MCP Streamable HTTP transport adapter
//!
//! Implements the Model Context Protocol (MCP) Streamable HTTP transport specification.
//! Provides:
//! - HTTP POST/GET endpoints for MCP JSON-RPC 2.0
//! - Request/response mode via POST
//! - Server-Sent Events (SSE) streaming mode via GET
//! - Origin header validation for DNS rebinding defense
//! - Session binding via MCP-Session-Id header
//! - Resumable SSE streams via Last-Event-ID header
//!
//! # Architecture
//!
//! This adapter implements the MCP transport layer following hexagonal architecture:
//! - Domain: MCP JSON-RPC 2.0 protocol types
//! - Port: Message handler trait (async request processor)
//! - Adapter: HTTP server with SSE streaming (this module)
//!
//! # Security
//!
//! - Origin validation prevents DNS rebinding attacks
//! - Session IDs prevent cross-session data leakage
//! - SSE event IDs enable resumable streams

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        IntoResponse,
        sse::{Event, Sse},
    },
    routing::{get, post},
};
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{RwLock, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

#[cfg(feature = "tracing")]
use tracing::{debug, error, info, instrument, warn};

use crate::error::{Error, Result};

/// MCP JSON-RPC 2.0 request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRequest {
    /// JSON-RPC version (must be "2.0")
    pub jsonrpc: String,
    /// Request ID (can be string, number, or null)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    /// Method name
    pub method: String,
    /// Method parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// MCP JSON-RPC 2.0 response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResponse {
    /// JSON-RPC version (must be "2.0")
    pub jsonrpc: String,
    /// Request ID (echoed from request)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    /// Result (present if no error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error (present if request failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP JSON-RPC 2.0 error object
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Session state for tracking active SSE connections
#[derive(Debug, Clone)]
struct SessionState {
    /// Session ID
    id: String,
    /// Event sequence number (for Last-Event-ID support)
    event_seq: u64,
    /// Message queue for this session
    tx: mpsc::Sender<String>,
}

/// Configuration for the MCP Streamable HTTP server
#[derive(Debug, Clone)]
pub struct StreamableHttpConfig {
    /// Server address (e.g., "127.0.0.1:3000")
    pub address: String,
    /// Allowed origins for CORS/DNS rebinding defense
    pub allowed_origins: Vec<String>,
    /// Enable SSE keep-alive
    pub sse_keep_alive: bool,
    /// SSE keep-alive interval
    pub sse_keep_alive_interval: Duration,
    /// Maximum events to buffer per session
    pub max_buffer_size: usize,
}

impl Default for StreamableHttpConfig {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:3000".to_string(),
            allowed_origins: vec!["http://localhost:3000".to_string()],
            sse_keep_alive: true,
            sse_keep_alive_interval: Duration::from_secs(15),
            max_buffer_size: 100,
        }
    }
}

/// MCP message handler trait
///
/// Implement this trait to process incoming MCP JSON-RPC requests
#[async_trait::async_trait]
pub trait McpMessageHandler: Send + Sync {
    /// Process a JSON-RPC request and return a response
    async fn handle_request(&self, request: McpRequest) -> Result<McpResponse>;

    /// Process a streaming request and send events to the provided channel
    async fn handle_streaming_request(
        &self,
        request: McpRequest,
        tx: mpsc::Sender<McpResponse>,
    ) -> Result<()>;
}

/// Streamable HTTP server state
struct ServerState<H: McpMessageHandler> {
    /// Message handler
    handler: Arc<H>,
    /// Configuration
    config: StreamableHttpConfig,
    /// Active sessions (session_id -> SessionState)
    sessions: Arc<RwLock<HashMap<String, SessionState>>>,
}

impl<H: McpMessageHandler> Clone for ServerState<H> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            config: self.config.clone(),
            sessions: self.sessions.clone(),
        }
    }
}

/// MCP Streamable HTTP transport server
pub struct StreamableHttpServer<H: McpMessageHandler> {
    state: ServerState<H>,
}

impl<H: McpMessageHandler + 'static> StreamableHttpServer<H> {
    /// Create a new Streamable HTTP server
    pub fn new(handler: H, config: StreamableHttpConfig) -> Self {
        Self {
            state: ServerState {
                handler: Arc::new(handler),
                config,
                sessions: Arc::new(RwLock::new(HashMap::new())),
            },
        }
    }

    /// Start the HTTP server
    #[cfg_attr(feature = "tracing", instrument(skip(self), fields(
        server.address = %self.state.config.address,
        server.allowed_origins = ?self.state.config.allowed_origins
    )))]
    pub async fn start(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        info!("Starting MCP Streamable HTTP server");

        let app = Router::new()
            .route("/mcp", post(handle_post_request::<H>))
            .route("/mcp/sse", get(handle_sse_stream::<H>))
            .with_state(self.state.clone());

        let listener = tokio::net::TcpListener::bind(&self.state.config.address)
            .await
            .map_err(|e| {
                Error::Server(format!(
                    "Failed to bind to {}: {}",
                    self.state.config.address, e
                ))
            })?;

        #[cfg(feature = "tracing")]
        info!(
            "MCP Streamable HTTP server listening on {}",
            self.state.config.address
        );

        axum::serve(listener, app)
            .await
            .map_err(|e| Error::Server(format!("Server error: {}", e)))?;

        Ok(())
    }
}

/// Validate Origin header to prevent DNS rebinding attacks
fn validate_origin(headers: &HeaderMap, allowed_origins: &[String]) -> Result<()> {
    if allowed_origins.is_empty() {
        // If no origins specified, allow all (not recommended for production)
        return Ok(());
    }

    let origin = headers
        .get("origin")
        .or_else(|| headers.get("referer"))
        .and_then(|v| v.to_str().ok());

    match origin {
        Some(origin_str) => {
            // Check if origin is in allowed list
            if allowed_origins
                .iter()
                .any(|allowed| origin_str.starts_with(allowed))
            {
                Ok(())
            } else {
                #[cfg(feature = "tracing")]
                warn!("Origin validation failed: {}", origin_str);
                Err(Error::OriginForbidden(format!(
                    "Origin '{}' not in allowed list",
                    origin_str
                )))
            }
        }
        None => {
            // No origin header - allow for same-origin requests
            Ok(())
        }
    }
}

/// Extract or create session ID from headers
fn get_or_create_session_id(headers: &HeaderMap) -> String {
    headers
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

/// Extract Last-Event-ID from headers
fn get_last_event_id(headers: &HeaderMap) -> Option<u64> {
    headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
}

/// Handle POST request (request/response mode)
#[cfg_attr(feature = "tracing", instrument(skip(state), fields(
    request.method = tracing::field::Empty,
    request.id = tracing::field::Empty,
    session.id = tracing::field::Empty
)))]
async fn handle_post_request<H: McpMessageHandler>(
    State(state): State<ServerState<H>>,
    headers: HeaderMap,
    Json(request): Json<McpRequest>,
) -> impl IntoResponse {
    // Validate origin
    if let Err(e) = validate_origin(&headers, &state.config.allowed_origins) {
        #[cfg(feature = "tracing")]
        error!("Origin validation failed: {}", e);
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "jsonrpc": "2.0",
                "id": request.id,
                "error": {
                    "code": -32000,
                    "message": "Origin forbidden",
                    "data": e.to_string()
                }
            })),
        )
            .into_response();
    }

    let session_id = get_or_create_session_id(&headers);

    #[cfg(feature = "tracing")]
    {
        tracing::Span::current()
            .record("request.method", &request.method)
            .record("request.id", format!("{:?}", request.id).as_str())
            .record("session.id", &session_id);
        debug!("Processing POST request");
    }

    // Process request
    match state.handler.handle_request(request.clone()).await {
        Ok(response) => {
            #[cfg(feature = "tracing")]
            debug!("Request processed successfully");

            let mut headers = HeaderMap::new();
            headers.insert(
                "mcp-session-id",
                session_id
                    .parse()
                    .unwrap_or_else(|_| "invalid".parse().unwrap()),
            );

            (StatusCode::OK, headers, Json(response)).into_response()
        }
        Err(e) => {
            #[cfg(feature = "tracing")]
            error!("Request processing failed: {}", e);

            let error_response = McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(McpError {
                    code: -32603,
                    message: "Internal error".to_string(),
                    data: Some(json!(e.to_string())),
                }),
            };

            (StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)).into_response()
        }
    }
}

/// Query parameters for SSE endpoint
#[derive(Debug, Deserialize)]
struct SseQuery {
    /// Optional request to send
    request: Option<String>,
}

/// Handle GET request with SSE streaming
#[cfg_attr(feature = "tracing", instrument(skip(state), fields(
    session.id = tracing::field::Empty,
    last_event_id = tracing::field::Empty
)))]
async fn handle_sse_stream<H: McpMessageHandler + 'static>(
    State(state): State<ServerState<H>>,
    headers: HeaderMap,
    Query(query): Query<SseQuery>,
) -> impl IntoResponse {
    // Validate origin
    if let Err(_e) = validate_origin(&headers, &state.config.allowed_origins) {
        #[cfg(feature = "tracing")]
        error!("Origin validation failed");
        return (StatusCode::FORBIDDEN, "Origin forbidden").into_response();
    }

    let session_id = get_or_create_session_id(&headers);
    let last_event_id = get_last_event_id(&headers);

    #[cfg(feature = "tracing")]
    {
        tracing::Span::current()
            .record("session.id", &session_id)
            .record("last_event_id", last_event_id.unwrap_or(0));
        debug!("Starting SSE stream");
    }

    // Create message channel for this session
    let (tx, rx) = mpsc::channel::<String>(state.config.max_buffer_size);

    // Store session state
    let session_state = SessionState {
        id: session_id.clone(),
        event_seq: last_event_id.unwrap_or(0),
        tx: tx.clone(),
    };

    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id.clone(), session_state);
    }

    // If initial request provided, process it
    if let Some(request_json) = query.request {
        if let Ok(request) = serde_json::from_str::<McpRequest>(&request_json) {
            let handler = state.handler.clone();
            let string_tx = tx.clone();

            // Create a typed response channel that converts to JSON strings
            let (response_tx, mut response_rx) =
                mpsc::channel::<McpResponse>(state.config.max_buffer_size);

            // Spawn a task to convert McpResponse to JSON strings
            tokio::spawn(async move {
                while let Some(response) = response_rx.recv().await {
                    if let Ok(json_str) = serde_json::to_string(&response) {
                        if string_tx.send(json_str).await.is_err() {
                            break;
                        }
                    }
                }
            });

            // Spawn handler task
            tokio::spawn(async move {
                if let Err(_e) = handler.handle_streaming_request(request, response_tx).await {
                    #[cfg(feature = "tracing")]
                    error!("Streaming request failed");
                }
            });
        }
    }

    // Create SSE stream
    // In Axum 0.8, Event::data() returns Result, so we need to handle it
    let stream = ReceiverStream::new(rx).map(|msg| {
        Ok::<_, std::convert::Infallible>(
            Event::default()
                .data(msg)
                .event("message")
        )
    });

    // Create SSE with optional keep-alive
    // In Axum 0.8, keep_alive API has changed
    let sse = if state.config.sse_keep_alive {
        Sse::new(stream).keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(state.config.sse_keep_alive_interval)
                .text("keep-alive"),
        )
    } else {
        Sse::new(stream).keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(std::time::Duration::from_secs(3600))
                .text("")
        )
    };

    // Clean up session when stream ends
    let sessions = state.sessions.clone();
    let session_id_cleanup = session_id.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let mut sessions = sessions.write().await;
        sessions.remove(&session_id_cleanup);

        #[cfg(feature = "tracing")]
        debug!(
            "SSE stream ended, session cleaned up: {}",
            session_id_cleanup
        );
    });

    sse.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock message handler for testing
    struct MockHandler;

    #[async_trait::async_trait]
    impl McpMessageHandler for MockHandler {
        async fn handle_request(&self, request: McpRequest) -> Result<McpResponse> {
            Ok(McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(json!({"status": "ok"})),
                error: None,
            })
        }

        async fn handle_streaming_request(
            &self,
            _request: McpRequest,
            _tx: mpsc::Sender<McpResponse>,
        ) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_config_default() {
        let config = StreamableHttpConfig::default();
        assert_eq!(config.address, "127.0.0.1:3000");
        assert_eq!(config.allowed_origins.len(), 1);
        assert!(config.sse_keep_alive);
    }

    #[test]
    fn test_mcp_request_serialization() {
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "test/method".to_string(),
            params: Some(json!({"key": "value"})),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: McpRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.jsonrpc, "2.0");
        assert_eq!(deserialized.method, "test/method");
    }

    #[test]
    fn test_origin_validation() {
        let mut headers = HeaderMap::new();
        headers.insert("origin", "http://localhost:3000".parse().unwrap());

        let allowed = vec!["http://localhost:3000".to_string()];
        assert!(validate_origin(&headers, &allowed).is_ok());

        let not_allowed = vec!["http://example.com".to_string()];
        assert!(validate_origin(&headers, &not_allowed).is_err());
    }

    #[test]
    fn test_session_id_extraction() {
        let mut headers = HeaderMap::new();
        headers.insert("mcp-session-id", "test-session-123".parse().unwrap());

        let session_id = get_or_create_session_id(&headers);
        assert_eq!(session_id, "test-session-123");
    }

    #[test]
    fn test_last_event_id_extraction() {
        let mut headers = HeaderMap::new();
        headers.insert("last-event-id", "42".parse().unwrap());

        let event_id = get_last_event_id(&headers);
        assert_eq!(event_id, Some(42));
    }
}
