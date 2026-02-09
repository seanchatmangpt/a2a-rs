//! WebSocket transport adapter for CONSTRUCT Runtime execution
//!
//! This adapter provides typed packet deserialization over WebSocket
//! and integrates the CONSTRUCT Runtime for Station-based execution.

use std::sync::Arc;
use tokio::sync::RwLock;

use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{Error as WsError, Message as WsMessage},
};

#[cfg(feature = "tracing")]
use tracing::{debug, error, info, instrument, warn};

use crate::{
    construct::{
        runtime::{Operation, PriorityClass, Runtime, RuntimeOutput},
        types::{
            CancelTaskRequest, CancelTaskResponse, GetExtendedCardRequest, GetExtendedCardResponse,
            GetTaskRequest, GetTaskResponse, JsonRpcId, ListTasksRequest, ListTasksResponse,
            PacketType, SendMessageRequest, SendMessageResponse, SendTaskRequest,
        },
    },
    domain::{A2AError, AgentCard, Message, Task},
};

#[cfg(feature = "receipts")]
use crate::construct::receipts::ReceiptChain;

/// WebSocket Server with CONSTRUCT Runtime integration
pub struct WebSocketConstructServer {
    /// The CONSTRUCT Runtime
    runtime: Arc<RwLock<Runtime>>,
    /// Server address
    address: String,
    /// Agent card
    agent_card: Arc<RwLock<Option<AgentCard>>>,
}

impl WebSocketConstructServer {
    /// Create a new WebSocket CONSTRUCT server
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
        None
    }

    /// Start the WebSocket server
    #[cfg_attr(feature = "tracing", instrument(skip(self), fields(
        server.address = %self.address,
    )))]
    pub async fn start(&self) -> Result<(), A2AError> {
        #[cfg(feature = "tracing")]
        info!("Starting WebSocket CONSTRUCT server");

        let listener = TcpListener::bind(&self.address).await.map_err(|e| {
            A2AError::Internal(format!("Failed to bind to {}: {}", self.address, e))
        })?;

        #[cfg(feature = "tracing")]
        info!("WebSocket CONSTRUCT server listening on {}", self.address);

        loop {
            let (stream, addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    #[cfg(feature = "tracing")]
                    error!("Failed to accept connection: {}", e);
                    continue;
                }
            };

            #[cfg(feature = "tracing")]
            info!("New connection from {}", addr);

            let runtime = self.runtime.clone();
            let agent_card = self.agent_card.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, runtime, agent_card).await {
                    #[cfg(feature = "tracing")]
                    error!("Connection error: {}", e);
                }
            });
        }
    }
}

/// Handle a WebSocket connection
async fn handle_connection(
    stream: TcpStream,
    runtime: Arc<RwLock<Runtime>>,
    agent_card: Arc<RwLock<Option<AgentCard>>>,
) -> Result<(), A2AError> {
    let ws_stream = accept_async(stream)
        .await
        .map_err(|e| A2AError::Internal(format!("WebSocket handshake failed: {}", e)))?;

    #[cfg(feature = "tracing")]
    debug!("WebSocket connection established");

    let (mut write, mut read) = ws_stream.split();

    while let Some(msg) = read.next().await {
        let msg: WsMessage = match msg {
            Ok(m) => m,
            Err(e) => {
                #[cfg(feature = "tracing")]
                error!("Failed to read message: {}", e);
                break;
            }
        };

        if msg.is_close() {
            #[cfg(feature = "tracing")]
            debug!("Client closed connection");
            break;
        }

        if msg.is_text() || msg.is_binary() {
            let text = msg.to_text().map_err(|e| {
                A2AError::Internal(format!("Failed to convert message to text: {}", e))
            })?;

            #[cfg(feature = "tracing")]
            debug!("Received message: {}", text);

            // Process the request
            let response = process_request(text, &runtime, &agent_card).await;

            // Send response
            let response_msg = WsMessage::Text(response);
            if let Err(e) = write.send(response_msg).await {
                #[cfg(feature = "tracing")]
                error!("Failed to send response: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Process a single WebSocket request
async fn process_request(
    raw_request: &str,
    runtime: &Arc<RwLock<Runtime>>,
    agent_card: &Arc<RwLock<Option<AgentCard>>>,
) -> String {
    // Parse the request to extract method
    let request: serde_json::Value = match serde_json::from_str(raw_request) {
        Ok(r) => r,
        Err(e) => {
            return create_error_response(None, -32700, "Parse error", &e.to_string());
        }
    };

    let method = match request.get("method").and_then(|m| m.as_str()) {
        Some(m) => m,
        None => {
            return create_error_response(
                request.get("id"),
                -32600,
                "Invalid Request",
                "Missing method field",
            );
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
            );
        }
    };

    #[cfg(feature = "tracing")]
    debug!("Processing packet type: {:?}", packet_type);

    // Convert to typed packet and Operation
    let operation_result: Result<(Operation, Option<JsonRpcId>), String> = match packet_type {
        PacketType::SendMessage => {
            let typed_request: SendMessageRequest = match serde_json::from_value(request.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return create_error_response(
                        request.get("id"),
                        -32700,
                        "Parse error",
                        &e.to_string(),
                    );
                }
            };

            Ok((
                Operation::SendMessage {
                    task_id: typed_request
                        .params
                        .message
                        .task_id
                        .clone()
                        .unwrap_or_else(|| format!("task-{}", uuid::Uuid::new_v4())),
                    message: typed_request.params.message.clone(),
                },
                typed_request.id,
            ))
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
                    );
                }
            };

            // Create a task from TaskSendParams
            let context_id = typed_request
                .params
                .session_id
                .clone()
                .unwrap_or_else(|| format!("ctx-{}", uuid::Uuid::new_v4()));
            let task = Task::builder()
                .id(typed_request.params.id.clone())
                .context_id(context_id)
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
                    );
                }
            };

            // GetTask is read-only
            return handle_get_task(runtime, typed_request).await;
        }
        PacketType::CancelTask => {
            let typed_request: CancelTaskRequest = match serde_json::from_value(request.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return create_error_response(
                        request.get("id"),
                        -32700,
                        "Parse error",
                        &e.to_string(),
                    );
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
            let typed_request: ListTasksRequest = match serde_json::from_value(request.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return create_error_response(
                        request.get("id"),
                        -32700,
                        "Parse error",
                        &e.to_string(),
                    );
                }
            };

            return handle_list_tasks(runtime, typed_request).await;
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
                        );
                    }
                };

            return handle_get_extended_card(agent_card, typed_request).await;
        }
        _ => {
            return create_error_response(
                request.get("id"),
                -32601,
                "Method not found",
                &format!("Unsupported packet type: {:?}", packet_type),
            );
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
            );
        }
    };

    // Execute via Runtime
    let output = {
        let mut runtime = runtime.write().await;
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
                );
            }
        }
    };

    // Convert RuntimeOutput to typed response
    convert_output_to_response(output, request_id, packet_type)
}

