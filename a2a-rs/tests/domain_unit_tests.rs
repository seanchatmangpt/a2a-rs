// Comprehensive unit tests for domain layer types
//
// This test file provides Chicago School TDD-style tests for all domain types:
// - Message, Part, Artifact, Role
// - Task, TaskState, TaskStatus
// - AgentCard, AgentSkill, AgentCapabilities, SecurityScheme
// - Validation errors and validators
// - Event types
//
// Run with: cargo test --test domain_unit_tests

use a2a_rs::domain::{
    AgentCapabilities, AgentCard, AgentExtension, AgentInterface, AgentProvider, AgentSkill,
    AuthorizationCodeOAuthFlow, Artifact, FileContent, Message, OAuthFlows, Part, Role,
    SecurityScheme, Task, TaskState, TaskStatus, TaskArtifactUpdateEvent,
    TaskStatusUpdateEvent,
};
use serde_json::{json, Map};

// ============================================================================
// Role Tests
// ============================================================================

#[test]
fn test_role_serialization_user() {
    let role = Role::User;
    let serialized = serde_json::to_string(&role).unwrap();
    assert_eq!(serialized, "\"user\"");
}

#[test]
fn test_role_serialization_agent() {
    let role = Role::Agent;
    let serialized = serde_json::to_string(&role).unwrap();
    assert_eq!(serialized, "\"agent\"");
}

#[test]
fn test_role_deserialization_user() {
    let serialized = "\"user\"";
    let role: Role = serde_json::from_str(serialized).unwrap();
    assert_eq!(role, Role::User);
}

#[test]
fn test_role_partial_eq() {
    assert_eq!(Role::User, Role::User);
    assert_ne!(Role::User, Role::Agent);
}

// ============================================================================
// FileContent Tests
// ============================================================================

#[test]
fn test_file_content_with_bytes_validates() {
    let file_content = FileContent {
        name: Some("test.txt".to_string()),
        mime_type: Some("text/plain".to_string()),
        bytes: Some("SGVsbG8=".to_string()),
        uri: None,
    };

    assert!(file_content.validate().is_ok());
}

#[test]
fn test_file_content_with_uri_validates() {
    let file_content = FileContent {
        name: Some("doc.pdf".to_string()),
        mime_type: Some("application/pdf".to_string()),
        bytes: None,
        uri: Some("https://example.com/doc.pdf".to_string()),
    };

    assert!(file_content.validate().is_ok());
}

#[test]
fn test_file_content_with_both_bytes_and_uri_fails() {
    let file_content = FileContent {
        name: Some("test.txt".to_string()),
        mime_type: Some("text/plain".to_string()),
        bytes: Some("SGVsbG8=".to_string()),
        uri: Some("https://example.com/test.txt".to_string()),
    };

    assert!(file_content.validate().is_err());
}

#[test]
fn test_file_content_with_neither_fails() {
    let file_content = FileContent {
        name: Some("test.txt".to_string()),
        mime_type: Some("text/plain".to_string()),
        bytes: None,
        uri: None,
    };

    assert!(file_content.validate().is_err());
}

// ============================================================================
// Part Tests
// ============================================================================

#[test]
fn test_part_text_creation() {
    let part = Part::text("Hello, world!".to_string());
    match part {
        Part::Text { text, metadata } => {
            assert_eq!(text, "Hello, world!");
            assert!(metadata.is_none());
        }
        _ => panic!("Expected Text part"),
    }
}

#[test]
fn test_part_data_creation() {
    let mut data = Map::new();
    data.insert("key".to_string(), json!("value"));

    let part = Part::data(data.clone());
    match part {
        Part::Data {
            data: d,
            metadata,
        } => {
            assert_eq!(d, data);
            assert!(metadata.is_none());
        }
        _ => panic!("Expected Data part"),
    }
}

