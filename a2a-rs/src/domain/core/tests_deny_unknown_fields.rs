//! Tests for deny_unknown_fields enforcement (Σ-completeness)
//!
//! This module contains comprehensive tests to verify that all domain types
//! reject unknown fields during deserialization. This property ensures the
//! input space is closed (Σ-complete) - only explicitly defined fields are
//! accepted, preventing injection of arbitrary data through unknown fields.
//!
//! ## Closure Property
//!
//! For each domain type T, the deserialization function deserialize: JSON → T
//! must satisfy:
//!
//! ```text
//! ∀ json ∈ JSON:
//!   if json contains field f ∉ fields(T)
//!   then deserialize(json) = Error
//! ```
//!
//! This is enforced via `#[serde(deny_unknown_fields)]` on all structs.
//!
//! ## Design Note
//!
//! While we deny unknown fields on structs, we still allow fields that hold
//! arbitrary data (like `metadata: Map<String, Value>`). These are orthogonal:
//! - `deny_unknown_fields` prevents unknown keys at the struct level
//! - `Map<String, Value>` allows arbitrary data within a known field
//!
//! Both are necessary for a well-typed protocol that also supports extensibility.

use serde_json::json;

use super::agent::*;
use super::message::*;
use super::task::*;

#[test]
fn test_agent_interface_rejects_unknown_fields() {
    let json = json!({
        "url": "https://example.com",
        "transport": "JSONRPC",
        "unknownField": "should fail"
    });

    let result: Result<AgentInterface, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "AgentInterface should reject unknown fields"
    );
    assert!(
        result.unwrap_err().to_string().contains("unknown field"),
        "Error should mention unknown field"
    );
}

#[test]
fn test_agent_extension_rejects_unknown_fields() {
    let json = json!({
        "uri": "https://example.com/ext",
        "description": "test",
        "extra": "not allowed"
    });

    let result: Result<AgentExtension, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "AgentExtension should reject unknown fields"
    );
}

#[test]
fn test_agent_extension_allows_arbitrary_params() {
    // params is explicitly a Map<String, Value> so arbitrary data is OK there
    let json = json!({
        "uri": "https://example.com/ext",
        "params": {
            "customKey": "customValue",
            "nested": { "arbitrary": "data" }
        }
    });

    let result: Result<AgentExtension, _> = serde_json::from_value(json);
    assert!(
        result.is_ok(),
        "AgentExtension should allow arbitrary params"
    );
}

#[test]
fn test_agent_card_signature_rejects_unknown_fields() {
    let json = json!({
        "protected": "test",
        "signature": "test",
        "unknownField": "fail"
    });

    let result: Result<AgentCardSignature, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "AgentCardSignature should reject unknown fields"
    );
}

#[test]
fn test_agent_provider_rejects_unknown_fields() {
    let json = json!({
        "organization": "Test Org",
        "url": "https://example.com",
        "extra": "not allowed"
    });

    let result: Result<AgentProvider, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "AgentProvider should reject unknown fields"
    );
}

#[test]
fn test_oauth_flows_rejects_unknown_fields() {
    let json = json!({
        "authorizationCode": {
            "authorizationUrl": "https://example.com/auth",
            "tokenUrl": "https://example.com/token",
            "scopes": {}
        },
        "unknownFlow": {}
    });

    let result: Result<OAuthFlows, _> = serde_json::from_value(json);
    assert!(result.is_err(), "OAuthFlows should reject unknown fields");
}

#[test]
fn test_authorization_code_flow_rejects_unknown_fields() {
    let json = json!({
        "authorizationUrl": "https://example.com/auth",
        "tokenUrl": "https://example.com/token",
        "scopes": {},
        "extra": "fail"
    });

    let result: Result<AuthorizationCodeOAuthFlow, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "AuthorizationCodeOAuthFlow should reject unknown fields"
    );
}

#[test]
fn test_client_credentials_flow_rejects_unknown_fields() {
    let json = json!({
        "tokenUrl": "https://example.com/token",
        "scopes": {},
        "unknown": "fail"
    });

    let result: Result<ClientCredentialsOAuthFlow, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "ClientCredentialsOAuthFlow should reject unknown fields"
    );
}

#[test]
fn test_implicit_flow_rejects_unknown_fields() {
    let json = json!({
        "authorizationUrl": "https://example.com/auth",
        "scopes": {},
        "extra": "fail"
    });

    let result: Result<ImplicitOAuthFlow, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "ImplicitOAuthFlow should reject unknown fields"
    );
}

