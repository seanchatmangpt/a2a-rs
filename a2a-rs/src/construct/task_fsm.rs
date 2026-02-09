//! Finite-state machine for task execution lifecycle
//!
//! This module implements a deterministic FSM for managing task state transitions.
//! Tasks are work orders with a well-defined lifecycle from submission to completion.
//!
//! # State Transition Graph
//!
//! ```text
//! submitted ──────> rejected (terminal)
//!    │
//!    ├──────> auth-required ──────> working
//!    │
//!    └──────> working ──────┬──────> completed (terminal)
//!                           │
//!                           ├──────> failed (terminal)
//!                           │
//!                           ├──────> canceled (terminal)
//!                           │
//!                           └──────> input-required ──────> working
//! ```
//!
//! # Example
//!
//! ```rust
//! use a2a_rs::construct::{TaskStateMachine, TransitionGuard};
//! use a2a_rs::domain::TaskState;
//!
//! let mut fsm = TaskStateMachine::new("task-123".to_string());
//! assert_eq!(fsm.current_state(), &TaskState::Submitted);
//!
//! // Transition to working
//! let result = fsm.transition_to(TaskState::Working, None, None);
//! assert!(result.is_ok());
//! assert_eq!(fsm.current_state(), &TaskState::Working);
//!
//! // Cannot transition back to submitted
//! let result = fsm.transition_to(TaskState::Submitted, None, None);
//! assert!(result.is_err());
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use crate::domain::{Artifact, FileContent, Message, Part, TaskState};

/// Errors that can occur during state transitions
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum StateTransitionError {
    /// Attempted transition is not allowed by the FSM rules
    #[error("Invalid transition from {from:?} to {to:?}")]
    InvalidTransition { from: TaskState, to: TaskState },

    /// Attempted to transition from a terminal state
    #[error("Cannot transition from terminal state {state:?}")]
    TransitionFromTerminalState { state: TaskState },

    /// Transition guard rejected the transition
    #[error("Transition guard rejected: {reason}")]
    GuardRejected { reason: String },

    /// Custom error from user-provided guard
    #[error("Custom guard error: {0}")]
    Custom(String),
}

/// Result of a state transition attempt
pub type TransitionResult<T> = Result<T, StateTransitionError>;

/// Record of a state transition with timestamp and optional context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    /// State before transition
    pub from: TaskState,
    /// State after transition
    pub to: TaskState,
    /// When the transition occurred
    pub timestamp: DateTime<Utc>,
    /// Optional message associated with the transition
    pub message: Option<Message>,
    /// Artifacts emitted during this transition
    pub artifacts: Vec<Artifact>,
}

/// Guard function type for validating transitions
///
/// Returns `Ok(())` if the transition is allowed, or an error describing why not.
pub type TransitionGuard =
    Box<dyn Fn(&TaskState, &TaskState, Option<&Message>) -> TransitionResult<()> + Send + Sync>;

/// Finite-state machine for task execution lifecycle
///
/// Manages state transitions for a task with deterministic rules and guards.
/// Emits artifacts on state changes and maintains a complete transition history.
pub struct TaskStateMachine {
    /// Unique identifier for the task
    task_id: String,
    /// Current state of the task
    current_state: TaskState,
    /// History of all state transitions
    transition_history: Vec<StateTransition>,
    /// Valid transitions defined by the FSM
    valid_transitions: HashMap<TaskState, Vec<TaskState>>,
    /// Optional custom guards (not serializable, so we don't derive Serialize)
    #[allow(clippy::type_complexity)]
    guards: Option<
        HashMap<
            (TaskState, TaskState),
            Box<dyn Fn(Option<&Message>) -> TransitionResult<()> + Send + Sync>,
        >,
    >,
}

impl Clone for TaskStateMachine {
    fn clone(&self) -> Self {
        Self {
            task_id: self.task_id.clone(),
            current_state: self.current_state.clone(),
            transition_history: self.transition_history.clone(),
            valid_transitions: self.valid_transitions.clone(),
            // Guards cannot be cloned (function pointers), so we skip them
            guards: None,
        }
    }
}

impl std::fmt::Debug for TaskStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskStateMachine")
            .field("task_id", &self.task_id)
            .field("current_state", &self.current_state)
            .field("transition_history", &self.transition_history)
            .field("valid_transitions", &self.valid_transitions)
            .field("guards", &"<function>")
            .finish()
    }
}

impl TaskStateMachine {
    /// Create a new FSM in the Submitted state
    pub fn new(task_id: String) -> Self {
        Self {
            task_id,
            current_state: TaskState::Submitted,
            transition_history: Vec::new(),
            valid_transitions: Self::default_transitions(),
            guards: None,
        }
    }