#[test]
fn test_part_file_from_bytes() {
    let part = Part::file_from_bytes(
        "SGVsbG8=".to_string(),
        Some("test.txt".to_string()),
        Some("text/plain".to_string()),
    );

    match part {
        Part::File { file, metadata } => {
            assert_eq!(file.name, Some("test.txt".to_string()));
            assert_eq!(file.mime_type, Some("text/plain".to_string()));
            assert_eq!(file.bytes, Some("SGVsbG8=".to_string()));
            assert!(file.uri.is_none());
            assert!(metadata.is_none());
        }
        _ => panic!("Expected File part"),
    }
}

#[test]
fn test_part_file_from_uri() {
    let part = Part::file_from_uri(
        "https://example.com/file.pdf".to_string(),
        Some("file.pdf".to_string()),
        Some("application/pdf".to_string()),
    );

    match part {
        Part::File { file, metadata } => {
            assert_eq!(file.name, Some("file.pdf".to_string()));
            assert!(file.bytes.is_none());
            assert_eq!(
                file.uri,
                Some("https://example.com/file.pdf".to_string())
            );
            assert!(metadata.is_none());
        }
        _ => panic!("Expected File part"),
    }
}

// ============================================================================
// Artifact Tests
// ============================================================================

#[test]
fn test_artifact_creation() {
    let artifact = Artifact {
        artifact_id: "artifact-123".to_string(),
        name: Some("Report".to_string()),
        description: Some("Analysis report".to_string()),
        parts: vec![Part::text("Report content")],
        metadata: None,
        extensions: None,
    };

    assert_eq!(artifact.artifact_id, "artifact-123");
    assert_eq!(artifact.name, Some("Report".to_string()));
    assert_eq!(artifact.parts.len(), 1);
}

// ============================================================================
// Message Tests
// ============================================================================

#[test]
fn test_message_user_text_helper() {
    let message = Message::user_text("Hello".to_string(), "msg-1".to_string());

    assert_eq!(message.role, Role::User);
    assert_eq!(message.parts.len(), 1);
    assert_eq!(message.message_id, "msg-1");
    assert!(message.task_id.is_none());
    assert!(message.context_id.is_none());
}

#[test]
fn test_message_agent_text_helper() {
    let message = Message::agent_text("Response".to_string(), "msg-2".to_string());

    assert_eq!(message.role, Role::Agent);
    assert_eq!(message.parts.len(), 1);
    assert_eq!(message.message_id, "msg-2");
}

#[test]
fn test_message_builder() {
    let message = Message::builder()
        .role(Role::User)
        .parts(vec![Part::text("Test")])
        .message_id("msg-builder".to_string())
        .task_id(Some("task-123".to_string()))
        .context_id(Some("ctx-456".to_string()))
        .build();

    assert_eq!(message.role, Role::User);
    assert_eq!(message.message_id, "msg-builder");
    assert_eq!(message.task_id, Some("task-123".to_string()));
    assert_eq!(message.context_id, Some("ctx-456".to_string()));
}

#[test]
fn test_message_add_part() {
    let mut message = Message::user_text("Initial".to_string(), "msg-add".to_string());
    assert_eq!(message.parts.len(), 1);

    message.add_part(Part::text("Second part"));
    assert_eq!(message.parts.len(), 2);

    message.add_part(Part::data(Map::new()));
    assert_eq!(message.parts.len(), 3);
}

#[test]
fn test_message_validate_success() {
    let message = Message::user_text("Valid".to_string(), "msg-valid".to_string());
    assert!(message.validate().is_ok());
}

#[test]
fn test_message_validate_with_invalid_kind() {
    let mut message = Message::user_text("Test".to_string(), "msg-kind".to_string());
    message.kind = "invalid".to_string();

    assert!(message.validate().is_err());
}

// ============================================================================
// TaskState Tests
// ============================================================================

#[test]
fn test_task_state_serialization_submitted() {
    let state = TaskState::Submitted;
    let serialized = serde_json::to_string(&state).unwrap();
    assert_eq!(serialized, "\"submitted\"");
}

#[test]
fn test_task_state_serialization_working() {
    let state = TaskState::Working;
    let serialized = serde_json::to_string(&state).unwrap();
    assert_eq!(serialized, "\"working\"");
}

