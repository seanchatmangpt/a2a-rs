//! Station trait: Agents as deterministic packet processors
//!
//! This module implements the core Station abstraction. Stations are NOT AI systems -
//! they are deterministic finite state machines that execute typed packet transformations.
//!
//! # Core Concept
//!
//! A **Station** processes packets through two phases:
//!
//! 1. **Admission (`admit`)**: Type-safe validation without state mutation
//! 2. **Stepping (`step`)**: Deterministic state transition with side effects
//!
//! # Design Principles
//!
//! - **No `serde_json::Value` at boundaries**: All inputs/outputs are strongly typed
//! - **Deterministic transitions**: Same input + state → same output (no LLM calls)
//! - **Ontology-aware**: Stations operate on `OntologyState`, not opaque databases
//! - **Receipt-based errors**: Refusals are typed, serializable, and auditable
//! - **Composable**: Stations can delegate to other stations
//!
//! # Example
//!
//! ```rust
//! use a2a_rs::construct::station::{Station, GetTaskStation};
//! use a2a_rs::construct::ontology::OntologyState;
//! use a2a_rs::construct::types::GetTaskRequest;
//! use a2a_rs::construct::types::JsonRpcId;
//!
//! let mut ontology = OntologyState::new();
//! let mut station = GetTaskStation;
//!
//! let request = GetTaskRequest {
//!     jsonrpc: "2.0".to_string(),
//!     id: Some(JsonRpcId::new_uuid()),
//!     method: "tasks/get".to_string(),
//!     params: a2a_rs::domain::TaskIdParams {
//!         task_id: "task-123".to_string(),
//!     },
//! };
//!
//! // Admission check before processing
//! if let Err(refusal) = GetTaskStation::admit(&ontology, &request) {
//!     println!("Admission refused: {}", refusal.reason);
//!     return;
//! }
//!
//! // Execute deterministic transition
//! match station.step(&mut ontology, request) {
//!     Ok(response) => println!("Task retrieved: {:?}", response.result),
//!     Err(refusal) => println!("Step failed: {}", refusal.reason),
//! }
//! ```

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;

use crate::construct::ontology::OntologyState;
use crate::construct::types::{
    CancelTaskRequest, CancelTaskResponse, GetExtendedCardRequest, GetExtendedCardResponse,
    GetTaskRequest, GetTaskResponse, JsonRpcError, ListTasksRequest, ListTasksResponse,
    SendMessageRequest, SendMessageResponse,
};
use crate::domain::error::A2AError;
use crate::domain::{AgentCard, Task, TaskPushNotificationConfig, TaskState};

/// Ontology state context for station operations
///
/// Stations operate on `OntologyState` - the complete protocol state model
/// including tasks, messages, agents, and notification configs.
pub type Ontology = OntologyState;

/// Refusal receipt returned when a station cannot process a packet
///
/// This is a typed, serializable error that explains WHY a packet was refused.
/// Unlike opaque exceptions, refusal receipts are auditable and can be persisted
/// as part of the receipt chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefusalReceipt {
    /// JSON-RPC error code
    pub code: i32,

    /// Human-readable refusal reason
    pub reason: String,

    /// Optional structured data providing refusal context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,

    /// The method that was refused
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

impl RefusalReceipt {
    /// Create a new refusal receipt
    pub fn new(code: i32, reason: String) -> Self {
        Self {
            code,
            reason,
            data: None,
            method: None,
        }
    }

    /// Create a refusal with additional data
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    /// Create a refusal with method context
    pub fn with_method(mut self, method: String) -> Self {
        self.method = Some(method);
        self
    }

    /// Convert to JSON-RPC error
    pub fn to_jsonrpc_error(&self) -> JsonRpcError {
        JsonRpcError {
            code: self.code,
            message: self.reason.clone(),
            data: self.data.clone(),
        }
    }
}

impl From<A2AError> for RefusalReceipt {
    fn from(error: A2AError) -> Self {
        let jsonrpc = error.to_jsonrpc_error();
        let code = jsonrpc
            .get("code")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .unwrap_or(-32603);
        let message = jsonrpc
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Internal error")
            .to_string();

        Self {
            code,
            reason: message,
            data: jsonrpc.get("data").cloned(),
            method: None,
        }
    }
}

impl std::fmt::Display for RefusalReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.reason)
    }
}

impl std::error::Error for RefusalReceipt {}

/// Station trait: Typed packet processor with admission control
///
/// Stations implement deterministic finite state machines. They:
/// 1. Accept typed packets (Input)
/// 2. Validate admission against current ontology state
/// 3. Execute deterministic state transitions
/// 4. Produce typed responses (Output)
///
/// # Type Parameters
///
/// - `Input`: Request type implementing `DeserializeOwned`
/// - `Output`: Response type (typically a struct with `result` field)
///
/// # Determinism
///
/// Stations MUST be deterministic: `step(&O, I) -> (O', Output)` must be reproducible
/// given the same ontology state and input. No LLM calls, no random numbers, no wall-clock time.
pub trait Station {
    /// Input packet type
    type Input: DeserializeOwned;

