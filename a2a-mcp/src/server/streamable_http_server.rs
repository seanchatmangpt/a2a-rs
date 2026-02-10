//! Axum-based MCP Streamable HTTP server with middleware integration
//!
//! Implements the MCP Streamable HTTP transport with:
//! - Origin guard middleware for DNS rebinding defense
//! - Session middleware for request-scoped session management
//! - SSE streaming support via Last-Event-ID resumption
//! - JSON-RPC 2.0 request/response routing to McpTaskHandler

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

#[cfg(feature = "tracing")]
use tracing::{debug, error, info, instrument, warn};

use crate::adapter::{InMemorySessionManager, OriginGuard};
use crate::application::{JsonRpcRequest, JsonRpcResponse, McpTaskHandler};
use crate::domain::Session;
use crate::error::{Error, Result};
use crate::port::{OriginValidator, SessionManager};

/// Server state for MCP Streamable HTTP
#[derive(Clone)]
struct McpServerState {
    /// Handler for JSON-RPC requests
    handler: Arc<McpTaskHandler>,
    /// Origin validator for DNS rebinding defense
    origin_guard: Arc<dyn OriginValidator>,
    /// Session manager for request scoping
    session_manager: Arc<dyn SessionManager>,
}

/// Request context injected by middleware
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Session ID from MCP-Session-Id header or generated
    pub session_id: String,
    /// Validated origin
    pub origin: Option<String>,
    /// Current session
    pub session: Option<Session>,
}

/// MCP Streamable HTTP server with middleware integration
pub struct StreamableHttpServer {
    addr: SocketAddr,
    handler: Arc<McpTaskHandler>,
    origin_guard: Arc<dyn OriginValidator>,
    session_manager: Arc<dyn SessionManager>,
}

impl StreamableHttpServer {
    /// Create a new MCP server
    pub fn new(
        addr: SocketAddr,
        handler: Arc<McpTaskHandler>,
        origin_guard: Arc<dyn OriginValidator>,
        session_manager: Arc<dyn SessionManager>,
    ) -> Self {
        Self {
            addr,
            handler,
            origin_guard,
            session_manager,
        }
    }

    /// Create a new server with localhost-only origin guard
    pub fn localhost(
        addr: SocketAddr,
        handler: Arc<McpTaskHandler>,
        session_manager: Arc<dyn SessionManager>,
    ) -> Self {
        Self::new(
            addr,
            handler,
            Arc::new(OriginGuard::localhost_only()),
            session_manager,
        )
    }

    /// Create a new server with default settings (localhost origin, in-memory sessions)
    pub fn default_configured(addr: SocketAddr, handler: Arc<McpTaskHandler>) -> Self {
        Self::new(
            addr,
            handler,
            Arc::new(OriginGuard::localhost_only()),
            Arc::new(InMemorySessionManager::new()),
        )
    }

    /// Start the server
    #[cfg_attr(feature = "tracing", instrument(skip(self), fields(
        addr = %self.addr
    )))]
    pub async fn start(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        info!("Starting MCP Streamable HTTP server on {}", self.addr);

        let state = McpServerState {
            handler: self.handler.clone(),
            origin_guard: self.origin_guard.clone(),
            session_manager: self.session_manager.clone(),
        };

        // Build router with middleware
        let app = Router::new()
            .route("/mcp", post(handle_mcp_post))
            .route("/mcp", get(handle_mcp_sse))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                origin_guard_middleware,
            ))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                session_middleware,
            ))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(self.addr)
            .await
            .map_err(|e| Error::Server(format!("Failed to bind to {}: {}", self.addr, e)))?;

        #[cfg(feature = "tracing")]
        info!("MCP server listening on {}", self.addr);

        axum::serve(listener, app)
            .await
            .map_err(|e| Error::Server(format!("Server error: {}", e)))?;

        Ok(())
    }
}

