//! Unified server exposing both MCP and A2A protocols on a single port
//!
//! This server auto-detects the protocol from incoming requests and routes
//! to the appropriate handler. It also provides bidirectional bridging between
//! the protocols.

use crate::adapter::{PathBasedDetector, UnifiedBridge};
use crate::domain::protocol::{BridgeConfig, Protocol};
use crate::port::ProtocolDetector;
use a2a_rs::domain::{
    agent::AgentCard,
    message::Message,
    task::{Task, TaskState, TaskStatus},
};
use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Json, Response, sse::Sse},
    routing::{any, get, post},
};
use rmcp::{Server as RmcpServer, Tool, ToolCall};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Unified server configuration
#[derive(Debug, Clone)]
pub struct UnifiedServerConfig {
    /// Server bind address
    pub address: SocketAddr,
    /// Bridge configuration
    pub bridge_config: BridgeConfig,
    /// Enable protocol detection logging
    pub log_detection: bool,
}

impl Default for UnifiedServerConfig {
    fn default() -> Self {
        Self {
            address: ([127, 0, 0, 1], 3000).into(),
            bridge_config: BridgeConfig::default(),
            log_detection: true,
        }
    }
}

/// Shared server state
#[derive(Clone)]
struct ServerState {
    /// Protocol detector
    detector: Arc<dyn ProtocolDetector>,
    /// Unified bridge
    bridge: Arc<UnifiedBridge>,
    /// MCP server (optional)
    rmcp_server: Option<Arc<RmcpServer>>,
    /// Task store for A2A tasks
    tasks: Arc<Mutex<HashMap<String, Task>>>,
    /// Configuration
    config: UnifiedServerConfig,
}

/// Unified server exposing both MCP and A2A protocols
pub struct UnifiedServer {
    state: ServerState,
}

impl UnifiedServer {
    /// Create a new unified server
    pub fn new(config: UnifiedServerConfig) -> Self {
        let detector = Arc::new(PathBasedDetector::new());
        let bridge = Arc::new(UnifiedBridge::new(config.bridge_config.clone()));

        Self {
            state: ServerState {
                detector,
                bridge,
                rmcp_server: None,
                tasks: Arc::new(Mutex::new(HashMap::new())),
                config,
            },
        }
    }

    /// Set the MCP server instance
    pub fn with_rmcp_server(mut self, rmcp_server: RmcpServer) -> Self {
        self.state.rmcp_server = Some(Arc::new(rmcp_server));
        self
    }

    /// Register an A2A agent to be exposed as MCP tools
    pub async fn register_a2a_agent(&self, url: String, agent_card: AgentCard) {
        self.state.bridge.register_a2a_agent(url, agent_card).await;
    }

    /// Register an MCP tool to be exposed as A2A agent capability
    pub async fn register_mcp_tool(&self, tool: Tool) {
        self.state.bridge.register_mcp_tool(tool).await;
    }

    /// Start the unified server
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Starting unified MCP + A2A server on {}",
            self.state.config.address
        );

        // Build router
        let app = Router::new()
            // Health and info endpoints
            .route("/health", get(health_check))
            .route("/info", get(server_info))
            .route("/stats", get(bridge_stats))
            // A2A-specific endpoints
            .route("/.well-known/agent-card", get(get_agent_card))
            .route("/tasks/send", post(handle_task_send))
            .route("/tasks/get", get(handle_task_get))
            // MCP-specific endpoints
            .route("/mcp", post(handle_mcp_request))
            .route("/mcp/sse", get(handle_mcp_sse))
            // Unified endpoint that auto-detects protocol
            .route("/api", any(handle_unified_request))
            .route("/api/*path", any(handle_unified_request))
            .with_state(self.state);

        // Start server
        let listener = tokio::net::TcpListener::bind(&self.state.config.address).await?;
        info!("Server listening on {}", self.state.config.address);

        axum::serve(listener, app).await?;

        Ok(())
    }
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    StatusCode::OK
}

