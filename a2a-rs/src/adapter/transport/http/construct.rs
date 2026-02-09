//! HTTP transport adapter for CONSTRUCT Runtime execution
//!
//! This adapter provides typed packet deserialization and integrates the
//! CONSTRUCT Runtime with HTTP transports for Station-based execution.

use std::sync::Arc;
use tokio::sync::RwLock;

use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use serde_json::Value;

#[cfg(feature = "tracing")]
use tracing::{debug, info, instrument};

use crate::{
    construct::{
        runtime::{Operation, PriorityClass, Runtime, RuntimeOutput},
        types::{
            CancelTaskRequest, CancelTaskResponse, GetExtendedCardRequest, GetExtendedCardResponse,
            GetTaskRequest, GetTaskResponse, ListTasksRequest, ListTasksResponse, PacketType,
            SendMessageRequest, SendMessageResponse, SendTaskRequest,
        },
    },
    domain::{A2AError, AgentCard, Task},
};

#[cfg(feature = "receipts")]
use crate::construct::receipts::ReceiptChain;

/// HTTP Server with CONSTRUCT Runtime integration
pub struct HttpConstructServer {
    /// The CONSTRUCT Runtime
    runtime: Arc<RwLock<Runtime>>,
    /// Server address
    address: String,
    /// Agent card for extended card endpoint
    agent_card: Arc<RwLock<Option<AgentCard>>>,
}

impl HttpConstructServer {
    /// Create a new HTTP CONSTRUCT server
    pub fn new(runtime: Runtime, address: String) -> Self {
        Self {
            runtime: Arc::new(RwLock::new(runtime)),
            address,
            agent_card: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the agent card
    pub async fn set_agent_card(&self, card: AgentCard) {
        *self.agent_card.write().await = Some(card);
    }

    /// Get the current receipt chain
    #[cfg(feature = "receipts")]
    pub async fn get_receipts(&self) -> Option<ReceiptChain> {
        // Receipts would be maintained by the Runtime internally
        // This is a placeholder for accessing them
        None
    }

    /// Start the HTTP server
    #[cfg_attr(feature = "tracing", instrument(skip(self), fields(
        server.address = %self.address,
    )))]
    pub async fn start(&self) -> Result<(), A2AError> {
        #[cfg(feature = "tracing")]
        info!("Starting HTTP CONSTRUCT server");

        let runtime = self.runtime.clone();
        let agent_card = self.agent_card.clone();

        let app = Router::new()
            .route("/", post(handle_construct_request))
            .route(
                "/.well-known/agent-card.json",
                axum::routing::get(handle_agent_card),
            )
            .with_state(ServerState {
                runtime,
                agent_card,
            });

        let listener = tokio::net::TcpListener::bind(&self.address)
            .await
            .map_err(|e| {
                A2AError::Internal(format!("Failed to bind to {}: {}", self.address, e))
            })?;

        #[cfg(feature = "tracing")]
        info!("HTTP CONSTRUCT server listening on {}", self.address);

        axum::serve(listener, app.into_make_service())
            .await
            .map_err(|e| A2AError::Internal(format!("Server error: {}", e)))?;

        Ok(())
    }
}

/// Server state for axum
#[derive(Clone)]
struct ServerState {
    runtime: Arc<RwLock<Runtime>>,
    agent_card: Arc<RwLock<Option<AgentCard>>>,
}