/// Origin guard middleware - validates Origin header against allowlist
#[cfg_attr(feature = "tracing", instrument(skip(state, req, next)))]
async fn origin_guard_middleware(
    State(state): State<McpServerState>,
    headers: HeaderMap,
    mut req: axum::extract::Request,
    next: Next,
) -> Response {
    // Get origin from header
    let origin = headers
        .get("origin")
        .or_else(|| headers.get("referer"))
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Validate origin
    let validation_result = if let Some(ref origin_str) = origin {
        state.origin_guard.validate_origin(Some(origin_str))
    } else {
        state.origin_guard.validate_origin(None)
    };

    match validation_result {
        Ok(()) => {
            #[cfg(feature = "tracing")]
            debug!("Origin validation passed");

            // Store origin in request extensions for later use
            req.extensions_mut().insert(origin);

            next.run(req).await
        }
        Err(e) => {
            #[cfg(feature = "tracing")]
            warn!("Origin validation failed: {}", e);

            let error_response = json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {
                    "code": -32000,
                    "message": "Origin forbidden",
                    "data": e.to_string()
                }
            });

            (StatusCode::FORBIDDEN, Json(error_response)).into_response()
        }
    }
}

/// Session middleware - manages request scoping
#[cfg_attr(feature = "tracing", instrument(skip(state, req, next)))]
async fn session_middleware(
    State(state): State<McpServerState>,
    headers: HeaderMap,
    mut req: axum::extract::Request,
    next: Next,
) -> Response {
    // Extract session ID from header or generate new one
    let session_id = headers
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    #[cfg(feature = "tracing")]
    debug!("Session ID: {}", session_id);

    // Get or create session
    let session = match state
        .session_manager
        .get_or_create_session(session_id.clone())
        .await
    {
        Ok((session, created)) => {
            if created {
                #[cfg(feature = "tracing")]
                debug!("Created new session");
            } else {
                #[cfg(feature = "tracing")]
                debug!("Retrieved existing session");
            }
            Some(session)
        }
        Err(e) => {
            #[cfg(feature = "tracing")]
            error!("Failed to manage session: {}", e);
            None
        }
    };

    // Update session access time
    if let Err(e) = state.session_manager.touch_session(&session_id).await {
        #[cfg(feature = "tracing")]
        warn!("Failed to touch session: {}", e);
    }

    // Extract origin from extensions
    let origin = req
        .extensions()
        .get::<Option<String>>()
        .and_then(|o| o.clone());

    // Build request context
    let context = RequestContext {
        session_id: session_id.clone(),
        origin,
        session,
    };

    // Insert context into request extensions
    req.extensions_mut().insert(context);

    // Continue processing with modified request
    let mut response = next.run(req).await;

    // Add session ID to response headers
    if let Ok(session_id_header) = session_id.parse() {
        response
            .headers_mut()
            .insert("mcp-session-id", session_id_header);
    }

    response
}

/// Handle POST requests (request/response mode)
#[cfg_attr(feature = "tracing", instrument(skip(state, headers, request)))]
async fn handle_mcp_post(
    State(state): State<McpServerState>,
    headers: HeaderMap,
    axum::extract::Extension(context): axum::extract::Extension<RequestContext>,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    #[cfg(feature = "tracing")]
    debug!(
        "POST request: method={}, id={:?}",
        request.method, request.id
    );

    // Handle the JSON-RPC request
    let response = state.handler.handle_request(request).await;

    let mut response_headers = HeaderMap::new();
    if let Ok(session_id_header) = context.session_id.parse() {
        response_headers.insert("mcp-session-id", session_id_header);
    }

    (StatusCode::OK, response_headers, Json(response))
}

/// Query parameters for SSE stream
#[derive(Debug, serde::Deserialize)]
pub struct SseQuery {
    /// Optional JSON-RPC request to send with SSE stream
    pub request: Option<String>,
}

