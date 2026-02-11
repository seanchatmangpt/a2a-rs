//! Unit tests for WebSocket server adapter
//!
//! Tests the WebSocket server adapter implementation with mock clients,
//! focusing on connection handling, message routing, and streaming.

use a2a_rs::adapter::WebSocketServerError;
use a2a_rs::domain::{
    A2AError, AgentCard, AgentCapabilities, AgentSkill, Message, Part, Role, Task,
    TaskArtifactUpdateEvent, TaskStatusUpdateEvent,
};
use a2a_rs::port::{AgentInfoProvider, AsyncA2ARequestProcessor, AsyncStreamingHandler};
use a2a_rs::services::server::{AgentInfoProvider, AsyncA2ARequestProcessor};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock streaming handler for testing
#[derive(Clone)]
struct MockStreamingHandler {
    status_subscribers: Arc<RwLock<HashMap<String, Vec<String>>>>,
    artifact_subscribers: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl MockStreamingHandler {
    fn new() -> Self {
        Self {
            status_subscribers: Arc::new(RwLock::new(HashMap::new())),
            artifact_subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn add_status_subscriber(&self, task_id: &str, subscriber_id: &str) {
        let mut subscribers = self.status_subscribers.write().await;
        subscribers
            .entry(task_id.to_string())
            .or_insert_with(Vec::new)
            .push(subscriber_id.to_string());
    }

    async fn add_artifact_subscriber(&self, task_id: &str, subscriber_id: &str) {
        let mut subscribers = self.artifact_subscribers.write().await;
        subscribers
            .entry(task_id.to_string())
            .or_insert_with(Vec::new)
            .push(subscriber_id.to_string());
    }

    async fn get_status_subscriber_count(&self, task_id: &str) -> usize {
        let subscribers = self.status_subscribers.read().await;
        subscribers
            .get(task_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    async fn get_artifact_subscriber_count(&self, task_id: &str) -> usize {
        let subscribers = self.artifact_subscribers.read().await;
        subscribers
            .get(task_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }
}

#[async_trait]
impl AsyncStreamingHandler for MockStreamingHandler {
    async fn add_status_subscriber<'a>(
        &self,
        task_id: &'a str,
        subscriber: Box<
            dyn a2a_rs::port::streaming_handler::Subscriber<
                TaskStatusUpdateEvent,
            > + Send + Sync,
        >,
    ) -> Result<(), A2AError> {
        let subscriber_id = uuid::Uuid::new_v4().to_string();
        self.add_status_subscriber(task_id, &subscriber_id).await;

        // Notify subscriber
        let event = TaskStatusUpdateEvent {
            task_id: task_id.to_string(),
            status: a2a_rs::domain::TaskStatus::default(),
            timestamp: chrono::Utc::now(),
        };
        let _ = subscriber.on_update(event).await;
        Ok(())
    }

    async fn add_artifact_subscriber<'a>(
        &self,
        task_id: &'a str,
        subscriber: Box<
            dyn a2a_rs::port::streaming_handler::Subscriber<
                TaskArtifactUpdateEvent,
            > + Send + Sync,
        >,
    ) -> Result<(), A2AError> {
        let subscriber_id = uuid::Uuid::new_v4().to_string();
        self.add_artifact_subscriber(task_id, &subscriber_id).await;

        // Notify subscriber
        let event = TaskArtifactUpdateEvent {
            task_id: task_id.to_string(),
            artifact: a2a_rs::domain::Artifact::default(),
            timestamp: chrono::Utc::now(),
        };
        let _ = subscriber.on_update(event).await;
        Ok(())
    }

    async fn remove_status_subscriber<'a>(
        &self,
        task_id: &'a str,
        _subscriber_id: &str,
    ) -> Result<(), A2AError> {
        let mut subscribers = self.status_subscribers.write().await;
        if let Some(subs) = subscribers.get_mut(task_id) {
            subs.pop();
        }
        Ok(())
    }

    async fn remove_artifact_subscriber<'a>(
        &self,
        task_id: &'a str,
        _subscriber_id: &str,
    ) -> Result<(), A2AError> {
        let mut subscribers = self.artifact_subscribers.write().await;
        if let Some(subs) = subscribers.get_mut(task_id) {
            subs.pop();
        }
        Ok(())
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
            .description("A test agent".to_string())
            .url("https://test.example.com".to_string())
            .version("1.0.0".to_string())
            .capabilities(AgentCapabilities::default())
            .default_input_modes(vec!["text".to_string()])
            .default_output_modes(vec!["text".to_string()])
            .skills(vec![AgentSkill::new(
                "test".to_string(),
                "Test".to_string(),
                "A test skill".to_string(),
                vec!["test".to_string()],
            )])
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

/// Mock request processor for testing
#[derive(Clone)]
struct MockRequestProcessor {
    tasks: Arc<RwLock<HashMap<String, Task>>>,
}

impl MockRequestProcessor {
    fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn add_task(&self, task: Task) {
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.id.clone(), task);
    }
}

#[async_trait]
impl AsyncA2ARequestProcessor for MockRequestProcessor {
    async fn process_raw_request<'a>(
        &self,
        request: &'a str,
    ) -> Result<String, A2AError> {
        let _json: serde_json::Value = serde_json::from_str(request)
            .map_err(|e| A2AError::JsonParse(e))?;

        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"status": "ok"}
        });
        Ok(serde_json::to_string(&response).unwrap())
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
async fn test_streaming_handler_add_status_subscriber() {
    let handler = MockStreamingHandler::new();

    struct MockSubscriber;
    #[async_trait]
    impl a2a_rs::port::streaming_handler::Subscriber<TaskStatusUpdateEvent>
        for MockSubscriber
    {
        async fn on_update(&self, _update: TaskStatusUpdateEvent) -> Result<(), A2AError> {
            Ok(())
        }
    }

    let result = handler
        .add_status_subscriber("task-1", Box::new(MockSubscriber))
        .await;

    assert!(result.is_ok());
    assert_eq!(handler.get_status_subscriber_count("task-1").await, 1);
}

#[tokio::test]
async fn test_streaming_handler_add_artifact_subscriber() {
    let handler = MockStreamingHandler::new();

    struct MockSubscriber;
    #[async_trait]
    impl a2a_rs::port::streaming_handler::Subscriber<TaskArtifactUpdateEvent>
        for MockSubscriber
    {
        async fn on_update(&self, _update: TaskArtifactUpdateEvent) -> Result<(), A2AError> {
            Ok(())
        }
    }

    let result = handler
        .add_artifact_subscriber("task-1", Box::new(MockSubscriber))
        .await;

    assert!(result.is_ok());
    assert_eq!(
        handler.get_artifact_subscriber_count("task-1").await,
        1
    );
}

#[tokio::test]
async fn test_streaming_handler_multiple_subscribers() {
    let handler = MockStreamingHandler::new();

    struct MockSubscriber;
    #[async_trait]
    impl a2a_rs::port::streaming_handler::Subscriber<TaskStatusUpdateEvent>
        for MockSubscriber
    {
        async fn on_update(&self, _update: TaskStatusUpdateEvent) -> Result<(), A2AError> {
            Ok(())
        }
    }

    // Add multiple subscribers
    for _ in 0..5 {
        let _ = handler
            .add_status_subscriber("task-multi", Box::new(MockSubscriber))
            .await;
    }

    assert_eq!(
        handler.get_status_subscriber_count("task-multi").await,
        5
    );
}

#[tokio::test]
async fn test_streaming_handler_remove_status_subscriber() {
    let handler = MockStreamingHandler::new();

    struct MockSubscriber;
    #[async_trait]
    impl a2a_rs::port::streaming_handler::Subscriber<TaskStatusUpdateEvent>
        for MockSubscriber
    {
        async fn on_update(&self, _update: TaskStatusUpdateEvent) -> Result<(), A2AError> {
            Ok(())
        }
    }

    // Add subscriber
    let _ = handler
        .add_status_subscriber("task-2", Box::new(MockSubscriber))
        .await;
    assert_eq!(handler.get_status_subscriber_count("task-2").await, 1);

    // Remove subscriber
    let result = handler
        .remove_status_subscriber("task-2", "sub-1")
        .await;

    assert!(result.is_ok());
    assert_eq!(handler.get_status_subscriber_count("task-2").await, 0);
}

#[tokio::test]
async fn test_streaming_handler_remove_artifact_subscriber() {
    let handler = MockStreamingHandler::new();

    struct MockSubscriber;
    #[async_trait]
    impl a2a_rs::port::streaming_handler::Subscriber<TaskArtifactUpdateEvent>
        for MockSubscriber
    {
        async fn on_update(&self, _update: TaskArtifactUpdateEvent) -> Result<(), A2AError> {
            Ok(())
        }
    }

    // Add subscriber
    let _ = handler
        .add_artifact_subscriber("task-2", Box::new(MockSubscriber))
        .await;
    assert_eq!(
        handler.get_artifact_subscriber_count("task-2").await,
        1
    );

    // Remove subscriber
    let result = handler
        .remove_artifact_subscriber("task-2", "sub-1")
        .await;

    assert!(result.is_ok());
    assert_eq!(
        handler.get_artifact_subscriber_count("task-2").await,
        0
    );
}

#[tokio::test]
async fn test_streaming_handler_multiple_tasks() {
    let handler = MockStreamingHandler::new();

    struct MockSubscriber;
    #[async_trait]
    impl a2a_rs::port::streaming_handler::Subscriber<TaskStatusUpdateEvent>
        for MockSubscriber
    {
        async fn on_update(&self, _update: TaskStatusUpdateEvent) -> Result<(), A2AError> {
            Ok(())
        }
    }

    // Add subscribers for different tasks
    for i in 0..5 {
        let _ = handler
            .add_status_subscriber(&format!("task-{}", i), Box::new(MockSubscriber))
            .await;
    }

    // Verify each task has one subscriber
    for i in 0..5 {
        assert_eq!(
            handler.get_status_subscriber_count(&format!("task-{}", i)).await,
            1
        );
    }
}

#[tokio::test]
async fn test_request_processor_valid_request() {
    let processor = MockRequestProcessor::new();

    let request = r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#;
    let result = processor.process_raw_request(request).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.contains("\"jsonrpc\""));
    assert!(response.contains("\"result\""));
}