    /// Output response type
    type Output;

    /// Check if packet can be admitted without state mutation
    ///
    /// This is a pure function that validates the input against current state.
    /// It MUST NOT mutate the ontology.
    ///
    /// # Arguments
    ///
    /// * `ontology` - Current protocol state (immutable reference)
    /// * `input` - Typed input packet
    ///
    /// # Returns
    ///
    /// - `Ok(())` if admission is granted
    /// - `Err(RefusalReceipt)` if packet should be refused
    fn admit(ontology: &Ontology, input: &Self::Input) -> Result<(), RefusalReceipt>;

    /// Execute deterministic state transition
    ///
    /// This method performs the actual processing: it mutates ontology state
    /// and produces a typed output.
    ///
    /// # Arguments
    ///
    /// * `ontology` - Mutable reference to protocol state
    /// * `input` - Typed input packet (consumed)
    ///
    /// # Returns
    ///
    /// - `Ok(Output)` on successful processing
    /// - `Err(RefusalReceipt)` if processing fails
    ///
    /// # Determinism Requirement
    ///
    /// Same (ontology, input) must always produce the same (ontology', output).
    fn step(
        &mut self,
        ontology: &mut Ontology,
        input: Self::Input,
    ) -> Result<Self::Output, RefusalReceipt>;
}

// ==================== Station Implementations ====================

/// Station for `message/send` requests
///
/// Processes incoming messages by creating or updating tasks and appending
/// messages to the task history.
pub struct SendMessageStation;

impl Station for SendMessageStation {
    type Input = SendMessageRequest;
    type Output = SendMessageResponse;

    fn admit(ontology: &Ontology, input: &Self::Input) -> Result<(), RefusalReceipt> {
        // Validate message content is present
        if input.params.message.parts.is_empty() {
            return Err(
                RefusalReceipt::new(-32602, "Message content cannot be empty".to_string())
                    .with_method("message/send".to_string()),
            );
        }

        // If task_id is provided in message, verify task exists
        if let Some(ref task_id) = input.params.message.task_id {
            if ontology.get_task(task_id).is_none() {
                return Err(
                    RefusalReceipt::new(-32001, format!("Task not found: {}", task_id))
                        .with_method("message/send".to_string()),
                );
            }
        }

        Ok(())
    }

    fn step(
        &mut self,
        ontology: &mut Ontology,
        input: Self::Input,
    ) -> Result<Self::Output, RefusalReceipt> {
        // Determine or create task
        let task_id = input
            .params
            .message
            .task_id
            .clone()
            .unwrap_or_else(|| format!("task-{}", uuid::Uuid::new_v4()));

        let context_id = input
            .params
            .message
            .context_id
            .clone()
            .unwrap_or_else(|| format!("ctx-{}", uuid::Uuid::new_v4()));

        // Get or create task
        let task = ontology.get_task(&task_id).cloned().unwrap_or_else(|| {
            Task::builder()
                .id(task_id.clone())
                .context_id(context_id)
                .build()
        });

        // Add task if not exists
        if ontology.get_task(&task_id).is_none() {
            ontology
                .put_task(task.clone())
                .map_err(RefusalReceipt::from)?;
        }

        // Append message to task
        ontology
            .add_message(&task_id, input.params.message.clone())
            .map_err(RefusalReceipt::from)?;

        // Get updated task
        let task = ontology
            .get_task(&task_id)
            .ok_or_else(|| RefusalReceipt::new(-32001, format!("Task not found: {}", task_id)))?
            .clone();

        Ok(SendMessageResponse {
            jsonrpc: "2.0".to_string(),
            id: input.id.clone(),
            result: Some(task),
            error: None,
        })
    }
}

/// Station for `tasks/get` requests
///
/// Retrieves task state from ontology without side effects.
pub struct GetTaskStation;

impl Station for GetTaskStation {
    type Input = GetTaskRequest;
    type Output = GetTaskResponse;

    fn admit(ontology: &Ontology, input: &Self::Input) -> Result<(), RefusalReceipt> {
        // Verify task exists
        if ontology.get_task(&input.params.id).is_none() {
            return Err(RefusalReceipt::new(
                -32001,
                format!("Task not found: {}", input.params.id),
            )
            .with_method("tasks/get".to_string()));
        }

        Ok(())
    }

    fn step(
        &mut self,
        ontology: &mut Ontology,
        input: Self::Input,
    ) -> Result<Self::Output, RefusalReceipt> {
        // Task existence already validated in admit
        let task = ontology
            .get_task(&input.params.id)
            .ok_or_else(|| {
                RefusalReceipt::new(-32001, format!("Task not found: {}", input.params.id))
            })?
            .clone();

        Ok(GetTaskResponse {
            jsonrpc: "2.0".to_string(),
            id: input.id.clone(),
            result: Some(task),
            error: None,
        })
    }
}