/// Handle a typed CONSTRUCT request
#[cfg_attr(feature = "tracing", instrument(skip(state), fields(
    request.method = %request.get("method").and_then(|v| v.as_str()).unwrap_or("unknown")
)))]
async fn handle_construct_request(
    State(state): State<ServerState>,
    Json(request): Json<Value>,
) -> impl IntoResponse {
    #[cfg(feature = "tracing")]
    debug!("Processing CONSTRUCT request");

    // Extract method to determine packet type
    let method = match request.get("method").and_then(|m| m.as_str()) {
        Some(m) => m,
        None => {
            return create_error_response(None, -32600, "Invalid Request", "Missing method field")
                .into_response();
        }
    };

    let packet_type = match PacketType::from_method(method) {
        Some(pt) => pt,
        None => {
            return create_error_response(
                request.get("id"),
                -32601,
                "Method not found",
                &format!("Unknown method: {}", method),
            )
            .into_response();
        }
    };

    #[cfg(feature = "tracing")]
    debug!("Packet type: {:?}", packet_type);

    // Convert request to typed packet and then to Operation
    let operation_result: Result<(Operation, Option<crate::construct::types::JsonRpcId>), String> =
        match packet_type {
            PacketType::SendMessage => {
                let typed_request: SendMessageRequest =
                    match serde_json::from_value(request.clone()) {
                        Ok(r) => r,
                        Err(e) => {
                            return create_error_response(
                                request.get("id"),
                                -32700,
                                "Parse error",
                                &e.to_string(),
                            )
                            .into_response();
                        }
                    };

                // MessageSendParams doesn't have task_id - need to extract from message context
                // For now, return error as this needs proper implementation
                return create_error_response(
                    request.get("id"),
                    -32603,
                    "Internal error",
                    "SendMessage not yet implemented in CONSTRUCT runtime",
                )
                .into_response();
            }
            PacketType::SendTask => {
                let typed_request: SendTaskRequest = match serde_json::from_value(request.clone()) {
                    Ok(r) => r,
                    Err(e) => {
                        return create_error_response(
                            request.get("id"),
                            -32700,
                            "Parse error",
                            &e.to_string(),
                        )
                        .into_response();
                    }
                };

                // TaskSendParams has id, message - construct a Task from these
                let task = Task::builder()
                    .id(typed_request.params.id.clone())
                    .context_id(
                        typed_request
                            .params
                            .session_id
                            .clone()
                            .unwrap_or_else(|| "default-session".to_string()),
                    )
                    .status(crate::domain::TaskStatus::default())
                    .build();

                Ok((
                    Operation::CreateTask {
                        task,
                        initial_message: Some(typed_request.params.message.clone()),
                        priority: Some(PriorityClass::Normal),
                    },
                    typed_request.id,
                ))
            }
            PacketType::GetTask => {
                let typed_request: GetTaskRequest = match serde_json::from_value(request.clone()) {
                    Ok(r) => r,
                    Err(e) => {
                        return create_error_response(
                            request.get("id"),
                            -32700,
                            "Parse error",
                            &e.to_string(),
                        )
                        .into_response();
                    }
                };

                // GetTask is read-only, doesn't need an Operation
                return handle_get_task(state, typed_request).await.into_response();
            }
            PacketType::CancelTask => {
                let typed_request: CancelTaskRequest = match serde_json::from_value(request.clone())
                {
                    Ok(r) => r,
                    Err(e) => {
                        return create_error_response(
                            request.get("id"),
                            -32700,
                            "Parse error",
                            &e.to_string(),
                        )
                        .into_response();
                    }
                };

                Ok((
                    Operation::CancelTask {
                        task_id: typed_request.params.id.clone(),
                    },
                    typed_request.id,
                ))
            }
            PacketType::ListTasks => {
                let typed_request: ListTasksRequest = match serde_json::from_value(request.clone())
                {
                    Ok(r) => r,
                    Err(e) => {
                        return create_error_response(
                            request.get("id"),
                            -32700,
                            "Parse error",
                            &e.to_string(),
                        )
                        .into_response();
                    }
                };

                // ListTasks is read-only
                return handle_list_tasks(state, typed_request)
                    .await
                    .into_response();
            }
            PacketType::GetExtendedCard => {
                let typed_request: GetExtendedCardRequest =
                    match serde_json::from_value(request.clone()) {
                        Ok(r) => r,
                        Err(e) => {
                            return create_error_response(
                                request.get("id"),
                                -32700,
                                "Parse error",
                                &e.to_string(),
                            )
                            .into_response();
                        }
                    };

                return handle_get_extended_card(state, typed_request)
                    .await
                    .into_response();
            }
            _ => {
                return create_error_response(
                    request.get("id"),
                    -32601,
                    "Method not found",
                    &format!("Unsupported packet type: {:?}", packet_type),
                )
                .into_response();
            }
        };

    let (operation, request_id) = match operation_result {
        Ok(op) => op,
        Err(e) => {
            return create_error_response(
                request.get("id"),
                -32603,
                "Internal error",
                &format!("Operation conversion failed: {}", e),
            )
            .into_response();
        }
    };

    // Execute via Runtime
    let output = {
        let mut runtime = state.runtime.write().await;
        match runtime.handle(operation) {
            Ok(output) => output,
            Err(e) => {
                let id_value = request_id
                    .as_ref()
                    .map(|id| serde_json::to_value(id).unwrap_or(Value::Null));
                return create_error_response(
                    id_value.as_ref(),
                    -32603,
                    "Internal error",
                    &e.to_string(),
                )
                .into_response();
            }
        }
    };

    // Convert RuntimeOutput to typed response
    convert_output_to_response(output, request_id, packet_type).into_response()
}