/// Server info endpoint
async fn server_info(State(state): State<ServerState>) -> impl IntoResponse {
    Json(json!({
        "name": "Unified MCP + A2A Server",
        "version": env!("CARGO_PKG_VERSION"),
        "protocols": ["mcp", "a2a"],
        "bridging": {
            "mcpToA2a": state.config.bridge_config.enable_mcp_to_a2a,
            "a2aToMcp": state.config.bridge_config.enable_a2a_to_mcp,
        }
    }))
}

/// Bridge statistics endpoint
async fn bridge_stats(State(state): State<ServerState>) -> impl IntoResponse {
    let stats = state.bridge.get_statistics().await;
    Json(stats)
}

/// Get A2A agent card
async fn get_agent_card(State(state): State<ServerState>) -> impl IntoResponse {
    let agent_card = state.bridge.get_agent_card().await;
    Json(agent_card)
}

/// Handle A2A task send
#[derive(Debug, Deserialize)]
struct TaskSendRequest {
    task_id: Option<String>,
    message: Message,
}

#[derive(Debug, Serialize)]
struct TaskSendResponse {
    task: Task,
}

async fn handle_task_send(
    State(state): State<ServerState>,
    Json(request): Json<TaskSendRequest>,
) -> Result<Json<TaskSendResponse>, StatusCode> {
    let task_id = request
        .task_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    info!("Handling A2A task send: {}", task_id);

    // Try to bridge A2A message to MCP tool call if applicable
    let task = if let Ok(tool_call) = state.bridge.bridge_a2a_to_mcp(&request.message).await {
        debug!("Bridged A2A message to MCP tool call: {}", tool_call.method);

        // Execute MCP tool call if server available
        if let Some(rmcp_server) = &state.rmcp_server {
            match rmcp_server.call_tool(tool_call).await {
                Ok(response) => {
                    // Bridge response back to A2A
                    state
                        .bridge
                        .bridge_mcp_response_to_a2a(&response, &task_id)
                        .await
                        .map_err(|e| {
                            error!("Failed to bridge MCP response: {}", e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?
                }
                Err(e) => {
                    error!("MCP tool call failed: {}", e);
                    // Create error task
                    Task {
                        id: task_id.clone(),
                        status: TaskStatus {
                            state: TaskState::Failed,
                            message: Some(format!("MCP tool call failed: {}", e)),
                        },
                        messages: vec![request.message],
                        artifacts: Vec::new(),
                        history_ttl: Some(3600),
                        metadata: None,
                    }
                }
            }
        } else {
            // No MCP server, create pending task
            Task {
                id: task_id.clone(),
                status: TaskStatus {
                    state: TaskState::Submitted,
                    message: Some("Task submitted (no MCP server)".to_string()),
                },
                messages: vec![request.message],
                artifacts: Vec::new(),
                history_ttl: Some(3600),
                metadata: None,
            }
        }
    } else {
        // Not bridgeable or bridging disabled, create regular task
        Task {
            id: task_id.clone(),
            status: TaskStatus {
                state: TaskState::Submitted,
                message: Some("Task submitted".to_string()),
            },
            messages: vec![request.message],
            artifacts: Vec::new(),
            history_ttl: Some(3600),
            metadata: None,
        }
    };

    // Store task
    {
        let mut tasks = state.tasks.lock().await;
        tasks.insert(task_id.clone(), task.clone());
    }

    Ok(Json(TaskSendResponse { task }))
}

/// Handle A2A task get
#[derive(Debug, Deserialize)]
struct TaskGetRequest {
    task_id: String,
}

async fn handle_task_get(
    State(state): State<ServerState>,
    Json(request): Json<TaskGetRequest>,
) -> Result<Json<Task>, StatusCode> {
    let tasks = state.tasks.lock().await;
    let task = tasks
        .get(&request.task_id)
        .ok_or(StatusCode::NOT_FOUND)?
        .clone();

    Ok(Json(task))
}

/// MCP JSON-RPC request
#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpRequest {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// MCP JSON-RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// Handle MCP request
async fn handle_mcp_request(
    State(state): State<ServerState>,
    Json(request): Json<McpRequest>,
) -> impl IntoResponse {
    info!("Handling MCP request: {}", request.method);

    // Handle tools/list method
    if request.method == "tools/list" {
        let tools = state.bridge.get_mcp_tools().await;
        let response = McpResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(json!({ "tools": tools })),
            error: None,
        };
        return Json(response).into_response();
    }

    // Handle tools/call method
    if request.method == "tools/call" {
        if let Some(params) = request.params {
            // Extract tool call from params
            if let Ok(tool_call) = serde_json::from_value::<ToolCall>(params) {
                // Try to bridge to A2A if applicable
                match state.bridge.bridge_mcp_to_a2a(&tool_call).await {
                    Ok(task) => {
                        debug!("Bridged MCP tool call to A2A task: {}", task.id);

                        // Bridge task result back to MCP
                        match state.bridge.bridge_a2a_task_to_mcp(&task).await {
                            Ok(tool_response) => {
                                let response = McpResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: request.id,
                                    result: Some(tool_response.result),
                                    error: None,
                                };
                                return Json(response).into_response();
                            }
                            Err(e) => {
                                error!("Failed to bridge A2A task to MCP: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Could not bridge MCP tool call to A2A: {}", e);
                    }
                }
            }
        }
    }

    // Fallback: method not supported
    let response = McpResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id,
        result: None,
        error: Some(McpError {
            code: -32601,
            message: "Method not found".to_string(),
            data: Some(json!({ "method": request.method })),
        }),
    };

    Json(response).into_response()
}

/// Handle MCP SSE endpoint
async fn handle_mcp_sse(State(_state): State<ServerState>) -> impl IntoResponse {
    // Placeholder for SSE streaming
    // In a full implementation, this would set up an SSE stream
    StatusCode::NOT_IMPLEMENTED
}

/// Unified request handler that auto-detects protocol
async fn handle_unified_request(
    State(state): State<ServerState>,
    uri: Uri,
    headers: HeaderMap,
    request: Request,
) -> impl IntoResponse {
    debug!("Handling unified request: {}", uri.path());

    // Extract body for protocol detection
    let body_bytes = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to read request body: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Failed to read request body" })),
            )
                .into_response();
        }
    };

    // Detect protocol
    let detected = state
        .detector
        .detect(&uri, &headers, Some(&body_bytes))
        .await;

    if state.config.log_detection {
        info!(
            "Protocol detected: {} (confidence: {}, method: {:?})",
            detected.protocol.as_str(),
            detected.confidence,
            detected.method
        );
    }

    // Route based on detected protocol
    match detected.protocol {
        Protocol::Mcp => {
            // Parse as MCP request
            match serde_json::from_slice::<McpRequest>(&body_bytes) {
                Ok(mcp_request) => {
                    // Handle as MCP request
                    handle_mcp_request(State(state), Json(mcp_request))
                        .await
                        .into_response()
                }
                Err(e) => {
                    error!("Failed to parse MCP request: {}", e);
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": "Invalid MCP request" })),
                    )
                        .into_response()
                }
            }
        }
        Protocol::A2a => {
            // Parse as A2A task send request
            match serde_json::from_slice::<TaskSendRequest>(&body_bytes) {
                Ok(task_request) => {
                    // Handle as A2A request
                    match handle_task_send(State(state), Json(task_request)).await {
                        Ok(response) => Json(response).into_response(),
                        Err(status) => status.into_response(),
                    }
                }
                Err(e) => {
                    error!("Failed to parse A2A request: {}", e);
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": "Invalid A2A request" })),
                    )
                        .into_response()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_server_config_default() {
        let config = UnifiedServerConfig::default();
        assert_eq!(config.address.port(), 3000);
        assert!(config.log_detection);
    }
}