#[test]
fn test_password_flow_rejects_unknown_fields() {
    let json = json!({
        "tokenUrl": "https://example.com/token",
        "scopes": {},
        "unknown": "fail"
    });

    let result: Result<PasswordOAuthFlow, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "PasswordOAuthFlow should reject unknown fields"
    );
}

#[test]
fn test_agent_capabilities_rejects_unknown_fields() {
    let json = json!({
        "streaming": true,
        "pushNotifications": false,
        "unknownCapability": true
    });

    let result: Result<AgentCapabilities, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "AgentCapabilities should reject unknown fields"
    );
}

#[test]
fn test_agent_skill_rejects_unknown_fields() {
    let json = json!({
        "id": "skill-1",
        "name": "Test Skill",
        "description": "A test skill",
        "tags": ["test"],
        "unknownField": "fail"
    });

    let result: Result<AgentSkill, _> = serde_json::from_value(json);
    assert!(result.is_err(), "AgentSkill should reject unknown fields");
}

#[test]
fn test_agent_card_rejects_unknown_fields() {
    let json = json!({
        "name": "Test Agent",
        "description": "Test description",
        "url": "https://example.com",
        "version": "1.0.0",
        "capabilities": {
            "streaming": false,
            "pushNotifications": false,
            "stateTransitionHistory": false
        },
        "defaultInputModes": ["text"],
        "defaultOutputModes": ["text"],
        "skills": [],
        "unknownField": "fail"
    });

    let result: Result<AgentCard, _> = serde_json::from_value(json);
    assert!(result.is_err(), "AgentCard should reject unknown fields");
}

#[test]
fn test_push_notification_auth_info_rejects_unknown_fields() {
    let json = json!({
        "schemes": ["bearer"],
        "unknownField": "fail"
    });

    let result: Result<PushNotificationAuthenticationInfo, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "PushNotificationAuthenticationInfo should reject unknown fields"
    );
}

#[test]
fn test_push_notification_config_rejects_unknown_fields() {
    let json = json!({
        "url": "https://example.com/notify",
        "unknownField": "fail"
    });

    let result: Result<PushNotificationConfig, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "PushNotificationConfig should reject unknown fields"
    );
}

// Message types

#[test]
fn test_file_content_rejects_unknown_fields() {
    let json = json!({
        "name": "test.txt",
        "mimeType": "text/plain",
        "bytes": "SGVsbG8=",
        "unknownField": "fail"
    });

    let result: Result<FileContent, _> = serde_json::from_value(json);
    assert!(result.is_err(), "FileContent should reject unknown fields");
}

#[test]
fn test_message_rejects_unknown_fields() {
    let json = json!({
        "role": "user",
        "parts": [],
        "messageId": "msg-123",
        "kind": "message",
        "unknownField": "fail"
    });

    let result: Result<Message, _> = serde_json::from_value(json);
    assert!(result.is_err(), "Message should reject unknown fields");
}

#[test]
fn test_message_allows_arbitrary_metadata() {
    let json = json!({
        "role": "user",
        "parts": [],
        "messageId": "msg-123",
        "kind": "message",
        "metadata": {
            "customKey": "customValue",
            "arbitrary": { "nested": "data" }
        }
    });

    let result: Result<Message, _> = serde_json::from_value(json);
    assert!(result.is_ok(), "Message should allow arbitrary metadata");
}

#[test]
fn test_artifact_rejects_unknown_fields() {
    let json = json!({
        "artifactId": "artifact-123",
        "parts": [],
        "unknownField": "fail"
    });

    let result: Result<Artifact, _> = serde_json::from_value(json);
    assert!(result.is_err(), "Artifact should reject unknown fields");
}

// Task types

#[test]
fn test_task_status_rejects_unknown_fields() {
    let json = json!({
        "state": "working",
        "unknownField": "fail"
    });

    let result: Result<TaskStatus, _> = serde_json::from_value(json);
    assert!(result.is_err(), "TaskStatus should reject unknown fields");
}

#[test]
fn test_task_rejects_unknown_fields() {
    let json = json!({
        "id": "task-123",
        "contextId": "ctx-456",
        "status": {
            "state": "submitted"
        },
        "kind": "task",
        "unknownField": "fail"
    });

    let result: Result<Task, _> = serde_json::from_value(json);
    assert!(result.is_err(), "Task should reject unknown fields");
}

#[test]
fn test_task_id_params_rejects_unknown_fields() {
    let json = json!({
        "id": "task-123",
        "unknownField": "fail"
    });

    let result: Result<TaskIdParams, _> = serde_json::from_value(json);
    assert!(result.is_err(), "TaskIdParams should reject unknown fields");
}

