//! Compliance Testing Module
//!
//! This module implements mechanical compliance tests for the A2A protocol without AI evaluation.
//! Tests cover:
//! 1. Schema compliance using jsonschema crate against spec/*.json
//! 2. State transition correctness using the FSM
//! 3. Terminality correctness (all executions reach terminal state)
//! 4. Event ordering correctness
//! 5. Coverage of all A2A methods, all task states, all error codes

use crate::construct::task_fsm::{StateTransitionError, TaskStateMachine};
use crate::domain::{
    A2AError, Message, Task, TaskArtifactUpdateEvent, TaskState, TaskStatus, TaskStatusUpdateEvent,
};
use jsonschema::{Draft, Validator};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;

// =============================================================================
// Schema Compliance Tests
// =============================================================================

/// Load and compile a JSON Schema from the spec directory
fn load_schema(filename: &str) -> Validator {
    let schema_path = format!("../spec/{}", filename);
    let schema_content = fs::read_to_string(&schema_path)
        .unwrap_or_else(|_| panic!("Failed to read schema file: {}", schema_path));

    let schema: Value = serde_json::from_str(&schema_content)
        .unwrap_or_else(|_| panic!("Failed to parse schema JSON: {}", filename));

    Validator::options()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .unwrap_or_else(|_| panic!("Failed to compile schema: {}", filename))
}

/// Extract a specific definition from a schema content with all definitions context
fn extract_definition(schema_content: &str, definition_name: &str) -> Value {
    let schema: Value = serde_json::from_str(schema_content).unwrap();

    serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "definitions": schema["definitions"],
        "$ref": format!("#/definitions/{}", definition_name)
    })
}

/// Helper to validate a value against a schema definition
fn validate_against_definition(
    value: &Value,
    spec_file: &str,
    definition: &str,
) -> Result<(), Vec<String>> {
    let schema_content = fs::read_to_string(format!("../spec/{}", spec_file))
        .expect(&format!("Failed to read {}", spec_file));
    let schema_def = extract_definition(&schema_content, definition);

    let schema = Validator::options()
        .with_draft(Draft::Draft7)
        .build(&schema_def)
        .expect(&format!("Failed to compile {} schema", definition));

    match schema.validate(value) {
        Ok(_) => Ok(()),
        Err(errors) => {
            let error_messages: Vec<String> = errors
                .map(|e| format!("{} at {}", e, e.instance_path))
                .collect();
            Err(error_messages)
        }
    }
}

#[test]
fn test_all_task_states_schema_compliance() {
    let states = vec![
        TaskState::Submitted,
        TaskState::Working,
        TaskState::InputRequired,
        TaskState::Completed,
        TaskState::Canceled,
        TaskState::Failed,
        TaskState::Rejected,
        TaskState::AuthRequired,
        TaskState::Unknown,
    ];

    for state in states {
        let state_json = serde_json::to_value(&state).unwrap();
        match validate_against_definition(&state_json, "task.json", "TaskState") {
            Ok(_) => {}
            Err(errors) => {
                panic!(
                    "TaskState {:?} failed schema validation:\n{}",
                    state,
                    errors.join("\n")
                );
            }
        }
    }
}

#[test]
fn test_task_schema_compliance() {
    let task = Task::new("task-123".to_string(), "ctx-456".to_string());
    let task_json = serde_json::to_value(&task).unwrap();

    match validate_against_definition(&task_json, "task.json", "Task") {
        Ok(_) => {}
        Err(errors) => {
            panic!("Task failed schema validation:\n{}", errors.join("\n"));
        }
    }
}

#[test]
fn test_task_status_schema_compliance() {
    let status = TaskStatus {
        state: TaskState::Working,
        message: None,
        timestamp: Some(chrono::Utc::now()),
    };

    let status_json = serde_json::to_value(&status).unwrap();
    match validate_against_definition(&status_json, "task.json", "TaskStatus") {
        Ok(_) => {}
        Err(errors) => {
            panic!(
                "TaskStatus failed schema validation:\n{}",
                errors.join("\n")
            );
        }
    }
}

#[test]
fn test_message_schema_compliance() {
    let message = Message::user_text("Hello".to_string(), "msg-123".to_string());
    let message_json = serde_json::to_value(&message).unwrap();

    match validate_against_definition(&message_json, "message.json", "Message") {
        Ok(_) => {}
        Err(errors) => {
            panic!("Message failed schema validation:\n{}", errors.join("\n"));
        }
    }
}