/// Station for `tasks/cancel` requests
///
/// Cancels a running task by transitioning to Cancelled state.
pub struct CancelTaskStation;

impl Station for CancelTaskStation {
    type Input = CancelTaskRequest;
    type Output = CancelTaskResponse;

    fn admit(ontology: &Ontology, input: &Self::Input) -> Result<(), RefusalReceipt> {
        // Verify task exists
        let task = ontology.get_task(&input.params.id).ok_or_else(|| {
            RefusalReceipt::new(-32001, format!("Task not found: {}", input.params.id))
        })?;

        // Verify task is cancelable (not already completed/canceled/failed)
        match task.status.state {
            TaskState::Completed | TaskState::Canceled | TaskState::Failed => {
                return Err(RefusalReceipt::new(
                    -32002,
                    format!(
                        "Task cannot be canceled: already in {:?} state",
                        task.status.state
                    ),
                )
                .with_method("tasks/cancel".to_string()));
            }
            _ => {}
        }

        Ok(())
    }

    fn step(
        &mut self,
        ontology: &mut Ontology,
        input: Self::Input,
    ) -> Result<Self::Output, RefusalReceipt> {
        // Get task (existence validated in admit)
        let mut task = ontology
            .get_task(&input.params.id)
            .ok_or_else(|| {
                RefusalReceipt::new(-32001, format!("Task not found: {}", input.params.id))
            })?
            .clone();

        // Transition to Canceled state
        task.status.state = TaskState::Canceled;

        // Update in ontology
        ontology
            .put_task(task.clone())
            .map_err(RefusalReceipt::from)?;

        Ok(CancelTaskResponse {
            jsonrpc: "2.0".to_string(),
            id: input.id.clone(),
            result: Some(task),
            error: None,
        })
    }
}

/// Station for `tasks/list` requests
///
/// Lists tasks with optional filtering and pagination.
pub struct ListTasksStation;

impl Station for ListTasksStation {
    type Input = ListTasksRequest;
    type Output = ListTasksResponse;

    fn admit(_ontology: &Ontology, _input: &Self::Input) -> Result<(), RefusalReceipt> {
        // List requests are always admissible (may return empty results)
        Ok(())
    }

    fn step(
        &mut self,
        ontology: &mut Ontology,
        input: Self::Input,
    ) -> Result<Self::Output, RefusalReceipt> {
        // Get all tasks
        let all_tasks: Vec<Task> = ontology.get_all_tasks().into_iter().cloned().collect();

        // Apply filters if present
        let mut filtered_tasks = all_tasks;

        if let Some(ref params) = input.params {
            if let Some(ref context_id) = params.context_id {
                filtered_tasks.retain(|t| &t.context_id == context_id);
            }

            if let Some(ref state) = params.status {
                filtered_tasks.retain(|t| &t.status.state == state);
            }
        }

        // Apply pagination
        let params = input.params.as_ref();
        let page_size = params.and_then(|p| p.page_size).unwrap_or(50).min(100) as usize; // Cap at 100
        let page_token = params.and_then(|p| p.page_token.clone());

        // For simplicity, treat page_token as an offset
        let offset = page_token
            .as_ref()
            .and_then(|t| t.parse::<usize>().ok())
            .unwrap_or(0);

        let total_size = filtered_tasks.len() as i32;
        let paginated: Vec<Task> = filtered_tasks
            .into_iter()
            .skip(offset)
            .take(page_size)
            .collect();

        let next_page_token = if offset + page_size < total_size as usize {
            (offset + page_size).to_string()
        } else {
            String::new()
        };

        Ok(ListTasksResponse {
            jsonrpc: "2.0".to_string(),
            id: input.id.clone(),
            result: Some(crate::domain::ListTasksResult {
                tasks: paginated,
                total_size,
                page_size: page_size as i32,
                next_page_token,
            }),
            error: None,
        })
    }
}

/// Station for `agent/getExtendedCard` requests
///
/// Returns the agent's extended card from ontology.
pub struct GetExtendedCardStation {
    /// The agent card to return
    pub agent_card: AgentCard,
}

impl Station for GetExtendedCardStation {
    type Input = GetExtendedCardRequest;
    type Output = GetExtendedCardResponse;

    fn admit(_ontology: &Ontology, _input: &Self::Input) -> Result<(), RefusalReceipt> {
        // Extended card requests are always admissible
        Ok(())
    }

    fn step(
        &mut self,
        _ontology: &mut Ontology,
        input: Self::Input,
    ) -> Result<Self::Output, RefusalReceipt> {
        Ok(GetExtendedCardResponse {
            jsonrpc: "2.0".to_string(),
            id: input.id.clone(),
            result: Some(self.agent_card.clone()),
            error: None,
        })
    }
}

// ==================== Station Registry ====================

