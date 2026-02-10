//! Domain types for bridging CLM operations to remote A2A agents.
//!
//! This module defines types that represent the state and lifecycle of tasks
//! orchestrated across remote A2A agents (e.g., osiris-macos, osiris-windows).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// A CLM (Compiler Lambda Manager) task bound to a remote A2A agent.
///
/// Maps compiler operations to A2A protocol tasks for distributed execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct A2AOrchestrationTask {
    /// Unique identifier for this orchestration task
    pub id: String,

    /// UUID for internal tracking and deduplication
    pub uuid: Uuid,

    /// Remote agent identifier (e.g., "osiris-macos", "osiris-windows")
    pub agent_id: String,

    /// Base URL of the remote A2A agent
    pub agent_url: String,

    /// The A2A task ID on the remote agent
    pub remote_task_id: String,

    /// Context ID for the compilation session
    pub context_id: String,

    /// Current state of the orchestration task
    pub state: OrchestrationTaskState,

    /// Timestamp when the task was created
    pub created_at: DateTime<Utc>,

    /// Timestamp of the last state update
    pub updated_at: DateTime<Utc>,

    /// Optional deadline for task completion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<DateTime<Utc>>,

    /// Operation details sent to the remote agent
    pub operation: OperationPayload,

    /// Artifacts produced by the remote agent
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub artifacts: Vec<ArtifactReference>,

    /// Current status message from the remote agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,

    /// Retry count if the task failed and was retried
    #[serde(default)]
    pub retry_count: u32,

    /// Maximum number of retries allowed
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Optional metadata for tracking and debugging
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: HashMap<String, Value>,
}

/// Default max retries for a task
fn default_max_retries() -> u32 {
    3
}

/// The state of an orchestration task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrchestrationTaskState {
    /// Task is being submitted to the remote agent
    Submitting,

    /// Task has been submitted and is queued on remote agent
    Submitted,

    /// Task is currently executing on the remote agent
    Executing,

    /// Task execution is paused (awaiting input or input required)
    Paused,

    /// Task has completed successfully
    Completed,

    /// Task was canceled
    Canceled,

    /// Task encountered an error
    Failed,

    /// Task state could not be determined
    Unknown,
}

impl OrchestrationTaskState {
    /// Returns true if the task is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            OrchestrationTaskState::Completed
                | OrchestrationTaskState::Canceled
                | OrchestrationTaskState::Failed
        )
    }

    /// Returns true if the task is still running or waiting.
    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }
}

/// The operation payload sent to a remote agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum OperationPayload {
    /// Compilation operation (parse, type-check, codegen, etc.)
    Compile {
        /// The source code or input to compile
        source: String,

        /// Target platform (e.g., "aarch64-apple-darwin", "x86_64-pc-windows-gnu")
        target: String,

        /// Compilation flags/options
        #[serde(skip_serializing_if = "Option::is_none")]
        flags: Option<Vec<String>>,

        /// Optimization level (0-3)
        #[serde(default)]
        opt_level: u8,
    },

    /// Link/assemble operation
    Link {
        /// List of object file URLs or paths
        objects: Vec<String>,

        /// Output format (e.g., "elf", "mach-o", "pe")
        output_format: String,
    },

    /// Analysis operation (type checking, invariant checking, etc.)
    Analyze {
        /// The code or artifact to analyze
        source: String,

        /// Analysis type (e.g., "type-check", "invariant-verify")
        analysis_type: String,

        /// Optional analysis parameters
        #[serde(skip_serializing_if = "HashMap::is_empty", default)]
        parameters: HashMap<String, Value>,
    },

    /// Custom operation with arbitrary payload
    Custom {
        /// Operation name
        op_type: String,

        /// Arbitrary operation data
        data: Value,
    },
}

/// A reference to an artifact produced by a remote agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactReference {
    /// Unique artifact identifier
    pub id: String,

    /// Human-readable artifact name
    pub name: String,

    /// MIME type or artifact kind (e.g., "application/octet-stream", "text/plain")
    pub content_type: String,

    /// URL where the artifact can be downloaded
    pub url: String,

    /// Size in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,

    /// SHA-256 hash for integrity verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,

    /// Timestamp when the artifact was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,

    /// Optional metadata about the artifact
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: HashMap<String, Value>,
}

/// A snapshot of task status at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrationSnapshot {
    /// Task ID
    pub task_id: String,

    /// Current state
    pub state: OrchestrationTaskState,

    /// Status message from the agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Percentage complete (0-100)
    #[serde(default)]
    pub progress: u8,

    /// Artifacts accumulated so far
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub artifacts: Vec<ArtifactReference>,

    /// Timestamp of this snapshot
    pub timestamp: DateTime<Utc>,
}

/// Events emitted during task orchestration (for streaming/SSE).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "eventType")]
pub enum OrchestrationEvent {
    /// Task state transitioned
    StateChanged {
        task_id: String,
        old_state: OrchestrationTaskState,
        new_state: OrchestrationTaskState,
        message: Option<String>,
        timestamp: DateTime<Utc>,
    },

    /// Task made progress
    ProgressUpdate {
        task_id: String,
        progress: u8,
        message: Option<String>,
        timestamp: DateTime<Utc>,
    },

    /// An artifact was produced
    ArtifactProduced {
        task_id: String,
        artifact: ArtifactReference,
        timestamp: DateTime<Utc>,
    },

    /// Task encountered a retryable error
    RetryScheduled {
        task_id: String,
        attempt: u32,
        max_attempts: u32,
        reason: String,
        retry_after_ms: u64,
        timestamp: DateTime<Utc>,
    },

