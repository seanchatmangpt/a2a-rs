//! Task wrapper adapter for MCP-to-A2A task bridging
//!
//! This adapter wraps long-running operations into durable MCP task IDs
//! and bridges them to the A2A task model.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use uuid::Uuid;

use a2a_rs::domain::TaskState as A2aTaskState;
use a2a_rs::port::AsyncTaskManager;

use crate::domain::{McpTask, McpTaskResult, McpTaskState};
use crate::error::{Error, Result};
use crate::port::mcp_task_manager::{McpTaskManager, mcp_task_error};

/// Task wrapper that bridges MCP tasks to A2A tasks
///
/// This adapter maintains a registry of MCP tasks and their associated
/// A2A tasks, handling the mapping between the two task models.
pub struct TaskWrapper {
    /// Registry of MCP tasks
    tasks: Arc<RwLock<HashMap<String, McpTask>>>,
    /// Task execution handles for cancellation
    handles: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    /// Optional A2A task manager for integration
    a2a_task_manager: Option<Arc<dyn AsyncTaskManager>>,
    /// Default context ID for A2A tasks
    default_context_id: String,
}

impl TaskWrapper {
    /// Create a new task wrapper
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            handles: Arc::new(Mutex::new(HashMap::new())),
            a2a_task_manager: None,
            default_context_id: "mcp-default".to_string(),
        }
    }

    /// Create a new task wrapper with A2A task manager integration
    pub fn with_a2a_manager(
        a2a_task_manager: Arc<dyn AsyncTaskManager>,
        default_context_id: String,
    ) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            handles: Arc::new(Mutex::new(HashMap::new())),
            a2a_task_manager: Some(a2a_task_manager),
            default_context_id,
        }
    }

    /// Convert MCP task state to A2A task state
    fn mcp_to_a2a_state(mcp_state: &McpTaskState) -> A2aTaskState {
        match mcp_state {
            McpTaskState::Pending => A2aTaskState::Submitted,
            McpTaskState::Running => A2aTaskState::Working,
            McpTaskState::Completed => A2aTaskState::Completed,
            McpTaskState::Failed => A2aTaskState::Failed,
            McpTaskState::Cancelled => A2aTaskState::Canceled,
        }
    }

    /// Convert A2A task state to MCP task state
    fn a2a_to_mcp_state(a2a_state: &A2aTaskState) -> McpTaskState {
        match a2a_state {
            A2aTaskState::Submitted => McpTaskState::Pending,
            A2aTaskState::Working => McpTaskState::Running,
            A2aTaskState::Completed => McpTaskState::Completed,
            A2aTaskState::Failed => McpTaskState::Failed,
            A2aTaskState::Canceled => McpTaskState::Cancelled,
            A2aTaskState::InputRequired => McpTaskState::Running,
            A2aTaskState::AuthRequired => McpTaskState::Running,
            A2aTaskState::Rejected => McpTaskState::Failed,
            A2aTaskState::Unknown => McpTaskState::Failed,
        }
    }

    /// Synchronize MCP task to A2A task manager if available
    async fn sync_to_a2a(&self, mcp_task: &McpTask) -> Result<()> {
        if let Some(manager) = &self.a2a_task_manager {
            // Check if A2A task exists, create if not
            let task_exists = manager.task_exists(&mcp_task.id).await.unwrap_or(false);

            if !task_exists {
                // Create A2A task
                manager
                    .create_task(&mcp_task.id, &self.default_context_id)
                    .await
                    .map_err(|e| Error::A2a(e.to_string()))?;
            }

            // Update A2A task status
            let a2a_state = Self::mcp_to_a2a_state(&mcp_task.state);
            manager
                .update_task_status(&mcp_task.id, a2a_state, None)
                .await
                .map_err(|e| Error::A2a(e.to_string()))?;
        }

        Ok(())
    }
}

impl Default for TaskWrapper {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskWrapper {
    /// Create a task with ergonomic closure syntax
    ///
    /// This is a convenience method that wraps the closure into a boxed operation
    /// for the trait method.
    pub async fn create_task<F, Fut>(&self, operation: F) -> Result<McpTask>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<Value>> + Send + 'static,
    {
        // Box the operation
        let boxed_op = Box::new(move || {
            Box::pin(operation())
                as Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send + 'static>>
        });
        self.create_task_boxed(boxed_op).await
    }
}

#[async_trait]
impl McpTaskManager for TaskWrapper {
    async fn create_task_boxed(
        &self,
        operation: crate::port::mcp_task_manager::BoxedTaskOperation,
    ) -> Result<McpTask> {
        // Generate unique task ID
        let task_id = Uuid::new_v4().to_string();

        // Create MCP task
        let task = McpTask::new(task_id.clone());

        // Store task
        {
            let mut task_map = self.tasks.write().await;
            task_map.insert(task_id.clone(), task.clone());
        }

        // Sync to A2A if manager is available
        self.sync_to_a2a(&task).await?;

        // Spawn background task
        let tasks_clone = self.tasks.clone();
        let task_id_clone = task_id.clone();
        let handle = tokio::spawn(async move {
            // Mark task as running
            {
                let mut task_map = tasks_clone.write().await;
                if let Some(task) = task_map.get_mut(&task_id_clone) {
                    task.mark_running();
                }
            }

            // Execute the operation
            let future = operation();
            let result = future.await;

            // Update task with result
            {
                let mut task_map = tasks_clone.write().await;
                if let Some(task) = task_map.get_mut(&task_id_clone) {
                    match result {
                        Ok(value) => {
                            task.mark_completed(value);
                        }
                        Err(e) => {
                            let error = mcp_task_error(-32000, e.to_string());
                            task.mark_failed(error);
                        }
                    }
                }
            }
        });

        // Store handle for potential cancellation
        {
            let mut handles = self.handles.lock().await;
            handles.insert(task_id.clone(), handle);
        }

        Ok(task)
    }

