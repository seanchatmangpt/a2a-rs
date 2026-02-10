//! Task lifecycle events for event sourcing
//!
//! These events represent state changes in the task lifecycle and are stored
//! in an append-only event log. They differ from streaming events (TaskStatusUpdateEvent)
//! which are used for real-time notifications.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::domain::core::{Message, TaskState};

/// Base event metadata common to all task lifecycle events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventMetadata {
    /// Unique event ID
    pub event_id: String,
    /// Timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
    /// Version number for optimistic locking
    pub version: i64,
    /// Optional causation ID (ID of the event that caused this event)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    /// Optional correlation ID (ID linking related events across aggregates)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

/// Event emitted when a task is created
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCreated {
    pub task_id: String,
    pub context_id: String,
    pub metadata: EventMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_metadata: Option<Map<String, Value>>,
}

/// Event emitted when a task transitions to working state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStarted {
    pub task_id: String,
    pub metadata: EventMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
}

/// Event emitted when a task completes successfully
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCompleted {
    pub task_id: String,
    pub metadata: EventMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
}

/// Event emitted when a task fails
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFailed {
    pub task_id: String,
    pub metadata: EventMetadata,
    pub error_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
}

/// Event emitted when a task is canceled
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCanceled {
    pub task_id: String,
    pub metadata: EventMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
}

/// Event emitted when a task requires input
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInputRequired {
    pub task_id: String,
    pub metadata: EventMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
}

/// Event emitted when a task is rejected
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRejected {
    pub task_id: String,
    pub metadata: EventMetadata,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
}

/// Unified task lifecycle event enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "eventType", rename_all = "kebab-case")]
pub enum TaskLifecycleEvent {
    TaskCreated(TaskCreated),
    TaskStarted(TaskStarted),
    TaskCompleted(TaskCompleted),
    TaskFailed(TaskFailed),
    TaskCanceled(TaskCanceled),
    TaskInputRequired(TaskInputRequired),
    TaskRejected(TaskRejected),
}

impl TaskLifecycleEvent {
    /// Get the task ID from any event variant
    pub fn task_id(&self) -> &str {
        match self {
            TaskLifecycleEvent::TaskCreated(e) => &e.task_id,
            TaskLifecycleEvent::TaskStarted(e) => &e.task_id,
            TaskLifecycleEvent::TaskCompleted(e) => &e.task_id,
            TaskLifecycleEvent::TaskFailed(e) => &e.task_id,
            TaskLifecycleEvent::TaskCanceled(e) => &e.task_id,
            TaskLifecycleEvent::TaskInputRequired(e) => &e.task_id,
            TaskLifecycleEvent::TaskRejected(e) => &e.task_id,
        }
    }

    /// Get the event metadata from any event variant
    pub fn metadata(&self) -> &EventMetadata {
        match self {
            TaskLifecycleEvent::TaskCreated(e) => &e.metadata,
            TaskLifecycleEvent::TaskStarted(e) => &e.metadata,
            TaskLifecycleEvent::TaskCompleted(e) => &e.metadata,
            TaskLifecycleEvent::TaskFailed(e) => &e.metadata,
            TaskLifecycleEvent::TaskCanceled(e) => &e.metadata,
            TaskLifecycleEvent::TaskInputRequired(e) => &e.metadata,
            TaskLifecycleEvent::TaskRejected(e) => &e.metadata,
        }
    }

    /// Get the message from any event variant that supports it
    pub fn message(&self) -> Option<&Message> {
        match self {
            TaskLifecycleEvent::TaskCreated(_) => None,
            TaskLifecycleEvent::TaskStarted(e) => e.message.as_ref(),
            TaskLifecycleEvent::TaskCompleted(e) => e.message.as_ref(),
            TaskLifecycleEvent::TaskFailed(e) => e.message.as_ref(),
            TaskLifecycleEvent::TaskCanceled(e) => e.message.as_ref(),
            TaskLifecycleEvent::TaskInputRequired(e) => e.message.as_ref(),
            TaskLifecycleEvent::TaskRejected(e) => e.message.as_ref(),
        }
    }

    /// Convert the event to its corresponding task state
    pub fn to_task_state(&self) -> TaskState {
        match self {
            TaskLifecycleEvent::TaskCreated(_) => TaskState::Submitted,
            TaskLifecycleEvent::TaskStarted(_) => TaskState::Working,
            TaskLifecycleEvent::TaskCompleted(_) => TaskState::Completed,
            TaskLifecycleEvent::TaskFailed(_) => TaskState::Failed,
            TaskLifecycleEvent::TaskCanceled(_) => TaskState::Canceled,
            TaskLifecycleEvent::TaskInputRequired(_) => TaskState::InputRequired,
            TaskLifecycleEvent::TaskRejected(_) => TaskState::Rejected,
        }
    }
}

/// Snapshot of a task's state at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub task_id: String,
    pub context_id: String,
    pub state: TaskState,
    pub version: i64,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<Message>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_metadata: Option<Map<String, Value>>,
}
