//! Port trait for orchestrating CLM operations on remote A2A agents.
//!
//! This port defines the interface for bridging compiler operations to
//! remote A2A agents (e.g., osiris-macos, osiris-windows).

use crate::domain::{
    A2AOrchestrationTask, OperationPayload, OrchestrationEvent, OrchestrationSnapshot,
};
use async_trait::async_trait;
use std::pin::Pin;
use thiserror::Error;

/// Errors that can occur during orchestration.
#[derive(Debug, Clone, Error)]
pub enum OrchestrationError {
    #[error("Failed to submit task to remote agent: {0}")]
    SubmissionFailed(String),

    #[error("Failed to fetch task status: {0}")]
    StatusFetchFailed(String),

    #[error("Failed to update task artifacts: {0}")]
    ArtifactUpdateFailed(String),

    #[error("Failed to cancel task: {0}")]
    CancellationFailed(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Remote agent error: {0}")]
    RemoteAgentError(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Timeout waiting for task: {0}")]
    Timeout(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Max retries exceeded: {0}")]
    MaxRetriesExceeded(String),
}

impl From<serde_json::Error> for OrchestrationError {
    fn from(err: serde_json::Error) -> Self {
        OrchestrationError::SerializationError(err.to_string())
    }
}

/// Result type for orchestration operations.
pub type OrchestrationResult<T> = Result<T, OrchestrationError>;

/// A stream of orchestration events.
pub type OrchestrationEventStream =
    Pin<Box<dyn futures::stream::Stream<Item = OrchestrationEvent>>>;

/// Port trait for orchestrating A2A tasks on remote agents.
///
/// This trait defines the interface for submitting CLM operations to remote
/// A2A agents and tracking their execution.
#[async_trait]
pub trait A2AOrchestratorPort: Send + Sync {
    /// Submit a new task to a remote A2A agent.
    ///
    /// Creates a task on the remote agent and returns the created task
    /// with assigned IDs and initial state.
    async fn submit_task(
        &self,
        agent_id: &str,
        agent_url: &str,
        context_id: &str,
        operation: OperationPayload,
    ) -> OrchestrationResult<A2AOrchestrationTask>;

    /// Get the current status of a task on a remote agent.
    ///
    /// Fetches the latest state and status message from the remote agent.
    async fn get_task_status(
        &self,
        task: &A2AOrchestrationTask,
    ) -> OrchestrationResult<OrchestrationSnapshot>;

    /// Stream status updates for a task as they occur on the remote agent.
    ///
    /// Returns a stream of events that can be consumed in real-time as the
    /// task progresses. This is typically implemented via Server-Sent Events (SSE),
    /// WebSocket, or polling.
    async fn stream_task_updates(
        &self,
        task: &A2AOrchestrationTask,
    ) -> OrchestrationResult<OrchestrationEventStream>;

    /// Update task artifacts from the remote agent.
    ///
    /// Fetches any new artifacts produced by the task and adds them to the
    /// orchestration task. Returns the updated task with new artifacts.
    async fn update_artifacts(&self, task: &mut A2AOrchestrationTask) -> OrchestrationResult<()>;

    /// Cancel a task on the remote agent.
    ///
    /// Sends a cancellation request to the remote agent. The task state
    /// is updated to `Canceled`.
    async fn cancel_task(&self, task: &mut A2AOrchestrationTask) -> OrchestrationResult<()>;

    /// Retry a failed task on the remote agent.
    ///
    /// Re-submits a failed task. Increments the retry counter and updates
    /// the task state to `Submitting`. Returns an error if max retries exceeded.
    async fn retry_task(
        &self,
        task: &mut A2AOrchestrationTask,
    ) -> OrchestrationResult<A2AOrchestrationTask>;

    /// Wait for a task to complete (reach terminal state) with optional timeout.
    ///
    /// Polls the remote agent until the task reaches a terminal state
    /// (Completed, Failed, Canceled) or the timeout expires.
    async fn wait_for_completion(
        &self,
        task: &mut A2AOrchestrationTask,
        timeout_secs: Option<u64>,
    ) -> OrchestrationResult<OrchestrationSnapshot>;

    /// List all tasks for a given context on a remote agent.
    ///
    /// Returns a list of tasks associated with the given context ID.
    async fn list_tasks(
        &self,
        agent_url: &str,
        context_id: &str,
    ) -> OrchestrationResult<Vec<A2AOrchestrationTask>>;

    /// Check health of a remote A2A agent.
    ///
    /// Verifies connectivity and availability of the agent.
    async fn check_agent_health(&self, agent_url: &str) -> OrchestrationResult<bool>;

    /// Get detailed error information for a failed task from the remote agent.
    ///
    /// Retrieves detailed error context, logs, or diagnostic information
    /// if available from the remote agent.
    async fn get_failure_details(&self, task: &A2AOrchestrationTask)
    -> OrchestrationResult<String>;
}

/// Configuration for the A2A orchestrator.
#[derive(Debug, Clone)]
pub struct A2AOrchestratorConfig {
    /// Default timeout in seconds for operations
    pub timeout_secs: u64,

    /// Whether to automatically retry failed tasks
    pub auto_retry: bool,

    /// Maximum number of retries per task
    pub max_retries: u32,

    /// Delay between retries in milliseconds
    pub retry_delay_ms: u64,

    /// Whether to stream updates via SSE/polling
    pub stream_updates: bool,

    /// Poll interval in milliseconds when streaming (if not using SSE)
    pub poll_interval_ms: u64,

    /// User agent string for HTTP requests
    pub user_agent: String,
}

impl Default for A2AOrchestratorConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 300, // 5 minutes
            auto_retry: true,
            max_retries: 3,
            retry_delay_ms: 1000,
            stream_updates: true,
            poll_interval_ms: 500,
            user_agent: "osiris-compiler/0.1.0".to_string(),
        }
    }
}

/// Trait for managing orchestration task lifecycle.
#[async_trait]
pub trait TaskLifecycleManager: Send + Sync {
    /// Initialize a new task lifecycle.
    async fn initialize_task(&self, task: &mut A2AOrchestrationTask) -> OrchestrationResult<()>;

    /// Handle task completion.
    async fn on_task_completed(&self, task: &A2AOrchestrationTask) -> OrchestrationResult<()>;

    /// Handle task failure.
    async fn on_task_failed(
        &self,
        task: &mut A2AOrchestrationTask,
        error: &str,
    ) -> OrchestrationResult<()>;

    /// Handle task cancellation.
    async fn on_task_canceled(&self, task: &A2AOrchestrationTask) -> OrchestrationResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestration_error_display() {
        let err = OrchestrationError::SubmissionFailed("test error".to_string());
        assert!(err.to_string().contains("test error"));
    }

    #[test]
    fn test_config_defaults() {
        let config = A2AOrchestratorConfig::default();
        assert_eq!(config.timeout_secs, 300);
        assert_eq!(config.max_retries, 3);
        assert!(config.auto_retry);
    }

    #[test]
    fn test_serialization_error_from_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let orch_err: OrchestrationError = json_err.into();
        assert!(matches!(
            orch_err,
            OrchestrationError::SerializationError(_)
        ));
    }
}