/// Station registry for method-based dispatch
///
/// Maps JSON-RPC method names to station implementations. This enables
/// dynamic dispatch while maintaining type safety at the station level.
///
/// # Example
///
/// ```rust
/// use a2a_rs::construct::station::StationRegistry;
/// use a2a_rs::construct::ontology::OntologyState;
///
/// let mut registry = StationRegistry::new();
/// let mut ontology = OntologyState::new();
///
/// // Registry contains default stations for core A2A methods
/// assert!(registry.has_method("tasks/get"));
/// assert!(registry.has_method("message/send"));
/// ```
pub struct StationRegistry {
    /// Map of method names to type-erased station handlers
    handlers: HashMap<String, Box<dyn StationHandler>>,
}

impl StationRegistry {
    /// Create a new registry with default A2A stations
    pub fn new() -> Self {
        let mut registry = Self {
            handlers: HashMap::new(),
        };

        // Register all A2A v0.3.0 core stations
        registry.register("message/send", Box::new(SendMessageStationHandler));
        registry.register(
            "message/stream",
            Box::new(SendStreamingMessageStationHandler),
        );
        registry.register("tasks/get", Box::new(GetTaskStationHandler));
        registry.register("tasks/cancel", Box::new(CancelTaskStationHandler));
        registry.register("tasks/list", Box::new(ListTasksStationHandler));
        registry.register("tasks/resubscribe", Box::new(TaskResubscribeStationHandler));
        registry.register(
            "tasks/pushNotificationConfig/set",
            Box::new(SetPushNotificationConfigStationHandler),
        );
        registry.register(
            "tasks/pushNotificationConfig/get",
            Box::new(GetPushNotificationConfigStationHandler),
        );
        registry.register(
            "tasks/pushNotificationConfig/list",
            Box::new(ListPushNotificationConfigsStationHandler),
        );
        registry.register(
            "tasks/pushNotificationConfig/delete",
            Box::new(DeletePushNotificationConfigStationHandler),
        );

        registry
    }

    /// Register a station handler for a method
    pub fn register(&mut self, method: &str, handler: Box<dyn StationHandler>) {
        self.handlers.insert(method.to_string(), handler);
    }

    /// Check if a method is registered
    pub fn has_method(&self, method: &str) -> bool {
        self.handlers.contains_key(method)
    }

    /// Dispatch a request to the appropriate station
    ///
    /// Returns a JSON-RPC response on success or refusal receipt on error.
    pub fn dispatch(
        &mut self,
        method: &str,
        ontology: &mut Ontology,
        params: serde_json::Value,
        id: Option<crate::construct::types::JsonRpcId>,
    ) -> Result<serde_json::Value, RefusalReceipt> {
        let handler = self
            .handlers
            .get_mut(method)
            .ok_or_else(|| RefusalReceipt::new(-32601, format!("Method not found: {}", method)))?;

        handler.handle(ontology, params, id)
    }
}

impl Default for StationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Type-erased handler trait for station dispatch
///
/// This trait enables storing different station types in a single registry
/// while maintaining the typed Station interface at each handler.
pub trait StationHandler: Send + Sync {
    /// Handle a request with JSON params
    fn handle(
        &mut self,
        ontology: &mut Ontology,
        params: serde_json::Value,
        id: Option<crate::construct::types::JsonRpcId>,
    ) -> Result<serde_json::Value, RefusalReceipt>;
}

// Handler implementations for each station

struct SendMessageStationHandler;

impl StationHandler for SendMessageStationHandler {
    fn handle(
        &mut self,
        ontology: &mut Ontology,
        params: serde_json::Value,
        id: Option<crate::construct::types::JsonRpcId>,
    ) -> Result<serde_json::Value, RefusalReceipt> {
        let request = SendMessageRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: "message/send".to_string(),
            params: serde_json::from_value(params)
                .map_err(|e| RefusalReceipt::new(-32602, format!("Invalid params: {}", e)))?,
        };

        SendMessageStation::admit(ontology, &request)?;
        let response = SendMessageStation.step(ontology, request)?;

        serde_json::to_value(response).map_err(|e| {
            RefusalReceipt::new(-32603, format!("Failed to serialize response: {}", e))
        })
    }
}

struct GetTaskStationHandler;

impl StationHandler for GetTaskStationHandler {
    fn handle(
        &mut self,
        ontology: &mut Ontology,
        params: serde_json::Value,
        id: Option<crate::construct::types::JsonRpcId>,
    ) -> Result<serde_json::Value, RefusalReceipt> {
        let request = GetTaskRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: "tasks/get".to_string(),
            params: serde_json::from_value(params)
                .map_err(|e| RefusalReceipt::new(-32602, format!("Invalid params: {}", e)))?,
        };

        GetTaskStation::admit(ontology, &request)?;
        let response = GetTaskStation.step(ontology, request)?;

        serde_json::to_value(response).map_err(|e| {
            RefusalReceipt::new(-32603, format!("Failed to serialize response: {}", e))
        })
    }
}

struct CancelTaskStationHandler;