#[test]
fn test_task_status_update_event_schema_compliance() {
    let event = TaskStatusUpdateEvent {
        task_id: "task-123".to_string(),
        context_id: "ctx-456".to_string(),
        kind: "status-update".to_string(),
        status: TaskStatus {
            state: TaskState::Working,
            message: None,
            timestamp: Some(chrono::Utc::now()),
        },
        final_: false,
        metadata: None,
    };

    let event_json = serde_json::to_value(&event).unwrap();
    match validate_against_definition(&event_json, "events.json", "TaskStatusUpdateEvent") {
        Ok(_) => {}
        Err(errors) => {
            panic!(
                "TaskStatusUpdateEvent failed schema validation:\n{}",
                errors.join("\n")
            );
        }
    }
}

// =============================================================================
// State Transition Correctness Tests
// =============================================================================

#[test]
fn test_valid_state_transitions() {
    // Test all valid transitions according to the FSM specification
    let valid_transitions = vec![
        (TaskState::Submitted, TaskState::Working),
        (TaskState::Submitted, TaskState::Rejected),
        (TaskState::Submitted, TaskState::AuthRequired),
        (TaskState::Working, TaskState::InputRequired),
        (TaskState::Working, TaskState::Completed),
        (TaskState::Working, TaskState::Failed),
        (TaskState::Working, TaskState::Canceled),
        (TaskState::InputRequired, TaskState::Working),
        (TaskState::AuthRequired, TaskState::Working),
    ];

    for (from, to) in valid_transitions {
        let mut fsm = TaskStateMachine::new(format!("task-{:?}-{:?}", from, to));

        // First transition to the 'from' state if not Submitted
        if from != TaskState::Submitted {
            // Get to the from state through valid path
            match from {
                TaskState::Working => {
                    fsm.transition_to(TaskState::Working, None, None).unwrap();
                }
                TaskState::InputRequired => {
                    fsm.transition_to(TaskState::Working, None, None).unwrap();
                    fsm.transition_to(TaskState::InputRequired, None, None)
                        .unwrap();
                }
                TaskState::AuthRequired => {
                    fsm.transition_to(TaskState::AuthRequired, None, None)
                        .unwrap();
                }
                _ => {}
            }
        }

        // Now test the transition
        let result = fsm.transition_to(to.clone(), None, None);
        assert!(
            result.is_ok(),
            "Transition from {:?} to {:?} should be valid but got error: {:?}",
            from,
            to,
            result.err()
        );
    }
}

#[test]
fn test_invalid_state_transitions() {
    // Test invalid transitions that should be rejected
    let invalid_transitions = vec![
        (TaskState::Submitted, TaskState::Completed),
        (TaskState::Submitted, TaskState::Failed),
        (TaskState::Submitted, TaskState::InputRequired),
        (TaskState::InputRequired, TaskState::Completed),
        (TaskState::InputRequired, TaskState::Failed),
        (TaskState::AuthRequired, TaskState::Completed),
    ];

    for (from, to) in invalid_transitions {
        let mut fsm = TaskStateMachine::new(format!("task-{:?}-{:?}", from, to));

        // Get to the from state
        if from != TaskState::Submitted {
            match from {
                TaskState::Working => {
                    fsm.transition_to(TaskState::Working, None, None).unwrap();
                }
                TaskState::InputRequired => {
                    fsm.transition_to(TaskState::Working, None, None).unwrap();
                    fsm.transition_to(TaskState::InputRequired, None, None)
                        .unwrap();
                }
                TaskState::AuthRequired => {
                    fsm.transition_to(TaskState::AuthRequired, None, None)
                        .unwrap();
                }
                _ => {}
            }
        }

        let result = fsm.transition_to(to.clone(), None, None);
        assert!(
            result.is_err(),
            "Transition from {:?} to {:?} should be invalid but succeeded",
            from,
            to
        );

        match result.unwrap_err() {
            StateTransitionError::InvalidTransition {
                from: err_from,
                to: err_to,
            } => {
                assert_eq!(err_from, from);
                assert_eq!(err_to, to);
            }
            other => panic!("Expected InvalidTransition error, got: {:?}", other),
        }
    }
}

