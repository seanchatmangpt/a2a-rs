//! Server that exposes RMCP tools as an A2A agent

use crate::adapter::{AxumSseStream, SseManager, SseManagerConfig, ToolToAgentAdapter};
use crate::error::{Error, Result};
use crate::transport::rmcp_to_a2a::RmcpToA2aTransport;
use a2a_rs::{AgentCard, Message, Part, Task, TaskState, TaskStatus};
use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    response::sse::Sse,
    routing::{get, post},
};
use rmcp::{Server as RmcpServer, ToolCall, ToolResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info};
use uuid::Uuid;

// Shared application state
#[derive(Clone)]
struct AppState {
    rmcp_server: Arc<RmcpServer>,
    adapter: Arc<ToolToAgentAdapter>,
    tasks: Arc<Mutex<HashMap<String, Task>>>,
    transport: Arc<RmcpToA2aTransport>,
    sse_manager: Arc<SseManager>,
}

/// A server that exposes RMCP tools as an A2A agent
pub struct RmcpA2aServer {
    rmcp_server: Arc<RmcpServer>,
    adapter: Arc<ToolToAgentAdapter>,
    tasks: Arc<Mutex<HashMap<String, Task>>>,
    transport: Arc<RmcpToA2aTransport>,
    sse_manager: Arc<SseManager>,
}

#[derive(Debug, Deserialize)]
struct TaskSendRequest {
    task_id: Option<String>,
    message: Message,
}

#[derive(Debug, Serialize)]
struct TaskSendResponse {
    task: Task,
}

#[derive(Debug, Deserialize)]
struct TaskGetRequest {
    task_id: String,
}

impl RmcpA2aServer {
    /// Create a new server that wraps an RMCP server
    pub fn new(rmcp_server: RmcpServer, adapter: ToolToAgentAdapter) -> Self {
        let converter = Arc::new(crate::message::MessageConverter::new());
        let sse_manager = Arc::new(SseManager::new(SseManagerConfig::default()));

        Self {
            rmcp_server: Arc::new(rmcp_server),
            adapter: Arc::new(adapter),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            transport: Arc::new(RmcpToA2aTransport::new(converter)),
            sse_manager,
        }
    }

    /// Create a new server with custom SSE configuration
    pub fn new_with_sse_config(
        rmcp_server: RmcpServer,
        adapter: ToolToAgentAdapter,
        sse_config: SseManagerConfig,
    ) -> Self {
        let converter = Arc::new(crate::message::MessageConverter::new());
        let sse_manager = Arc::new(SseManager::new(sse_config));

        Self {
            rmcp_server: Arc::new(rmcp_server),
            adapter: Arc::new(adapter),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            transport: Arc::new(RmcpToA2aTransport::new(converter)),
            sse_manager,
        }
    }

    /// Start serving A2A requests
    pub async fn serve(&self, addr: SocketAddr) -> Result<()> {
        let state = AppState {
            rmcp_server: self.rmcp_server.clone(),
            adapter: self.adapter.clone(),
            tasks: self.tasks.clone(),
            transport: self.transport.clone(),
            sse_manager: self.sse_manager.clone(),
        };

        // Set up HTTP server for A2A protocol
        let app = Router::new()
            .route("/.well-known/agent-card", get(get_agent_card))
            .route("/tasks/send", post(handle_task_send))
            .route("/tasks/sendSubscribe", post(handle_task_send_subscribe))
            .route("/tasks/get", get(handle_task_get))
            .with_state(state);

        // Start the server
        info!("Starting A2A agent server on {}", addr);
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await
            .map_err(|e| Error::Server(e.to_string()))
    }
}

// Route handlers
async fn get_agent_card(State(state): State<AppState>) -> Json<AgentCard> {
    Json(state.adapter.generate_agent_card())
}