    /// Task completed with result
    Completed {
        task_id: String,
        success: bool,
        message: String,
        artifacts: Vec<ArtifactReference>,
        timestamp: DateTime<Utc>,
    },
}

impl A2AOrchestrationTask {
    /// Create a new orchestration task.
    pub fn new(
        agent_id: String,
        agent_url: String,
        remote_task_id: String,
        context_id: String,
        operation: OperationPayload,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            uuid: Uuid::new_v4(),
            agent_id,
            agent_url,
            remote_task_id,
            context_id,
            state: OrchestrationTaskState::Submitting,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deadline: None,
            operation,
            artifacts: Vec::new(),
            status_message: None,
            retry_count: 0,
            max_retries: default_max_retries(),
            metadata: HashMap::new(),
        }
    }

    /// Update the task state and timestamp.
    pub fn set_state(&mut self, new_state: OrchestrationTaskState, message: Option<String>) {
        self.state = new_state;
        self.updated_at = Utc::now();
        if message.is_some() {
            self.status_message = message;
        }
    }

    /// Add an artifact to the task.
    pub fn add_artifact(&mut self, artifact: ArtifactReference) {
        self.artifacts.push(artifact);
        self.updated_at = Utc::now();
    }

    /// Check if the task can be retried.
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries && self.state == OrchestrationTaskState::Failed
    }

    /// Increment retry count.
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
        if self.can_retry() {
            self.state = OrchestrationTaskState::Submitting;
        }
        self.updated_at = Utc::now();
    }

    /// Create a snapshot of the current task state.
    pub fn snapshot(&self) -> OrchestrationSnapshot {
        OrchestrationSnapshot {
            task_id: self.id.clone(),
            state: self.state,
            message: self.status_message.clone(),
            progress: 0, // TODO: derive from state
            artifacts: self.artifacts.clone(),
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = A2AOrchestrationTask::new(
            "osiris-macos".to_string(),
            "https://macos-agent.local/api".to_string(),
            "task-123".to_string(),
            "ctx-456".to_string(),
            OperationPayload::Compile {
                source: "fn main() {}".to_string(),
                target: "aarch64-apple-darwin".to_string(),
                flags: None,
                opt_level: 2,
            },
        );

        assert_eq!(task.agent_id, "osiris-macos");
        assert_eq!(task.state, OrchestrationTaskState::Submitting);
        assert_eq!(task.retry_count, 0);
        assert_eq!(task.max_retries, 3);
    }

    #[test]
    fn test_state_transitions() {
        let mut task = A2AOrchestrationTask::new(
            "osiris-macos".to_string(),
            "https://macos-agent.local/api".to_string(),
            "task-123".to_string(),
            "ctx-456".to_string(),
            OperationPayload::Compile {
                source: "fn main() {}".to_string(),
                target: "aarch64-apple-darwin".to_string(),
                flags: None,
                opt_level: 2,
            },
        );

        assert!(!task.state.is_terminal());

        task.set_state(
            OrchestrationTaskState::Executing,
            Some("Running".to_string()),
        );
        assert_eq!(task.state, OrchestrationTaskState::Executing);
        assert!(!task.state.is_terminal());

        task.set_state(OrchestrationTaskState::Completed, Some("Done".to_string()));
        assert!(task.state.is_terminal());
    }

    #[test]
    fn test_artifact_addition() {
        let mut task = A2AOrchestrationTask::new(
            "osiris-macos".to_string(),
            "https://macos-agent.local/api".to_string(),
            "task-123".to_string(),
            "ctx-456".to_string(),
            OperationPayload::Compile {
                source: "fn main() {}".to_string(),
                target: "aarch64-apple-darwin".to_string(),
                flags: None,
                opt_level: 2,
            },
        );

        assert!(task.artifacts.is_empty());

        let artifact = ArtifactReference {
            id: "art-1".to_string(),
            name: "output.o".to_string(),
            content_type: "application/octet-stream".to_string(),
            url: "https://macos-agent.local/artifacts/output.o".to_string(),
            size: Some(1024),
            hash: None,
            created_at: None,
            metadata: HashMap::new(),
        };

        task.add_artifact(artifact);
        assert_eq!(task.artifacts.len(), 1);
    }

    #[test]
    fn test_retry_logic() {
        let mut task = A2AOrchestrationTask::new(
            "osiris-macos".to_string(),
            "https://macos-agent.local/api".to_string(),
            "task-123".to_string(),
            "ctx-456".to_string(),
            OperationPayload::Compile {
                source: "fn main() {}".to_string(),
                target: "aarch64-apple-darwin".to_string(),
                flags: None,
                opt_level: 2,
            },
        );

        task.set_state(
            OrchestrationTaskState::Failed,
            Some("Network timeout".to_string()),
        );
        assert!(task.can_retry());

        task.increment_retry();
        assert_eq!(task.retry_count, 1);
        assert_eq!(task.state, OrchestrationTaskState::Submitting);

        // Exhaust retries
        task.retry_count = 3;
        assert!(!task.can_retry());
    }

    #[test]
    fn test_orchestration_event_serialization() {
        let event = OrchestrationEvent::StateChanged {
            task_id: "task-123".to_string(),
            old_state: OrchestrationTaskState::Submitting,
            new_state: OrchestrationTaskState::Executing,
            message: Some("Started".to_string()),
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: OrchestrationEvent = serde_json::from_str(&json).unwrap();

        assert!(matches!(
            deserialized,
            OrchestrationEvent::StateChanged { .. }
        ));
    }
}