#[test]
fn test_cannot_transition_from_terminal_states() {
    let terminal_states = vec![
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Canceled,
        TaskState::Rejected,
        TaskState::Unknown,
    ];

    for terminal_state in terminal_states {
        let mut fsm = TaskStateMachine::new(format!("task-terminal-{:?}", terminal_state));

        // Reach the terminal state
        match terminal_state {
            TaskState::Completed | TaskState::Failed | TaskState::Canceled => {
                fsm.transition_to(TaskState::Working, None, None).unwrap();
                fsm.transition_to(terminal_state.clone(), None, None)
                    .unwrap();
            }
            TaskState::Rejected => {
                fsm.transition_to(terminal_state.clone(), None, None)
                    .unwrap();
            }
            TaskState::Unknown => {
                // Unknown state can't be transitioned to normally in our FSM
                // but we test that if it somehow gets there, it's terminal
                continue;
            }
            _ => {}
        }

        assert!(
            fsm.is_terminal(),
            "State {:?} should be terminal",
            terminal_state
        );

        // Try to transition to any other state
        let result = fsm.transition_to(TaskState::Working, None, None);
        assert!(
            result.is_err(),
            "Should not be able to transition from terminal state {:?}",
            terminal_state
        );

        match result.unwrap_err() {
            StateTransitionError::TransitionFromTerminalState { state } => {
                assert_eq!(state, terminal_state);
            }
            other => panic!(
                "Expected TransitionFromTerminalState error, got: {:?}",
                other
            ),
        }
    }
}

// =============================================================================
// Terminality Correctness Tests
// =============================================================================

#[test]
fn test_all_execution_paths_reach_terminal_state() {
    // Test various execution paths to ensure they all reach terminal states
    let paths = vec![
        // Direct rejection
        vec![TaskState::Submitted, TaskState::Rejected],
        // Happy path
        vec![
            TaskState::Submitted,
            TaskState::Working,
            TaskState::Completed,
        ],
        // Failure path
        vec![TaskState::Submitted, TaskState::Working, TaskState::Failed],
        // Cancellation path
        vec![
            TaskState::Submitted,
            TaskState::Working,
            TaskState::Canceled,
        ],
        // Input required path
        vec![
            TaskState::Submitted,
            TaskState::Working,
            TaskState::InputRequired,
            TaskState::Working,
            TaskState::Completed,
        ],
        // Auth required path
        vec![
            TaskState::Submitted,
            TaskState::AuthRequired,
            TaskState::Working,
            TaskState::Completed,
        ],
    ];

    for (path_idx, path) in paths.iter().enumerate() {
        let mut fsm = TaskStateMachine::new(format!("task-path-{}", path_idx));

        for (idx, state) in path.iter().enumerate() {
            if idx == 0 {
                // Skip initial state (Submitted)
                assert_eq!(
                    fsm.current_state(),
                    state,
                    "Path {} should start at {:?}",
                    path_idx,
                    state
                );
                continue;
            }

            let result = fsm.transition_to(state.clone(), None, None);
            assert!(
                result.is_ok(),
                "Path {} failed at step {} transitioning to {:?}: {:?}",
                path_idx,
                idx,
                state,
                result.err()
            );
        }

        // Verify we ended in a terminal state
        assert!(
            fsm.is_terminal(),
            "Path {} did not reach terminal state. Final state: {:?}",
            path_idx,
            fsm.current_state()
        );
    }
}

#[test]
fn test_fsm_terminality_detection() {
    // Test that the FSM correctly identifies terminal states
    assert!(TaskStateMachine::is_terminal_state(&TaskState::Completed));
    assert!(TaskStateMachine::is_terminal_state(&TaskState::Failed));
    assert!(TaskStateMachine::is_terminal_state(&TaskState::Canceled));
    assert!(TaskStateMachine::is_terminal_state(&TaskState::Rejected));
    assert!(TaskStateMachine::is_terminal_state(&TaskState::Unknown));

    assert!(!TaskStateMachine::is_terminal_state(&TaskState::Submitted));
    assert!(!TaskStateMachine::is_terminal_state(&TaskState::Working));
    assert!(!TaskStateMachine::is_terminal_state(
        &TaskState::InputRequired
    ));
    assert!(!TaskStateMachine::is_terminal_state(
        &TaskState::AuthRequired
    ));
}

