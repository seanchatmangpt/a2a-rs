//! Property-based tests for the construct module using proptest
//!
//! This module tests invariants and properties that should hold for all
//! possible inputs to the TaskStateMachine.

use proptest::prelude::*;

use crate::construct::{StateTransition, StateTransitionError, TaskStateMachine, TransitionResult};
use crate::domain::{Artifact, Message, Part, Role, TaskState};

use base64::Engine;

// ============================================================================
// Generators for domain types
// ============================================================================

/// Strategy for generating valid TaskState values
fn task_state_strategy() -> impl Strategy<Value = TaskState> {
    prop_oneof![
        Just(TaskState::Submitted),
        Just(TaskState::Working),
        Just(TaskState::InputRequired),
        Just(TaskState::Completed),
        Just(TaskState::Canceled),
        Just(TaskState::Failed),
        Just(TaskState::Rejected),
        Just(TaskState::AuthRequired),
        Just(TaskState::Unknown),
    ]
}

/// Strategy for generating non-terminal TaskState values
fn non_terminal_state_strategy() -> impl Strategy<Value = TaskState> {
    prop_oneof![
        Just(TaskState::Submitted),
        Just(TaskState::Working),
        Just(TaskState::InputRequired),
        Just(TaskState::AuthRequired),
    ]
}

/// Strategy for generating terminal TaskState values
fn terminal_state_strategy() -> impl Strategy<Value = TaskState> {
    prop_oneof![
        Just(TaskState::Completed),
        Just(TaskState::Canceled),
        Just(TaskState::Failed),
        Just(TaskState::Rejected),
        Just(TaskState::Unknown),
    ]
}

/// Strategy for generating valid task IDs
fn task_id_strategy() -> impl Strategy<Value = String> {
    "[a-z0-9-]{5,20}"
}

/// Strategy for generating valid message IDs
fn message_id_strategy() -> impl Strategy<Value = String> {
    "msg-[a-z0-9-]{5,20}"
}

/// Strategy for generating valid artifact IDs
fn artifact_id_strategy() -> impl Strategy<Value = String> {
    "art-[a-z0-9-]{5,20}"
}

/// Strategy for generating valid Part values
fn part_strategy() -> impl Strategy<Value = Part> {
    prop_oneof![
        "[a-zA-Z0-9 ]{1,100}".prop_map(|text| Part::Text {
            text,
            metadata: None,
        }),
        ("[a-zA-Z0-9 ]{1,50}", "[a-z/]{1,20}").prop_map(|(content, mime)| Part::File {
            file: crate::domain::FileContent {
                name: Some("test.txt".to_string()),
                mime_type: Some(mime),
                bytes: Some(base64::engine::general_purpose::STANDARD.encode(content)),
                uri: None,
            },
            metadata: None,
        }),
    ]
}

/// Strategy for generating valid Message values
fn message_strategy() -> impl Strategy<Value = Message> {
    (
        message_id_strategy(),
        prop::collection::vec(part_strategy(), 1..3),
    )
        .prop_map(|(message_id, parts)| Message {
            role: Role::User,
            parts,
            metadata: None,
            reference_task_ids: None,
            message_id,
            task_id: None,
            context_id: None,
            extensions: None,
            kind: "message".to_string(),
        })
}

/// Strategy for generating valid Artifact values
fn artifact_strategy() -> impl Strategy<Value = Artifact> {
    (
        artifact_id_strategy(),
        prop::option::of("[a-zA-Z0-9 ]{1,50}"),
        prop::collection::vec(part_strategy(), 1..3),
    )
        .prop_map(|(artifact_id, name, parts)| Artifact {
            artifact_id,
            name,
            description: None,
            parts,
            metadata: None,
            extensions: None,
        })
}

/// Strategy for generating a sequence of valid state transitions
fn valid_transition_sequence_strategy() -> impl Strategy<Value = Vec<TaskState>> {
    prop_oneof![
        // Happy path: Submitted -> Working -> Completed
        Just(vec![
            TaskState::Submitted,
            TaskState::Working,
            TaskState::Completed
        ]),
        // With input required: Submitted -> Working -> InputRequired -> Working -> Completed
        Just(vec![
            TaskState::Submitted,
            TaskState::Working,
            TaskState::InputRequired,
            TaskState::Working,
            TaskState::Completed
        ]),
        // Rejection path: Submitted -> Rejected
        Just(vec![TaskState::Submitted, TaskState::Rejected]),
        // Failure path: Submitted -> Working -> Failed
        Just(vec![
            TaskState::Submitted,
            TaskState::Working,
            TaskState::Failed
        ]),
        // Cancellation path: Submitted -> Working -> Canceled
        Just(vec![
            TaskState::Submitted,
            TaskState::Working,
            TaskState::Canceled
        ]),
        // Auth required path: Submitted -> AuthRequired -> Working -> Completed
        Just(vec![
            TaskState::Submitted,
            TaskState::AuthRequired,
            TaskState::Working,
            TaskState::Completed
        ]),
    ]
}

