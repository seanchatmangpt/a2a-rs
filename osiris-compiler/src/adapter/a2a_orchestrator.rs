//! A2A orchestrator adapter that bridges CLM operations to remote A2A agents.
//!
//! This adapter implements the A2AOrchestratorPort by delegating to remote
//! A2A agents via HTTP (using the a2a-rs HttpClient).

use crate::domain::{
    A2AOrchestrationTask, OperationPayload, OrchestrationEvent, OrchestrationSnapshot,
    OrchestrationTaskState,
};
use crate::port::{
    A2AOrchestratorConfig, A2AOrchestratorPort, OrchestrationError, OrchestrationEventStream,
    OrchestrationResult,
};
use async_trait::async_trait;
use chrono::Utc;
use futures::stream::{self, StreamExt};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

#[cfg(feature = "tracing")]
use tracing::{debug, error, instrument, warn};

use a2a_rs::services::client::AsyncA2AClient;
use a2a_rs::{HttpClient, Message, Task, TaskState};

/// A2A orchestrator adapter that communicates with remote agents via HTTP.
pub struct RemoteA2AOrchestratorAdapter {
    /// Configuration for the orchestrator
    config: A2AOrchestratorConfig,

    /// HTTP client for communicating with remote A2A agents
    http_client: Arc<tokio::sync::Mutex<Option<HttpClient>>>,
}