    /// Create a new FSM with custom transition rules
    pub fn with_transitions(
        task_id: String,
        valid_transitions: HashMap<TaskState, Vec<TaskState>>,
    ) -> Self {
        Self {
            task_id,
            current_state: TaskState::Submitted,
            transition_history: Vec::new(),
            valid_transitions,
            guards: None,
        }
    }

    /// Define the default valid state transitions
    fn default_transitions() -> HashMap<TaskState, Vec<TaskState>> {
        let mut transitions = HashMap::new();

        // From Submitted
        transitions.insert(
            TaskState::Submitted,
            vec![
                TaskState::Working,
                TaskState::Rejected,
                TaskState::AuthRequired,
            ],
        );

        // From Working
        transitions.insert(
            TaskState::Working,
            vec![
                TaskState::InputRequired,
                TaskState::Completed,
                TaskState::Failed,
                TaskState::Canceled,
            ],
        );

        // From InputRequired
        transitions.insert(TaskState::InputRequired, vec![TaskState::Working]);

        // From AuthRequired
        transitions.insert(TaskState::AuthRequired, vec![TaskState::Working]);

        // Terminal states have no outgoing transitions
        transitions.insert(TaskState::Completed, vec![]);
        transitions.insert(TaskState::Failed, vec![]);
        transitions.insert(TaskState::Canceled, vec![]);
        transitions.insert(TaskState::Rejected, vec![]);
        transitions.insert(TaskState::Unknown, vec![]);

        transitions
    }

    /// Get the current state
    pub fn current_state(&self) -> &TaskState {
        &self.current_state
    }

    /// Get the task ID
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Check if the current state is terminal
    pub fn is_terminal(&self) -> bool {
        Self::is_terminal_state(&self.current_state)
    }

    /// Check if a given state is terminal
    pub fn is_terminal_state(state: &TaskState) -> bool {
        matches!(
            state,
            TaskState::Completed
                | TaskState::Failed
                | TaskState::Canceled
                | TaskState::Rejected
                | TaskState::Unknown
        )
    }

    /// Check if a transition from one state to another is valid
    pub fn is_valid_transition(&self, from: &TaskState, to: &TaskState) -> bool {
        self.valid_transitions
            .get(from)
            .map(|allowed| allowed.contains(to))
            .unwrap_or(false)
    }

    /// Check if a transition to the target state is allowed from current state
    pub fn can_transition_to(&self, to: &TaskState) -> bool {
        !self.is_terminal() && self.is_valid_transition(&self.current_state, to)
    }

    /// Get all valid transitions from the current state
    pub fn allowed_transitions(&self) -> Vec<&TaskState> {
        self.valid_transitions
            .get(&self.current_state)
            .map(|states| states.iter().collect())
            .unwrap_or_default()
    }

    /// Get the complete transition history
    pub fn history(&self) -> &[StateTransition] {
        &self.transition_history
    }

    /// Attempt to transition to a new state
    ///
    /// # Arguments
    ///
    /// * `to` - Target state
    /// * `message` - Optional message to associate with the transition
    /// * `artifacts` - Optional artifacts to emit during the transition
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Current state is terminal
    /// - Transition is not valid according to FSM rules
    /// - A guard rejects the transition
    pub fn transition_to(
        &mut self,
        to: TaskState,
        message: Option<Message>,
        artifacts: Option<Vec<Artifact>>,
    ) -> TransitionResult<StateTransition> {
        // Check if we're in a terminal state
        if self.is_terminal() {
            return Err(StateTransitionError::TransitionFromTerminalState {
                state: self.current_state.clone(),
            });
        }

        // Check if transition is valid
        if !self.is_valid_transition(&self.current_state, &to) {
            return Err(StateTransitionError::InvalidTransition {
                from: self.current_state.clone(),
                to,
            });
        }

        // Check custom guards if present
        if let Some(guards) = &self.guards {
            let key = (self.current_state.clone(), to.clone());
            if let Some(guard) = guards.get(&key) {
                guard(message.as_ref())?;
            }
        }

        // Perform the transition
        let transition = StateTransition {
            from: self.current_state.clone(),
            to: to.clone(),
            timestamp: Utc::now(),
            message,
            artifacts: artifacts.unwrap_or_default(),
        };

        self.current_state = to;
        self.transition_history.push(transition.clone());

        Ok(transition)
    }

    /// Add a custom guard for a specific transition
    ///
    /// The guard will be called before the transition is performed.
    /// If it returns an error, the transition is rejected.
    pub fn add_guard<F>(&mut self, from: TaskState, to: TaskState, guard: F)
    where
        F: Fn(Option<&Message>) -> TransitionResult<()> + Send + Sync + 'static,
    {
        if self.guards.is_none() {
            self.guards = Some(HashMap::new());
        }
        if let Some(guards) = &mut self.guards {
            guards.insert((from, to), Box::new(guard));
        }
    }