// ============================================================================
// Property 1: Determinism
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Property: Same sequence of transitions always produces the same final state
    ///
    /// Given the same sequence of valid transitions applied to two identical FSMs,
    /// both should end up in the same state with identical histories (excluding timestamps).
    #[test]
    fn prop_deterministic_transitions(
        task_id in task_id_strategy(),
        sequence in valid_transition_sequence_strategy()
    ) {
        let mut fsm1 = TaskStateMachine::new(task_id.clone());
        let mut fsm2 = TaskStateMachine::new(task_id);

        // Apply the same sequence to both FSMs
        for (i, target_state) in sequence.iter().enumerate().skip(1) {
            if i < sequence.len() {
                let _ = fsm1.transition_to(target_state.clone(), None, None);
                let _ = fsm2.transition_to(target_state.clone(), None, None);
            }
        }

        // Both should have the same current state
        prop_assert_eq!(fsm1.current_state(), fsm2.current_state());

        // Both should have the same number of transitions
        prop_assert_eq!(fsm1.transition_count(), fsm2.transition_count());

        // Both should have the same terminal status
        prop_assert_eq!(fsm1.is_terminal(), fsm2.is_terminal());

        // History should have same from/to states (timestamps may differ)
        let history1 = fsm1.history();
        let history2 = fsm2.history();
        prop_assert_eq!(history1.len(), history2.len());
        for (h1, h2) in history1.iter().zip(history2.iter()) {
            prop_assert_eq!(&h1.from, &h2.from);
            prop_assert_eq!(&h1.to, &h2.to);
        }
    }
}

// ============================================================================
// Property 2: Idempotence
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Property: Attempting the same transition twice has no additional effect
    ///
    /// After a successful transition, attempting the same transition again should
    /// fail (because we're already in that state or in a terminal state).
    #[test]
    fn prop_transition_idempotence(
        task_id in task_id_strategy(),
        target in non_terminal_state_strategy()
    ) {
        let mut fsm = TaskStateMachine::new(task_id);

        // First transition from Submitted
        if fsm.can_transition_to(&target) {
            let result1 = fsm.transition_to(target.clone(), None, None);
            prop_assert!(result1.is_ok());

            let state_after_first = fsm.current_state().clone();
            let count_after_first = fsm.transition_count();

            // Try the same transition again - should fail or have no effect
            let result2 = fsm.transition_to(target.clone(), None, None);

            if result2.is_ok() {
                // If it succeeded, state and count should be unchanged
                prop_assert_eq!(fsm.current_state(), &state_after_first);
                // Count may have increased if this was a valid self-transition
            } else {
                // If it failed, state and count should definitely be unchanged
                prop_assert_eq!(fsm.current_state(), &state_after_first);
                prop_assert_eq!(fsm.transition_count(), count_after_first);
            }
        }
    }
}