    async fn get_task(&self, task_id: &str) -> Result<McpTask> {
        let task_map = self.tasks.read().await;
        task_map
            .get(task_id)
            .cloned()
            .ok_or_else(|| Error::TaskNotFound(task_id.to_string()))
    }

    async fn get_task_result(&self, task_id: &str) -> Result<McpTaskResult> {
        let task = self.get_task(task_id).await?;
        Ok(task.to_result())
    }

    async fn cancel_task(&self, task_id: &str) -> Result<()> {
        // Cancel the task handle if it exists
        {
            let mut handles = self.handles.lock().await;
            if let Some(handle) = handles.remove(task_id) {
                handle.abort();
            }
        }

        // Update task state
        {
            let mut task_map = self.tasks.write().await;
            if let Some(task) = task_map.get_mut(task_id) {
                task.mark_cancelled();

                // Sync to A2A
                if let Some(manager) = &self.a2a_task_manager {
                    let _ = manager
                        .update_task_status(task_id, A2aTaskState::Canceled, None)
                        .await;
                }
            } else {
                return Err(Error::TaskNotFound(task_id.to_string()));
            }
        }

        Ok(())
    }

    async fn list_tasks(&self) -> Result<Vec<McpTask>> {
        let task_map = self.tasks.read().await;
        Ok(task_map.values().cloned().collect())
    }

    async fn cleanup_old_tasks(&self, max_age_seconds: i64) -> Result<usize> {
        let mut task_map = self.tasks.write().await;
        let now = Utc::now();

        let mut to_remove = Vec::new();
        for (task_id, task) in task_map.iter() {
            // Only clean up completed, failed, or cancelled tasks
            if matches!(
                task.state,
                McpTaskState::Completed | McpTaskState::Failed | McpTaskState::Cancelled
            ) {
                let age = now.signed_duration_since(task.updated_at).num_seconds();
                if age > max_age_seconds {
                    to_remove.push(task_id.clone());
                }
            }
        }

        let count = to_remove.len();
        for task_id in to_remove {
            task_map.remove(&task_id);
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_task() {
        let wrapper = TaskWrapper::new();

        let task = wrapper
            .create_task(|| async { Ok(Value::String("test result".to_string())) })
            .await
            .unwrap();

        assert_eq!(task.state, McpTaskState::Pending);

        // Allow task to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let retrieved = wrapper.get_task(&task.id).await.unwrap();
        assert_eq!(retrieved.state, McpTaskState::Completed);
        assert!(retrieved.result.is_some());
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let wrapper = TaskWrapper::new();

        let task = wrapper
            .create_task(|| async {
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                Ok(Value::Null)
            })
            .await
            .unwrap();

        // Cancel the task
        wrapper.cancel_task(&task.id).await.unwrap();

        let retrieved = wrapper.get_task(&task.id).await.unwrap();
        assert_eq!(retrieved.state, McpTaskState::Cancelled);
    }

    #[tokio::test]
    async fn test_task_failure() {
        let wrapper = TaskWrapper::new();

        let task = wrapper
            .create_task(|| async { Err(Error::TaskProcessing("test error".to_string())) })
            .await
            .unwrap();

        // Allow task to fail
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let retrieved = wrapper.get_task(&task.id).await.unwrap();
        assert_eq!(retrieved.state, McpTaskState::Failed);
        assert!(retrieved.error.is_some());
    }

    #[tokio::test]
    async fn test_list_tasks() {
        let wrapper = TaskWrapper::new();

        wrapper
            .create_task(|| async { Ok(Value::Null) })
            .await
            .unwrap();
        wrapper
            .create_task(|| async { Ok(Value::Null) })
            .await
            .unwrap();

        let tasks = wrapper.list_tasks().await.unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[tokio::test]
    async fn test_cleanup_old_tasks() {
        let wrapper = TaskWrapper::new();

        let task = wrapper
            .create_task(|| async { Ok(Value::Null) })
            .await
            .unwrap();

        // Allow task to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Cleanup tasks older than 0 seconds (should remove the completed task)
        let count = wrapper.cleanup_old_tasks(0).await.unwrap();
        assert_eq!(count, 1);

        let tasks = wrapper.list_tasks().await.unwrap();
        assert_eq!(tasks.len(), 0);
    }
}
