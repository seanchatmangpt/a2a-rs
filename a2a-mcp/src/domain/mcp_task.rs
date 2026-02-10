//! MCP task domain types
//!
//! These types represent MCP-specific task primitives that bridge to A2A tasks.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP task state following MCP specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpTaskState {
    /// Task is pending execution
    Pending,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed with an error
    Failed,
    /// Task was cancelled
    Cancelled,
}

/// MCP task representation
///
/// Wraps long-running operations into durable task IDs for polling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTask {
    /// Unique task identifier
    pub id: String,
    /// Current state of the task
    pub state: McpTaskState,
    /// Optional result when task completes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Optional error when task fails
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpTaskError>,
    /// When the task was created
    pub created_at: DateTime<Utc>,
    /// When the task was last updated
    pub updated_at: DateTime<Utc>,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

/// MCP task error
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTaskError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Optional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Parameters for getting a task
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTaskGetParams {
    /// Task ID to retrieve
    pub task_id: String,
}

/// Parameters for getting task result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTaskResultParams {
    /// Task ID to get result for
    pub task_id: String,
}

/// Result for task result query
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTaskResult {
    /// Task ID
    pub task_id: String,
    /// Task state
    pub state: McpTaskState,
    /// Result value (only present if completed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error (only present if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpTaskError>,
}

impl McpTask {
    /// Create a new pending task
    pub fn new(id: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            state: McpTaskState::Pending,
            result: None,
            error: None,
            created_at: now,
            updated_at: now,
            metadata: None,
        }
    }

    /// Mark task as running
    pub fn mark_running(&mut self) {
        self.state = McpTaskState::Running;
        self.updated_at = Utc::now();
    }

    /// Mark task as completed with result
    pub fn mark_completed(&mut self, result: Value) {
        self.state = McpTaskState::Completed;
        self.result = Some(result);
        self.updated_at = Utc::now();
    }

    /// Mark task as failed with error
    pub fn mark_failed(&mut self, error: McpTaskError) {
        self.state = McpTaskState::Failed;
        self.error = Some(error);
        self.updated_at = Utc::now();
    }

    /// Mark task as cancelled
    pub fn mark_cancelled(&mut self) {
        self.state = McpTaskState::Cancelled;
        self.updated_at = Utc::now();
    }

    /// Convert to task result
    pub fn to_result(&self) -> McpTaskResult {
        McpTaskResult {
            task_id: self.id.clone(),
            state: self.state.clone(),
            result: self.result.clone(),
            error: self.error.clone(),
        }
    }
}
