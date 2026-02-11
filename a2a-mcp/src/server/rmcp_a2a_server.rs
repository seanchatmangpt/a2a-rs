//! Server that exposes RMCP tools as an A2A agent
//!
//! This server bridges RMCP tools to the A2A protocol, allowing A2A clients
//! to interact with RMCP tools through the A2A task interface.

use crate::adapter::{AxumSseStream, SseManager, SseManagerConfig, ToolToAgentAdapter};
use crate::error::{Error, Result};
use a2a_rs::{
    domain::core::message::{Message, Part, Role},
    AgentCard, Task, TaskState, TaskStatus,
};
use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    response::sse::Sse,
    routing::{get, post},
    response::IntoResponse,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};
use uuid::Uuid;

// Placeholder for RMCP server handler
// TODO: Integrate with actual rmcp::ServerHandler trait
type RmcpHandler = Arc<dyn std::any::Any + Send + Sync>;

// Shared application state
#[derive(Clone)]
struct AppState {
    rmcp_handler: RmcpHandler,
    adapter: Arc<ToolToAgentAdapter>,
    tasks: Arc<Mutex<HashMap<String, Task>>>,
    sse_manager: Arc<SseManager>,
}

/// A server that exposes RMCP tools as an A2A agent
pub struct RmcpA2aServer {
    rmcp_handler: RmcpHandler,
    adapter: Arc<ToolToAgentAdapter>,
    tasks: Arc<Mutex<HashMap<String, Task>>>,
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
    pub fn new<H: 'static + Send + Sync>(rmcp_handler: H, adapter: ToolToAgentAdapter) -> Self {
        let sse_manager = Arc::new(SseManager::new(SseManagerConfig::default()));

        Self {
            rmcp_handler: Arc::new(rmcp_handler),
            adapter: Arc::new(adapter),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            sse_manager,
        }
    }

    /// Create a new server with custom SSE configuration
    pub fn new_with_sse_config<H: 'static + Send + Sync>(
        rmcp_handler: H,
        adapter: ToolToAgentAdapter,
        sse_config: SseManagerConfig,
    ) -> Self {
        let sse_manager = Arc::new(SseManager::new(sse_config));

        Self {
            rmcp_handler: Arc::new(rmcp_handler),
            adapter: Arc::new(adapter),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            sse_manager,
        }
    }

    /// Start serving A2A requests
    pub async fn serve(&self, addr: SocketAddr) -> Result<()> {
        let state = AppState {
            rmcp_handler: self.rmcp_handler.clone(),
            adapter: self.adapter.clone(),
            tasks: self.tasks.clone(),
            sse_manager: self.sse_manager.clone(),
        };

        // Set up HTTP server for A2A protocol
        let app = Router::new()
            .route("/.well-known/agent-card", get(get_agent_card))
            .route("/tasks/send", post(handle_task_send))
            .route("/tasks/sendSubscribe", post(handle_task_send_subscribe))
            .route("/tasks/get", get(handle_task_get))
            .with_state(state);

        // Start server using Axum 0.7+ pattern
        info!("Starting A2A agent server on {}", addr);
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| Error::Server(format!("Failed to bind to address: {}", e)))?;

        axum::serve(listener, app)
            .await
            .map_err(|e| Error::Server(e.to_string()))
    }
}

// Route handlers
async fn get_agent_card(State(state): State<AppState>) -> Json<AgentCard> {
    Json(state.adapter.generate_agent_card())
}