    /// Transition to Working state (common operation)
    pub fn start_working(&mut self, message: Option<Message>) -> TransitionResult<StateTransition> {
        self.transition_to(TaskState::Working, message, None)
    }

    /// Transition to Completed state (terminal)
    pub fn complete(
        &mut self,
        message: Option<Message>,
        artifacts: Option<Vec<Artifact>>,
    ) -> TransitionResult<StateTransition> {
        self.transition_to(TaskState::Completed, message, artifacts)
    }

    /// Transition to Failed state (terminal)
    pub fn fail(
        &mut self,
        message: Option<Message>,
        artifacts: Option<Vec<Artifact>>,
    ) -> TransitionResult<StateTransition> {
        self.transition_to(TaskState::Failed, message, artifacts)
    }

    /// Transition to Canceled state (terminal)
    pub fn cancel(
        &mut self,
        message: Option<Message>,
        artifacts: Option<Vec<Artifact>>,
    ) -> TransitionResult<StateTransition> {
        self.transition_to(TaskState::Canceled, message, artifacts)
    }

    /// Transition to InputRequired state
    pub fn request_input(&mut self, message: Option<Message>) -> TransitionResult<StateTransition> {
        self.transition_to(TaskState::InputRequired, message, None)
    }

    /// Transition to Rejected state (terminal)
    pub fn reject(&mut self, message: Option<Message>) -> TransitionResult<StateTransition> {
        self.transition_to(TaskState::Rejected, message, None)
    }

    /// Get the most recent transition
    pub fn last_transition(&self) -> Option<&StateTransition> {
        self.transition_history.last()
    }

    /// Count transitions
    pub fn transition_count(&self) -> usize {
        self.transition_history.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let fsm = TaskStateMachine::new("task-1".to_string());
        assert_eq!(fsm.current_state(), &TaskState::Submitted);
        assert!(!fsm.is_terminal());
        assert_eq!(fsm.transition_count(), 0);
    }