#[test]
fn test_task_query_params_rejects_unknown_fields() {
    let json = json!({
        "id": "task-123",
        "historyLength": 10,
        "unknownField": "fail"
    });

    let result: Result<TaskQueryParams, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "TaskQueryParams should reject unknown fields"
    );
}

#[test]
fn test_message_send_configuration_rejects_unknown_fields() {
    let json = json!({
        "acceptedOutputModes": ["text"],
        "unknownField": "fail"
    });

    let result: Result<MessageSendConfiguration, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "MessageSendConfiguration should reject unknown fields"
    );
}

#[test]
fn test_message_send_params_rejects_unknown_fields() {
    let json = json!({
        "message": {
            "role": "user",
            "parts": [],
            "messageId": "msg-123",
            "kind": "message"
        },
        "unknownField": "fail"
    });

    let result: Result<MessageSendParams, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "MessageSendParams should reject unknown fields"
    );
}

#[test]
fn test_task_send_params_rejects_unknown_fields() {
    let json = json!({
        "id": "task-123",
        "message": {
            "role": "user",
            "parts": [],
            "messageId": "msg-123",
            "kind": "message"
        },
        "unknownField": "fail"
    });

    let result: Result<TaskSendParams, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "TaskSendParams should reject unknown fields"
    );
}

#[test]
fn test_task_push_notification_config_rejects_unknown_fields() {
    let json = json!({
        "taskId": "task-123",
        "pushNotificationConfig": {
            "url": "https://example.com/notify"
        },
        "unknownField": "fail"
    });

    let result: Result<TaskPushNotificationConfig, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "TaskPushNotificationConfig should reject unknown fields"
    );
}

#[test]
fn test_list_tasks_params_rejects_unknown_fields() {
    let json = json!({
        "contextId": "ctx-123",
        "pageSize": 10,
        "unknownField": "fail"
    });

    let result: Result<ListTasksParams, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "ListTasksParams should reject unknown fields"
    );
}

#[test]
fn test_list_tasks_result_rejects_unknown_fields() {
    let json = json!({
        "tasks": [],
        "totalSize": 0,
        "pageSize": 10,
        "nextPageToken": "",
        "unknownField": "fail"
    });

    let result: Result<ListTasksResult, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "ListTasksResult should reject unknown fields"
    );
}

#[test]
fn test_get_task_push_notification_config_params_rejects_unknown_fields() {
    let json = json!({
        "id": "task-123",
        "unknownField": "fail"
    });

    let result: Result<GetTaskPushNotificationConfigParams, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "GetTaskPushNotificationConfigParams should reject unknown fields"
    );
}

#[test]
fn test_list_task_push_notification_config_params_rejects_unknown_fields() {
    let json = json!({
        "id": "task-123",
        "unknownField": "fail"
    });

    let result: Result<ListTaskPushNotificationConfigParams, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "ListTaskPushNotificationConfigParams should reject unknown fields"
    );
}

#[test]
fn test_delete_task_push_notification_config_params_rejects_unknown_fields() {
    let json = json!({
        "id": "task-123",
        "pushNotificationConfigId": "config-456",
        "unknownField": "fail"
    });

    let result: Result<DeleteTaskPushNotificationConfigParams, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "DeleteTaskPushNotificationConfigParams should reject unknown fields"
    );
}

// Test that valid data still works

#[test]
fn test_valid_agent_card_still_works() {
    let json = json!({
        "name": "Test Agent",
        "description": "Test description",
        "url": "https://example.com",
        "version": "1.0.0",
        "protocolVersion": "0.3.0",
        "preferredTransport": "JSONRPC",
        "capabilities": {
            "streaming": false,
            "pushNotifications": false,
            "stateTransitionHistory": false
        },
        "defaultInputModes": ["text"],
        "defaultOutputModes": ["text"],
        "skills": []
    });

    let result: Result<AgentCard, _> = serde_json::from_value(json);
    assert!(result.is_ok(), "Valid AgentCard should deserialize");
}

#[test]
fn test_valid_message_still_works() {
    let json = json!({
        "role": "user",
        "parts": [
            {
                "kind": "text",
                "text": "Hello"
            }
        ],
        "messageId": "msg-123",
        "kind": "message"
    });

    let result: Result<Message, _> = serde_json::from_value(json);
    assert!(result.is_ok(), "Valid Message should deserialize");
}

#[test]
fn test_valid_task_still_works() {
    let json = json!({
        "id": "task-123",
        "contextId": "ctx-456",
        "status": {
            "state": "submitted"
        },
        "kind": "task"
    });

    let result: Result<Task, _> = serde_json::from_value(json);
    assert!(result.is_ok(), "Valid Task should deserialize");
}