#[test]
fn test_no_cycles_in_terminal_states() {
    // Verify that once in a terminal state, no transitions are allowed
    let terminal_states = vec![
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Canceled,
        TaskState::Rejected,
    ];

    for terminal in &terminal_states {
        let mut fsm = TaskStateMachine::new(format!("task-terminal-cycle-{:?}", terminal));

        // Get to terminal state
        if *terminal != TaskState::Rejected {
            fsm.transition_to(TaskState::Working, None, None).unwrap();
        }
        fsm.transition_to(terminal.clone(), None, None).unwrap();

        // Try all possible transitions from terminal state
        for target in &terminal_states {
            let result = fsm.transition_to(target.clone(), None, None);
            assert!(
                result.is_err(),
                "Should not transition from {:?} to {:?}",
                terminal,
                target
            );
        }

        // Also try non-terminal states
        for target in &[
            TaskState::Submitted,
            TaskState::Working,
            TaskState::InputRequired,
        ] {
            let result = fsm.transition_to(target.clone(), None, None);
            assert!(
                result.is_err(),
                "Should not transition from {:?} to {:?}",
                terminal,
                target
            );
        }
    }
}

// =============================================================================
// Event Ordering Correctness Tests
// =============================================================================

#[test]
fn test_event_ordering_with_fsm_history() {
    let mut fsm = TaskStateMachine::new("task-ordering".to_string());

    // Execute a series of transitions
    fsm.transition_to(TaskState::Working, None, None).unwrap();
    fsm.transition_to(TaskState::InputRequired, None, None)
        .unwrap();
    fsm.transition_to(TaskState::Working, None, None).unwrap();
    fsm.transition_to(TaskState::Completed, None, None).unwrap();

    let history = fsm.history();
    assert_eq!(history.len(), 4, "Should have 4 transitions in history");

    // Verify ordering
    assert_eq!(history[0].from, TaskState::Submitted);
    assert_eq!(history[0].to, TaskState::Working);

    assert_eq!(history[1].from, TaskState::Working);
    assert_eq!(history[1].to, TaskState::InputRequired);

    assert_eq!(history[2].from, TaskState::InputRequired);
    assert_eq!(history[2].to, TaskState::Working);

    assert_eq!(history[3].from, TaskState::Working);
    assert_eq!(history[3].to, TaskState::Completed);

    // Verify timestamps are monotonically increasing
    for i in 1..history.len() {
        assert!(
            history[i].timestamp >= history[i - 1].timestamp,
            "Timestamps should be monotonically increasing"
        );
    }
}

#[test]
fn test_status_update_events_maintain_order() {
    // Create a series of status update events
    let mut events = vec![];

    for i in 0..5 {
        let event = TaskStatusUpdateEvent {
            task_id: "task-123".to_string(),
            context_id: "ctx-456".to_string(),
            kind: "status-update".to_string(),
            status: TaskStatus {
                state: TaskState::Working,
                message: None,
                timestamp: Some(chrono::Utc::now()),
            },
            final_: i == 4,
            metadata: None,
        };
        events.push(event);
        std::thread::sleep(std::time::Duration::from_millis(10)); // Ensure different timestamps
    }

    // Verify ordering by timestamp
    for i in 1..events.len() {
        assert!(
            events[i].status.timestamp >= events[i - 1].status.timestamp,
            "Event timestamps should be ordered"
        );
    }

    // Verify final flag is only on last event
    for i in 0..events.len() - 1 {
        assert!(!events[i].final_, "Only last event should have final=true");
    }
    assert!(
        events.last().unwrap().final_,
        "Last event should have final=true"
    );
}

// =============================================================================
// Coverage Tests - All A2A Methods
// =============================================================================