    #[test]
    fn test_valid_transition_submitted_to_working() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());
        let result = fsm.start_working(None);
        assert!(result.is_ok());
        assert_eq!(fsm.current_state(), &TaskState::Working);
        assert_eq!(fsm.transition_count(), 1);
    }

    #[test]
    fn test_valid_transition_working_to_completed() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());
        fsm.start_working(None).unwrap();
        let result = fsm.complete(None, None);
        assert!(result.is_ok());
        assert_eq!(fsm.current_state(), &TaskState::Completed);
        assert!(fsm.is_terminal());
    }

    #[test]
    fn test_valid_transition_working_to_input_required() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());
        fsm.start_working(None).unwrap();
        let result = fsm.request_input(None);
        assert!(result.is_ok());
        assert_eq!(fsm.current_state(), &TaskState::InputRequired);
        assert!(!fsm.is_terminal());
    }

    #[test]
    fn test_valid_transition_input_required_to_working() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());
        fsm.start_working(None).unwrap();
        fsm.request_input(None).unwrap();
        let result = fsm.start_working(None);
        assert!(result.is_ok());
        assert_eq!(fsm.current_state(), &TaskState::Working);
    }

    #[test]
    fn test_invalid_transition_submitted_to_completed() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());
        let result = fsm.complete(None, None);
        assert!(result.is_err());
        match result.unwrap_err() {
            StateTransitionError::InvalidTransition { from, to } => {
                assert_eq!(from, TaskState::Submitted);
                assert_eq!(to, TaskState::Completed);
            }
            _ => panic!("Expected InvalidTransition error"),
        }
    }

    #[test]
    fn test_cannot_transition_from_terminal_state() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());
        fsm.start_working(None).unwrap();
        fsm.complete(None, None).unwrap();

        let result = fsm.start_working(None);
        assert!(result.is_err());
        match result.unwrap_err() {
            StateTransitionError::TransitionFromTerminalState { state } => {
                assert_eq!(state, TaskState::Completed);
            }
            _ => panic!("Expected TransitionFromTerminalState error"),
        }
    }

    #[test]
    fn test_terminal_states() {
        assert!(TaskStateMachine::is_terminal_state(&TaskState::Completed));
        assert!(TaskStateMachine::is_terminal_state(&TaskState::Failed));
        assert!(TaskStateMachine::is_terminal_state(&TaskState::Canceled));
        assert!(TaskStateMachine::is_terminal_state(&TaskState::Rejected));
        assert!(!TaskStateMachine::is_terminal_state(&TaskState::Working));
        assert!(!TaskStateMachine::is_terminal_state(&TaskState::Submitted));
    }

    #[test]
    fn test_allowed_transitions() {
        let fsm = TaskStateMachine::new("task-1".to_string());
        let allowed = fsm.allowed_transitions();
        assert_eq!(allowed.len(), 3);
        assert!(allowed.contains(&&TaskState::Working));
        assert!(allowed.contains(&&TaskState::Rejected));
        assert!(allowed.contains(&&TaskState::AuthRequired));
    }

    #[test]
    fn test_transition_history() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());
        fsm.start_working(None).unwrap();
        fsm.request_input(None).unwrap();
        fsm.start_working(None).unwrap();
        fsm.complete(None, None).unwrap();

        assert_eq!(fsm.transition_count(), 4);
        let history = fsm.history();
        assert_eq!(history[0].from, TaskState::Submitted);
        assert_eq!(history[0].to, TaskState::Working);
        assert_eq!(history[1].from, TaskState::Working);
        assert_eq!(history[1].to, TaskState::InputRequired);
        assert_eq!(history[2].from, TaskState::InputRequired);
        assert_eq!(history[2].to, TaskState::Working);
        assert_eq!(history[3].from, TaskState::Working);
        assert_eq!(history[3].to, TaskState::Completed);
    }

    #[test]
    fn test_artifacts_emission() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());
        fsm.start_working(None).unwrap();

        let artifact = Artifact {
            artifact_id: "art-1".to_string(),
            name: Some("result.json".to_string()),
            description: None,
            parts: vec![Part::File {
                file: FileContent {
                    name: Some("result.json".to_string()),
                    mime_type: None,
                    bytes: None,
                    uri: Some("file:///result.json".to_string()),
                },
                metadata: None,
            }],
            metadata: None,
            extensions: None,
        };

        let result = fsm.complete(None, Some(vec![artifact.clone()]));
        assert!(result.is_ok());

        let transition = result.unwrap();
        assert_eq!(transition.artifacts.len(), 1);
        assert_eq!(transition.artifacts[0].artifact_id, "art-1");
    }

    #[test]
    fn test_custom_guard_allows_transition() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());

        // Add a guard that always allows
        fsm.add_guard(TaskState::Submitted, TaskState::Working, |_| Ok(()));

        let result = fsm.start_working(None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_custom_guard_rejects_transition() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());

        // Add a guard that always rejects
        fsm.add_guard(TaskState::Submitted, TaskState::Working, |_| {
            Err(StateTransitionError::GuardRejected {
                reason: "Not allowed".to_string(),
            })
        });

        let result = fsm.start_working(None);
        assert!(result.is_err());
        match result.unwrap_err() {
            StateTransitionError::GuardRejected { reason } => {
                assert_eq!(reason, "Not allowed");
            }
            _ => panic!("Expected GuardRejected error"),
        }
    }

    #[test]
    fn test_last_transition() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());
        assert!(fsm.last_transition().is_none());

        fsm.start_working(None).unwrap();
        let last = fsm.last_transition();
        assert!(last.is_some());
        assert_eq!(last.unwrap().to, TaskState::Working);

        fsm.complete(None, None).unwrap();
        let last = fsm.last_transition();
        assert!(last.is_some());
        assert_eq!(last.unwrap().to, TaskState::Completed);
    }

    #[test]
    fn test_full_lifecycle() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());

        // Submitted -> Working
        assert!(fsm.start_working(None).is_ok());
        assert_eq!(fsm.current_state(), &TaskState::Working);

        // Working -> InputRequired
        assert!(fsm.request_input(None).is_ok());
        assert_eq!(fsm.current_state(), &TaskState::InputRequired);

        // InputRequired -> Working
        assert!(fsm.start_working(None).is_ok());
        assert_eq!(fsm.current_state(), &TaskState::Working);

        // Working -> Completed
        assert!(fsm.complete(None, None).is_ok());
        assert_eq!(fsm.current_state(), &TaskState::Completed);
        assert!(fsm.is_terminal());

        // Cannot transition from terminal state
        assert!(fsm.start_working(None).is_err());
    }

    #[test]
    fn test_rejection_path() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());

        // Submitted -> Rejected (direct rejection without starting work)
        assert!(fsm.reject(None).is_ok());
        assert_eq!(fsm.current_state(), &TaskState::Rejected);
        assert!(fsm.is_terminal());
    }

    #[test]
    fn test_failure_path() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());

        fsm.start_working(None).unwrap();
        assert!(fsm.fail(None, None).is_ok());
        assert_eq!(fsm.current_state(), &TaskState::Failed);
        assert!(fsm.is_terminal());
    }

    #[test]
    fn test_cancellation_path() {
        let mut fsm = TaskStateMachine::new("task-1".to_string());

        fsm.start_working(None).unwrap();
        assert!(fsm.cancel(None, None).is_ok());
        assert_eq!(fsm.current_state(), &TaskState::Canceled);
        assert!(fsm.is_terminal());
    }
}