/// Handle get task request (read-only)
async fn handle_get_task(state: ServerState, request: GetTaskRequest) -> impl IntoResponse {
    let runtime = state.runtime.read().await;
    let task = runtime.ontology().get_task(&request.params.id);

    let response = GetTaskResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id,
        result: task.cloned(),
        error: None,
    };

    (StatusCode::OK, Json(response))
}

/// Handle list tasks request (read-only)
async fn handle_list_tasks(state: ServerState, request: ListTasksRequest) -> impl IntoResponse {
    let runtime = state.runtime.read().await;

    // Simple implementation - list all tasks
    // In production, this would support filtering and pagination
    let tasks: Vec<Task> = runtime
        .ontology()
        .get_all_tasks()
        .into_iter()
        .cloned()
        .collect();

    let total_size = tasks.len() as i32;
    let response = ListTasksResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id,
        result: Some(crate::domain::ListTasksResult {
            tasks,
            total_size,
            page_size: total_size,
            next_page_token: String::new(),
        }),
        error: None,
    };

    (StatusCode::OK, Json(response))
}

/// Handle get extended card request
async fn handle_get_extended_card(
    state: ServerState,
    request: GetExtendedCardRequest,
) -> impl IntoResponse {
    let card = state.agent_card.read().await.clone();

    let response = GetExtendedCardResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id,
        result: card,
        error: None,
    };

    (StatusCode::OK, Json(response))
}

/// Handle agent card endpoint
async fn handle_agent_card(State(state): State<ServerState>) -> impl IntoResponse {
    let card = state.agent_card.read().await.clone();

    match card {
        Some(c) => (StatusCode::OK, Json(c)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Agent card not configured"})),
        )
            .into_response(),
    }
}

/// Convert RuntimeOutput to typed JSON-RPC response
fn convert_output_to_response(
    output: RuntimeOutput,
    request_id: Option<crate::construct::types::JsonRpcId>,
    packet_type: PacketType,
) -> impl IntoResponse {
    // If there were errors, return error response
    if !output.errors.is_empty() {
        let error = &output.errors[0];
        let id_value = request_id
            .as_ref()
            .map(|id| serde_json::to_value(id).unwrap_or(Value::Null));
        return create_error_response(
            id_value.as_ref(),
            -32603,
            "Internal error",
            &error.to_string(),
        )
        .into_response();
    }

    // Extract task from output
    let task = output.tasks.first().cloned();

    match packet_type {
        PacketType::SendMessage | PacketType::SendTask => {
            let response = SendMessageResponse {
                jsonrpc: "2.0".to_string(),
                id: request_id,
                result: task,
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        PacketType::CancelTask => {
            let response = CancelTaskResponse {
                jsonrpc: "2.0".to_string(),
                id: request_id,
                result: task,
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        _ => {
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": task,
            });
            (StatusCode::OK, Json(response)).into_response()
        }
    }
}

/// Create a JSON-RPC error response
fn create_error_response(
    id: Option<&Value>,
    code: i32,
    message: &str,
    data: &str,
) -> (StatusCode, Json<Value>) {
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
            "data": data,
        }
    });

    (StatusCode::OK, Json(response))
}