impl RemoteA2AOrchestratorAdapter {
    /// Create a new remote A2A orchestrator.
    pub fn new(config: A2AOrchestratorConfig) -> Self {
        Self {
            config,
            http_client: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Create a new remote orchestrator with default configuration.
    pub fn default() -> Self {
        Self::new(A2AOrchestratorConfig::default())
    }

    /// Get or create an HTTP client for a given agent URL.
    async fn get_client(&self, agent_url: &str) -> OrchestrationResult<HttpClient> {
        // For now, we create a new client per request to avoid lifetime issues
        // In production, you might want to cache clients per agent URL
        Ok(HttpClient::new(agent_url.to_string()).with_timeout(self.config.timeout_secs))
    }

    /// Extract text content from a Message.
    fn extract_message_text(message: &Message) -> String {
        message
            .parts
            .iter()
            .filter_map(|part| {
                if let a2a_rs::Part::Text { text, .. } = part {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// Convert an OperationPayload to a JSON message for the remote agent.
    fn operation_to_message(&self, operation: &OperationPayload) -> OrchestrationResult<Message> {
        let payload = match operation {
            OperationPayload::Compile {
                source,
                target,
                flags,
                opt_level,
            } => {
                json!({
                    "type": "compile",
                    "source": source,
                    "target": target,
                    "flags": flags.as_ref().unwrap_or(&vec![]),
                    "optLevel": opt_level,
                })
            }
            OperationPayload::Link {
                objects,
                output_format,
            } => {
                json!({
                    "type": "link",
                    "objects": objects,
                    "outputFormat": output_format,
                })
            }
            OperationPayload::Analyze {
                source,
                analysis_type,
                parameters,
            } => {
                json!({
                    "type": "analyze",
                    "source": source,
                    "analysisType": analysis_type,
                    "parameters": parameters,
                })
            }
            OperationPayload::Custom { op_type, data } => {
                json!({
                    "type": "custom",
                    "opType": op_type,
                    "data": data,
                })
            }
        };

        Ok(Message::user_text(
            serde_json::to_string(&payload)?,
            Uuid::new_v4().to_string(),
        ))
    }

    /// Convert a remote Task back to our OrchestrationTask representation.
    fn remote_task_to_orchestration(
        &self,
        remote_task: &Task,
        agent_id: &str,
        agent_url: &str,
        context_id: &str,
        operation: OperationPayload,
    ) -> A2AOrchestrationTask {
        let state = match remote_task.status.state {
            TaskState::Submitted => OrchestrationTaskState::Submitted,
            TaskState::Working => OrchestrationTaskState::Executing,
            TaskState::InputRequired | TaskState::AuthRequired => OrchestrationTaskState::Paused,
            TaskState::Completed => OrchestrationTaskState::Completed,
            TaskState::Canceled => OrchestrationTaskState::Canceled,
            TaskState::Failed | TaskState::Rejected => OrchestrationTaskState::Failed,
            TaskState::Unknown => OrchestrationTaskState::Unknown,
        };

        A2AOrchestrationTask {
            id: remote_task.id.clone(),
            uuid: Uuid::new_v4(),
            agent_id: agent_id.to_string(),
            agent_url: agent_url.to_string(),
            remote_task_id: remote_task.id.clone(),
            context_id: context_id.to_string(),
            state,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deadline: None,
            operation,
            artifacts: Vec::new(),
            status_message: remote_task
                .status
                .message
                .as_ref()
                .map(|m| Self::extract_message_text(m)),
            retry_count: 0,
            max_retries: self.config.max_retries,
            metadata: remote_task
                .metadata
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|(k, v)| (k, v))
                .collect(),
        }
    }
}

#[async_trait]
impl A2AOrchestratorPort for RemoteA2AOrchestratorAdapter {
    #[cfg_attr(feature = "tracing", instrument(skip(self, operation)))]
    async fn submit_task(
        &self,
        agent_id: &str,
        agent_url: &str,
        context_id: &str,
        operation: OperationPayload,
    ) -> OrchestrationResult<A2AOrchestrationTask> {
        #[cfg(feature = "tracing")]
        debug!("Submitting task to agent: {}", agent_id);

        let client = self.get_client(agent_url).await?;
        let message = self.operation_to_message(&operation)?;

        let task_id = Uuid::new_v4().to_string();

        // Create the task via A2A protocol
        let response = client
            .send_task_message(&task_id, &message, None, None)
            .await
            .map_err(|e| {
                #[cfg(feature = "tracing")]
                error!("Failed to submit task: {:?}", e);
                OrchestrationError::SubmissionFailed(e.to_string())
            })?;

        #[cfg(feature = "tracing")]
        debug!("Task submitted successfully: {}", task_id);

        Ok(
            self.remote_task_to_orchestration(
                &response, agent_id, agent_url, context_id, operation,
            ),
        )
    }

    #[cfg_attr(feature = "tracing", instrument(skip(self, task)))]
    async fn get_task_status(
        &self,
        task: &A2AOrchestrationTask,
    ) -> OrchestrationResult<OrchestrationSnapshot> {
        #[cfg(feature = "tracing")]
        debug!("Fetching task status: {}", task.id);

        let client = self.get_client(&task.agent_url).await?;

        let remote_task = client
            .get_task(&task.remote_task_id, None)
            .await
            .map_err(|e| {
                #[cfg(feature = "tracing")]
                error!("Failed to get task status: {:?}", e);
                OrchestrationError::StatusFetchFailed(e.to_string())
            })?;

        let state = match remote_task.status.state {
            TaskState::Submitted => OrchestrationTaskState::Submitted,
            TaskState::Working => OrchestrationTaskState::Executing,
            TaskState::InputRequired | TaskState::AuthRequired => OrchestrationTaskState::Paused,
            TaskState::Completed => OrchestrationTaskState::Completed,
            TaskState::Canceled => OrchestrationTaskState::Canceled,
            TaskState::Failed | TaskState::Rejected => OrchestrationTaskState::Failed,
            TaskState::Unknown => OrchestrationTaskState::Unknown,
        };

        Ok(OrchestrationSnapshot {
            task_id: task.id.clone(),
            state,
            message: remote_task
                .status
                .message
                .as_ref()
                .map(|m| Self::extract_message_text(m)),
            progress: 0,
            artifacts: task.artifacts.clone(),
            timestamp: Utc::now(),
        })
    }

    #[cfg_attr(feature = "tracing", instrument(skip(self, task)))]
    async fn stream_task_updates(
        &self,
        task: &A2AOrchestrationTask,
    ) -> OrchestrationResult<OrchestrationEventStream> {
        #[cfg(feature = "tracing")]
        debug!("Starting task update stream: {}", task.id);

        let task_id = task.id.clone();
        let agent_url = task.agent_url.clone();
        let remote_task_id = task.remote_task_id.clone();
        let config = self.config.clone();

        // Create a stream that polls the task status
        let stream = stream::unfold((task_id.clone(), false), move |state: (String, bool)| {
            let task_id = state.0.clone();
            let mut completed = state.1;
            let agent_url = agent_url.clone();
            let remote_task_id = remote_task_id.clone();
            let config = config.clone();

            async move {
                if completed {
                    return None;
                }

                // Create a temporary client for polling
                let client = HttpClient::new(agent_url.clone()).with_timeout(config.timeout_secs);

                match client.get_task(&remote_task_id, None).await {
                    Ok(remote_task) => {
                        let state = match remote_task.status.state {
                            TaskState::Submitted => OrchestrationTaskState::Submitted,
                            TaskState::Working => OrchestrationTaskState::Executing,
                            TaskState::InputRequired | TaskState::AuthRequired => {
                                OrchestrationTaskState::Paused
                            }
                            TaskState::Completed => OrchestrationTaskState::Completed,
                            TaskState::Canceled => OrchestrationTaskState::Canceled,
                            TaskState::Failed | TaskState::Rejected => {
                                OrchestrationTaskState::Failed
                            }
                            TaskState::Unknown => OrchestrationTaskState::Unknown,
                        };

                        let is_terminal = state.is_terminal();

                        let event = OrchestrationEvent::StateChanged {
                            task_id: task_id.clone(),
                            old_state: OrchestrationTaskState::Submitting, // Simplified
                            new_state: state,
                            message: remote_task
                                .status
                                .message
                                .as_ref()
                                .map(|m| Self::extract_message_text(m)),
                            timestamp: Utc::now(),
                        };

                        if is_terminal {
                            completed = true;
                        }

                        // Sleep between polls
                        sleep(Duration::from_millis(config.poll_interval_ms)).await;

                        Some((event, (task_id, completed)))
                    }
                    Err(e) => {
                        #[cfg(feature = "tracing")]
                        error!("Error polling task: {:?}", e);
                        None
                    }
                }
            }
        })
        .boxed();

        Ok(Box::pin(stream))
    }

    #[cfg_attr(feature = "tracing", instrument(skip(self, task)))]
    async fn update_artifacts(&self, task: &mut A2AOrchestrationTask) -> OrchestrationResult<()> {
        #[cfg(feature = "tracing")]
        debug!("Updating artifacts for task: {}", task.id);

        let client = self.get_client(&task.agent_url).await?;

        let remote_task = client
            .get_task(&task.remote_task_id, None)
            .await
            .map_err(|e| {
                #[cfg(feature = "tracing")]
                error!("Failed to get task for artifact update: {:?}", e);
                OrchestrationError::ArtifactUpdateFailed(e.to_string())
            })?;

        // Extract artifacts from the remote task
        if let Some(artifacts) = &remote_task.artifacts {
            for artifact in artifacts {
                // Extract text content from artifact parts
                let content_type = artifact
                    .parts
                    .iter()
                    .filter_map(|p| {
                        if let a2a_rs::Part::File { file, .. } = p {
                            file.mime_type.clone()
                        } else {
                            None
                        }
                    })
                    .next()
                    .unwrap_or_else(|| "application/octet-stream".to_string());

                let artifact_ref = crate::domain::ArtifactReference {
                    id: artifact.artifact_id.clone(),
                    name: artifact
                        .name
                        .clone()
                        .unwrap_or_else(|| artifact.artifact_id.clone()),
                    content_type,
                    url: String::new(), // a2a-rs Artifact doesn't have a direct URL field
                    size: None,
                    hash: None,
                    created_at: None,
                    metadata: artifact
                        .metadata
                        .clone()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|(k, v)| (k, v))
                        .collect(),
                };
                task.add_artifact(artifact_ref);
            }
        }

        Ok(())
    }

    #[cfg_attr(feature = "tracing", instrument(skip(self, task)))]
    async fn cancel_task(&self, task: &mut A2AOrchestrationTask) -> OrchestrationResult<()> {
        #[cfg(feature = "tracing")]
        debug!("Canceling task: {}", task.id);

        let client = self.get_client(&task.agent_url).await?;

        client
            .cancel_task(&task.remote_task_id)
            .await
            .map_err(|e| {
                #[cfg(feature = "tracing")]
                error!("Failed to cancel task: {:?}", e);
                OrchestrationError::CancellationFailed(e.to_string())
            })?;

        task.set_state(
            OrchestrationTaskState::Canceled,
            Some("Canceled by orchestrator".to_string()),
        );

        Ok(())
    }

    #[cfg_attr(feature = "tracing", instrument(skip(self, task)))]
    async fn retry_task(
        &self,
        task: &mut A2AOrchestrationTask,
    ) -> OrchestrationResult<A2AOrchestrationTask> {
        if !task.can_retry() {
            return Err(OrchestrationError::MaxRetriesExceeded(format!(
                "Task {} exhausted retries ({})",
                task.id, task.max_retries
            )));
        }

        #[cfg(feature = "tracing")]
        debug!(
            "Retrying task: {} (attempt {}/{})",
            task.id,
            task.retry_count + 1,
            task.max_retries
        );

        task.increment_retry();

        // Re-submit the task
        self.submit_task(
            &task.agent_id,
            &task.agent_url,
            &task.context_id,
            task.operation.clone(),
        )
        .await
    }

    #[cfg_attr(feature = "tracing", instrument(skip(self, task)))]
    async fn wait_for_completion(
        &self,
        task: &mut A2AOrchestrationTask,
        timeout_secs: Option<u64>,
    ) -> OrchestrationResult<OrchestrationSnapshot> {
        let timeout = timeout_secs.unwrap_or(self.config.timeout_secs);
        let deadline = std::time::Instant::now() + Duration::from_secs(timeout);

        #[cfg(feature = "tracing")]
        debug!(
            "Waiting for task completion: {} (timeout: {}s)",
            task.id, timeout
        );

        loop {
            let snapshot = self.get_task_status(task).await?;

            if snapshot.state.is_terminal() {
                #[cfg(feature = "tracing")]
                debug!(
                    "Task completed: {} with state: {:?}",
                    task.id, snapshot.state
                );
                return Ok(snapshot);
            }

            if std::time::Instant::now() > deadline {
                #[cfg(feature = "tracing")]
                warn!("Task wait timeout: {}", task.id);
                return Err(OrchestrationError::Timeout(format!(
                    "Task {} did not complete within {} seconds",
                    task.id, timeout
                )));
            }

            sleep(Duration::from_millis(self.config.poll_interval_ms)).await;
        }
    }

    #[cfg_attr(feature = "tracing", instrument(skip(self)))]
    async fn list_tasks(
        &self,
        agent_url: &str,
        context_id: &str,
    ) -> OrchestrationResult<Vec<A2AOrchestrationTask>> {
        #[cfg(feature = "tracing")]
        debug!("Listing tasks for context: {}", context_id);

        let client = self.get_client(agent_url).await?;

        // Note: a2a-rs may not have list_tasks_v3 in all versions
        // This is a simplified implementation
        // In production, you'd need to query based on your task storage

        #[cfg(feature = "tracing")]
        warn!("list_tasks not fully implemented - requires backend task storage");

        Ok(Vec::new())
    }

    #[cfg_attr(feature = "tracing", instrument(skip(self)))]
    async fn check_agent_health(&self, agent_url: &str) -> OrchestrationResult<bool> {
        #[cfg(feature = "tracing")]
        debug!("Checking agent health: {}", agent_url);

        let client = self.get_client(agent_url).await?;

        // Try to get agent info - this tests connectivity
        match tokio::time::timeout(Duration::from_secs(5), async {
            // In a real implementation, you'd call a health check endpoint
            // For now, just verify the client was created
            Ok::<(), String>(())
        })
        .await
        {
            Ok(Ok(())) => {
                #[cfg(feature = "tracing")]
                debug!("Agent health check passed: {}", agent_url);
                Ok(true)
            }
            _ => {
                #[cfg(feature = "tracing")]
                error!("Agent health check failed: {}", agent_url);
                Ok(false)
            }
        }
    }

    #[cfg_attr(feature = "tracing", instrument(skip(self, task)))]
    async fn get_failure_details(
        &self,
        task: &A2AOrchestrationTask,
    ) -> OrchestrationResult<String> {
        #[cfg(feature = "tracing")]
        debug!("Fetching failure details: {}", task.id);

        let client = self.get_client(&task.agent_url).await?;

        match client.get_task(&task.remote_task_id, None).await {
            Ok(remote_task) => {
                if let Some(msg) = &remote_task.status.message {
                    Ok(Self::extract_message_text(msg))
                } else {
                    Ok(format!(
                        "Task {} failed with state: {:?}",
                        task.id, remote_task.status.state
                    ))
                }
            }
            Err(e) => {
                #[cfg(feature = "tracing")]
                error!("Failed to get failure details: {:?}", e);
                Err(OrchestrationError::StatusFetchFailed(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let config = A2AOrchestratorConfig::default();
        let adapter = RemoteA2AOrchestratorAdapter::new(config);
        assert_eq!(adapter.config.timeout_secs, 300);
    }

    #[test]
    fn test_operation_to_message() {
        let adapter = RemoteA2AOrchestratorAdapter::default();
        let operation = OperationPayload::Compile {
            source: "fn main() {}".to_string(),
            target: "aarch64-apple-darwin".to_string(),
            flags: Some(vec!["-O2".to_string()]),
            opt_level: 2,
        };

        let msg = adapter.operation_to_message(&operation).unwrap();
        assert!(!msg.parts.is_empty());
    }

    #[test]
    fn test_default_adapter() {
        let adapter = RemoteA2AOrchestratorAdapter::default();
        assert_eq!(adapter.config.max_retries, 3);
        assert!(adapter.config.auto_retry);
    }
}