async fn handle_task_send(
    State(state): State<AppState>,
    Json(request): Json<TaskSendRequest>,
) -> Result<Json<TaskSendResponse>> {
    // Create new task or update existing task
    let task_id = request
        .task_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let message = request.message;

    // For new tasks, create entry in the task store
    let mut tasks = state.tasks.lock().unwrap();
    if !tasks.contains_key(&task_id) {
        let task = Task {
            id: task_id.clone(),
            status: TaskStatus {
                state: TaskState::Submitted,
                message: Some("Task submitted".to_string()),
            },
            messages: vec![message.clone()],
            artifacts: Vec::new(),
            history_ttl: Some(3600), // 1 hour default
            metadata: None,
        };
        tasks.insert(task_id.clone(), task);
    } else {
        // Add message to existing task
        if let Some(task) = tasks.get_mut(&task_id) {
            task.messages.push(message.clone());
            task.status.state = TaskState::Working;
            task.status.message = Some("Processing input".to_string());
        }
    }

    let task = tasks.get(&task_id).unwrap().clone();
    drop(tasks);

    // Process task with RMCP tools
    process_task(&state, &task_id).await?;

    // Return the updated task
    let tasks = state.tasks.lock().unwrap();
    let updated_task = tasks
        .get(&task_id)
        .ok_or_else(|| Error::TaskNotFound(task_id.clone()))?
        .clone();

    Ok(Json(TaskSendResponse { task: updated_task }))
}

async fn handle_task_send_subscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TaskSendRequest>,
) -> Result<Sse<AxumSseStream>> {
    // Create new task or update existing task
    let task_id = request
        .task_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let message = request.message;

    info!("Setting up SSE stream for task: {}", task_id);

    // Initialize SSE stream for this task
    state.sse_manager.init_stream(&task_id)?;

    // Check for Last-Event-ID header for resumption
    let last_event_id = headers
        .get("Last-Event-ID")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    if let Some(ref last_id) = last_event_id {
        info!("Resuming SSE stream from event ID: {}", last_id);
    }

    // For new tasks, create entry in the task store
    let mut tasks = state.tasks.lock().unwrap();
    if !tasks.contains_key(&task_id) {
        let task = Task {
            id: task_id.clone(),
            status: TaskStatus {
                state: TaskState::Submitted,
                message: Some("Task submitted".to_string()),
            },
            messages: vec![message.clone()],
            artifacts: Vec::new(),
            history_ttl: Some(3600), // 1 hour default
            metadata: None,
        };
        tasks.insert(task_id.clone(), task.clone());

        // Publish initial task state to SSE stream
        let task_data = serde_json::to_value(&task).map_err(|e| Error::Json(e))?;
        state
            .sse_manager
            .publish(&task_id, "task.created", task_data)?;
    } else {
        // Add message to existing task
        if let Some(task) = tasks.get_mut(&task_id) {
            task.messages.push(message.clone());
            task.status.state = TaskState::Working;
            task.status.message = Some("Processing input".to_string());

            // Publish update to SSE stream
            let task_data = serde_json::to_value(&task).map_err(|e| Error::Json(e))?;
            state
                .sse_manager
                .publish(&task_id, "task.updated", task_data)?;
        }
    }
    drop(tasks);

    // Clone state for async task processing
    let state_clone = state.clone();
    let task_id_clone = task_id.clone();

    // Process task asynchronously and publish events to SSE stream
    tokio::spawn(async move {
        if let Err(e) = process_task_with_streaming(&state_clone, &task_id_clone).await {
            error!("Error processing task {}: {}", task_id_clone, e);

            // Publish error event
            let error_data = serde_json::json!({
                "error": e.to_string(),
                "taskId": task_id_clone,
            });
            let _ = state_clone
                .sse_manager
                .publish(&task_id_clone, "task.error", error_data);
        }
    });

    // Subscribe to SSE stream with optional resume
    let stream = state
        .sse_manager
        .subscribe(&task_id, last_event_id.as_deref())?;
    let axum_stream = AxumSseStream::new(stream);

    Ok(Sse::new(axum_stream))
}

async fn handle_task_get(
    State(state): State<AppState>,
    Json(request): Json<TaskGetRequest>,
) -> Result<Json<Task>> {
    let tasks = state.tasks.lock().unwrap();
    let task = tasks
        .get(&request.task_id)
        .ok_or_else(|| Error::TaskNotFound(request.task_id.clone()))?
        .clone();

    Ok(Json(task))
}