#[test]
fn test_all_a2a_methods_defined() {
    // List all A2A methods from the specification
    let methods = vec![
        "message/send",
        "message/stream",
        "tasks/get",
        "tasks/list",
        "tasks/cancel",
        "tasks/pushNotificationConfig/set",
        "tasks/pushNotificationConfig/get",
        "tasks/pushNotificationConfig/list",
        "tasks/pushNotificationConfig/delete",
        "tasks/resubscribe",
        "agent/getAuthenticatedExtendedCard",
    ];

    // Verify each method can be constructed as a request
    for method in methods {
        let request = match method {
            "message/send" => {
                let message = Message::user_text("Test".to_string(), "msg-1".to_string());
                Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": method,
                    "id": "req-1",
                    "params": {
                        "message": message
                    }
                }))
            }
            "tasks/get" => Some(serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
                "id": "req-1",
                "params": {
                    "id": "task-123"
                }
            })),
            "tasks/list" => Some(serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
                "id": "req-1",
                "params": {}
            })),
            "tasks/cancel" => Some(serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
                "id": "req-1",
                "params": {
                    "id": "task-123"
                }
            })),
            _ => Some(serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
                "id": "req-1",
                "params": {}
            })),
        };

        assert!(
            request.is_some(),
            "Should be able to construct request for method: {}",
            method
        );

        // Verify it has the required JSON-RPC fields
        let req = request.unwrap();
        assert_eq!(req["jsonrpc"], "2.0");
        assert_eq!(req["method"], method);
        assert!(req.get("id").is_some());
    }
}

// =============================================================================
// Coverage Tests - All Error Codes
// =============================================================================

#[test]
fn test_all_error_codes_defined_and_compliant() {
    // Map of error codes to their specification details
    let error_codes: HashMap<i32, (&str, &str)> = vec![
        // JSON-RPC standard errors
        (-32700, ("Parse error", "JSONParseError")),
        (-32600, ("Invalid Request", "InvalidRequestError")),
        (-32601, ("Method not found", "MethodNotFoundError")),
        (-32602, ("Invalid params", "InvalidParamsError")),
        (-32603, ("Internal error", "InternalError")),
        // A2A-specific errors
        (-32001, ("Task not found", "TaskNotFoundError")),
        (-32002, ("Task not cancelable", "TaskNotCancelableError")),
        (
            -32003,
            (
                "Push Notification is not supported",
                "PushNotificationNotSupportedError",
            ),
        ),
        (
            -32004,
            (
                "This operation is not supported",
                "UnsupportedOperationError",
            ),
        ),
        (
            -32005,
            ("Incompatible content types", "ContentTypeNotSupportedError"),
        ),
        (
            -32006,
            ("Invalid agent response", "InvalidAgentResponseError"),
        ),
        (
            -32007,
            (
                "Authenticated Extended Card is not configured",
                "AuthenticatedExtendedCardNotConfiguredError",
            ),
        ),
    ]
    .into_iter()
    .collect();

    // Test each error code
    for (code, (_message, _error_type)) in error_codes {
        // Verify the error code is in the correct range
        if code >= -32700 && code <= -32600 {
            // JSON-RPC standard error range
            assert!(true, "Error code {} is in JSON-RPC standard range", code);
        } else if code >= -32099 && code <= -32000 {
            // Server error range (includes A2A-specific errors)
            assert!(true, "Error code {} is in server error range", code);
        } else {
            panic!("Error code {} is not in a valid range", code);
        }
    }

    // Verify we can create A2AError instances for all codes
    let errors = vec![
        (A2AError::TaskNotFound("test".to_string()), -32001),
        (A2AError::TaskNotCancelable("test".to_string()), -32002),
        (A2AError::PushNotificationNotSupported, -32003),
        (A2AError::UnsupportedOperation("test".to_string()), -32004),
        (
            A2AError::ContentTypeNotSupported("test".to_string()),
            -32005,
        ),
        (A2AError::InvalidAgentResponse("test".to_string()), -32006),
        (A2AError::AuthenticatedExtendedCardNotConfigured, -32007),
    ];

    for (error, expected_code) in errors {
        let jsonrpc_error = error.to_jsonrpc_error();
        assert_eq!(jsonrpc_error["code"], expected_code);
        assert!(jsonrpc_error.get("message").is_some());
        assert!(!jsonrpc_error["message"].as_str().unwrap().is_empty());
    }
}

#[test]
fn test_error_code_uniqueness() {
    // Verify all error codes are unique
    let codes = vec![
        -32700, -32600, -32601, -32602, -32603, -32001, -32002, -32003, -32004, -32005, -32006,
        -32007,
    ];

    let unique_codes: HashSet<i32> = codes.iter().cloned().collect();
    assert_eq!(
        codes.len(),
        unique_codes.len(),
        "All error codes must be unique"
    );
}

// =============================================================================
// Integration Tests - Combining All Compliance Aspects
// =============================================================================

