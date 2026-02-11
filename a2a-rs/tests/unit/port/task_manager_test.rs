//! Unit tests for AsyncTaskManager port trait
//!
//! Tests the contract and behavior of the AsyncTaskManager port trait
//! using mock implementations.

use a2a_rs::domain::core::task::{
    Task, TaskIdParams, TaskQueryParams, TaskState, TaskStatus,
};
use a2a_rs::domain::error::A2AError;
use a2a_rs::port::task_manager::AsyncTaskManager;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock implementation of AsyncTaskManager for testing
#[derive(Debug, Clone)]
struct MockTaskManager {
    tasks: Arc<RwLock<HashMap<String, Task>>>,
}

impl MockTaskManager {
    fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn add_task(&self, task: Task) {
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.id.clone(), task);
    }
}

#[async_trait]
impl AsyncTaskManager for MockTaskManager {
    async fn create_task<'a>(
        &self,
        task_id: &'a str,
        context_id: &'a str,
    ) -> Result<Task, A2AError> {
        let task = Task::new(task_id.to_string(), context_id.to_string());
        self.add_task(task.clone()).await;
        Ok(task)
    }

    async fn get_task<'a>(
        &self,
        task_id: &'a str,
        _history_length: Option<u32>,
    ) -> Result<Task, A2AError> {
        let tasks = self.tasks.read().await;
        tasks
            .get(task_id)
            .cloned()
            .ok_or_else(|| A2AError::TaskNotFound(task_id.to_string()))
    }

    async fn update_task_status<'a>(
        &self,
        task_id: &'a str,
        state: TaskState,
        message: Option<a2a_rs::domain::core::message::Message>,
    ) -> Result<Task, A2AError> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| A2AError::TaskNotFound(task_id.to_string()))?;

        task.update_status(state, message);
        Ok(task.clone())
    }

    async fn cancel_task<'a>(&self, task_id: &'a str) -> Result<Task, A2AError> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| A2AError::TaskNotFound(task_id.to_string()))?;

        task.update_status(TaskState::Canceled, None);
        Ok(task.clone())
    }

    async fn task_exists<'a>(&self, task_id: &'a str) -> Result<bool, A2AError> {
        let tasks = self.tasks.read().await;
        Ok(tasks.contains_key(task_id))
    }

    async fn list_tasks<'a>(
        &self,
        context_id: Option<&'a str>,
        _limit: Option<u32>,
    ) -> Result<Vec<Task>, A2AError> {
        let tasks = self.tasks.read().await;
        let result: Vec<Task> = tasks
            .values()
            .filter(|t| context_id.map_or(true, |ctx| t.context_id == ctx))
            .cloned()
            .collect();
        Ok(result)
    }
}

#[tokio::test]
async fn test_create_task() {
    let manager = MockTaskManager::new();

    let task = manager
        .create_task("task-1", "context-1")
        .await
        .expect("Task should be created");

    assert_eq!(task.id, "task-1");
    assert_eq!(task.context_id, "context-1");
    assert_eq!(task.status.state, TaskState::Submitted);

    // Verify task exists
    assert!(manager.task_exists("task-1").await.unwrap());
}

#[tokio::test]
async fn test_get_task() {
    let manager = MockTaskManager::new();

    // Create a task first
    let _ = manager.create_task("task-2", "context-2").await;

    // Get the task
    let task = manager.get_task("task-2", None).await.expect("Task should be found");

    assert_eq!(task.id, "task-2");
    assert_eq!(task.context_id, "context-2");
}

#[tokio::test]
async fn test_get_task_not_found() {
    let manager = MockTaskManager::new();

    let result = manager.get_task("nonexistent", None).await;

    assert!(matches!(result, Err(A2AError::TaskNotFound(_))));
    assert_eq!(result.unwrap_err().to_string(), "Task not found: nonexistent");
}

#[tokio::test]
async fn test_update_task_status() {
    let manager = MockTaskManager::new();

    // Create a task
    let _ = manager.create_task("task-3", "context-3").await;

    // Update status to Working
    let updated = manager
        .update_task_status("task-3", TaskState::Working, None)
        .await
        .expect("Status should be updated");

    assert_eq!(updated.status.state, TaskState::Working);

    // Verify the update persisted
    let task = manager.get_task("task-3", None).await.unwrap();
    assert_eq!(task.status.state, TaskState::Working);
}

