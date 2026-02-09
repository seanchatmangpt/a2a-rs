//! Typed packet system for A2A protocol
//!
//! This module provides a strongly-typed packet system that eliminates the use
//! of `serde_json::Value` at boundaries, enforcing closed-world semantics where
//! only known, well-typed data is accepted.
//!
//! # Design Principles
//!
//! 1. **Zero tolerance for untyped data**: All request types use concrete types
//! 2. **Fail-fast on unknown fields**: `#[serde(deny_unknown_fields)]` enforced
//! 3. **Type-safe dispatch**: `PacketType` enum enables compile-time guarantees
//! 4. **Explicit ID type**: `JsonRpcId` replaces `serde_json::Value`

use serde::{Deserialize, Serialize};

use crate::domain::{
    AgentCard, DeleteTaskPushNotificationConfigParams, GetTaskPushNotificationConfigParams,
    ListTaskPushNotificationConfigParams, ListTasksParams, ListTasksResult, MessageSendParams,
    Task, TaskIdParams, TaskPushNotificationConfig, TaskQueryParams, TaskSendParams,
};

/// Strongly-typed JSON-RPC ID
///
/// Replaces `serde_json::Value` with explicit enum variants. This enforces
/// closed-world semantics: only known ID types (String, Number, Null) are valid.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcId {
    String(String),
    Number(i64),
    Null,
}

impl JsonRpcId {
    /// Generate a new UUID-based string ID
    pub fn new_uuid() -> Self {
        JsonRpcId::String(uuid::Uuid::new_v4().to_string())
    }

    /// Create from a string
    pub fn from_string(s: String) -> Self {
        JsonRpcId::String(s)
    }

    /// Create from a number
    pub fn from_number(n: i64) -> Self {
        JsonRpcId::Number(n)
    }
}

impl Default for JsonRpcId {
    fn default() -> Self {
        JsonRpcId::Null
    }
}

impl std::fmt::Display for JsonRpcId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonRpcId::String(s) => write!(f, "{}", s),
            JsonRpcId::Number(n) => write!(f, "{}", n),
            JsonRpcId::Null => write!(f, "null"),
        }
    }
}

/// Packet trait for type-safe handling of all A2A requests
///
/// All request types implement this trait, enabling unified handling
/// while maintaining strong typing at boundaries.
pub trait Packet: Serialize + for<'de> Deserialize<'de> {
    /// Get the JSON-RPC method name
    fn method(&self) -> &str;

    /// Get the request ID
    fn id(&self) -> Option<&JsonRpcId>;

    /// Get the JSON-RPC version (always "2.0")
    fn jsonrpc(&self) -> &str {
        "2.0"
    }
}

/// Packet type discriminant for dispatch
///
/// This enum enables compile-time type-safe dispatch without using
/// `serde_json::Value` or dynamic typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PacketType {
    SendMessage,
    SendMessageStreaming,
    SendTask,
    SendTaskStreaming,
    GetTask,
    CancelTask,
    SetTaskPushNotification,
    GetTaskPushNotification,
    TaskResubscription,
    GetExtendedCard,
    GetAuthenticatedExtendedCard,
    ListTasks,
    GetTaskPushNotificationConfig,
    ListTaskPushNotificationConfigs,
    DeleteTaskPushNotificationConfig,
}

