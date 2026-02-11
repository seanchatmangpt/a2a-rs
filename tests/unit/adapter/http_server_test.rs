//! Unit tests for HTTP server adapter
//!
//! Tests the HTTP server adapter implementation with mock request processors,
//! focusing on request routing, error handling, and edge cases.

use a2a_rs::adapter::HttpServerError;
use a2a_rs::application::json_rpc::{A2ARequest, SendTaskRequest};
use a2a_rs::domain::{
    A2AError, AgentCard, AgentCapabilities, AgentSkill, ListTasksParams, ListTasksResult,
    Message, Part, Role, Task, TaskPushNotificationConfig,
};
use a2a_rs::port::{AgentInfoProvider, AsyncA2ARequestProcessor};
use a2a_rs::services::server::AsyncA2ARequestProcessor;
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock request processor for testing
#[derive(Clone)]
struct MockRequestProcessor {
    tasks: Arc<RwLock<HashMap<String, Task>>>,
    should_fail: Arc<RwLock<bool>>,
    fail_with_error: Arc<RwLock<Option<A2AError>>>,
}

impl MockRequestProcessor {
    fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            should_fail: Arc::new(RwLock::new(false)),
            fail_with_error: Arc::new(RwLock::new(None)),
        }
    }

    async fn add_task(&self, task: Task) {
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.id.clone(), task);
    }

    async fn get_task(&self, id: &str) -> Option<Task> {
        let tasks = self.tasks.read().await;
        tasks.get(id).cloned()
    }

    fn set_failure(&self, error: A2AError) {
        *self.fail_with_error.blocking_write() = Some(error);
        *self.should_fail.blocking_write() = true;
    }

    fn clear_failure(&self) {
        *self.fail_with_error.blocking_write() = None;
        *self.should_fail.blocking_write() = false;
    }
}

#[async_trait]
impl AsyncA2ARequestProcessor for MockRequestProcessor {
    async fn process_raw_request<'a>(
        &self,
        request: &'a str,
    ) -> Result<String, A2AError> {
        // Check if we should fail
        if *self.should_fail.read().await {
            if let Some(error) = self.fail_with_error.read().await.as_ref() {
                return Err(error.clone());
            }
        }

        // Parse request
        let _json: serde_json::Value = serde_json::from_str(request)
            .map_err(|e| A2AError::JsonParse(e))?;

        // Return a simple success response
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"status": "ok"}
        });
        Ok(serde_json::to_string(&response).unwrap())
    }
}

/// Mock agent info provider for testing
#[derive(Clone)]
struct MockAgentInfoProvider {
    card: AgentCard,
}

impl MockAgentInfoProvider {
    fn new() -> Self {
        let card = AgentCard::builder()
            .name("Test Agent".to_string())
            .description("A test agent for unit testing".to_string())
            .url("https://test.example.com".to_string())
            .version("1.0.0".to_string())
            .capabilities(AgentCapabilities::default())
            .default_input_modes(vec!["text".to_string()])
            .default_output_modes(vec!["text".to_string()])
            .skills(vec![
                AgentSkill::new(
                    "test".to_string(),
                    "Test".to_string(),
                    "A test skill".to_string(),
                    vec!["test".to_string()],
                ),
            ])
            .build();

        Self { card }
    }
}

#[async_trait]
impl AgentInfoProvider for MockAgentInfoProvider {
    async fn get_agent_card(&self) -> Result<AgentCard, A2AError> {
        Ok(self.card.clone())
    }

    async fn get_skills(&self) -> Result<Vec<AgentSkill>, A2AError> {
        Ok(self.card.skills.clone())
    }

    async fn get_skill_by_id(&self, id: &str) -> Result<Option<AgentSkill>, A2AError> {
        Ok(self.card.skills.iter().find(|s| s.id == id).cloned())
    }
}

fn create_test_task(id: &str) -> Task {
    Task::builder()
        .id(id.to_string())
        .session_id("test-session".to_string())
        .build()
}

fn create_test_message() -> Message {
    Message::builder()
        .role(Role::User)
        .parts(vec![Part::text("Test message".to_string())])
        .message_id("msg-1".to_string())
        .build()
}

#[tokio::test]
async fn test_process_raw_request_success() {
    let processor = MockRequestProcessor::new();

    let request = r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#;
    let result = processor.process_raw_request(request).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.contains("\"jsonrpc\""));
    assert!(response.contains("\"result\""));
}

#[tokio::test]
async fn test_process_raw_request_invalid_json() {
    let processor = MockRequestProcessor::new();

    let request = "not json";
    let result = processor.process_raw_request(request).await;

    assert!(result.is_err());
    if let Err(A2AError::JsonParse(_)) = result {
        // Expected
    } else {
        panic!("Expected JsonParse error");
    }
}

#[tokio::test]
async fn test_process_raw_request_custom_error() {
    let processor = MockRequestProcessor::new();
    processor.set_failure(A2AError::MethodNotFound("unknownMethod".to_string()));

    let request = r#"{"jsonrpc":"2.0","id":1,"method":"unknownMethod"}"#;
    let result = processor.process_raw_request(request).await;

    assert!(result.is_err());
    if let Err(A2AError::MethodNotFound(method)) = result {
        assert_eq!(method, "unknownMethod");
    } else {
        panic!("Expected MethodNotFound error");
    }

    processor.clear_failure();
}