/// Handle get task request (read-only)
async fn handle_get_task(runtime: &Arc<RwLock<Runtime>>, request: GetTaskRequest) -> String {
    let runtime = runtime.read().await;
    let task = runtime.ontology().get_task(&request.params.id);

    let response = GetTaskResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id,
        result: task.cloned(),
        error: None,
    };

    serde_json::to_string(&response).unwrap_or_else(|e| {
        create_error_response(
            None,
            -32603,
            "Internal error",
            &format!("Failed to serialize response: {}", e),
        )
    })
}

/// Handle list tasks request (read-only)
async fn handle_list_tasks(runtime: &Arc<RwLock<Runtime>>, request: ListTasksRequest) -> String {
    let runtime = runtime.read().await;
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

    serde_json::to_string(&response).unwrap_or_else(|e| {
        create_error_response(
            None,
            -32603,
            "Internal error",
            &format!("Failed to serialize response: {}", e),
        )
    })
}

/// Handle get extended card request
async fn handle_get_extended_card(
    agent_card: &Arc<RwLock<Option<AgentCard>>>,
    request: GetExtendedCardRequest,
) -> String {
    let card = agent_card.read().await.clone();

    let response = GetExtendedCardResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id,
        result: card,
        error: None,
    };

    serde_json::to_string(&response).unwrap_or_else(|e| {
        create_error_response(
            None,
            -32603,
            "Internal error",
            &format!("Failed to serialize response: {}", e),
        )
    })
}

/// Convert RuntimeOutput to typed JSON-RPC response string
fn convert_output_to_response(
    output: RuntimeOutput,
    request_id: Option<JsonRpcId>,
    packet_type: PacketType,
) -> String {
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
        );
    }

    // Extract task from output
    let task = output.tasks.first().cloned();

    let response = match packet_type {
        PacketType::SendMessage | PacketType::SendTask => {
            serde_json::to_value(SendMessageResponse {
                jsonrpc: "2.0".to_string(),
                id: request_id,
                result: task,
                error: None,
            })
        }
        PacketType::CancelTask => serde_json::to_value(CancelTaskResponse {
            jsonrpc: "2.0".to_string(),
            id: request_id,
            result: task,
            error: None,
        }),
        _ => Ok(serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": task,
        })),
    };

    match response {
        Ok(r) => serde_json::to_string(&r).unwrap_or_else(|e| {
            create_error_response(
                None,
                -32603,
                "Internal error",
                &format!("Failed to serialize response: {}", e),
            )
        }),
        Err(e) => create_error_response(
            None,
            -32603,
            "Internal error",
            &format!("Failed to create response: {}", e),
        ),
    }
}

/// Create a JSON-RPC error response string
fn create_error_response(
    id: Option<&serde_json::Value>,
    code: i32,
    message: &str,
    data: &str,
) -> String {
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
            "data": data,
        }
    });

    serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"Internal error","data":"Failed to serialize error"}}"#.to_string()
    })
}
