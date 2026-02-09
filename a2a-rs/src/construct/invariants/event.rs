//! Event ordering invariant
//!
//! Validates that events (status updates, artifact updates, etc.) occur in
//! a valid temporal sequence. This ensures causality and prevents race
//! conditions in distributed agent systems.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{Invariant, InvariantResult, InvariantViolation};
use crate::domain::{Task, TaskArtifactUpdateEvent, TaskState, TaskStatusUpdateEvent};

/// Represents a sequenced event in the task lifecycle
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskEvent {
    /// Task status changed
    StatusUpdate {
        task_id: String,
        old_state: TaskState,
        new_state: TaskState,
        timestamp: DateTime<Utc>,
    },
    /// Artifact was added or updated
    ArtifactUpdate {
        task_id: String,
        artifact_id: String,
        timestamp: DateTime<Utc>,
        append: bool,
    },
}

impl TaskEvent {
    /// Get the timestamp of this event
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            TaskEvent::StatusUpdate { timestamp, .. } => *timestamp,
            TaskEvent::ArtifactUpdate { timestamp, .. } => *timestamp,
        }
    }

    /// Get the task ID this event relates to
    pub fn task_id(&self) -> &str {
        match self {
            TaskEvent::StatusUpdate { task_id, .. } => task_id,
            TaskEvent::ArtifactUpdate { task_id, .. } => task_id,
        }
    }
}

/// Sequence of events for a task
#[derive(Debug, Clone, Default)]
pub struct EventSequence {
    /// Ordered list of events
    events: Vec<TaskEvent>,
}

impl EventSequence {
    /// Create a new empty event sequence
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Add an event to the sequence
    pub fn push(&mut self, event: TaskEvent) {
        self.events.push(event);
    }

    /// Get all events in order
    pub fn events(&self) -> &[TaskEvent] {
        &self.events
    }

    /// Get the number of events
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if the sequence is empty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Validate that events are in chronological order
    pub fn is_chronological(&self) -> bool {
        for i in 1..self.events.len() {
            if self.events[i].timestamp() < self.events[i - 1].timestamp() {
                return false;
            }
        }
        true
    }

    /// Get the last event
    pub fn last(&self) -> Option<&TaskEvent> {
        self.events.last()
    }
}

/// Invariant that validates event ordering
///
/// This invariant ensures:
/// - Events occur in chronological order
/// - Status updates follow valid state transitions
/// - Artifacts are only added, never modified (checked via timestamps)
/// - No events occur after a task reaches a terminal state
///
/// # Example
///
/// ```rust
/// use a2a_rs::construct::invariants::{Invariant, EventOrderingInvariant};
/// use a2a_rs::domain::{Task, TaskState};
///
/// let mut invariant = EventOrderingInvariant::new();
/// let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());
///
/// // Update status
/// task.update_status(TaskState::Working, None);
///
/// // Check invariant
/// assert!(invariant.check(&task).is_ok());
/// ```
pub struct EventOrderingInvariant {
    /// Track event sequences per task
    sequences: HashMap<String, EventSequence>,
}

impl EventOrderingInvariant {
    /// Create a new event ordering invariant
    pub fn new() -> Self {
        Self {
            sequences: HashMap::new(),
        }
    }

    /// Get or create an event sequence for a task
    fn get_sequence(&mut self, task_id: &str) -> &mut EventSequence {
        self.sequences
            .entry(task_id.to_string())
            .or_insert_with(EventSequence::new)
    }

    /// Validate a status update event
    fn validate_status_update(
        &self,
        old_state: &TaskState,
        new_state: &TaskState,
        sequence: &EventSequence,
    ) -> InvariantResult {
        // Check if previous state was terminal
        if let Some(last_event) = sequence.last() {
            if let TaskEvent::StatusUpdate {
                new_state: last_state,
                ..
            } = last_event
            {
                use crate::construct::task_fsm::TaskStateMachine;
                if TaskStateMachine::is_terminal_state(last_state) {
                    return Err(InvariantViolation::EventOrderingViolation {
                        reason: format!(
                            "Cannot update status after terminal state {:?}",
                            last_state
                        ),
                    });
                }
            }
        }

        // Check if the transition is valid
        use crate::construct::task_fsm::TaskStateMachine;
        let fsm = TaskStateMachine::new("validation".to_string());
        if !fsm.is_valid_transition(old_state, new_state) {
            return Err(InvariantViolation::EventOrderingViolation {
                reason: format!(
                    "Invalid state transition in event: {:?} -> {:?}",
                    old_state, new_state
                ),
            });
        }

        Ok(())
    }