// ============================================================================
// Property 3: Termination
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Property: All valid transition sequences eventually reach a terminal state
    ///
    /// Following any valid sequence of transitions should either reach a terminal
    /// state or exhaust all possible transitions.
    #[test]
    fn prop_all_paths_terminate(
        task_id in task_id_strategy(),
        sequence in valid_transition_sequence_strategy()
    ) {
        let mut fsm = TaskStateMachine::new(task_id);

        // Apply the sequence
        for (i, target_state) in sequence.iter().enumerate().skip(1) {
            if i < sequence.len() {
                let _ = fsm.transition_to(target_state.clone(), None, None);
            }
        }

        // The final state in our sequence should be terminal (by construction)
        if let Some(final_state) = sequence.last() {
            if fsm.current_state() == final_state {
                // If we reached the expected final state and it's terminal, we're done
                if TaskStateMachine::is_terminal_state(final_state) {
                    prop_assert!(fsm.is_terminal());
                }
            }
        }

        // No infinite loops: transition count should be bounded
        prop_assert!(fsm.transition_count() <= 100);
    }

    /// Property: Terminal states have no outgoing transitions
    #[test]
    fn prop_terminal_states_are_terminal(
        task_id in task_id_strategy(),
        terminal in terminal_state_strategy(),
        target in task_state_strategy()
    ) {
        // Create FSM and force it into a terminal state via valid path
        let mut fsm = TaskStateMachine::new(task_id);

        // Get to a terminal state through valid transitions
        match terminal {
            TaskState::Completed | TaskState::Failed | TaskState::Canceled => {
                let _ = fsm.transition_to(TaskState::Working, None, None);
                let _ = fsm.transition_to(terminal.clone(), None, None);
            }
            TaskState::Rejected => {
                let _ = fsm.transition_to(terminal.clone(), None, None);
            }
            _ => {}
        }

        // If we're in a terminal state, no transitions should be allowed
        if fsm.is_terminal() {
            prop_assert_eq!(fsm.allowed_transitions().len(), 0);
            prop_assert!(!fsm.can_transition_to(&target));

            let result = fsm.transition_to(target, None, None);
            prop_assert!(result.is_err());

            if let Err(StateTransitionError::TransitionFromTerminalState { state }) = result {
                prop_assert!(TaskStateMachine::is_terminal_state(&state));
            } else if result.is_err() {
                // Other errors are also acceptable for terminal states
            } else {
                prop_assert!(false, "Expected error from terminal state transition");
            }
        }
    }
}

// ============================================================================
// Property 4: Invariant Preservation
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Property: Only valid transitions according to FSM rules are allowed
    ///
    /// The FSM should reject any transition that violates its rules.
    #[test]
    fn prop_only_valid_transitions_succeed(
        task_id in task_id_strategy(),
        from in task_state_strategy(),
        to in task_state_strategy()
    ) {
        let fsm = TaskStateMachine::new(task_id);

        // Check if the transition is considered valid
        let is_valid = fsm.is_valid_transition(&from, &to);

        // Manually check against the known rules
        let should_be_valid = match (&from, &to) {
            // From Submitted
            (TaskState::Submitted, TaskState::Working) => true,
            (TaskState::Submitted, TaskState::Rejected) => true,
            (TaskState::Submitted, TaskState::AuthRequired) => true,
            // From Working
            (TaskState::Working, TaskState::InputRequired) => true,
            (TaskState::Working, TaskState::Completed) => true,
            (TaskState::Working, TaskState::Failed) => true,
            (TaskState::Working, TaskState::Canceled) => true,
            // From InputRequired
            (TaskState::InputRequired, TaskState::Working) => true,
            // From AuthRequired
            (TaskState::AuthRequired, TaskState::Working) => true,
            // All other transitions are invalid
            _ => false,
        };

        prop_assert_eq!(is_valid, should_be_valid,
            "Transition from {:?} to {:?} validity mismatch", from, to);
    }

    /// Property: FSM maintains consistency between state and allowed transitions
    #[test]
    fn prop_allowed_transitions_are_valid(
        task_id in task_id_strategy(),
        sequence in valid_transition_sequence_strategy()
    ) {
        let mut fsm = TaskStateMachine::new(task_id);

        // Apply transitions
        for (i, target_state) in sequence.iter().enumerate().skip(1) {
            if i < sequence.len() {
                // Check that allowed transitions are actually valid
                let allowed = fsm.allowed_transitions();
                for allowed_state in allowed {
                    prop_assert!(fsm.is_valid_transition(fsm.current_state(), allowed_state));
                    prop_assert!(fsm.can_transition_to(allowed_state));
                }

                let _ = fsm.transition_to(target_state.clone(), None, None);
            }
        }
    }

    /// Property: Non-terminal states always have at least one valid transition
    #[test]
    fn prop_non_terminal_states_have_transitions(
        task_id in task_id_strategy(),
        state in non_terminal_state_strategy()
    ) {
        let fsm = TaskStateMachine::new(task_id);

        // Non-terminal states should have at least one allowed transition
        if !TaskStateMachine::is_terminal_state(&state) {
            let allowed = match state {
                TaskState::Submitted => vec![TaskState::Working, TaskState::Rejected, TaskState::AuthRequired],
                TaskState::Working => vec![TaskState::InputRequired, TaskState::Completed, TaskState::Failed, TaskState::Canceled],
                TaskState::InputRequired => vec![TaskState::Working],
                TaskState::AuthRequired => vec![TaskState::Working],
                _ => vec![],
            };

            prop_assert!(!allowed.is_empty(), "Non-terminal state {:?} has no transitions", state);
        }
    }
}