#[test]
fn test_task_state_serialization_completed() {
    let state = TaskState::Completed;
    let serialized = serde_json::to_string(&state).unwrap();
    assert_eq!(serialized, "\"completed\"");
}

#[test]
fn test_task_state_all_states() {
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
        // Verify each state can be serialized and deserialized
        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: TaskState = serde_json::from_str(&serialized).unwrap();
        assert_eq!(state, deserialized);
    }
}

// ============================================================================
// TaskStatus Tests
// ============================================================================

#[test]
fn test_task_status_default() {
    let status = TaskStatus::default();
    assert_eq!(status.state, TaskState::Submitted);
    assert!(status.message.is_none());
    assert!(status.timestamp.is_some());
}

#[test]
fn test_task_status_with_message() {
    let message = Message::agent_text("Status update".to_string(), "msg-status".to_string());
    let status = TaskStatus {
        state: TaskState::Working,
        message: Some(message),
        timestamp: Some(chrono::Utc::now()),
    };

    assert!(status.message.is_some());
}

// ============================================================================
// Task Tests
// ============================================================================

#[test]
fn test_task_new() {
    let task = Task::new("task-new".to_string(), "ctx-new".to_string());

    assert_eq!(task.id, "task-new");
    assert_eq!(task.context_id, "ctx-new");
    assert_eq!(task.status.state, TaskState::Submitted);
    assert!(task.artifacts.is_none());
    assert!(task.history.is_none());
}

#[test]
fn test_task_update_status_with_message() {
    let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());
    let message = Message::user_text("Update".to_string(), "msg-update".to_string());

    task.update_status(TaskState::Working, Some(message));

    assert_eq!(task.status.state, TaskState::Working);
    assert!(task.status.message.is_some());
    assert!(task.history.is_some());
    assert_eq!(task.history.as_ref().unwrap().len(), 1);
}

#[test]
fn test_task_update_status_without_message() {
    let mut task = Task::new("task-2".to_string(), "ctx-2".to_string());

    task.update_status(TaskState::Working, None);

    assert_eq!(task.status.state, TaskState::Working);
    assert!(task.status.message.is_none());
}

#[test]
fn test_task_with_limited_history_zero() {
    let mut task = Task::new("task-3".to_string(), "ctx-3".to_string());

    // Add some history
    for i in 0..3 {
        task.update_status(
            TaskState::Working,
            Some(Message::user_text(
                format!("Message {}", i),
                format!("msg-{}", i),
            )),
        );
    }

    // Limit to 0 should remove history
    let limited = task.with_limited_history(Some(0));
    assert!(limited.history.is_none());
}

#[test]
fn test_task_with_limited_history_partial() {
    let mut task = Task::new("task-4".to_string(), "ctx-4".to_string());

    // Add 3 messages
    for i in 0..3 {
        task.update_status(
            TaskState::Working,
            Some(Message::user_text(
                format!("Message {}", i),
                format!("msg-{}", i),
            )),
        );
    }

    // Limit to 2 should keep only 2 most recent
    let limited = task.with_limited_history(Some(2));
    assert_eq!(limited.history.as_ref().unwrap().len(), 2);
}

#[test]
fn test_task_add_artifact() {
    let mut task = Task::new("task-5".to_string(), "ctx-5".to_string());
    assert!(task.artifacts.is_none());

    let artifact = Artifact {
        artifact_id: "artifact-1".to_string(),
        name: Some("Artifact 1".to_string()),
        description: None,
        parts: vec![Part::text("Content")],
        metadata: None,
        extensions: None,
    };

    task.add_artifact(artifact);

    assert!(task.artifacts.is_some());
    assert_eq!(task.artifacts.as_ref().unwrap().len(), 1);
}

#[test]
fn test_task_validate_success() {
    let task = Task::new("task-6".to_string(), "ctx-6".to_string());
    assert!(task.validate().is_ok());
}