impl StationHandler for CancelTaskStationHandler {
    fn handle(
        &mut self,
        ontology: &mut Ontology,
        params: serde_json::Value,
        id: Option<crate::construct::types::JsonRpcId>,
    ) -> Result<serde_json::Value, RefusalReceipt> {
        let request = CancelTaskRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: "tasks/cancel".to_string(),
            params: serde_json::from_value(params)
                .map_err(|e| RefusalReceipt::new(-32602, format!("Invalid params: {}", e)))?,
        };

        CancelTaskStation::admit(ontology, &request)?;
        let response = CancelTaskStation.step(ontology, request)?;

        serde_json::to_value(response).map_err(|e| {
            RefusalReceipt::new(-32603, format!("Failed to serialize response: {}", e))
        })
    }
}

struct ListTasksStationHandler;

impl StationHandler for ListTasksStationHandler {
    fn handle(
        &mut self,
        ontology: &mut Ontology,
        params: serde_json::Value,
        id: Option<crate::construct::types::JsonRpcId>,
    ) -> Result<serde_json::Value, RefusalReceipt> {
        let request = ListTasksRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: "tasks/list".to_string(),
            params: serde_json::from_value(params)
                .map_err(|e| RefusalReceipt::new(-32602, format!("Invalid params: {}", e)))?,
        };

        ListTasksStation::admit(ontology, &request)?;
        let response = ListTasksStation.step(ontology, request)?;

        serde_json::to_value(response).map_err(|e| {
            RefusalReceipt::new(-32603, format!("Failed to serialize response: {}", e))
        })
    }
}

// ==================== Additional Station Implementations (A2A v0.3.0) ====================

/// Station for `message/stream` requests
///
/// Sends a message with streaming updates (initial response only - streaming handled externally).
pub struct SendStreamingMessageStation;

impl Station for SendStreamingMessageStation {
    type Input = crate::construct::types::SendMessageStreamingRequest;
    type Output = SendMessageResponse;

    fn admit(ontology: &Ontology, input: &Self::Input) -> Result<(), RefusalReceipt> {
        // Same validation as SendMessage - streaming is handled at transport layer
        if input.params.message.parts.is_empty() {
            return Err(
                RefusalReceipt::new(-32602, "Message content cannot be empty".to_string())
                    .with_method("message/stream".to_string()),
            );
        }

        if let Some(ref task_id) = input.params.message.task_id {
            if ontology.get_task(task_id).is_none() {
                return Err(
                    RefusalReceipt::new(-32001, format!("Task not found: {}", task_id))
                        .with_method("message/stream".to_string()),
                );
            }
        }

        Ok(())
    }

    fn step(
        &mut self,
        ontology: &mut Ontology,
        input: Self::Input,
    ) -> Result<Self::Output, RefusalReceipt> {
        // Streaming messages create tasks the same way as normal messages
        // The streaming part is handled at the transport layer (WebSocket/SSE)
        let task_id = input
            .params
            .message
            .task_id
            .clone()
            .unwrap_or_else(|| format!("task-{}", uuid::Uuid::new_v4()));

        let context_id = input
            .params
            .message
            .context_id
            .clone()
            .unwrap_or_else(|| format!("ctx-{}", uuid::Uuid::new_v4()));

        // Get or create task
        let task = ontology.get_task(&task_id).cloned().unwrap_or_else(|| {
            Task::builder()
                .id(task_id.clone())
                .context_id(context_id)
                .build()
        });

        // Add task if not exists
        if ontology.get_task(&task_id).is_none() {
            ontology
                .put_task(task.clone())
                .map_err(RefusalReceipt::from)?;
        }

        ontology
            .add_message(&task_id, input.params.message.clone())
            .map_err(RefusalReceipt::from)?;

        // Get updated task
        let task = ontology
            .get_task(&task_id)
            .ok_or_else(|| RefusalReceipt::new(-32001, format!("Task not found: {}", task_id)))?
            .clone();

        Ok(SendMessageResponse {
            jsonrpc: "2.0".to_string(),
            id: input.id.clone(),
            result: Some(task),
            error: None,
        })
    }
}

/// Station for `tasks/resubscribe` requests
///
/// Re-subscribes to an existing streaming task after disconnect.
pub struct TaskResubscribeStation;

impl Station for TaskResubscribeStation {
    type Input = crate::construct::types::TaskResubscriptionRequest;
    type Output = GetTaskResponse;

    fn admit(ontology: &Ontology, input: &Self::Input) -> Result<(), RefusalReceipt> {
        // Verify task exists
        if ontology.get_task(&input.params.id).is_none() {
            return Err(RefusalReceipt::new(
                -32001,
                format!("Task not found: {}", input.params.id),
            )
            .with_method("tasks/resubscribe".to_string()));
        }

        Ok(())
    }

    fn step(
        &mut self,
        ontology: &mut Ontology,
        input: Self::Input,
    ) -> Result<Self::Output, RefusalReceipt> {
        // Resubscription returns current task state
        // Actual event replay is handled at transport layer
        let task = ontology
            .get_task(&input.params.id)
            .ok_or_else(|| {
                RefusalReceipt::new(-32001, format!("Task not found: {}", input.params.id))
            })?
            .clone();

        Ok(GetTaskResponse {
            jsonrpc: "2.0".to_string(),
            id: input.id.clone(),
            result: Some(task),
            error: None,
        })
    }
}