#[tokio::test]
async fn test_cancel_task() {
    let manager = MockTaskManager::new();

    // Create a task
    let _ = manager.create_task("task-4", "context-4").await;

    // Cancel the task
    let canceled = manager
        .cancel_task("task-4")
        .await
        .expect("Task should be canceled");

    assert_eq!(canceled.status.state, TaskState::Canceled);

    // Verify the cancellation persisted
    let task = manager.get_task("task-4", None).await.unwrap();
    assert_eq!(task.status.state, TaskState::Canceled);
}

#[tokio::test]
async fn test_cancel_task_not_found() {
    let manager = MockTaskManager::new();

    let result = manager.cancel_task("nonexistent").await;

    assert!(matches!(result, Err(A2AError::TaskNotFound(_))));
}

#[tokio::test]
async fn test_task_exists() {
    let manager = MockTaskManager::new();

    // Task doesn't exist yet
    assert!(!manager.task_exists("task-5").await.unwrap());

    // Create the task
    let _ = manager.create_task("task-5", "context-5").await;

    // Now it exists
    assert!(manager.task_exists("task-5").await.unwrap());
}

#[tokio::test]
async fn test_list_tasks() {
    let manager = MockTaskManager::new();

    // Create multiple tasks
    let _ = manager.create_task("task-6", "context-a").await;
    let _ = manager.create_task("task-7", "context-a").await;
    let _ = manager.create_task("task-8", "context-b").await;

    // List all tasks
    let all_tasks = manager.list_tasks(None, None).await.unwrap();
    assert_eq!(all_tasks.len(), 3);

    // List by context
    let context_tasks = manager.list_tasks(Some("context-a"), None).await.unwrap();
    assert_eq!(context_tasks.len(), 2);
    assert!(context_tasks.iter().all(|t| t.context_id == "context-a"));

    let context_b_tasks = manager.list_tasks(Some("context-b"), None).await.unwrap();
    assert_eq!(context_b_tasks.len(), 1);
}

#[tokio::test]
async fn test_get_task_metadata() {
    let manager = MockTaskManager::new();

    // Create a task (which has no metadata by default)
    let _ = manager.create_task("task-9", "context-9").await;

    // Get metadata - should return empty map
    let metadata = manager.get_task_metadata("task-9").await.unwrap();
    assert!(metadata.is_empty());
}

#[tokio::test]
async fn test_get_task_metadata_not_found() {
    let manager = MockTaskManager::new();

    let result = manager.get_task_metadata("nonexistent").await;

    assert!(matches!(result, Err(A2AError::TaskNotFound(_))));
}

#[tokio::test]
async fn test_validate_task_params_success() {
    let manager = MockTaskManager::new();

    let params = TaskQueryParams {
        id: "valid-task-id".to_string(),
        history_length: Some(10),
        metadata: None,
    };

    let result = manager.validate_task_params(&params).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_validate_task_params_empty_id() {
    let manager = MockTaskManager::new();

    let params = TaskQueryParams {
        id: "   ".to_string(), // Whitespace only
        history_length: None,
        metadata: None,
    };

    let result = manager.validate_task_params(&params).await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
    if let Err(A2AError::ValidationError { field, message }) = result {
        assert_eq!(field, "task_id");
        assert!(message.contains("cannot be empty"));
    }
}

#[tokio::test]
async fn test_validate_task_params_history_too_large() {
    let manager = MockTaskManager::new();

    let params = TaskQueryParams {
        id: "valid-id".to_string(),
        history_length: Some(1001), // Exceeds limit of 1000
        metadata: None,
    };

    let result = manager.validate_task_params(&params).await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
    if let Err(A2AError::ValidationError { field, message }) = result {
        assert_eq!(field, "history_length");
        assert!(message.contains("cannot exceed 1000"));
    }
}

#[tokio::test]
async fn test_validate_task_params_history_at_limit() {
    let manager = MockTaskManager::new();

    let params = TaskQueryParams {
        id: "valid-id".to_string(),
        history_length: Some(1000), // Exactly at limit
        metadata: None,
    };

    let result = manager.validate_task_params(&params).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_task_validated() {
    let manager = MockTaskManager::new();

    // Create a task
    let _ = manager.create_task("task-10", "context-10").await;

    // Get with valid params
    let params = TaskQueryParams {
        id: "task-10".to_string(),
        history_length: Some(5),
        metadata: None,
    };

    let result = manager.get_task_validated(&params).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().id, "task-10");
}

#[tokio::test]
async fn test_get_task_validated_with_invalid_params() {
    let manager = MockTaskManager::new();

    let params = TaskQueryParams {
        id: "".to_string(), // Invalid empty ID
        history_length: None,
        metadata: None,
    };

    let result = manager.get_task_validated(&params).await;
    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
}

#[tokio::test]
async fn test_cancel_task_validated() {
    let manager = MockTaskManager::new();

    // Create a task
    let _ = manager.create_task("task-11", "context-11").await;

    // Cancel with valid params
    let params = TaskIdParams {
        id: "task-11".to_string(),
        metadata: None,
    };

    let result = manager.cancel_task_validated(&params).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().status.state, TaskState::Canceled);
}