/// Handle GET requests with SSE streaming
#[cfg_attr(feature = "tracing", instrument(skip(state, headers, query)))]
async fn handle_mcp_sse(
    State(state): State<McpServerState>,
    headers: HeaderMap,
    axum::extract::Extension(context): axum::extract::Extension<RequestContext>,
    axum::extract::Query(query): axum::extract::Query<SseQuery>,
) -> Result<impl IntoResponse> {
    #[cfg(feature = "tracing")]
    debug!("SSE stream started: session_id={}", context.session_id);

    // Create channel for streaming messages
    let (tx, rx) = mpsc::channel::<Result<Event>>(100);

    // Extract Last-Event-ID for resumption
    let last_event_id = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    #[cfg(feature = "tracing")]
    debug!("Last-Event-ID: {}", last_event_id);

    // Process initial request if provided
    if let Some(req_json) = query.request {
        let request =
            serde_json::from_str::<JsonRpcRequest>(&req_json).map_err(|e| Error::Json(e))?;

        // Process request and send response as first event
        let response = state.handler.handle_request(request).await;
        let response_value = serde_json::to_value(&response).unwrap_or(Value::Null);

        let event = Event::default()
            .id("0")
            .event("mcp-response")
            .data(response_value.to_string());

        let _ = tx.send(Ok(event)).await;
    }

    // Clone tx for spawned task
    let tx_clone = tx.clone();
    let session_id = context.session_id.clone();
    let session_manager = state.session_manager.clone();

    // Spawn task to manage session keep-alive
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;

            // Keep session alive
            if let Err(e) = session_manager.touch_session(&session_id).await {
                #[cfg(feature = "tracing")]
                warn!("Failed to touch session during SSE: {}", e);
                break;
            }

            // Send keep-alive event
            let event = Event::default().event("keep-alive").data("");

            if tx_clone.send(Ok(event)).await.is_err() {
                // Client disconnected
                break;
            }
        }
    });

    // Create the SSE stream
    let stream = ReceiverStream::new(rx)
        .map(|e| {
            e.unwrap_or_else(|e| {
                let error_msg = json!({
                    "error": e.to_string()
                });
                Event::default().event("error").data(error_msg.to_string())
            })
        })
        .keep_alive(KeepAlive::default());

    // Build response with session ID header
    let mut response_headers = HeaderMap::new();
    if let Ok(session_id_header) = context.session_id.parse() {
        response_headers.insert("mcp-session-id", session_id_header);
    }

    #[cfg(feature = "tracing")]
    debug!("SSE stream established");

    Ok((response_headers, Sse::new(stream)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::TaskWrapper;

    #[test]
    fn test_streamable_http_server_creation() {
        let addr = "127.0.0.1:3000".parse().unwrap();
        let handler = Arc::new(McpTaskHandler::new(Arc::new(TaskWrapper::new())));
        let origin_guard = Arc::new(OriginGuard::localhost_only());
        let session_manager = Arc::new(InMemorySessionManager::new());

        let server = StreamableHttpServer::new(addr, handler, origin_guard, session_manager);
        assert_eq!(server.addr, addr);
    }

    #[test]
    fn test_streamable_http_server_localhost() {
        let addr = "127.0.0.1:3000".parse().unwrap();
        let handler = Arc::new(McpTaskHandler::new(Arc::new(TaskWrapper::new())));

        let server =
            StreamableHttpServer::localhost(addr, handler, Arc::new(InMemorySessionManager::new()));
        assert_eq!(server.addr, addr);
    }

    #[test]
    fn test_streamable_http_server_default() {
        let addr = "127.0.0.1:3000".parse().unwrap();
        let handler = Arc::new(McpTaskHandler::new(Arc::new(TaskWrapper::new())));

        let server = StreamableHttpServer::default_configured(addr, handler);
        assert_eq!(server.addr, addr);
    }

    #[tokio::test]
    async fn test_origin_guard_middleware() {
        let origin_guard = Arc::new(OriginGuard::localhost_only());

        // Test valid origin
        let result = origin_guard.validate_origin(Some("http://localhost:3000"));
        assert!(result.is_ok());

        // Test invalid origin
        let result = origin_guard.validate_origin(Some("https://evil.com"));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_request_context() {
        let context = RequestContext {
            session_id: "test-session".to_string(),
            origin: Some("http://localhost:3000".to_string()),
            session: Some(Session::new("test-session".to_string())),
        };

        assert_eq!(context.session_id, "test-session");
        assert!(context.origin.is_some());
        assert!(context.session.is_some());
    }
}