#[test]
fn test_task_validate_invalid_kind() {
    let mut task = Task::new("task-7".to_string(), "ctx-7".to_string());
    task.kind = "invalid".to_string();

    assert!(task.validate().is_err());
}

#[test]
fn test_task_validate_duplicate_message_ids() {
    let mut task = Task::new("task-8".to_string(), "ctx-8".to_string());

    // Add messages with duplicate IDs
    task.update_status(
        TaskState::Working,
        Some(Message::user_text("Msg 1".to_string(), "msg-dup".to_string())),
    );
    task.update_status(
        TaskState::Working,
        Some(Message::agent_text("Msg 2".to_string(), "msg-dup".to_string())),
    );

    assert!(task.validate().is_err());
}

// ============================================================================
// SecurityScheme Tests
// ============================================================================

#[test]
fn test_security_scheme_api_key() {
    let scheme = SecurityScheme::ApiKey {
        location: "header".to_string(),
        name: "X-API-Key".to_string(),
        description: Some("API Key authentication".to_string()),
    };

    let json = serde_json::to_value(&scheme).unwrap();
    assert_eq!(json["type"], "apiKey");
    assert_eq!(json["in"], "header");
    assert_eq!(json["name"], "X-API-Key");
}

#[test]
fn test_security_scheme_http_bearer() {
    let scheme = SecurityScheme::Http {
        scheme: "bearer".to_string(),
        bearer_format: Some("JWT".to_string()),
        description: Some("Bearer authentication".to_string()),
    };

    let json = serde_json::to_value(&scheme).unwrap();
    assert_eq!(json["type"], "http");
    assert_eq!(json["scheme"], "bearer");
    assert_eq!(json["bearerFormat"], "JWT");
}

#[test]
fn test_security_scheme_mutual_tls() {
    let scheme = SecurityScheme::MutualTls {
        description: Some("Mutual TLS authentication".to_string()),
    };

    let json = serde_json::to_value(&scheme).unwrap();
    assert_eq!(json["type"], "mutualTLS");
}

// ============================================================================
// AgentSkill Tests
// ============================================================================

#[test]
fn test_agent_skill_new() {
    let skill = AgentSkill::new(
        "skill-id".to_string(),
        "Test Skill".to_string(),
        "A test skill".to_string(),
        vec!["test".to_string(), "demo".to_string()],
    );

    assert_eq!(skill.id, "skill-id");
    assert_eq!(skill.name, "Test Skill");
    assert_eq!(skill.description, "A test skill");
    assert_eq!(skill.tags.len(), 2);
    assert!(skill.examples.is_none());
    assert!(skill.input_modes.is_none());
    assert!(skill.output_modes.is_none());
}

#[test]
fn test_agent_skill_with_examples() {
    let skill = AgentSkill::new(
        "skill-1".to_string(),
        "Skill".to_string(),
        "Description".to_string(),
        vec!["test".to_string()],
    )
    .with_examples(vec!["Example 1".to_string(), "Example 2".to_string()]);

    assert!(skill.examples.is_some());
    let examples = skill.examples.unwrap();
    assert_eq!(examples.len(), 2);
}

#[test]
fn test_agent_skill_with_input_modes() {
    let skill = AgentSkill::new(
        "skill-2".to_string(),
        "Skill".to_string(),
        "Description".to_string(),
        vec!["test".to_string()],
    )
    .with_input_modes(vec!["text".to_string(), "image".to_string()]);

    assert!(skill.input_modes.is_some());
    assert_eq!(skill.input_modes.unwrap().len(), 2);
}

#[test]
fn test_agent_skill_with_output_modes() {
    let skill = AgentSkill::new(
        "skill-3".to_string(),
        "Skill".to_string(),
        "Description".to_string(),
        vec!["test".to_string()],
    )
    .with_output_modes(vec!["text".to_string(), "json".to_string()]);

    assert!(skill.output_modes.is_some());
    assert_eq!(skill.output_modes.unwrap().len(), 2);
}