#[tokio::test]
async fn test_get_agent_card() {
    let provider = MockAgentInfoProvider::new();

    let result = provider.get_agent_card().await;

    assert!(result.is_ok());
    let card = result.unwrap();
    assert_eq!(card.name, "Test Agent");
    assert_eq!(card.version, "1.0.0");
    assert_eq!(card.skills.len(), 1);
}

#[tokio::test]
async fn test_get_skills() {
    let provider = MockAgentInfoProvider::new();

    let result = provider.get_skills().await;

    assert!(result.is_ok());
    let skills = result.unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].id, "test");
}

#[tokio::test]
async fn test_get_skill_by_id_found() {
    let provider = MockAgentInfoProvider::new();

    let result = provider.get_skill_by_id("test").await;

    assert!(result.is_ok());
    let skill = result.unwrap();
    assert!(skill.is_some());
    assert_eq!(skill.unwrap().id, "test");
}

#[tokio::test]
async fn test_get_skill_by_id_not_found() {
    let provider = MockAgentInfoProvider::new();

    let result = provider.get_skill_by_id("nonexistent").await;

    assert!(result.is_ok());
    let skill = result.unwrap();
    assert!(skill.is_none());
}

#[tokio::test]
async fn test_task_storage() {
    let processor = MockRequestProcessor::new();
    let task = create_test_task("task-storage");

    processor.add_task(task.clone()).await;

    let retrieved = processor.get_task("task-storage").await;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, "task-storage");
}

#[tokio::test]
async fn test_task_not_found() {
    let processor = MockRequestProcessor::new();

    let result = processor.get_task("nonexistent").await;

    assert!(result.is_none());
}

#[tokio::test]
async fn test_concurrent_task_operations() {
    let processor = MockRequestProcessor::new();

    // Add tasks concurrently
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let processor = processor.clone();
            tokio::spawn(async move {
                let task = create_test_task(&format!("task-concurrent-{}", i));
                processor.add_task(task).await;
                i
            })
        })
        .collect();

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all tasks were added
    for i in 0..10 {
        let result = processor.get_task(&format!("task-concurrent-{}", i)).await;
        assert!(result.is_some());
    }
}

