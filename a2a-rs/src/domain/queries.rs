//! Query types for CQRS read models
//!
//! Queries represent requests for data from read-optimized views.
//! Unlike commands, queries do not modify state.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::core::{Message, Task, TaskState};

/// Query for retrieving tasks filtered by status.
///
/// Returns tasks in a specific state, useful for displaying
/// tasks by category (e.g., all working tasks, all completed tasks).
///
/// # Example
/// ```rust
/// use a2a_rs::{GetTasksByStatus, TaskState};
///
/// let query = GetTasksByStatus {
///     status: TaskState::Working,
///     limit: Some(50),
///     offset: Some(0),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTasksByStatus {
    /// Task status to filter by
    pub status: TaskState,
    /// Maximum number of results to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,
    /// Number of results to skip (for pagination)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i32>,
    /// Optional context ID filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
}

/// Query for retrieving messages by agent.
///
/// Returns all messages sent to or from a specific agent,
/// useful for audit trails and conversation history.
///
/// # Example
/// ```rust
/// use a2a_rs::GetMessagesByAgent;
/// use chrono::Utc;
///
/// let query = GetMessagesByAgent {
///     agent_id: "agent-123".to_string(),
///     start_time: None,
///     end_time: None,
///     limit: Some(100),
///     offset: Some(0),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMessagesByAgent {
    /// Agent ID to filter messages by
    pub agent_id: String,
    /// Optional start time for time range filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<DateTime<Utc>>,
    /// Optional end time for time range filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,
    /// Maximum number of results to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,
    /// Number of results to skip (for pagination)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i32>,
}

/// Query for retrieving agent statistics.
///
/// Returns aggregated statistics about an agent's activity,
/// including task counts by status and message counts.
///
/// # Example
/// ```rust
/// use a2a_rs::GetAgentStats;
/// use chrono::Utc;
///
/// let query = GetAgentStats {
///     agent_id: "agent-123".to_string(),
///     start_time: None,
///     end_time: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAgentStats {
    /// Agent ID to get statistics for
    pub agent_id: String,
    /// Optional start time for time range filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<DateTime<Utc>>,
    /// Optional end time for time range filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,
}

/// Read model for task list results.
///
/// Optimized view of tasks with minimal data for list displays.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListView {
    /// List of tasks
    pub tasks: Vec<Task>,
    /// Total count (before pagination)
    pub total_count: i32,
    /// Current page offset
    pub offset: i32,
    /// Current page limit
    pub limit: i32,
}

/// Read model for message list results.
///
/// Optimized view of messages with agent context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageListView {
    /// List of messages
    pub messages: Vec<Message>,
    /// Agent ID these messages are associated with
    pub agent_id: String,
    /// Total count (before pagination)
    pub total_count: i32,
    /// Current page offset
    pub offset: i32,
    /// Current page limit
    pub limit: i32,
}

/// Read model for agent statistics.
///
/// Aggregated statistics about an agent's activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatsView {
    /// Agent ID
    pub agent_id: String,
    /// Total number of tasks
    pub total_tasks: i32,
    /// Tasks by status
    pub tasks_by_status: Map<String, Value>,
    /// Total number of messages sent
    pub total_messages_sent: i32,
    /// Total number of messages received
    pub total_messages_received: i32,
    /// Time range for these statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}