/// Station for `tasks/pushNotificationConfig/set` requests
///
/// Configures webhook for task status updates.
pub struct SetPushNotificationConfigStation;

impl Station for SetPushNotificationConfigStation {
    type Input = crate::construct::types::SetTaskPushNotificationRequest;
    type Output = crate::construct::types::SetTaskPushNotificationResponse;

    fn admit(ontology: &Ontology, input: &Self::Input) -> Result<(), RefusalReceipt> {
        // Verify task exists
        if ontology.get_task(&input.params.task_id).is_none() {
            return Err(RefusalReceipt::new(
                -32001,
                format!("Task not found: {}", input.params.task_id),
            )
            .with_method("tasks/pushNotificationConfig/set".to_string()));
        }

        // Validate webhook URL
        if input.params.push_notification_config.url.is_empty() {
            return Err(
                RefusalReceipt::new(-32602, "Webhook URL cannot be empty".to_string())
                    .with_method("tasks/pushNotificationConfig/set".to_string()),
            );
        }

        Ok(())
    }

    fn step(
        &mut self,
        ontology: &mut Ontology,
        input: Self::Input,
    ) -> Result<Self::Output, RefusalReceipt> {
        // Store notification config in ontology
        let config = input.params.clone();

        ontology
            .put_notification_config(&config.task_id, config.clone())
            .map_err(RefusalReceipt::from)?;

        Ok(crate::construct::types::SetTaskPushNotificationResponse {
            jsonrpc: "2.0".to_string(),
            id: input.id.clone(),
            result: Some(config),
            error: None,
        })
    }
}

/// Station for `tasks/pushNotificationConfig/get` requests
///
/// Retrieves push notification configuration for a task (A2A v0.3.0).
pub struct GetPushNotificationConfigStation;

impl Station for GetPushNotificationConfigStation {
    type Input = crate::construct::types::GetTaskPushNotificationConfigRequest;
    type Output = crate::construct::types::GetTaskPushNotificationConfigResponse;

    fn admit(ontology: &Ontology, input: &Self::Input) -> Result<(), RefusalReceipt> {
        // Verify task exists if params provided
        if let Some(ref params) = input.params {
            if ontology.get_task(&params.id).is_none() {
                return Err(
                    RefusalReceipt::new(-32001, format!("Task not found: {}", params.id))
                        .with_method("tasks/pushNotificationConfig/get".to_string()),
                );
            }
        }

        Ok(())
    }

    fn step(
        &mut self,
        ontology: &mut Ontology,
        input: Self::Input,
    ) -> Result<Self::Output, RefusalReceipt> {
        let params = input.params.as_ref().ok_or_else(|| {
            RefusalReceipt::new(-32602, "Missing params".to_string())
                .with_method("tasks/pushNotificationConfig/get".to_string())
        })?;

        let config = ontology
            .get_notification_config(&params.id)
            .ok_or_else(|| {
                RefusalReceipt::new(-32001, "Push notification config not found".to_string())
                    .with_method("tasks/pushNotificationConfig/get".to_string())
            })?
            .clone();

        Ok(
            crate::construct::types::GetTaskPushNotificationConfigResponse {
                jsonrpc: "2.0".to_string(),
                id: input.id.clone(),
                result: Some(config),
                error: None,
            },
        )
    }
}

/// Station for `tasks/pushNotificationConfig/list` requests
///
/// Lists all push notification configurations for a task (A2A v0.3.0).
pub struct ListPushNotificationConfigsStation;

impl Station for ListPushNotificationConfigsStation {
    type Input = crate::construct::types::ListTaskPushNotificationConfigRequest;
    type Output = crate::construct::types::ListTaskPushNotificationConfigResponse;

    fn admit(ontology: &Ontology, input: &Self::Input) -> Result<(), RefusalReceipt> {
        // Verify task exists
        if ontology.get_task(&input.params.id).is_none() {
            return Err(RefusalReceipt::new(
                -32001,
                format!("Task not found: {}", input.params.id),
            )
            .with_method("tasks/pushNotificationConfig/list".to_string()));
        }

        Ok(())
    }

    fn step(
        &mut self,
        ontology: &mut Ontology,
        input: Self::Input,
    ) -> Result<Self::Output, RefusalReceipt> {
        // Get the config for this task (currently only one config per task)
        let configs = if let Some(config) = ontology.get_notification_config(&input.params.id) {
            vec![config.clone()]
        } else {
            vec![]
        };

        Ok(
            crate::construct::types::ListTaskPushNotificationConfigResponse {
                jsonrpc: "2.0".to_string(),
                id: input.id.clone(),
                result: Some(configs),
                error: None,
            },
        )
    }
}

/// Station for `tasks/pushNotificationConfig/delete` requests
///
/// Deletes a push notification configuration (A2A v0.3.0).
pub struct DeletePushNotificationConfigStation;