impl PacketType {
    /// Get the JSON-RPC method name for this packet type
    pub fn method(&self) -> &'static str {
        match self {
            PacketType::SendMessage => "message/send",
            PacketType::SendMessageStreaming => "message/stream",
            PacketType::SendTask => "tasks/send",
            PacketType::SendTaskStreaming => "tasks/sendSubscribe",
            PacketType::GetTask => "tasks/get",
            PacketType::CancelTask => "tasks/cancel",
            PacketType::SetTaskPushNotification => "tasks/pushNotificationConfig/set",
            PacketType::GetTaskPushNotification => "tasks/pushNotificationConfig/get",
            PacketType::TaskResubscription => "tasks/resubscribe",
            PacketType::GetExtendedCard => "agent/getExtendedCard",
            PacketType::GetAuthenticatedExtendedCard => "agent/getAuthenticatedExtendedCard",
            PacketType::ListTasks => "tasks/list",
            PacketType::GetTaskPushNotificationConfig => "tasks/pushNotificationConfig/get",
            PacketType::ListTaskPushNotificationConfigs => "tasks/pushNotificationConfig/list",
            PacketType::DeleteTaskPushNotificationConfig => "tasks/pushNotificationConfig/delete",
        }
    }

    /// Parse a method string into a PacketType
    pub fn from_method(method: &str) -> Option<Self> {
        match method {
            "message/send" => Some(PacketType::SendMessage),
            "message/stream" => Some(PacketType::SendMessageStreaming),
            "tasks/send" => Some(PacketType::SendTask),
            "tasks/sendSubscribe" => Some(PacketType::SendTaskStreaming),
            "tasks/get" => Some(PacketType::GetTask),
            "tasks/cancel" => Some(PacketType::CancelTask),
            "tasks/pushNotificationConfig/set" => Some(PacketType::SetTaskPushNotification),
            "tasks/pushNotificationConfig/get" => Some(PacketType::GetTaskPushNotificationConfig),
            "tasks/resubscribe" => Some(PacketType::TaskResubscription),
            "agent/getExtendedCard" => Some(PacketType::GetExtendedCard),
            "agent/getAuthenticatedExtendedCard" => Some(PacketType::GetAuthenticatedExtendedCard),
            "tasks/list" => Some(PacketType::ListTasks),
            "tasks/pushNotificationConfig/list" => {
                Some(PacketType::ListTaskPushNotificationConfigs)
            }
            "tasks/pushNotificationConfig/delete" => {
                Some(PacketType::DeleteTaskPushNotificationConfig)
            }
            _ => None,
        }
    }
}

// ============================================================================
// Request Types - All with #[serde(deny_unknown_fields)]
// ============================================================================

/// Request to send a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendMessageRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_message_send_method")]
    pub method: String,
    pub params: MessageSendParams,
}

impl Packet for SendMessageRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl SendMessageRequest {
    pub fn new(params: MessageSendParams) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "message/send".to_string(),
            params,
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Request to send a message with streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendMessageStreamingRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_message_stream_method")]
    pub method: String,
    pub params: MessageSendParams,
}

impl Packet for SendMessageStreamingRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl SendMessageStreamingRequest {
    pub fn new(params: MessageSendParams) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "message/stream".to_string(),
            params,
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Request to send a task (legacy)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendTaskRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_tasks_send_method")]
    pub method: String,
    pub params: TaskSendParams,
}

impl Packet for SendTaskRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl SendTaskRequest {
    pub fn new(params: TaskSendParams) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "tasks/send".to_string(),
            params,
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Request to send a task with streaming (legacy)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendTaskStreamingRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_tasks_send_subscribe_method")]
    pub method: String,
    pub params: TaskSendParams,
}

impl Packet for SendTaskStreamingRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl SendTaskStreamingRequest {
    pub fn new(params: TaskSendParams) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "tasks/sendSubscribe".to_string(),
            params,
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Request to get a task
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetTaskRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_tasks_get_method")]
    pub method: String,
    pub params: TaskQueryParams,
}

impl Packet for GetTaskRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl GetTaskRequest {
    pub fn new(params: TaskQueryParams) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "tasks/get".to_string(),
            params,
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Request to cancel a task
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelTaskRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_tasks_cancel_method")]
    pub method: String,
    pub params: TaskIdParams,
}

impl Packet for CancelTaskRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl CancelTaskRequest {
    pub fn new(params: TaskIdParams) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "tasks/cancel".to_string(),
            params,
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Request to set task push notification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetTaskPushNotificationRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_push_notification_set_method")]
    pub method: String,
    pub params: TaskPushNotificationConfig,
}