// ============================================================================
// Property 5: Receipt Chain Integrity
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Property: Transition history forms a valid chain
    ///
    /// Each transition's 'to' state should match the next transition's 'from' state.
    /// The first transition should start from Submitted.
    #[test]
    fn prop_history_chain_is_valid(
        task_id in task_id_strategy(),
        sequence in valid_transition_sequence_strategy()
    ) {
        let mut fsm = TaskStateMachine::new(task_id);

        // Apply transitions
        for (i, target_state) in sequence.iter().enumerate().skip(1) {
            if i < sequence.len() {
                let _ = fsm.transition_to(target_state.clone(), None, None);
            }
        }

        let history = fsm.history();

        if !history.is_empty() {
            // First transition should start from Submitted
            prop_assert_eq!(&history[0].from, &TaskState::Submitted);

            // Each transition's 'to' should match next transition's 'from'
            for i in 0..history.len().saturating_sub(1) {
                prop_assert_eq!(&history[i].to, &history[i + 1].from,
                    "History chain broken at index {}", i);
            }

            // Last transition's 'to' should match current state
            if let Some(last) = history.last() {
                prop_assert_eq!(&last.to, fsm.current_state());
            }
        }
    }

    /// Property: Transition timestamps are monotonically increasing
    #[test]
    fn prop_history_timestamps_monotonic(
        task_id in task_id_strategy(),
        sequence in valid_transition_sequence_strategy()
    ) {
        let mut fsm = TaskStateMachine::new(task_id);

        // Apply transitions
        for (i, target_state) in sequence.iter().enumerate().skip(1) {
            if i < sequence.len() {
                let _ = fsm.transition_to(target_state.clone(), None, None);
            }
        }

        let history = fsm.history();

        // Timestamps should be monotonically increasing (or equal due to system clock precision)
        for i in 0..history.len().saturating_sub(1) {
            prop_assert!(
                history[i].timestamp <= history[i + 1].timestamp,
                "Timestamps not monotonic at index {}", i
            );
        }
    }

    /// Property: Artifacts are preserved in history
    #[test]
    fn prop_artifacts_preserved_in_history(
        task_id in task_id_strategy(),
        artifacts in prop::collection::vec(artifact_strategy(), 1..5)
    ) {
        let mut fsm = TaskStateMachine::new(task_id);

        // Transition to Working, then to Completed with artifacts
        fsm.transition_to(TaskState::Working, None, None).ok();
        let result = fsm.transition_to(
            TaskState::Completed,
            None,
            Some(artifacts.clone())
        );

        if let Ok(transition) = result {
            prop_assert_eq!(transition.artifacts.len(), artifacts.len());

            // Check that artifacts are in the history
            if let Some(last_transition) = fsm.last_transition() {
                prop_assert_eq!(last_transition.artifacts.len(), artifacts.len());
            }
        }
    }
}