impl Station for DeletePushNotificationConfigStation {
    type Input = crate::construct::types::DeleteTaskPushNotificationConfigRequest;
    type Output = crate::construct::types::DeleteTaskPushNotificationConfigResponse;

    fn admit(ontology: &Ontology, input: &Self::Input) -> Result<(), RefusalReceipt> {
        // Verify task exists
        if ontology.get_task(&input.params.id).is_none() {
            return Err(RefusalReceipt::new(
                -32001,
                format!("Task not found: {}", input.params.id),
            )
            .with_method("tasks/pushNotificationConfig/delete".to_string()));
        }

        // Verify config exists
        if ontology.get_notification_config(&input.params.id).is_none() {
            return Err(RefusalReceipt::new(
                -32001,
                format!(
                    "Push notification config not found: {}",
                    input.params.push_notification_config_id
                ),
            )
            .with_method("tasks/pushNotificationConfig/delete".to_string()));
        }

        Ok(())
    }

    fn step(
        &mut self,
        ontology: &mut Ontology,
        input: Self::Input,
    ) -> Result<Self::Output, RefusalReceipt> {
        ontology.remove_notification_config(&input.params.id);

        Ok(
            crate::construct::types::DeleteTaskPushNotificationConfigResponse {
                jsonrpc: "2.0".to_string(),
                id: input.id.clone(),
                result: Some(serde_json::json!({})),
                error: None,
            },
        )
    }
}

/// Station for `agent/getAuthenticatedExtendedCard` requests
///
/// Returns the authenticated extended agent card (A2A v0.3.0).
pub struct GetAuthenticatedExtendedCardStation {
    /// The authenticated agent card to return
    pub agent_card: AgentCard,
}

impl Station for GetAuthenticatedExtendedCardStation {
    type Input = crate::construct::types::GetAuthenticatedExtendedCardRequest;
    type Output = crate::construct::types::GetAuthenticatedExtendedCardResponse;

    fn admit(_ontology: &Ontology, _input: &Self::Input) -> Result<(), RefusalReceipt> {
        // Authenticated card requests always admissible
        // Authentication check handled at transport layer
        Ok(())
    }

    fn step(
        &mut self,
        _ontology: &mut Ontology,
        input: Self::Input,
    ) -> Result<Self::Output, RefusalReceipt> {
        Ok(
            crate::construct::types::GetAuthenticatedExtendedCardResponse {
                jsonrpc: "2.0".to_string(),
                id: input.id.clone(),
                result: Some(self.agent_card.clone()),
                error: None,
            },
        )
    }
}

// ==================== Additional StationHandler Implementations ====================

struct SendStreamingMessageStationHandler;

impl StationHandler for SendStreamingMessageStationHandler {
    fn handle(
        &mut self,
        ontology: &mut Ontology,
        params: serde_json::Value,
        id: Option<crate::construct::types::JsonRpcId>,
    ) -> Result<serde_json::Value, RefusalReceipt> {
        let request = crate::construct::types::SendMessageStreamingRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: "message/stream".to_string(),
            params: serde_json::from_value(params)
                .map_err(|e| RefusalReceipt::new(-32602, format!("Invalid params: {}", e)))?,
        };

        SendStreamingMessageStation::admit(ontology, &request)?;
        let response = SendStreamingMessageStation.step(ontology, request)?;

        serde_json::to_value(response).map_err(|e| {
            RefusalReceipt::new(-32603, format!("Failed to serialize response: {}", e))
        })
    }
}

struct TaskResubscribeStationHandler;

impl StationHandler for TaskResubscribeStationHandler {
    fn handle(
        &mut self,
        ontology: &mut Ontology,
        params: serde_json::Value,
        id: Option<crate::construct::types::JsonRpcId>,
    ) -> Result<serde_json::Value, RefusalReceipt> {
        let request = crate::construct::types::TaskResubscriptionRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: "tasks/resubscribe".to_string(),
            params: serde_json::from_value(params)
                .map_err(|e| RefusalReceipt::new(-32602, format!("Invalid params: {}", e)))?,
        };

        TaskResubscribeStation::admit(ontology, &request)?;
        let response = TaskResubscribeStation.step(ontology, request)?;

        serde_json::to_value(response).map_err(|e| {
            RefusalReceipt::new(-32603, format!("Failed to serialize response: {}", e))
        })
    }
}

struct SetPushNotificationConfigStationHandler;

impl StationHandler for SetPushNotificationConfigStationHandler {
    fn handle(
        &mut self,
        ontology: &mut Ontology,
        params: serde_json::Value,
        id: Option<crate::construct::types::JsonRpcId>,
    ) -> Result<serde_json::Value, RefusalReceipt> {
        let request = crate::construct::types::SetTaskPushNotificationRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: "tasks/pushNotificationConfig/set".to_string(),
            params: serde_json::from_value(params)
                .map_err(|e| RefusalReceipt::new(-32602, format!("Invalid params: {}", e)))?,
        };

