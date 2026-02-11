//! Unit tests for AsyncMessageHandler port trait
//!
//! Tests the contract and behavior of the AsyncMessageHandler port trait
//! using mock implementations.

use a2a_rs::domain::core::message::{Message, Part, Role};
use a2a_rs::domain::core::task::{Task, TaskState};
use a2a_rs::domain::error::A2AError;
use a2a_rs::port::message_handler::AsyncMessageHandler;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock implementation of AsyncMessageHandler for testing
#[derive(Debug, Clone)]
struct MockMessageHandler {
    tasks: Arc<RwLock<HashMap<String, Task>>>,
    messages: Arc<RwLock<Vec<Message>>>,
}

impl MockMessageHandler {
    fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn add_task(&self, task: Task) {
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.id.clone(), task);
    }

    async fn get_message_count(&self) -> usize {
        self.messages.read().await.len()
    }
}

#[async_trait]
impl AsyncMessageHandler for MockMessageHandler {
    async fn process_message<'a>(
        &self,
        task_id: &'a str,
        message: &'a Message,
        _session_id: Option<&'a str>,
    ) -> Result<Task, A2AError> {
        // Store the message for tracking
        let mut messages = self.messages.write().await;
        messages.push(message.clone());

        // Update task status to Working
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| A2AError::TaskNotFound(task_id.to_string()))?;

        task.update_status(TaskState::Working, Some(message.clone()));

        Ok(task.clone())
    }

    async fn validate_message<'a>(&self, message: &'a Message) -> Result<(), A2AError> {
        // Custom validation: check that message has parts
        if message.parts.is_empty() {
            return Err(A2AError::ValidationError {
                field: "parts".to_string(),
                message: "Message must have at least one part".to_string(),
            });
        }

        // Validate each part
        for (idx, part) in message.parts.iter().enumerate() {
            match part {
                Part::Text { text, .. } => {
                    if text.trim().is_empty() {
                        return Err(A2AError::ValidationError {
                            field: format!("parts[{}].text", idx),
                            message: "Text part cannot be empty".to_string(),
                        });
                    }
                }
                Part::File { file, .. } => {
                    file.validate()?;
                }
                Part::Data { data, .. } => {
                    if data.is_empty() {
                        return Err(A2AError::ValidationError {
                            field: format!("parts[{}].data", idx),
                            message: "Data part cannot be empty".to_string(),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    async fn transform_message(&self, message: Message) -> Result<Message, A2AError> {
        // Custom transformation: add metadata to indicate processing
        let mut transformed = message.clone();

        if let Some(metadata) = &mut transformed.metadata {
            metadata.insert(
                "transformed".to_string(),
                serde_json::json!(true),
            );
        } else {
            let mut metadata = serde_json::Map::new();
            metadata.insert("transformed".to_string(), serde_json::json!(true));
            transformed.metadata = Some(metadata);
        }

        Ok(transformed)
    }
}

fn create_test_message(message_id: &str) -> Message {
    Message::builder()
        .role(Role::User)
        .parts(vec![Part::text("Hello, agent!".to_string())])
        .message_id(message_id.to_string())
        .build()
}

#[tokio::test]
async fn test_process_message() {
    let handler = MockMessageHandler::new();

    // Create a task
    let task = Task::new("task-1".to_string(), "context-1".to_string());
    handler.add_task(task).await;

    // Process a message
    let message = create_test_message("msg-1");
    let result = handler
        .process_message("task-1", &message, Some("session-1"))
        .await;

    assert!(result.is_ok());
    let updated_task = result.unwrap();
    assert_eq!(updated_task.status.state, TaskState::Working);
    assert!(updated_task.status.message.is_some());

    // Verify message was stored
    assert_eq!(handler.get_message_count().await, 1);
}

#[tokio::test]
async fn test_process_message_task_not_found() {
    let handler = MockMessageHandler::new();

    let message = create_test_message("msg-2");
    let result = handler
        .process_message("nonexistent", &message, None)
        .await;

    assert!(matches!(result, Err(A2AError::TaskNotFound(_)));
}

#[tokio::test]
async fn test_validate_message_success() {
    let handler = MockMessageHandler::new();

    let message = create_test_message("msg-3");
    let result = handler.validate_message(&message).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_validate_message_empty_parts() {
    let handler = MockMessageHandler::new();

    let message = Message::builder()
        .role(Role::User)
        .parts(vec![]) // Empty parts
        .message_id("msg-empty".to_string())
        .build();

    let result = handler.validate_message(&message).await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
    if let Err(A2AError::ValidationError { field, message }) = result {
        assert_eq!(field, "parts");
        assert!(message.contains("at least one part"));
    }
}

#[tokio::test]
async fn test_validate_message_empty_text() {
    let handler = MockMessageHandler::new();

    let message = Message::builder()
        .role(Role::User)
        .parts(vec![Part::text("   ".to_string())]) // Whitespace only
        .message_id("msg-empty-text".to_string())
        .build();

    let result = handler.validate_message(&message).await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
}

#[tokio::test]
async fn test_validate_message_empty_data() {
    let handler = MockMessageHandler::new();

    let message = Message::builder()
        .role(Role::User)
        .parts(vec![Part::data(serde_json::Map::new())]) // Empty data
        .message_id("msg-empty-data".to_string())
        .build();

    let result = handler.validate_message(&message).await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
}

#[tokio::test]
async fn test_validate_message_invalid_file() {
    let handler = MockMessageHandler::new();

    // Create a file part with both bytes and URI (invalid)
    let file_part = Part::File {
        file: a2a_rs::domain::core::message::FileContent {
            name: Some("test.txt".to_string()),
            mime_type: Some("text/plain".to_string()),
            bytes: Some("SGVsbG8=".to_string()),
            uri: Some("https://example.com/test.txt".to_string()), // Both set!
        },
        metadata: None,
    };

    let message = Message::builder()
        .role(Role::User)
        .parts(vec![file_part])
        .message_id("msg-invalid-file".to_string())
        .build();

    let result = handler.validate_message(&message).await;

    assert!(matches!(result, Err(A2AError::InvalidParams(_)));
}

#[tokio::test]
async fn test_transform_message() {
    let handler = MockMessageHandler::new();

    let message = create_test_message("msg-4");
    let result = handler.transform_message(message).await;

    assert!(result.is_ok());
    let transformed = result.unwrap();

    // Check that transformation metadata was added
    assert!(transformed.metadata.is_some());
    let metadata = transformed.metadata.unwrap();
    assert_eq!(metadata.get("transformed"), Some(&serde_json::json!(true)));
}

#[tokio::test]
async fn test_transform_message_preserves_content() {
    let handler = MockMessageHandler::new();

    let message = create_test_message("msg-5");
    let original_text = message.parts[0].get_text().unwrap().to_string();

    let transformed = handler.transform_message(message).await.unwrap();
    let transformed_text = transformed.parts[0].get_text().unwrap();

    // Content should be preserved
    assert_eq!(original_text, transformed_text);
}

#[tokio::test]
async fn test_handle_message_flow() {
    let handler = MockMessageHandler::new();

    // Create a task
    let task = Task::new("task-2".to_string(), "context-2".to_string());
    handler.add_task(task).await;

    // Handle the full message flow
    let message = create_test_message("msg-6");
    let result = handler
        .handle_message_flow("task-2", message, Some("session-2"))
        .await;

    assert!(result.is_ok());
    let updated_task = result.unwrap();
    assert_eq!(updated_task.status.state, TaskState::Working);
}

#[tokio::test]
async fn test_handle_message_flow_validation_fails() {
    let handler = MockMessageHandler::new();

    // Create a task
    let task = Task::new("task-3".to_string(), "context-3".to_string());
    handler.add_task(task).await;

    // Create invalid message (empty parts)
    let invalid_message = Message::builder()
        .role(Role::User)
        .parts(vec![])
        .message_id("msg-invalid".to_string())
        .build();

    let result = handler
        .handle_message_flow("task-3", invalid_message, None)
        .await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
}

#[tokio::test]
async fn test_handle_message_flow_with_transformation() {
    let handler = MockMessageHandler::new();

    // Create a task
    let task = Task::new("task-4".to_string(), "context-4".to_string());
    handler.add_task(task).await;

    let message = create_test_message("msg-7");
    let original_id = message.message_id.clone();

    let result = handler
        .handle_message_flow("task-4", message, None)
        .await;

    assert!(result.is_ok());

    // Verify transformation was applied
    let updated_task = result.unwrap();
    if let Some(status_msg) = &updated_task.status.message {
        if let Some(metadata) = &status_msg.metadata {
            assert_eq!(
                metadata.get("transformed"),
                Some(&serde_json::json!(true))
            );
        }
    }
}

#[tokio::test]
async fn test_process_multiple_messages() {
    let handler = MockMessageHandler::new();

    // Create a task
    let task = Task::new("task-5".to_string(), "context-5".to_string());
    handler.add_task(task).await;

    // Process multiple messages
    for i in 0..5 {
        let message = Message::builder()
            .role(Role::User)
            .parts(vec![Part::text(format!("Message {}", i))])
            .message_id(format!("msg-{}", i))
            .build();

        handler
            .process_message("task-5", &message, None)
            .await
            .unwrap();
    }

    // Verify all messages were processed
    assert_eq!(handler.get_message_count().await, 5);
}

#[tokio::test]
async fn test_process_message_with_session_id() {
    let handler = MockMessageHandler::new();

    // Create a task
    let task = Task::new("task-6".to_string(), "context-6".to_string());
    handler.add_task(task).await;

    let message = create_test_message("msg-8");
    let result = handler
        .process_message("task-6", &message, Some("test-session-123"))
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_validate_message_with_multiple_parts() {
    let handler = MockMessageHandler::new();

    let message = Message::builder()
        .role(Role::User)
        .parts(vec![
            Part::text("First part".to_string()),
            Part::text("Second part".to_string()),
            Part::data(serde_json::json!({"key": "value"}).as_object().unwrap().clone()),
        ])
        .message_id("msg-multi".to_string())
        .build();

    let result = handler.validate_message(&message).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_message_role_user() {
    let handler = MockMessageHandler::new();

    let message = Message::builder()
        .role(Role::User)
        .parts(vec![Part::text("User message".to_string())])
        .message_id("msg-user".to_string())
        .build();

    let result = handler.validate_message(&message).await;
    assert!(result.is_ok());
    assert_eq!(message.role, Role::User);
}

#[tokio::test]
async fn test_message_role_agent() {
    let handler = MockMessageHandler::new();

    let message = Message::builder()
        .role(Role::Agent)
        .parts(vec![Part::text("Agent response".to_string())])
        .message_id("msg-agent".to_string())
        .build();

    let result = handler.validate_message(&message).await;
    assert!(result.is_ok());
    assert_eq!(message.role, Role::Agent);
}

#[tokio::test]
async fn test_concurrent_message_processing() {
    let handler = MockMessageHandler::new();

    // Create multiple tasks
    for i in 0..10 {
        let task = Task::new(format!("task-concurrent-{}", i), "context-concurrent".to_string());
        handler.add_task(task).await;
    }

    // Process messages concurrently
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let handler = handler.clone();
            tokio::spawn(async move {
                let message = Message::builder()
                    .role(Role::User)
                    .parts(vec![Part::text(format!("Message {}", i))])
                    .message_id(format!("msg-concurrent-{}", i))
                    .build();

                handler
                    .process_message(&format!("task-concurrent-{}", i), &message, None)
                    .await
            })
        })
        .collect();

    // Wait for all to complete
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }

    // Verify all messages were processed
    assert_eq!(handler.get_message_count().await, 10);
}

#[tokio::test]
async fn test_message_with_metadata() {
    let handler = MockMessageHandler::new();

    let mut metadata = serde_json::Map::new();
    metadata.insert("priority".to_string(), serde_json::json!("high"));
    metadata.insert("source".to_string(), serde_json::json!("test"));

    let message = Message::builder()
        .role(Role::User)
        .parts(vec![Part::text("Test message".to_string())])
        .message_id("msg-metadata".to_string())
        .metadata(metadata.clone())
        .build();

    let result = handler.validate_message(&message).await;
    assert!(result.is_ok());
    assert_eq!(message.metadata, Some(metadata));
}

#[tokio::test]
async fn test_message_with_reference_task_ids() {
    let handler = MockMessageHandler::new();

    let message = Message::builder()
        .role(Role::User)
        .parts(vec![Part::text("Referencing previous tasks".to_string())])
        .message_id("msg-refs".to_string())
        .reference_task_ids(vec!["task-1".to_string(), "task-2".to_string()])
        .build();

    let result = handler.validate_message(&message).await;
    assert!(result.is_ok());
    assert_eq!(message.reference_task_ids, Some(vec!["task-1".to_string(), "task-2".to_string()]));
}