#[tokio::test]
async fn test_cancel_task_validated_with_empty_id() {
    let manager = MockTaskManager::new();

    let params = TaskIdParams {
        id: "".to_string(),
        metadata: None,
    };

    let result = manager.cancel_task_validated(&params).await;
    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
}

#[tokio::test]
async fn test_default_list_tasks_v3_unsupported() {
    // This test verifies that the default implementation returns UnsupportedOperation
    struct MinimalTaskManager;

    #[async_trait]
    impl AsyncTaskManager for MinimalTaskManager {
        async fn create_task<'a>(
            &self,
            _task_id: &'a str,
            _context_id: &'a str,
        ) -> Result<Task, A2AError> {
            Err(A2AError::UnsupportedOperation("Not implemented".to_string()))
        }

        async fn get_task<'a>(
            &self,
            _task_id: &'a str,
            _history_length: Option<u32>,
        ) -> Result<Task, A2AError> {
            Err(A2AError::UnsupportedOperation("Not implemented".to_string()))
        }

        async fn update_task_status<'a>(
            &self,
            _task_id: &'a str,
            _state: TaskState,
            _message: Option<a2a_rs::domain::core::message::Message>,
        ) -> Result<Task, A2AError> {
            Err(A2AError::UnsupportedOperation("Not implemented".to_string()))
        }

        async fn cancel_task<'a>(&self, _task_id: &'a str) -> Result<Task, A2AError> {
            Err(A2AError::UnsupportedOperation("Not implemented".to_string()))
        }

        async fn task_exists<'a>(&self, _task_id: &'a str) -> Result<bool, A2AError> {
            Err(A2AError::UnsupportedOperation("Not implemented".to_string()))
        }
    }

    let manager = MinimalTaskManager;
    let params = &a2a_rs::domain::core::task::ListTasksParams::default();

    let result = manager.list_tasks_v3(params).await;

    assert!(matches!(
        result,
        Err(A2AError::UnsupportedOperation(_))
    ));
}

#[tokio::test]
async fn test_concurrent_task_operations() {
    let manager = MockTaskManager::new();

    // Spawn multiple concurrent tasks
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let manager = manager.clone();
            tokio::spawn(async move {
                let task_id = format!("task-concurrent-{}", i);
                manager.create_task(&task_id, "context-concurrent").await
            })
        })
        .collect();

    // Wait for all to complete
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }

    // Verify all tasks exist
    for i in 0..10 {
        let task_id = format!("task-concurrent-{}", i);
        assert!(manager.task_exists(&task_id).await.unwrap());
    }
}

#[tokio::test]
async fn test_task_state_transitions() {
    let manager = MockTaskManager::new();

    // Create task in Submitted state
    let _ = manager.create_task("task-12", "context-12").await;
    let task = manager.get_task("task-12", None).await.unwrap();
    assert_eq!(task.status.state, TaskState::Submitted);

    // Transition to Working
    manager
        .update_task_status("task-12", TaskState::Working, None)
        .await
        .unwrap();
    let task = manager.get_task("task-12", None).await.unwrap();
    assert_eq!(task.status.state, TaskState::Working);

    // Transition to Completed
    manager
        .update_task_status("task-12", TaskState::Completed, None)
        .await
        .unwrap();
    let task = manager.get_task("task-12", None).await.unwrap();
    assert_eq!(task.status.state, TaskState::Completed);
}