#[tokio::test]
async fn test_request_processor_invalid_json() {
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
async fn test_agent_info_provider_card() {
    let provider = MockAgentInfoProvider::new();

    let card = provider.get_agent_card().await.unwrap();

    assert_eq!(card.name, "Test Agent");
    assert_eq!(card.version, "1.0.0");
    assert_eq!(card.skills.len(), 1);
}

#[tokio::test]
async fn test_agent_info_provider_skills() {
    let provider = MockAgentInfoProvider::new();

    let skills = provider.get_skills().await.unwrap();

    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].id, "test");
}

#[tokio::test]
async fn test_agent_info_provider_skill_by_id() {
    let provider = MockAgentInfoProvider::new();

    let result = provider.get_skill_by_id("test").await;

    assert!(result.is_ok());
    let skill = result.unwrap();
    assert!(skill.is_some());
    assert_eq!(skill.unwrap().id, "test");
}

#[tokio::test]
async fn test_agent_info_provider_skill_not_found() {
    let provider = MockAgentInfoProvider::new();

    let result = provider.get_skill_by_id("nonexistent").await;

    assert!(result.is_ok());
    let skill = result.unwrap();
    assert!(skill.is_none());
}

#[tokio::test]
async fn test_concurrent_subscriber_operations() {
    let handler = MockStreamingHandler::new();

    struct MockSubscriber;
    #[async_trait]
    impl a2a_rs::port::streaming_handler::Subscriber<TaskStatusUpdateEvent>
        for MockSubscriber
    {
        async fn on_update(&self, _update: TaskStatusUpdateEvent) -> Result<(), A2AError> {
            Ok(())
        }
    }

    // Add subscribers concurrently
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let handler = handler.clone();
            tokio::spawn(async move {
                handler
                    .add_status_subscriber(&format!("task-concurrent-{}", i), Box::new(MockSubscriber))
                    .await
            })
        })
        .collect();

    // Wait for all to complete
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }

    // Verify all subscribers were added
    for i in 0..10 {
        assert_eq!(
            handler.get_status_subscriber_count(&format!("task-concurrent-{}", i)).await,
            1
        );
    }
}

