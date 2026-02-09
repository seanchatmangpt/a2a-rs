//! Tests for deny_unknown_fields on event types

use serde_json::json;

use super::task_events::*;

#[test]
fn test_task_status_update_event_rejects_unknown_fields() {
    let json = json!({
        "taskId": "task-123",
        "contextId": "ctx-456",
        "kind": "status-update",
        "status": {
            "state": "working"
        },
        "final": false,
        "unknownField": "fail"
    });

    let result: Result<TaskStatusUpdateEvent, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "TaskStatusUpdateEvent should reject unknown fields"
    );
}

#[test]
fn test_task_artifact_update_event_rejects_unknown_fields() {
    let json = json!({
        "taskId": "task-123",
        "contextId": "ctx-456",
        "kind": "artifact-update",
        "artifact": {
            "artifactId": "artifact-123",
            "parts": []
        },
        "unknownField": "fail"
    });

    let result: Result<TaskArtifactUpdateEvent, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "TaskArtifactUpdateEvent should reject unknown fields"
    );
}

#[test]
fn test_task_status_update_event_allows_arbitrary_metadata() {
    let json = json!({
        "taskId": "task-123",
        "contextId": "ctx-456",
        "kind": "status-update",
        "status": {
            "state": "working"
        },
        "final": false,
        "metadata": {
            "customKey": "value",
            "nested": {
                "data": "here"
            }
        }
    });

    let result: Result<TaskStatusUpdateEvent, _> = serde_json::from_value(json);
    assert!(
        result.is_ok(),
        "TaskStatusUpdateEvent should allow arbitrary metadata"
    );
}

#[test]
fn test_valid_task_status_update_event_still_works() {
    let json = json!({
        "taskId": "task-123",
        "contextId": "ctx-456",
        "kind": "status-update",
        "status": {
            "state": "working"
        },
        "final": false
    });

    let result: Result<TaskStatusUpdateEvent, _> = serde_json::from_value(json);
    assert!(
        result.is_ok(),
        "Valid TaskStatusUpdateEvent should deserialize"
    );
}

#[test]
fn test_valid_task_artifact_update_event_still_works() {
    let json = json!({
        "taskId": "task-123",
        "contextId": "ctx-456",
        "kind": "artifact-update",
        "artifact": {
            "artifactId": "artifact-123",
            "parts": [
                {
                    "kind": "text",
                    "text": "Result data"
                }
            ]
        }
    });

    let result: Result<TaskArtifactUpdateEvent, _> = serde_json::from_value(json);
    assert!(
        result.is_ok(),
        "Valid TaskArtifactUpdateEvent should deserialize"
    );
}