impl Packet for SetTaskPushNotificationRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl SetTaskPushNotificationRequest {
    pub fn new(params: TaskPushNotificationConfig) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "tasks/pushNotificationConfig/set".to_string(),
            params,
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Request to get task push notification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetTaskPushNotificationRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_push_notification_get_method")]
    pub method: String,
    pub params: TaskIdParams,
}

impl Packet for GetTaskPushNotificationRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl GetTaskPushNotificationRequest {
    pub fn new(params: TaskIdParams) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "tasks/pushNotificationConfig/get".to_string(),
            params,
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Request for task resubscription
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskResubscriptionRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_tasks_resubscribe_method")]
    pub method: String,
    pub params: TaskQueryParams,
}

impl Packet for TaskResubscriptionRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl TaskResubscriptionRequest {
    pub fn new(params: TaskQueryParams) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "tasks/resubscribe".to_string(),
            params,
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Empty params type for agent/getExtendedCard
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GetExtendedCardParams {}

/// Request to get an extended agent card
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetExtendedCardRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_agent_get_extended_card_method")]
    pub method: String,
    #[serde(default)]
    pub params: GetExtendedCardParams,
}

impl Packet for GetExtendedCardRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl GetExtendedCardRequest {
    pub fn new() -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "agent/getExtendedCard".to_string(),
            params: GetExtendedCardParams::default(),
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

impl Default for GetExtendedCardRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Empty params type for agent/getAuthenticatedExtendedCard
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GetAuthenticatedExtendedCardParams {}

/// Request to get an authenticated extended agent card (v0.3.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetAuthenticatedExtendedCardRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_agent_get_authenticated_extended_card_method")]
    pub method: String,
    #[serde(default)]
    pub params: GetAuthenticatedExtendedCardParams,
}

impl Packet for GetAuthenticatedExtendedCardRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl GetAuthenticatedExtendedCardRequest {
    pub fn new() -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "agent/getAuthenticatedExtendedCard".to_string(),
            params: GetAuthenticatedExtendedCardParams::default(),
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

impl Default for GetAuthenticatedExtendedCardRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Request to list tasks with filtering and pagination (v0.3.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListTasksRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_tasks_list_method")]
    pub method: String,
    pub params: Option<ListTasksParams>,
}

impl Packet for ListTasksRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl ListTasksRequest {
    pub fn new(params: Option<ListTasksParams>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "tasks/list".to_string(),
            params,
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Request to get push notification config for a task (v0.3.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetTaskPushNotificationConfigRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_push_notification_get_method")]
    pub method: String,
    pub params: Option<GetTaskPushNotificationConfigParams>,
}

impl Packet for GetTaskPushNotificationConfigRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl GetTaskPushNotificationConfigRequest {
    pub fn new(params: GetTaskPushNotificationConfigParams) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "tasks/pushNotificationConfig/get".to_string(),
            params: Some(params),
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Request to list all push notification configs for a task (v0.3.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListTaskPushNotificationConfigRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_push_notification_list_method")]
    pub method: String,
    pub params: ListTaskPushNotificationConfigParams,
}

impl Packet for ListTaskPushNotificationConfigRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl ListTaskPushNotificationConfigRequest {
    pub fn new(params: ListTaskPushNotificationConfigParams) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "tasks/pushNotificationConfig/list".to_string(),
            params,
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Request to delete a push notification config (v0.3.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteTaskPushNotificationConfigRequest {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(default = "default_push_notification_delete_method")]
    pub method: String,
    pub params: DeleteTaskPushNotificationConfigParams,
}

impl Packet for DeleteTaskPushNotificationConfigRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn id(&self) -> Option<&JsonRpcId> {
        self.id.as_ref()
    }
}

impl DeleteTaskPushNotificationConfigRequest {
    pub fn new(params: DeleteTaskPushNotificationConfigParams) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::new_uuid()),
            method: "tasks/pushNotificationConfig/delete".to_string(),
            params,
        }
    }

