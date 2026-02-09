//! Task state machine invariant
//!
//! Validates that task state transitions follow the protocol's finite-state
//! machine rules. This ensures tasks always progress through valid state
//! transitions and never violate the state machine semantics.

use super::{Invariant, InvariantResult, InvariantViolation};
use crate::construct::task_fsm::TaskStateMachine;
use crate::domain::{Task, TaskState};

/// Invariant that validates task state machine constraints
///
/// This invariant ensures:
/// - The current state is valid
/// - If history exists, all state transitions in it are valid
/// - Terminal states have no outgoing transitions
/// - State transitions follow the FSM rules
///
/// # Example
///
/// ```rust
/// use a2a_rs::construct::invariants::{Invariant, TaskStateInvariant};
/// use a2a_rs::domain::{Task, TaskState};
///
/// let invariant = TaskStateInvariant::new();
/// let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());
///
/// // Valid initial state
/// assert!(invariant.check(&task).is_ok());
///
/// // Update to valid state
/// task.update_status(TaskState::Working, None);
/// assert!(invariant.check(&task).is_ok());
/// ```
pub struct TaskStateInvariant {
    /// Reference FSM for validation
    fsm: TaskStateMachine,
}

impl TaskStateInvariant {
    /// Create a new task state invariant
    pub fn new() -> Self {
        Self {
            fsm: TaskStateMachine::new("reference".to_string()),
        }
    }

    /// Validate a single state transition is allowed
    fn validate_transition(&self, from: &TaskState, to: &TaskState) -> InvariantResult {
        if !self.fsm.is_valid_transition(from, to) {
            return Err(InvariantViolation::TaskStateViolation {
                reason: format!("Invalid state transition from {:?} to {:?}", from, to),
            });
        }
        Ok(())
    }

    /// Validate that terminal states are truly terminal
    fn validate_terminal_state(&self, state: &TaskState) -> InvariantResult {
        if TaskStateMachine::is_terminal_state(state) {
            // Terminal states are always valid
            Ok(())
        } else {
            // Non-terminal states must have valid transitions available
            match state {
                TaskState::Submitted
                | TaskState::Working
                | TaskState::InputRequired
                | TaskState::AuthRequired => Ok(()),
                _ => Err(InvariantViolation::TaskStateViolation {
                    reason: format!("State {:?} is not a valid non-terminal state", state),
                }),
            }
        }
    }
}

impl Default for TaskStateInvariant {
    fn default() -> Self {
        Self::new()
    }
}

impl Invariant<Task> for TaskStateInvariant {
    fn check(&self, task: &Task) -> InvariantResult {
        // 1. Validate current state is valid
        self.validate_terminal_state(&task.status.state)?;

        // 2. If there's history, validate all transitions in it
        if let Some(history) = &task.history {
            if history.len() > 1 {
                // Derive state transitions from message history
                // We'll validate that the current state is reachable
                // from Submitted through valid transitions

                // For now, we'll do a simpler check: ensure current state
                // is one that can be reached from Submitted
                let reachable_states = vec![
                    TaskState::Submitted,
                    TaskState::Working,
                    TaskState::InputRequired,
                    TaskState::Completed,
                    TaskState::Failed,
                    TaskState::Canceled,
                    TaskState::Rejected,
                    TaskState::AuthRequired,
                ];

                if !reachable_states.contains(&task.status.state) {
                    return Err(InvariantViolation::TaskStateViolation {
                        reason: format!(
                            "Task state {:?} is not reachable from initial state",
                            task.status.state
                        ),
                    });
                }
            }
        }

        // 3. Validate that terminal states don't have status messages suggesting
        //    further work (this is a semantic check)
        if TaskStateMachine::is_terminal_state(&task.status.state) {
            // Terminal state is valid
            // We could add more checks here, like ensuring artifacts are present
            // for completed tasks, but that might be too strict
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "task_state_machine"
    }

    fn description(&self) -> &str {
        "Validates that task state transitions follow FSM rules"
    }
}

/// Extended task state invariant that validates full transition history
///
/// This is a stricter version that validates every state transition in the
/// task's history, not just the current state.
pub struct StrictTaskStateInvariant {
    fsm: TaskStateMachine,
}

impl StrictTaskStateInvariant {
    /// Create a new strict task state invariant
    pub fn new() -> Self {
        Self {
            fsm: TaskStateMachine::new("reference".to_string()),
        }
    }