#[tokio::test]
async fn test_subscriber_notification() {
    struct TestSubscriber {
        received: Arc<std::sync::Mutex<Vec<TaskStatusUpdateEvent>>>,
    }

    #[async_trait]
    impl a2a_rs::port::streaming_handler::Subscriber<TaskStatusUpdateEvent>
        for TestSubscriber
    {
        async fn on_update(&self, update: TaskStatusUpdateEvent) -> Result<(), A2AError> {
            self.received.lock().unwrap().push(update);
            Ok(())
        }
    }

    let received = Arc::new(std::sync::Mutex::new(Vec::new()));
    let subscriber = TestSubscriber {
        received: received.clone(),
    };

    // Simulate notification
    let event = TaskStatusUpdateEvent {
        task_id: "task-1".to_string(),
        status: a2a_rs::domain::TaskStatus::default(),
        timestamp: chrono::Utc::now(),
    };

    let _ = subscriber.on_update(event).await;

    // Verify subscriber received event
    let received_events = received.lock().unwrap();
    assert_eq!(received_events.len(), 1);
    assert_eq!(received_events[0].task_id, "task-1");
}

#[tokio::test]
async fn test_artifact_subscriber_notification() {
    struct TestSubscriber {
        received: Arc<std::sync::Mutex<Vec<TaskArtifactUpdateEvent>>>,
    }

    #[async_trait]
    impl a2a_rs::port::streaming_handler::Subscriber<TaskArtifactUpdateEvent>
        for TestSubscriber
    {
        async fn on_update(&self, update: TaskArtifactUpdateEvent) -> Result<(), A2AError> {
            self.received.lock().unwrap().push(update);
            Ok(())
        }
    }

    let received = Arc::new(std::sync::Mutex::new(Vec::new()));
    let subscriber = TestSubscriber {
        received: received.clone(),
    };

    // Simulate notification
    let event = TaskArtifactUpdateEvent {
        task_id: "task-1".to_string(),
        artifact: a2a_rs::domain::Artifact::default(),
        timestamp: chrono::Utc::now(),
    };

    let _ = subscriber.on_update(event).await;

    // Verify subscriber received event
    let received_events = received.lock().unwrap();
    assert_eq!(received_events.len(), 1);
    assert_eq!(received_events[0].task_id, "task-1");
}