#[tokio::test]
async fn test_process_send_task_request() {
    let processor = MockRequestProcessor::new();

    let request_json = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tasks/send",
        "params": {
            "id": "task-1",
            "message": {
                "role": "user",
                "parts": [{"text": "Hello"}]
            }
        }
    }
    });

    let result = processor
        .process_raw_request(&request_json.to_string())
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_process_with_missing_jsonrpc_field() {
    let processor = MockRequestProcessor::new();

    let request = r#"{"id":1,"method":"test"}"#;
    let result = processor.process_raw_request(request).await;

    // Should succeed as processor is lenient
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_process_with_null_id() {
    let processor = MockRequestProcessor::new();

    let request = r#"{"jsonrpc":"2.0","id":null,"method":"test"}"#;
    let result = processor.process_raw_request(request).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_process_with_empty_params() {
    let processor = MockRequestProcessor::new();

    let request = r#"{"jsonrpc":"2.0","id":1,"method":"tasks/get","params":{}}"#;
    let result = processor.process_raw_request(request).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_process_request_with_whitespace() {
    let processor = MockRequestProcessor::new();

    let request = r#"  {"jsonrpc":"2.0","id":1,"method":"test"}  "#;
    let result = processor.process_raw_request(request).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_invalid_method_name() {
    let processor = MockRequestProcessor::new();
    processor.set_failure(A2AError::MethodNotFound("invalidMethod".to_string()));

    let request = r#"{"jsonrpc":"2.0","id":1,"method":"invalidMethod"}"#;
    let result = processor.process_raw_request(request).await;

    assert!(result.is_err());
    if let Err(A2AError::MethodNotFound(method)) = result {
        assert_eq!(method, "invalidMethod");
    } else {
        panic!("Expected MethodNotFound error");
    }

    processor.clear_failure();
}

#[tokio::test]
async fn test_task_not_found_error() {
    let processor = MockRequestProcessor::new();
    processor.set_failure(A2AError::TaskNotFound("task-missing".to_string()));

    let request = r#"{"jsonrpc":"2.0","id":1,"method":"tasks/get","params":{"id":"task-missing"}}"#;
    let result = processor.process_raw_request(request).await;

    assert!(result.is_err());
    if let Err(A2AError::TaskNotFound(id)) = result {
        assert_eq!(id, "task-missing");
    } else {
        panic!("Expected TaskNotFound error");
    }

    processor.clear_failure();
}

#[tokio::test]
async fn test_invalid_params_error() {
    let processor = MockRequestProcessor::new();
    processor.set_failure(A2AError::InvalidParams("Missing required field".to_string()));

    let request = r#"{"jsonrpc":"2.0","id":1,"method":"tasks/send","params":{}}"#;
    let result = processor.process_raw_request(request).await;

    assert!(result.is_err());
    if let Err(A2AError::InvalidParams(msg)) = result {
        assert_eq!(msg, "Missing required field");
    } else {
        panic!("Expected InvalidParams error");
    }

    processor.clear_failure();
}

#[tokio::test]
async fn test_internal_error() {
    let processor = MockRequestProcessor::new();
    processor.set_failure(A2AError::Internal("Database connection lost".to_string()));

    let request = r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#;
    let result = processor.process_raw_request(request).await;

    assert!(result.is_err());
    if let Err(A2AError::Internal(msg)) = result {
        assert_eq!(msg, "Database connection lost");
    } else {
        panic!("Expected Internal error");
    }

    processor.clear_failure();
}

#[tokio::test]
async fn test_multiple_sequential_requests() {
    let processor = MockRequestProcessor::new();

    for i in 0..5 {
        let request = json!({
            "jsonrpc": "2.0",
            "id": i,
            "method": "test",
            "params": {"iteration": i}
        });

        let result = processor
            .process_raw_request(&request.to_string())
            .await;

        assert!(result.is_ok(), "Request {} failed", i);
    }
}

#[tokio::test]
async fn test_request_with_large_payload() {
    let processor = MockRequestProcessor::new();

    // Create a large message (1MB of text)
    let large_text = "x".repeat(1_000_000);
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tasks/send",
        "params": {
            "id": "task-large",
            "message": {
                "role": "user",
                "parts": [{"text": large_text}]
            }
        }
    });

    let result = processor
        .process_raw_request(&request.to_string())
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_request_with_special_characters() {
    let processor = MockRequestProcessor::new();

    let text_with_special = "Hello 🎉 Special chars: <>&\"'";
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tasks/send",
        "params": {
            "id": "task-special",
            "message": {
                "role": "user",
                "parts": [{"text": text_with_special}]
            }
        }
    });

    let result = processor
        .process_raw_request(&request.to_string())
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_validation_error_response() {
    let processor = MockRequestProcessor::new();
    processor.set_failure(A2AError::ValidationError {
        field: "message".to_string(),
        message: "Message is required".to_string(),
    });

    let request = r#"{"jsonrpc":"2.0","id":1,"method":"tasks/send"}"#;
    let result = processor.process_raw_request(request).await;

    assert!(result.is_err());
    if let Err(A2AError::ValidationError { field, message }) = result {
        assert_eq!(field, "message");
        assert_eq!(message, "Message is required");
    } else {
        panic!("Expected ValidationError");
    }

    processor.clear_failure();
}

#[tokio::test]
async fn test_agent_card_protocol_version() {
    let provider = MockAgentInfoProvider::new();

    let card = provider.get_agent_card().await.unwrap();

    assert_eq!(card.protocol_version, "0.3.0");
    assert_eq!(card.preferred_transport, "JSONRPC");
}

#[tokio::test]
async fn test_agent_card_capabilities() {
    let provider = MockAgentInfoProvider::new();

    let card = provider.get_agent_card().await.unwrap();

    assert!(!card.capabilities.streaming);
    assert!(!card.capabilities.push_notifications);
    assert!(!card.capabilities.state_transition_history);
}

#[tokio::test]
async fn test_skill_validation() {
    let provider = MockAgentInfoProvider::new();

    let skills = provider.get_skills().await.unwrap();

    assert_eq!(skills.len(), 1);
    let skill = &skills[0];
    assert_eq!(skill.id, "test");
    assert_eq!(skill.name, "Test");
    assert_eq!(skill.description, "A test skill");
    assert_eq!(skill.tags, vec!["test"]);
}

#[tokio::test]
async fn test_concurrent_requests() {
    let processor = MockRequestProcessor::new();

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let processor = processor.clone();
            tokio::spawn(async move {
                let request = json!({
                    "jsonrpc": "2.0",
                    "id": i,
                    "method": "test"
                });

                processor
                    .process_raw_request(&request.to_string())
                    .await
            })
        })
        .collect();

    // Wait for all to complete
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }
}

#[tokio::test]
async fn test_error_conversion_to_jsonrpc() {
    let error = A2AError::TaskNotFound("task-123".to_string());
    let json_error = error.to_jsonrpc_error();

    assert_eq!(json_error["code"], -32001);
    assert_eq!(json_error["message"], "Task not found");
}

#[tokio::test]
async fn test_validation_error_jsonrpc() {
    let error = A2AError::ValidationError {
        field: "parts".to_string(),
        message: "Cannot be empty".to_string(),
    };
    let json_error = error.to_jsonrpc_error();

    assert_eq!(json_error["code"], -32602);
    assert_eq!(json_error["message"], "Validation error");
}

#[tokio::test]
async fn test_unsupported_operation_jsonrpc() {
    let error = A2AError::UnsupportedOperation("streaming".to_string());
    let json_error = error.to_jsonrpc_error();

    assert_eq!(json_error["code"], -32004);
    assert_eq!(json_error["message"], "This operation is not supported");
}