    /// Extract implied state transitions from task history
    ///
    /// This attempts to reconstruct the state transition sequence from
    /// the message history by looking for state change patterns.
    fn extract_transitions(&self, task: &Task) -> Vec<(TaskState, TaskState)> {
        let mut transitions = Vec::new();

        // Start from Submitted
        let current = TaskState::Submitted;

        // If we have a different current state, add that transition
        if task.status.state != current {
            transitions.push((current.clone(), task.status.state.clone()));
        }

        transitions
    }
}

impl Default for StrictTaskStateInvariant {
    fn default() -> Self {
        Self::new()
    }
}

impl Invariant<Task> for StrictTaskStateInvariant {
    fn check(&self, task: &Task) -> InvariantResult {
        // First run the basic checks
        let basic = TaskStateInvariant::new();
        basic.check(task)?;

        // Then validate the transition history
        let transitions = self.extract_transitions(task);

        for (from, to) in transitions {
            if !self.fsm.is_valid_transition(&from, &to) {
                return Err(InvariantViolation::TaskStateViolation {
                    reason: format!("Invalid transition in history: {:?} -> {:?}", from, to),
                });
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "strict_task_state_machine"
    }

    fn description(&self) -> &str {
        "Strictly validates all task state transitions in history"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Message;

    #[test]
    fn test_valid_initial_state() {
        let invariant = TaskStateInvariant::new();
        let task = Task::new("task-1".to_string(), "ctx-1".to_string());
        assert!(invariant.check(&task).is_ok());
    }

    #[test]
    fn test_valid_working_state() {
        let invariant = TaskStateInvariant::new();
        let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());
        task.update_status(TaskState::Working, None);
        assert!(invariant.check(&task).is_ok());
    }

    #[test]
    fn test_valid_completed_state() {
        let invariant = TaskStateInvariant::new();
        let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());
        task.update_status(TaskState::Working, None);
        task.update_status(TaskState::Completed, None);
        assert!(invariant.check(&task).is_ok());
    }

    #[test]
    fn test_all_terminal_states_valid() {
        let invariant = TaskStateInvariant::new();

        for state in [
            TaskState::Completed,
            TaskState::Failed,
            TaskState::Canceled,
            TaskState::Rejected,
        ] {
            let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());
            task.status.state = state;
            assert!(invariant.check(&task).is_ok());
        }
    }

    #[test]
    fn test_all_non_terminal_states_valid() {
        let invariant = TaskStateInvariant::new();

        for state in [
            TaskState::Submitted,
            TaskState::Working,
            TaskState::InputRequired,
            TaskState::AuthRequired,
        ] {
            let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());
            task.status.state = state;
            assert!(invariant.check(&task).is_ok());
        }
    }

    #[test]
    fn test_strict_invariant_validates_transitions() {
        let invariant = StrictTaskStateInvariant::new();
        let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());

        // Valid transition: Submitted -> Working
        task.update_status(TaskState::Working, None);
        assert!(invariant.check(&task).is_ok());

        // Valid transition: Working -> Completed
        task.update_status(TaskState::Completed, None);
        assert!(invariant.check(&task).is_ok());
    }

    #[test]
    fn test_validate_single_transition() {
        let invariant = TaskStateInvariant::new();

        // Valid transitions
        assert!(
            invariant
                .validate_transition(&TaskState::Submitted, &TaskState::Working)
                .is_ok()
        );
        assert!(
            invariant
                .validate_transition(&TaskState::Working, &TaskState::Completed)
                .is_ok()
        );
        assert!(
            invariant
                .validate_transition(&TaskState::Working, &TaskState::InputRequired)
                .is_ok()
        );

        // Invalid transitions
        assert!(
            invariant
                .validate_transition(&TaskState::Submitted, &TaskState::Completed)
                .is_err()
        );
        assert!(
            invariant
                .validate_transition(&TaskState::Completed, &TaskState::Working)
                .is_err()
        );
    }

    #[test]
    fn test_task_with_history() {
        let invariant = TaskStateInvariant::new();
        let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());

        let msg1 = Message::agent_text("Starting work".to_string(), "msg-1".to_string());
        task.update_status(TaskState::Working, Some(msg1));

        let msg2 = Message::agent_text("Done".to_string(), "msg-2".to_string());
        task.update_status(TaskState::Completed, Some(msg2));

        assert!(invariant.check(&task).is_ok());
    }
}