    pub fn with_id(mut self, id: JsonRpcId) -> Self {
        self.id = Some(id);
        self
    }
}

// ============================================================================
// Response Types - All with #[serde(deny_unknown_fields)]
// ============================================================================

/// JSON-RPC error object
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Response to a send message request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendMessageResponse {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Task>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Response to a get task request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetTaskResponse {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Task>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Response to a cancel task request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelTaskResponse {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Task>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Response to a list tasks request (v0.3.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListTasksResponse {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ListTasksResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Response to a get extended card request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetExtendedCardResponse {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<AgentCard>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Response to a get authenticated extended card request (v0.3.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetAuthenticatedExtendedCardResponse {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<AgentCard>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Response to a set task push notification request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetTaskPushNotificationResponse {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TaskPushNotificationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Response to a get task push notification config request (v0.3.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetTaskPushNotificationConfigResponse {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TaskPushNotificationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Response to a list task push notification configs request (v0.3.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListTaskPushNotificationConfigResponse {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Vec<TaskPushNotificationConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Response to a delete task push notification config request (v0.3.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteTaskPushNotificationConfigResponse {
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

// ============================================================================
// Default functions for serde
// ============================================================================

fn default_jsonrpc() -> String {
    "2.0".to_string()
}

fn default_message_send_method() -> String {
    "message/send".to_string()
}

fn default_message_stream_method() -> String {
    "message/stream".to_string()
}

fn default_tasks_send_method() -> String {
    "tasks/send".to_string()
}

fn default_tasks_send_subscribe_method() -> String {
    "tasks/sendSubscribe".to_string()
}

fn default_tasks_get_method() -> String {
    "tasks/get".to_string()
}

fn default_tasks_cancel_method() -> String {
    "tasks/cancel".to_string()
}

fn default_push_notification_set_method() -> String {
    "tasks/pushNotificationConfig/set".to_string()
}

fn default_push_notification_get_method() -> String {
    "tasks/pushNotificationConfig/get".to_string()
}

fn default_tasks_resubscribe_method() -> String {
    "tasks/resubscribe".to_string()
}

fn default_agent_get_extended_card_method() -> String {
    "agent/getExtendedCard".to_string()
}

fn default_agent_get_authenticated_extended_card_method() -> String {
    "agent/getAuthenticatedExtendedCard".to_string()
}

fn default_tasks_list_method() -> String {
    "tasks/list".to_string()
}

fn default_push_notification_list_method() -> String {
    "tasks/pushNotificationConfig/list".to_string()
}

fn default_push_notification_delete_method() -> String {
    "tasks/pushNotificationConfig/delete".to_string()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_id_display() {
        assert_eq!(JsonRpcId::String("test".to_string()).to_string(), "test");
        assert_eq!(JsonRpcId::Number(42).to_string(), "42");
        assert_eq!(JsonRpcId::Null.to_string(), "null");
    }

    #[test]
    fn test_packet_type_roundtrip() {
        for packet_type in [
            PacketType::SendMessage,
            PacketType::GetTask,
            PacketType::CancelTask,
            PacketType::ListTasks,
        ] {
            let method = packet_type.method();
            let parsed = PacketType::from_method(method);
            assert_eq!(parsed, Some(packet_type));
        }
    }

    #[test]
    fn test_deny_unknown_fields() {
        // This should fail because "unknown_field" is not in the struct
        let json = r#"{
            "jsonrpc": "2.0",
            "id": "test-123",
            "method": "message/send",
            "params": {
                "taskId": "task-456",
                "message": {
                    "role": "user",
                    "parts": [{"text": "Hello"}]
                }
            },
            "unknown_field": "should_fail"
        }"#;

        let result: Result<SendMessageRequest, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Should reject unknown fields");
    }

    #[test]
    fn test_send_message_request_serialization() {
        use crate::domain::{Message, MessageSendParams};

        let message = Message::user_text("Hello".to_string(), "msg-1".to_string());
        let params = MessageSendParams {
            message,
            configuration: None,
            metadata: None,
        };

        let request = SendMessageRequest::new(params);
        let json = serde_json::to_string(&request).unwrap();

        // Verify it deserializes back
        let parsed: SendMessageRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.method, "message/send");
    }
}
