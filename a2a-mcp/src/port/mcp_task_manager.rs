//! Port definition for MCP task management
//!
//! Defines the contract for managing MCP tasks that bridge to A2A tasks.

use async_trait::async_trait;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

use crate::domain::{McpTask, McpTaskError, McpTaskResult};
use crate::error::Result;

/// Type alias for boxed async task operations
pub type BoxedTaskOperation = Box<
    dyn FnOnce() -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'static>> + Send + 'static,
>;

/// Port trait for managing MCP tasks
///
/// This trait defines the contract for task management in MCP,
/// bridging long-running operations to durable task IDs.
#[async_trait]
pub trait McpTaskManager: Send + Sync {
    /// Create a new task and return its ID
    ///
    /// # Arguments
    /// * `operation` - A boxed closure that performs the async operation
    ///
    /// # Returns
    /// The created task with a unique ID
    async fn create_task_boxed(&self, operation: BoxedTaskOperation) -> Result<McpTask>;

    /// Get a task by ID
    ///
    /// # Arguments
    /// * `task_id` - The unique task identifier
    ///
    /// # Returns
    /// The task if found, error otherwise
    async fn get_task(&self, task_id: &str) -> Result<McpTask>;

    /// Get the result of a completed task
    ///
    /// # Arguments
    /// * `task_id` - The unique task identifier
    ///
    /// # Returns
    /// The task result including state and value/error
    async fn get_task_result(&self, task_id: &str) -> Result<McpTaskResult>;

    /// Cancel a running task
    ///
    /// # Arguments
    /// * `task_id` - The unique task identifier
    ///
    /// # Returns
    /// Ok if cancelled successfully
    async fn cancel_task(&self, task_id: &str) -> Result<()>;

    /// List all tasks (optionally filtered)
    ///
    /// # Returns
    /// Vector of all tasks
    async fn list_tasks(&self) -> Result<Vec<McpTask>>;

    /// Clean up completed or failed tasks older than the specified duration
    ///
    /// # Arguments
    /// * `max_age_seconds` - Maximum age in seconds for completed/failed tasks
    ///
    /// # Returns
    /// Number of tasks cleaned up
    async fn cleanup_old_tasks(&self, max_age_seconds: i64) -> Result<usize>;
}

/// Helper function to create an MCP task error
pub fn mcp_task_error(code: i32, message: impl Into<String>) -> McpTaskError {
    McpTaskError {
        code,
        message: message.into(),
        data: None,
    }
}

/// Helper function to create an MCP task error with data
pub fn mcp_task_error_with_data(
    code: i32,
    message: impl Into<String>,
    data: Value,
) -> McpTaskError {
    McpTaskError {
        code,
        message: message.into(),
        data: Some(data),
    }
}