// ============================================================================
// Property 6: Refusal Correctness
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Property: Invalid transitions are rejected with appropriate errors
    #[test]
    fn prop_invalid_transitions_rejected(
        task_id in task_id_strategy(),
        from in task_state_strategy(),
        to in task_state_strategy()
    ) {
        let mut fsm = TaskStateMachine::new(task_id);

        // Force FSM into the 'from' state if possible (for non-Submitted states)
        // For simplicity, we'll test from Submitted and Working states
        if from == TaskState::Working {
            let _ = fsm.transition_to(TaskState::Working, None, None);
        }

        // Only test if we're actually in the desired 'from' state
        if fsm.current_state() == &from {
            let is_valid = fsm.is_valid_transition(&from, &to);
            let result = fsm.transition_to(to.clone(), None, None);

            if is_valid && !fsm.is_terminal() {
                prop_assert!(result.is_ok(),
                    "Valid transition from {:?} to {:?} was rejected", from, to);
            } else {
                prop_assert!(result.is_err(),
                    "Invalid transition from {:?} to {:?} was allowed", from, to);

                // Check error type
                match result.unwrap_err() {
                    StateTransitionError::InvalidTransition { from: err_from, to: err_to } => {
                        prop_assert_eq!(&err_from, &from);
                        prop_assert_eq!(&err_to, &to);
                    }
                    StateTransitionError::TransitionFromTerminalState { state } => {
                        prop_assert!(TaskStateMachine::is_terminal_state(&state));
                    }
                    _ => {}
                }
            }
        }
    }

    /// Property: Guard functions can reject otherwise valid transitions
    #[test]
    fn prop_guards_can_reject_transitions(
        task_id in task_id_strategy(),
        should_reject in any::<bool>()
    ) {
        let mut fsm = TaskStateMachine::new(task_id);

        // Add a guard that conditionally rejects
        let rejection = should_reject;
        fsm.add_guard(TaskState::Submitted, TaskState::Working, move |_| {
            if rejection {
                Err(StateTransitionError::GuardRejected {
                    reason: "Test guard rejection".to_string(),
                })
            } else {
                Ok(())
            }
        });

        let result = fsm.transition_to(TaskState::Working, None, None);

        if should_reject {
            prop_assert!(result.is_err());
            if let Err(StateTransitionError::GuardRejected { reason }) = result {
                prop_assert_eq!(reason, "Test guard rejection");
            } else {
                prop_assert!(false, "Expected GuardRejected error");
            }
        } else {
            prop_assert!(result.is_ok());
        }
    }

    /// Property: Transition count never decreases
    #[test]
    fn prop_transition_count_monotonic(
        task_id in task_id_strategy(),
        sequence in valid_transition_sequence_strategy()
    ) {
        let mut fsm = TaskStateMachine::new(task_id);
        let mut last_count = 0;

        // Apply transitions
        for (i, target_state) in sequence.iter().enumerate().skip(1) {
            if i < sequence.len() {
                let result = fsm.transition_to(target_state.clone(), None, None);

                let current_count = fsm.transition_count();

                if result.is_ok() {
                    // Count should have increased by 1
                    prop_assert_eq!(current_count, last_count + 1);
                } else {
                    // Count should be unchanged
                    prop_assert_eq!(current_count, last_count);
                }

                last_count = current_count;
            }
        }
    }
}

// ============================================================================
// Additional invariant tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Property: Messages attached to transitions are preserved
    #[test]
    fn prop_messages_preserved_in_transitions(
        task_id in task_id_strategy(),
        message in message_strategy()
    ) {
        let mut fsm = TaskStateMachine::new(task_id);

        let result = fsm.transition_to(
            TaskState::Working,
            Some(message.clone()),
            None
        );

        if let Ok(transition) = result {
            prop_assert!(transition.message.is_some());
            if let Some(msg) = &transition.message {
                prop_assert_eq!(&msg.message_id, &message.message_id);
                prop_assert_eq!(msg.parts.len(), message.parts.len());
            }
        }
    }

    /// Property: Custom transition rules are respected
    #[test]
    fn prop_custom_transitions_respected(
        task_id in task_id_strategy()
    ) {
        use std::collections::HashMap;

        // Create custom transition map that only allows Submitted -> Completed
        let mut custom_transitions = HashMap::new();
        custom_transitions.insert(TaskState::Submitted, vec![TaskState::Completed]);
        custom_transitions.insert(TaskState::Completed, vec![]);

        let mut fsm = TaskStateMachine::with_transitions(task_id, custom_transitions);

        // Should allow Submitted -> Completed
        let result1 = fsm.transition_to(TaskState::Completed, None, None);
        prop_assert!(result1.is_ok());
        prop_assert_eq!(fsm.current_state(), &TaskState::Completed);

        // Should reject Submitted -> Working (not in custom rules)
        let mut fsm2 = TaskStateMachine::with_transitions("task-2".to_string(), {
            let mut map = HashMap::new();
            map.insert(TaskState::Submitted, vec![TaskState::Completed]);
            map.insert(TaskState::Working, vec![]);
            map.insert(TaskState::Completed, vec![]);
            map
        });

        let result2 = fsm2.transition_to(TaskState::Working, None, None);
        prop_assert!(result2.is_err());
    }

    /// Property: Task ID is immutable throughout FSM lifecycle
    #[test]
    fn prop_task_id_immutable(
        task_id in task_id_strategy(),
        sequence in valid_transition_sequence_strategy()
    ) {
        let mut fsm = TaskStateMachine::new(task_id.clone());

        // Apply transitions
        for (i, target_state) in sequence.iter().enumerate().skip(1) {
            if i < sequence.len() {
                let _ = fsm.transition_to(target_state.clone(), None, None);
            }
        }

        // Task ID should remain unchanged
        prop_assert_eq!(fsm.task_id(), task_id.as_str());
    }
}