// ============================================================================
// AgentCapabilities Tests
// ============================================================================

#[test]
fn test_agent_capabilities_default() {
    let capabilities = AgentCapabilities::default();

    assert_eq!(capabilities.streaming, false);
    assert_eq!(capabilities.push_notifications, false);
    assert_eq!(capabilities.state_transition_history, false);
}

#[test]
fn test_agent_capabilities_with_features() {
    let mut capabilities = AgentCapabilities::default();
    capabilities.streaming = true;
    capabilities.push_notifications = true;
    capabilities.state_transition_history = true;

    assert!(capabilities.streaming);
    assert!(capabilities.push_notifications);
    assert!(capabilities.state_transition_history);
}

#[test]
fn test_agent_capabilities_with_extensions() {
    let extension = AgentExtension {
        uri: "https://example.com/ext".to_string(),
        description: None,
        required: None,
        params: None,
    };

    let capabilities = AgentCapabilities {
        streaming: true,
        push_notifications: false,
        state_transition_history: false,
        extensions: Some(vec![extension]),
    };

    assert!(capabilities.extensions.is_some());
    assert_eq!(capabilities.extensions.unwrap().len(), 1);
}

// ============================================================================
// AgentCard Tests
// ============================================================================

#[test]
fn test_agent_card_builder_minimal() {
    let card = AgentCard::builder()
        .name("Minimal Agent".to_string())
        .description("A minimal agent".to_string())
        .url("https://example.com".to_string())
        .version("1.0.0".to_string())
        .capabilities(AgentCapabilities::default())
        .skills(vec![])
        .build();

    assert_eq!(card.name, "Minimal Agent");
    assert_eq!(card.description, "A minimal agent");
    assert_eq!(card.url, "https://example.com");
    assert_eq!(card.version, "1.0.0");
    assert_eq!(card.protocol_version, "0.3.0");
}

#[test]
fn test_agent_card_with_provider() {
    let provider = AgentProvider {
        organization: "Test Org".to_string(),
        url: "https://test.org".to_string(),
    };

    let card = AgentCard::builder()
        .name("Agent with Provider".to_string())
        .description("Test".to_string())
        .url("https://example.com".to_string())
        .version("1.0.0".to_string())
        .capabilities(AgentCapabilities::default())
        .skills(vec![])
        .provider(provider)
        .build();

    assert!(card.provider.is_some());
    let card_provider = card.provider.unwrap();
    assert_eq!(card_provider.organization, "Test Org");
}

#[test]
fn test_agent_card_with_capabilities() {
    let mut capabilities = AgentCapabilities::default();
    capabilities.streaming = true;
    capabilities.push_notifications = true;

    let card = AgentCard::builder()
        .name("Capable Agent".to_string())
        .description("Test".to_string())
        .url("https://example.com".to_string())
        .version("1.0.0".to_string())
        .capabilities(capabilities)
        .skills(vec![])
        .build();

    assert!(card.capabilities.streaming);
    assert!(card.capabilities.push_notifications);
}

#[test]
fn test_agent_card_with_skills() {
    let skill1 = AgentSkill::new(
        "skill-1".to_string(),
        "Skill 1".to_string(),
        "First skill".to_string(),
        vec!["test".to_string()],
    );

    let skill2 = AgentSkill::new(
        "skill-2".to_string(),
        "Skill 2".to_string(),
        "Second skill".to_string(),
        vec!["test".to_string()],
    );

    let card = AgentCard::builder()
        .name("Skilled Agent".to_string())
        .description("Test".to_string())
        .url("https://example.com".to_string())
        .version("1.0.0".to_string())
        .capabilities(AgentCapabilities::default())
        .skills(vec![skill1, skill2])
        .build();

    assert_eq!(card.skills.len(), 2);
}

// ============================================================================
// TaskStatusUpdateEvent Tests
// ============================================================================