        SetPushNotificationConfigStation::admit(ontology, &request)?;
        let response = SetPushNotificationConfigStation.step(ontology, request)?;

        serde_json::to_value(response).map_err(|e| {
            RefusalReceipt::new(-32603, format!("Failed to serialize response: {}", e))
        })
    }
}

struct GetPushNotificationConfigStationHandler;

impl StationHandler for GetPushNotificationConfigStationHandler {
    fn handle(
        &mut self,
        ontology: &mut Ontology,
        params: serde_json::Value,
        id: Option<crate::construct::types::JsonRpcId>,
    ) -> Result<serde_json::Value, RefusalReceipt> {
        let request = crate::construct::types::GetTaskPushNotificationConfigRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: "tasks/pushNotificationConfig/get".to_string(),
            params: serde_json::from_value(params)
                .map_err(|e| RefusalReceipt::new(-32602, format!("Invalid params: {}", e)))?,
        };

        GetPushNotificationConfigStation::admit(ontology, &request)?;
        let response = GetPushNotificationConfigStation.step(ontology, request)?;

        serde_json::to_value(response).map_err(|e| {
            RefusalReceipt::new(-32603, format!("Failed to serialize response: {}", e))
        })
    }
}

struct ListPushNotificationConfigsStationHandler;

impl StationHandler for ListPushNotificationConfigsStationHandler {
    fn handle(
        &mut self,
        ontology: &mut Ontology,
        params: serde_json::Value,
        id: Option<crate::construct::types::JsonRpcId>,
    ) -> Result<serde_json::Value, RefusalReceipt> {
        let request = crate::construct::types::ListTaskPushNotificationConfigRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: "tasks/pushNotificationConfig/list".to_string(),
            params: serde_json::from_value(params)
                .map_err(|e| RefusalReceipt::new(-32602, format!("Invalid params: {}", e)))?,
        };

        ListPushNotificationConfigsStation::admit(ontology, &request)?;
        let response = ListPushNotificationConfigsStation.step(ontology, request)?;

        serde_json::to_value(response).map_err(|e| {
            RefusalReceipt::new(-32603, format!("Failed to serialize response: {}", e))
        })
    }
}

struct DeletePushNotificationConfigStationHandler;

impl StationHandler for DeletePushNotificationConfigStationHandler {
    fn handle(
        &mut self,
        ontology: &mut Ontology,
        params: serde_json::Value,
        id: Option<crate::construct::types::JsonRpcId>,
    ) -> Result<serde_json::Value, RefusalReceipt> {
        let request = crate::construct::types::DeleteTaskPushNotificationConfigRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: "tasks/pushNotificationConfig/delete".to_string(),
            params: serde_json::from_value(params)
                .map_err(|e| RefusalReceipt::new(-32602, format!("Invalid params: {}", e)))?,
        };

        DeletePushNotificationConfigStation::admit(ontology, &request)?;
        let response = DeletePushNotificationConfigStation.step(ontology, request)?;

        serde_json::to_value(response).map_err(|e| {
            RefusalReceipt::new(-32603, format!("Failed to serialize response: {}", e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Message, Role};

    #[test]
    fn test_send_message_station_creates_task() {
        let mut ontology = OntologyState::new();
        let mut station = SendMessageStation;

        let request = SendMessageRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(crate::construct::types::JsonRpcId::new_uuid()),
            method: "message/send".to_string(),
            params: crate::domain::MessageSendParams {
                message: Message::user_text("Hello".to_string(), "msg-1".to_string()),
                configuration: None,
                metadata: None,
            },
        };

        // Should admit
        assert!(SendMessageStation::admit(&ontology, &request).is_ok());

        // Should create new task
        let response = station.step(&mut ontology, request).unwrap();
        assert!(response.result.is_some());
        assert_eq!(ontology.task_count(), 1);
    }

    #[test]
    fn test_get_task_station_refuses_missing_task() {
        let ontology = OntologyState::new();

        let request = GetTaskRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(crate::construct::types::JsonRpcId::new_uuid()),
            method: "tasks/get".to_string(),
            params: crate::domain::TaskQueryParams {
                id: "nonexistent".to_string(),
                history_length: None,
                metadata: None,
            },
        };

        // Should refuse admission
        let result = GetTaskStation::admit(&ontology, &request);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32001); // TASK_NOT_FOUND
    }

    #[test]
    fn test_station_registry_dispatch() {
        let mut registry = StationRegistry::new();
        let mut ontology = OntologyState::new();

        // Create a task first
        let task = Task::builder()
            .id("task-123".to_string())
            .context_id("ctx-1".to_string())
            .build();
        ontology.put_task(task).unwrap();

        // Dispatch tasks/get
        let params = serde_json::json!({
            "id": "task-123"
        });

        let result = registry.dispatch(
            "tasks/get",
            &mut ontology,
            params,
            Some(crate::construct::types::JsonRpcId::from_string(
                "req-1".to_string(),
            )),
        );

        assert!(result.is_ok());
    }
}