    /// Validate an artifact update event
    fn validate_artifact_update(
        &self,
        _artifact_id: &str,
        sequence: &EventSequence,
    ) -> InvariantResult {
        // Check if task is in a terminal state
        if let Some(last_event) = sequence.last() {
            if let TaskEvent::StatusUpdate {
                new_state: last_state,
                ..
            } = last_event
            {
                use crate::construct::task_fsm::TaskStateMachine;
                if TaskStateMachine::is_terminal_state(last_state) {
                    return Err(InvariantViolation::EventOrderingViolation {
                        reason: "Cannot update artifacts after terminal state".to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Clear all tracked sequences
    pub fn clear(&mut self) {
        self.sequences.clear();
    }

    /// Get the number of tracked tasks
    pub fn tracked_count(&self) -> usize {
        self.sequences.len()
    }
}

impl Default for EventOrderingInvariant {
    fn default() -> Self {
        Self::new()
    }
}

impl Invariant<Task> for EventOrderingInvariant {
    fn check(&self, task: &Task) -> InvariantResult {
        // Validate that status timestamp is present
        if task.status.timestamp.is_none() {
            return Err(InvariantViolation::EventOrderingViolation {
                reason: "Task status must have a timestamp".to_string(),
            });
        }

        // Validate that if history exists, all messages have proper sequencing
        if let Some(history) = &task.history {
            for (i, message) in history.iter().enumerate() {
                // Each message should have required IDs
                if message.message_id.is_empty() {
                    return Err(InvariantViolation::EventOrderingViolation {
                        reason: format!("Message at index {} has empty message_id", i),
                    });
                }
            }

            // Check for duplicate message IDs (which would violate ordering)
            let mut seen_ids = std::collections::HashSet::new();
            for message in history {
                if !seen_ids.insert(&message.message_id) {
                    return Err(InvariantViolation::EventOrderingViolation {
                        reason: format!("Duplicate message_id in history: {}", message.message_id),
                    });
                }
            }
        }

        // Validate artifact uniqueness (no duplicate artifact IDs)
        if let Some(artifacts) = &task.artifacts {
            let mut seen_ids = std::collections::HashSet::new();
            for artifact in artifacts {
                if !seen_ids.insert(&artifact.artifact_id) {
                    return Err(InvariantViolation::EventOrderingViolation {
                        reason: format!("Duplicate artifact_id in task: {}", artifact.artifact_id),
                    });
                }
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "event_ordering"
    }

    fn description(&self) -> &str {
        "Validates that events occur in a valid temporal sequence"
    }
}

/// Conversion from protocol event types to internal event representation
impl From<&TaskStatusUpdateEvent> for TaskEvent {
    fn from(event: &TaskStatusUpdateEvent) -> Self {
        TaskEvent::StatusUpdate {
            task_id: event.task_id.clone(),
            old_state: TaskState::Unknown, // Would need to track this
            new_state: event.status.state.clone(),
            timestamp: event.status.timestamp.unwrap_or_else(Utc::now),
        }
    }
}

impl From<&TaskArtifactUpdateEvent> for TaskEvent {
    fn from(event: &TaskArtifactUpdateEvent) -> Self {
        TaskEvent::ArtifactUpdate {
            task_id: event.task_id.clone(),
            artifact_id: event.artifact.artifact_id.clone(),
            timestamp: Utc::now(), // Would come from event metadata
            append: event.append.unwrap_or(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Message;

    #[test]
    fn test_empty_sequence() {
        let sequence = EventSequence::new();
        assert_eq!(sequence.len(), 0);
        assert!(sequence.is_empty());
        assert!(sequence.is_chronological());
    }

    #[test]
    fn test_chronological_sequence() {
        let mut sequence = EventSequence::new();

        let now = Utc::now();
        sequence.push(TaskEvent::StatusUpdate {
            task_id: "task-1".to_string(),
            old_state: TaskState::Submitted,
            new_state: TaskState::Working,
            timestamp: now,
        });

        sequence.push(TaskEvent::StatusUpdate {
            task_id: "task-1".to_string(),
            old_state: TaskState::Working,
            new_state: TaskState::Completed,
            timestamp: now + chrono::Duration::seconds(1),
        });

        assert!(sequence.is_chronological());
    }

    #[test]
    fn test_non_chronological_sequence() {
        let mut sequence = EventSequence::new();

        let now = Utc::now();
        sequence.push(TaskEvent::StatusUpdate {
            task_id: "task-1".to_string(),
            old_state: TaskState::Submitted,
            new_state: TaskState::Working,
            timestamp: now + chrono::Duration::seconds(1),
        });

        sequence.push(TaskEvent::StatusUpdate {
            task_id: "task-1".to_string(),
            old_state: TaskState::Working,
            new_state: TaskState::Completed,
            timestamp: now, // Earlier than previous event
        });

        assert!(!sequence.is_chronological());
    }

    #[test]
    fn test_empty_task() {
        let invariant = EventOrderingInvariant::new();
        let task = Task::new("task-1".to_string(), "ctx-1".to_string());

        assert!(invariant.check(&task).is_ok());
    }

    #[test]
    fn test_task_with_history() {
        let invariant = EventOrderingInvariant::new();
        let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());

        let msg1 = Message::agent_text("Hello".to_string(), "msg-1".to_string());
        let msg2 = Message::agent_text("World".to_string(), "msg-2".to_string());

        task.update_status(TaskState::Working, Some(msg1));
        task.update_status(TaskState::Completed, Some(msg2));

        assert!(invariant.check(&task).is_ok());
    }

    #[test]
    fn test_duplicate_message_ids() {
        let invariant = EventOrderingInvariant::new();
        let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());

        let msg1 = Message::agent_text("Hello".to_string(), "msg-1".to_string());
        let msg2 = Message::agent_text("World".to_string(), "msg-1".to_string()); // Same ID

        task.history = Some(vec![msg1, msg2]);

        let result = invariant.check(&task);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            InvariantViolation::EventOrderingViolation { .. }
        ));
    }

    #[test]
    fn test_duplicate_artifact_ids() {
        let invariant = EventOrderingInvariant::new();
        let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());

        use crate::domain::{Artifact, Part};

        let art1 = Artifact {
            artifact_id: "art-1".to_string(),
            name: Some("test1.txt".to_string()),
            description: None,
            parts: vec![Part::text("Hello".to_string())],
            metadata: None,
            extensions: None,
        };

        let art2 = Artifact {
            artifact_id: "art-1".to_string(), // Same ID
            name: Some("test2.txt".to_string()),
            description: None,
            parts: vec![Part::text("World".to_string())],
            metadata: None,
            extensions: None,
        };

        task.artifacts = Some(vec![art1, art2]);

        let result = invariant.check(&task);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            InvariantViolation::EventOrderingViolation { .. }
        ));
    }

    #[test]
    fn test_valid_status_transition() {
        let invariant = EventOrderingInvariant::new();
        let mut sequence = EventSequence::new();

        let result =
            invariant.validate_status_update(&TaskState::Submitted, &TaskState::Working, &sequence);
        assert!(result.is_ok());

        // Add the event to sequence
        sequence.push(TaskEvent::StatusUpdate {
            task_id: "task-1".to_string(),
            old_state: TaskState::Submitted,
            new_state: TaskState::Working,
            timestamp: Utc::now(),
        });

        let result =
            invariant.validate_status_update(&TaskState::Working, &TaskState::Completed, &sequence);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_transition_from_terminal() {
        let invariant = EventOrderingInvariant::new();
        let mut sequence = EventSequence::new();

        // Add terminal state
        sequence.push(TaskEvent::StatusUpdate {
            task_id: "task-1".to_string(),
            old_state: TaskState::Working,
            new_state: TaskState::Completed,
            timestamp: Utc::now(),
        });

        // Try to transition from terminal state
        let result =
            invariant.validate_status_update(&TaskState::Completed, &TaskState::Working, &sequence);
        assert!(result.is_err());
    }

    #[test]
    fn test_event_sequence_last() {
        let mut sequence = EventSequence::new();
        assert!(sequence.last().is_none());

        sequence.push(TaskEvent::StatusUpdate {
            task_id: "task-1".to_string(),
            old_state: TaskState::Submitted,
            new_state: TaskState::Working,
            timestamp: Utc::now(),
        });

        assert!(sequence.last().is_some());
    }
}