#[test]
fn test_status_event_creation() {
    let event = TaskStatusUpdateEvent {
        task_id: "task-1".to_string(),
        context_id: "ctx-1".to_string(),
        kind: "status-update".to_string(),
        status: TaskStatus {
            state: TaskState::Working,
            message: None,
            timestamp: None,
        },
        final_: false,
        metadata: None,
    };

    assert_eq!(event.task_id, "task-1");
    assert_eq!(event.context_id, "ctx-1");
    assert_eq!(event.kind, "status-update");
    assert_eq!(event.status.state, TaskState::Working);
    assert_eq!(event.final_, false);
}

#[test]
fn test_status_event_with_message() {
    let message = Message::agent_text("Status update".to_string(), "msg-status".to_string());

    let event = TaskStatusUpdateEvent {
        task_id: "task-2".to_string(),
        context_id: "ctx-2".to_string(),
        kind: "status-update".to_string(),
        status: TaskStatus {
            state: TaskState::Working,
            message: Some(message),
            timestamp: Some(chrono::Utc::now()),
        },
        final_: false,
        metadata: None,
    };

    assert!(event.status.message.is_some());
}

// ============================================================================
// TaskArtifactUpdateEvent Tests
// ============================================================================

#[test]
fn test_artifact_event_creation() {
    let event = TaskArtifactUpdateEvent {
        task_id: "task-art".to_string(),
        context_id: "ctx-art".to_string(),
        kind: "artifact-update".to_string(),
        artifact: Artifact {
            artifact_id: "artifact-1".to_string(),
            name: Some("Test Artifact".to_string()),
            description: None,
            parts: vec![],
            metadata: None,
            extensions: None,
        },
        append: None,
        last_chunk: None,
        metadata: None,
    };

    assert_eq!(event.task_id, "task-art");
    assert_eq!(event.kind, "artifact-update");
    assert_eq!(event.artifact.artifact_id, "artifact-1");
}

#[test]
fn test_artifact_event_with_append() {
    let event = TaskArtifactUpdateEvent {
        task_id: "task-append".to_string(),
        context_id: "ctx-append".to_string(),
        kind: "artifact-update".to_string(),
        artifact: Artifact {
            artifact_id: "artifact-append".to_string(),
            name: None,
            description: None,
            parts: vec![],
            metadata: None,
            extensions: None,
        },
        append: Some(true),
        last_chunk: None,
        metadata: None,
    };

    assert_eq!(event.append, Some(true));
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_complete_message_flow() {
    // Create user message
    let user_msg = Message::user_text("Generate report".to_string(), "msg-1".to_string());

    // Create agent response
    let agent_msg = Message::agent_text("Working on it".to_string(), "msg-2".to_string());

    // Create artifact
    let artifact = Artifact {
        artifact_id: "artifact-1".to_string(),
        name: Some("Report".to_string()),
        description: None,
        parts: vec![Part::text("Report content")],
        metadata: None,
        extensions: None,
    };

    assert_eq!(user_msg.role, Role::User);
    assert_eq!(agent_msg.role, Role::Agent);
    assert_eq!(artifact.artifact_id, "artifact-1");
}

#[test]
fn test_task_lifecycle_events() {
    let task_id = "task-lifecycle".to_string();
    let context_id = "ctx-lifecycle".to_string();

    // Initial status
    let submitted_event = TaskStatusUpdateEvent {
        task_id: task_id.clone(),
        context_id: context_id.clone(),
        kind: "status-update".to_string(),
        status: TaskStatus {
            state: TaskState::Submitted,
            message: None,
            timestamp: Some(chrono::Utc::now()),
        },
        final_: false,
        metadata: None,
    };

    // Working status
    let working_event = TaskStatusUpdateEvent {
        task_id: task_id.clone(),
        context_id: context_id.clone(),
        kind: "status-update".to_string(),
        status: TaskStatus {
            state: TaskState::Working,
            message: None,
            timestamp: Some(chrono::Utc::now()),
        },
        final_: false,
        metadata: None,
    };

    assert_eq!(submitted_event.status.state, TaskState::Submitted);
    assert_eq!(working_event.status.state, TaskState::Working);
}