#[axum::debug_handler]
async fn handle_task_send(
    State(state): State<AppState>,
    Json(request): Json<TaskSendRequest>,
) -> std::result::Result<Json<TaskSendResponse>, Error> {
    // Create new task or update existing task
    let task_id = request
        .task_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let message = request.message;

    // For new tasks, create entry in the task store
    let mut tasks = state.tasks.lock().await;
    if !tasks.contains_key(&task_id) {
        let context_id = format!("{}-ctx", task_id);
        let task = Task {
            id: task_id.clone(),
            context_id,
            status: TaskStatus {
                state: TaskState::Submitted,
                message: None,
                timestamp: Some(chrono::Utc::now()),
            },
            artifacts: None,
            history: Some(vec![message.clone()]),
            metadata: None,
            kind: "task".to_string(),
        };
        tasks.insert(task_id.clone(), task);
    } else {
        // Add message to existing task
        if let Some(task) = tasks.get_mut(&task_id) {
            if let Some(ref mut history) = task.history {
                history.push(message.clone());
            } else {
                task.history = Some(vec![message.clone()]);
            }
            task.status.state = TaskState::Working;
            task.status.message = None;
        }
    }

    let _task = tasks.get(&task_id).unwrap().clone();
    drop(tasks);

    // Process task with RMCP tools
    process_task(&state, &task_id).await?;

    // Return the updated task
    let tasks = state.tasks.lock().await;
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
) -> std::result::Result<Sse<AxumSseStream>, Error> {
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
    let mut tasks = state.tasks.lock().await;
    if !tasks.contains_key(&task_id) {
        let context_id = format!("{}-ctx", task_id);
        let task = Task {
            id: task_id.clone(),
            context_id,
            status: TaskStatus {
                state: TaskState::Submitted,
                message: None,
                timestamp: Some(chrono::Utc::now()),
            },
            artifacts: None,
            history: Some(vec![message.clone()]),
            metadata: None,
            kind: "task".to_string(),
        };
        tasks.insert(task_id.clone(), task.clone());

        // Publish initial task state to SSE stream
        let task_data = serde_json::to_value(&task)?;
        state
            .sse_manager
            .publish(&task_id, "task.created", task_data)?;
    } else {
        // Add message to existing task
        if let Some(task) = tasks.get_mut(&task_id) {
            if let Some(ref mut history) = task.history {
                history.push(message.clone());
            } else {
                task.history = Some(vec![message.clone()]);
            }
            task.status.state = TaskState::Working;
            task.status.message = None;

            // Publish update to SSE stream
            let task_data = serde_json::to_value(&task)?;
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
) -> std::result::Result<Json<Task>, Error> {
    let tasks = state.tasks.lock().await;
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
        let tasks = state.tasks.lock().await;
        tasks
            .get(task_id)
            .ok_or_else(|| Error::TaskNotFound(task_id.to_string()))?
            .clone()
    };

    // Extract the last user message
    let last_message = task
        .history
        .as_ref()
        .and_then(|h| h.iter().filter(|msg| msg.role == Role::User).last())
        .ok_or_else(|| Error::TaskProcessing("No user message found".into()))?;

    // Extract tool name and parameters from message
    let (tool_name, params) = state.adapter.extract_tool_call(last_message)?;

    // Update task status
    {
        let mut tasks = state.tasks.lock().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.status.state = TaskState::Working;
            task.status.message = None;
            task.status.timestamp = Some(chrono::Utc::now());
        }
    }

    // TODO: Call RMCP tool through proper handler interface
    // For now, simulate a tool response
    debug!("Would call RMCP tool: {} with params: {:?}", tool_name, params);

    // Simulate tool result
    let tool_result = serde_json::json!({
        "tool": tool_name,
        "result": "Tool call simulation - RMCP integration pending",
        "params": params,
    });

    // Create agent response message
    let agent_message = Message {
        role: Role::Agent,
        parts: vec![Part::Data {
            data: tool_result.as_object().unwrap_or(&Map::new()).clone(),
            metadata: None,
        }],
        message_id: Uuid::new_v4().to_string(),
        context_id: None,
        task_id: Some(task_id.to_string()),
        reference_task_ids: None,
        metadata: None,
        extensions: None,
        kind: "message".to_string(),
    };

    // Update task with response
    {
        let mut tasks = state.tasks.lock().await;
        if let Some(task) = tasks.get_mut(task_id) {
            if let Some(ref mut history) = task.history {
                history.push(agent_message);
            } else {
                task.history = Some(vec![agent_message]);
            }
            task.status.state = TaskState::Completed;
            task.status.message = None;
            task.status.timestamp = Some(chrono::Utc::now());
        }
    }

    info!("Task {} completed successfully", task_id);
    Ok(())
}

// Helper function to process a task with SSE streaming support
async fn process_task_with_streaming(state: &AppState, task_id: &str) -> Result<()> {
    // Get the task
    let task = {
        let tasks = state.tasks.lock().await;
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
        .history
        .as_ref()
        .and_then(|h| h.iter().filter(|msg| msg.role == Role::User).last())
        .ok_or_else(|| Error::TaskProcessing("No user message found".into()))?;

    // Extract tool name and parameters from message
    let (tool_name, params) = state.adapter.extract_tool_call(last_message)?;

    // Update task status
    {
        let mut tasks = state.tasks.lock().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.status.state = TaskState::Working;
            task.status.message = None;
            task.status.timestamp = Some(chrono::Utc::now());
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

    // TODO: Call RMCP tool through proper handler interface
    debug!("Would call RMCP tool: {} with params: {:?}", tool_name, params);

    // Simulate tool result
    let tool_result = serde_json::json!({
        "tool": tool_name,
        "result": "Tool call simulation - RMCP integration pending",
        "params": params,
    });

    // Publish tool response event
    let tool_response_data = serde_json::json!({
        "taskId": task_id,
        "tool": tool_name,
        "result": tool_result,
    });
    state
        .sse_manager
        .publish(task_id, "tool.response", tool_response_data)?;

    // Create agent response message
    let agent_message = Message {
        role: Role::Agent,
        parts: vec![Part::Data {
            data: tool_result.as_object().unwrap_or(&Map::new()).clone(),
            metadata: None,
        }],
        message_id: Uuid::new_v4().to_string(),
        context_id: None,
        task_id: Some(task_id.to_string()),
        reference_task_ids: None,
        metadata: None,
        extensions: None,
        kind: "message".to_string(),
    };

    // Update task with response
    let updated_task = {
        let mut tasks = state.tasks.lock().await;
        if let Some(task) = tasks.get_mut(task_id) {
            if let Some(ref mut history) = task.history {
                history.push(agent_message);
            } else {
                task.history = Some(vec![agent_message]);
            }
            task.status.state = TaskState::Completed;
            task.status.message = None;
            task.status.timestamp = Some(chrono::Utc::now());
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