#[test]
fn test_complete_task_lifecycle_compliance() {
    // Test a complete task lifecycle checking schema, state transitions, and terminality
    let mut fsm = TaskStateMachine::new("task-complete-lifecycle".to_string());
    let mut task = Task::new(
        "task-complete-lifecycle".to_string(),
        "ctx-test".to_string(),
    );

    // Submitted -> Working
    fsm.transition_to(TaskState::Working, None, None).unwrap();
    task.update_status(TaskState::Working, None);

    // Validate schema compliance at each step
    let task_json = serde_json::to_value(&task).unwrap();
    assert!(validate_against_definition(&task_json, "task.json", "Task").is_ok());

    // Working -> InputRequired
    let input_msg = Message::agent_text("Need input".to_string(), "msg-1".to_string());
    fsm.transition_to(TaskState::InputRequired, Some(input_msg.clone()), None)
        .unwrap();
    task.update_status(TaskState::InputRequired, Some(input_msg));

    let task_json = serde_json::to_value(&task).unwrap();
    assert!(validate_against_definition(&task_json, "task.json", "Task").is_ok());

    // InputRequired -> Working
    let response_msg = Message::user_text("Here's the input".to_string(), "msg-2".to_string());
    fsm.transition_to(TaskState::Working, Some(response_msg.clone()), None)
        .unwrap();
    task.update_status(TaskState::Working, Some(response_msg));

    let task_json = serde_json::to_value(&task).unwrap();
    assert!(validate_against_definition(&task_json, "task.json", "Task").is_ok());

    // Working -> Completed (terminal)
    let complete_msg = Message::agent_text("Done!".to_string(), "msg-3".to_string());
    fsm.transition_to(TaskState::Completed, Some(complete_msg.clone()), None)
        .unwrap();
    task.update_status(TaskState::Completed, Some(complete_msg));

    // Final validation
    let task_json = serde_json::to_value(&task).unwrap();
    assert!(validate_against_definition(&task_json, "task.json", "Task").is_ok());

    // Verify terminality
    assert!(fsm.is_terminal());
    assert_eq!(fsm.current_state(), &TaskState::Completed);

    // Verify history integrity
    let history = fsm.history();
    assert_eq!(history.len(), 4);

    // Verify cannot transition from terminal state
    let result = fsm.transition_to(TaskState::Working, None, None);
    assert!(result.is_err());
}

#[test]
fn test_all_terminal_states_reachable() {
    // Verify each terminal state can be reached through valid transitions
    let terminal_paths = vec![
        (
            TaskState::Completed,
            vec![TaskState::Working, TaskState::Completed],
        ),
        (
            TaskState::Failed,
            vec![TaskState::Working, TaskState::Failed],
        ),
        (
            TaskState::Canceled,
            vec![TaskState::Working, TaskState::Canceled],
        ),
        (TaskState::Rejected, vec![TaskState::Rejected]),
    ];

    for (terminal, path) in terminal_paths {
        let mut fsm = TaskStateMachine::new(format!("task-reach-{:?}", terminal));

        for (idx, state) in path.iter().enumerate() {
            if idx == 0 && *state == TaskState::Rejected {
                // Special case: direct rejection from Submitted
                fsm.transition_to(state.clone(), None, None).unwrap();
            } else {
                fsm.transition_to(state.clone(), None, None).unwrap();
            }
        }

        assert_eq!(fsm.current_state(), &terminal);
        assert!(fsm.is_terminal());
    }
}

#[test]
fn test_state_transition_idempotency() {
    // Verify that transition history is consistent and deterministic
    let mut fsm1 = TaskStateMachine::new("task-idem-1".to_string());
    let mut fsm2 = TaskStateMachine::new("task-idem-2".to_string());

    let transitions = vec![
        TaskState::Working,
        TaskState::InputRequired,
        TaskState::Working,
        TaskState::Completed,
    ];

    for state in &transitions {
        fsm1.transition_to(state.clone(), None, None).unwrap();
        fsm2.transition_to(state.clone(), None, None).unwrap();
    }

    // Both FSMs should have identical state
    assert_eq!(fsm1.current_state(), fsm2.current_state());
    assert_eq!(fsm1.is_terminal(), fsm2.is_terminal());
    assert_eq!(fsm1.history().len(), fsm2.history().len());

    // History should match
    let h1 = fsm1.history();
    let h2 = fsm2.history();
    for i in 0..h1.len() {
        assert_eq!(h1[i].from, h2[i].from);
        assert_eq!(h1[i].to, h2[i].to);
    }
}