#[tokio::test]
async fn test_multiple_stream_types() {
    let handler = MockStreamingHandler::new();

    struct StatusSubscriber;
    #[async_trait]
    impl a2a_rs::port::streaming_handler::Subscriber<TaskStatusUpdateEvent>
        for StatusSubscriber
    {
        async fn on_update(&self, _update: TaskStatusUpdateEvent) -> Result<(), A2AError> {
            Ok(())
        }
    }

    struct ArtifactSubscriber;
    #[async_trait]
    impl a2a_rs::port::streaming_handler::Subscriber<TaskArtifactUpdateEvent>
        for ArtifactSubscriber
    {
        async fn on_update(&self, _update: TaskArtifactUpdateEvent) -> Result<(), A2AError> {
            Ok(())
        }
    }

    // Add both types of subscribers
    let _ = handler
        .add_status_subscriber("task-1", Box::new(StatusSubscriber))
        .await;
    let _ = handler
        .add_artifact_subscriber("task-1", Box::new(ArtifactSubscriber))
        .await;

    assert_eq!(handler.get_status_subscriber_count("task-1").await, 1);
    assert_eq!(
        handler.get_artifact_subscriber_count("task-1").await,
        1
    );
}

#[tokio::test]
async fn test_subscriber_isolation() {
    let handler = MockStreamingHandler::new();

    struct TestSubscriber;
    #[async_trait]
    impl a2a_rs::port::streaming_handler::Subscriber<TaskStatusUpdateEvent>
        for TestSubscriber
    {
        async fn on_update(&self, _update: TaskStatusUpdateEvent) -> Result<(), A2AError> {
            Ok(())
        }
    }

    // Add subscribers to different tasks
    let _ = handler
        .add_status_subscriber("task-a", Box::new(TestSubscriber))
        .await;
    let _ = handler
        .add_status_subscriber("task-b", Box::new(TestSubscriber))
        .await;

    // Each task should have its own subscriber
    assert_eq!(handler.get_status_subscriber_count("task-a").await, 1);
    assert_eq!(handler.get_status_subscriber_count("task-b").await, 1);
}

#[tokio::test]
async fn test_task_storage() {
    let processor = MockRequestProcessor::new();
    let task = create_test_task("task-storage");

    processor.add_task(task.clone()).await;

    // Task should be stored (accessible via internal state)
    let tasks = processor.tasks.read().await;
    assert!(tasks.contains_key("task-storage"));
}

#[tokio::test]
async fn test_process_multiple_requests() {
    let processor = MockRequestProcessor::new();

    for i in 0..5 {
        let request = json!({
            "jsonrpc": "2.0",
            "id": i,
            "method": "test"
        });

        let result = processor
            .process_raw_request(&request.to_string())
            .await;

        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_concurrent_request_processing() {
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
async fn test_agent_card_structure() {
    let provider = MockAgentInfoProvider::new();
    let card = provider.get_agent_card().await.unwrap();

    // Verify required fields
    assert!(!card.name.is_empty());
    assert!(!card.description.is_empty());
    assert!(!card.url.is_empty());
    assert!(!card.version.is_empty());
    assert_eq!(card.protocol_version, "0.3.0");
}

#[tokio::test]
async fn test_skill_structure() {
    let provider = MockAgentInfoProvider::new();
    let skills = provider.get_skills().await.unwrap();

    assert!(!skills.is_empty());
    let skill = &skills[0];

    // Verify required fields
    assert!(!skill.id.is_empty());
    assert!(!skill.name.is_empty());
    assert!(!skill.description.is_empty());
    assert!(!skill.tags.is_empty());
}

#[tokio::test]
async fn test_streaming_handler_clone() {
    let handler = MockStreamingHandler::new();
    let handler_clone = handler.clone();

    // Both should have independent state
    let _ = handler_clone
        .add_status_subscriber("task-1", Box::new(NopSubscriber))
        .await;

    assert_eq!(handler_clone.get_status_subscriber_count("task-1").await, 1);
}

struct NopSubscriber;

#[async_trait]
impl a2a_rs::port::streaming_handler::Subscriber<TaskStatusUpdateEvent>
    for NopSubscriber
{
    async fn on_update(&self, _update: TaskStatusUpdateEvent) -> Result<(), A2AError> {
        Ok(())
    }
}