// Helper function to process a task using RMCP tools
async fn process_task(state: &AppState, task_id: &str) -> Result<()> {
    // Get the task
    let task = {
        let tasks = state.tasks.lock().unwrap();
        tasks
            .get(task_id)
            .ok_or_else(|| Error::TaskNotFound(task_id.to_string()))?
            .clone()
    };

    // Extract the last user message
    let last_message = task
        .messages
        .iter()
        .filter(|msg| msg.role == "user")
        .last()
        .ok_or_else(|| Error::TaskProcessing("No user message found".into()))?;

    // Extract tool name and parameters from message
    let (tool_name, params) = state.adapter.extract_tool_call(last_message)?;

    // Update task status
    {
        let mut tasks = state.tasks.lock().unwrap();
        if let Some(task) = tasks.get_mut(task_id) {
            task.status.state = TaskState::Working;
            task.status.message = Some(format!("Calling tool: {}", tool_name));
        }
    }

    // Call RMCP tool
    let tool_call = ToolCall {
        method: tool_name.clone(),
        params,
    };

    debug!("Calling RMCP tool: {}", tool_name);
    let tool_response = state
        .rmcp_server
        .call_tool(tool_call)
        .await
        .map_err(|e| Error::RmcpToolCall(format!("Error calling tool {}: {}", tool_name, e)))?;

    // Create agent response message
    let agent_message = Message {
        role: "agent".to_string(),
        parts: vec![Part::Data {
            data: tool_response.result.clone(),
            mime_type: Some("application/json".to_string()),
        }],
    };

    // Update task with response
    {
        let mut tasks = state.tasks.lock().unwrap();
        if let Some(task) = tasks.get_mut(task_id) {
            task.messages.push(agent_message);
            task.status.state = TaskState::Completed;
            task.status.message = Some("Task completed".to_string());
        }
    }

    info!("Task {} completed successfully", task_id);
    Ok(())
}

// Helper function to process a task with SSE streaming support
async fn process_task_with_streaming(state: &AppState, task_id: &str) -> Result<()> {
    // Get the task
    let task = {
        let tasks = state.tasks.lock().unwrap();
        tasks
            .get(task_id)
            .ok_or_else(|| Error::TaskNotFound(task_id.to_string()))?
            .clone()
    };

    // Publish status update: starting
    let status_data = serde_json::json!({
        "taskId": task_id,
        "state": "working",
        "message": "Processing task",
    });
    state
        .sse_manager
        .publish(task_id, "task.status", status_data)?;

    // Extract the last user message
    let last_message = task
        .messages
        .iter()
        .filter(|msg| msg.role == "user")
        .last()
        .ok_or_else(|| Error::TaskProcessing("No user message found".into()))?;

    // Extract tool name and parameters from message
    let (tool_name, params) = state.adapter.extract_tool_call(last_message)?;

    // Update task status
    {
        let mut tasks = state.tasks.lock().unwrap();
        if let Some(task) = tasks.get_mut(task_id) {
            task.status.state = TaskState::Working;
            task.status.message = Some(format!("Calling tool: {}", tool_name));
        }
    }

    // Publish status update: calling tool
    let tool_status_data = serde_json::json!({
        "taskId": task_id,
        "state": "working",
        "message": format!("Calling tool: {}", tool_name),
        "tool": tool_name,
    });
    state
        .sse_manager
        .publish(task_id, "task.status", tool_status_data)?;

    // Call RMCP tool
    let tool_call = ToolCall {
        method: tool_name.clone(),
        params,
    };

    debug!("Calling RMCP tool: {}", tool_name);
    let tool_response = state
        .rmcp_server
        .call_tool(tool_call)
        .await
        .map_err(|e| Error::RmcpToolCall(format!("Error calling tool {}: {}", tool_name, e)))?;

    // Publish tool response event
    let tool_response_data = serde_json::json!({
        "taskId": task_id,
        "tool": tool_name,
        "result": tool_response.result,
    });
    state
        .sse_manager
        .publish(task_id, "tool.response", tool_response_data)?;

    // Create agent response message
    let agent_message = Message {
        role: "agent".to_string(),
        parts: vec![Part::Data {
            data: tool_response.result.clone(),
            mime_type: Some("application/json".to_string()),
        }],
    };

    // Update task with response
    let updated_task = {
        let mut tasks = state.tasks.lock().unwrap();
        if let Some(task) = tasks.get_mut(task_id) {
            task.messages.push(agent_message);
            task.status.state = TaskState::Completed;
            task.status.message = Some("Task completed".to_string());
            task.clone()
        } else {
            return Err(Error::TaskNotFound(task_id.to_string()));
        }
    };

    // Publish final task completion event
    let completion_data = serde_json::to_value(&updated_task).map_err(|e| Error::Json(e))?;
    state
        .sse_manager
        .publish(task_id, "task.completed", completion_data)?;

    info!("Task {} completed successfully", task_id);
    Ok(())
}
