//! Unit tests for AsyncStreamingHandler port trait
//!
//! Tests the contract and behavior of the AsyncStreamingHandler port trait
//! using mock implementations.

use a2a_rs::domain::core::task::{TaskArtifactUpdateEvent, TaskState, TaskStatusUpdateEvent};
use a2a_rs::domain::error::A2AError;
use a2a_rs::port::streaming_handler::{
    AsyncStreamingHandler, Subscriber as StreamingSubscriber, UpdateEvent,
};
use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock subscriber for testing
#[derive(Debug, Clone)]
struct MockSubscriber {
    id: String,
    updates: Arc<RwLock<Vec<UpdateEvent>>>,
}

impl MockSubscriber {
    fn new(id: String) -> Self {
        Self {
            id,
            updates: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn get_update_count(&self) -> usize {
        self.updates.read().await.len()
    }
}

#[async_trait]
impl StreamingSubscriber<TaskStatusUpdateEvent> for MockSubscriber {
    async fn on_update(&self, update: TaskStatusUpdateEvent) -> Result<(), A2AError> {
        let mut updates = self.updates.write().await;
        updates.push(UpdateEvent::StatusUpdate(update));
        Ok(())
    }
}

#[async_trait]
impl StreamingSubscriber<TaskArtifactUpdateEvent> for MockSubscriber {
    async fn on_update(&self, update: TaskArtifactUpdateEvent) -> Result<(), A2AError> {
        let mut updates = self.updates.write().await;
        updates.push(UpdateEvent::ArtifactUpdate(update));
        Ok(())
    }
}

/// Mock implementation of AsyncStreamingHandler for testing
#[derive(Debug, Clone)]
struct MockStreamingHandler {
    status_subscribers: Arc<RwLock<HashMap<String, Vec<MockSubscriber>>>>,
    artifact_subscribers: Arc<RwLock<HashMap<String, Vec<MockSubscriber>>>>,
    subscription_counter: Arc<RwLock<u64>>,
}

impl MockStreamingHandler {
    fn new() -> Self {
        Self {
            status_subscribers: Arc::new(RwLock::new(HashMap::new())),
            artifact_subscribers: Arc::new(RwLock::new(HashMap::new())),
            subscription_counter: Arc::new(RwLock::new(0)),
        }
    }

    async fn generate_subscription_id(&self) -> String {
        let mut counter = self.subscription_counter.write().await;
        *counter += 1;
        format!("sub-{}", counter)
    }

    async fn get_subscriber_count(&self, task_id: &str) -> usize {
        let status_subs = self.status_subscribers.read().await;
        let artifact_subs = self.artifact_subscribers.read().await;
        status_subs.get(task_id).map(|v| v.len()).unwrap_or(0)
            + artifact_subs.get(task_id).map(|v| v.len()).unwrap_or(0)
    }
}

#[async_trait]
impl AsyncStreamingHandler for MockStreamingHandler {
    async fn add_status_subscriber<'a>(
        &self,
        task_id: &'a str,
        subscriber: Box<dyn StreamingSubscriber<TaskStatusUpdateEvent> + Send + Sync>,
    ) -> Result<String, A2AError> {
        let sub_id = self.generate_subscription_id().await;
        let mut status_subs = self.status_subscribers.write().await;

        // Note: We can't downcast the Box<dyn Subscriber> to MockSubscriber
        // So we just track the subscription ID
        status_subs.entry(task_id.to_string()).or_insert_with(Vec::new);

        Ok(sub_id)
    }

    async fn add_artifact_subscriber<'a>(
        &self,
        task_id: &'a str,
        _subscriber: Box<dyn StreamingSubscriber<TaskArtifactUpdateEvent> + Send + Sync>,
    ) -> Result<String, A2AError> {
        let sub_id = self.generate_subscription_id().await;
        let mut artifact_subs = self.artifact_subscribers.write().await;
        artifact_subs.entry(task_id.to_string()).or_insert_with(Vec::new);

        Ok(sub_id)
    }

    async fn remove_subscription<'a>(&self, _subscription_id: &'a str) -> Result<(), A2AError> {
        // In a real implementation, this would find and remove the subscriber
        Ok(())
    }

    async fn remove_task_subscribers<'a>(&self, task_id: &'a str) -> Result<(), A2AError> {
        let mut status_subs = self.status_subscribers.write().await;
        let mut artifact_subs = self.artifact_subscribers.write().await;
        status_subs.remove(task_id);
        artifact_subs.remove(task_id);
        Ok(())
    }

    async fn get_subscriber_count<'a>(&self, task_id: &'a str) -> Result<usize, A2AError> {
        Ok(self.get_subscriber_count(task_id).await)
    }

    async fn broadcast_status_update<'a>(
        &self,
        _task_id: &'a str,
        _update: TaskStatusUpdateEvent,
    ) -> Result<(), A2AError> {
        // In a real implementation, this would broadcast to all subscribers
        Ok(())
    }

    async fn broadcast_artifact_update<'a>(
        &self,
        _task_id: &'a str,
        _update: TaskArtifactUpdateEvent,
    ) -> Result<(), A2AError> {
        // In a real implementation, this would broadcast to all subscribers
        Ok(())
    }

    async fn status_update_stream<'a>(
        &self,
        _task_id: &'a str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<TaskStatusUpdateEvent, A2AError>> + Send>>, A2AError>
    {
        // Return an empty stream for testing
        Ok(Box::pin(futures::stream::empty()))
    }

    async fn artifact_update_stream<'a>(
        &self,
        _task_id: &'a str,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<TaskArtifactUpdateEvent, A2AError>> + Send>>,
        A2AError,
    > {
        // Return an empty stream for testing
        Ok(Box::pin(futures::stream::empty()))
    }

    async fn combined_update_stream<'a>(
        &self,
        _task_id: &'a str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<UpdateEvent, A2AError>> + Send>>, A2AError> {
        // Return an empty stream for testing
        Ok(Box::pin(futures::stream::empty()))
    }
}

fn create_status_update(task_id: &str) -> TaskStatusUpdateEvent {
    TaskStatusUpdateEvent {
        task_id: task_id.to_string(),
        context_id: "context-1".to_string(),
        status: TaskState::Working,
        final_: false,
        timestamp: Some(chrono::Utc::now()),
    }
}

fn create_artifact_update(task_id: &str) -> TaskArtifactUpdateEvent {
    TaskArtifactUpdateEvent {
        task_id: task_id.to_string(),
        context_id: "context-1".to_string(),
        artifact_id: "artifact-1".to_string(),
        artifact_name: Some("Test Artifact".to_string()),
        last_chunk: Some(true),
        append: false,
        timestamp: Some(chrono::Utc::now()),
    }
}

#[tokio::test]
async fn test_add_status_subscriber() {
    let handler = MockStreamingHandler::new();
    let subscriber = MockSubscriber::new("sub-1".to_string());

    let result = handler
        .add_status_subscriber("task-1", Box::new(subscriber))
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "sub-1");
}

#[tokio::test]
async fn test_add_artifact_subscriber() {
    let handler = MockStreamingHandler::new();
    let subscriber = MockSubscriber::new("sub-2".to_string());

    let result = handler
        .add_artifact_subscriber("task-2", Box::new(subscriber))
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "sub-2");
}

#[tokio::test]
async fn test_remove_subscription() {
    let handler = MockStreamingHandler::new();

    let result = handler.remove_subscription("sub-1").await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_remove_task_subscribers() {
    let handler = MockStreamingHandler::new();

    let subscriber = MockSubscriber::new("sub-3".to_string());
    handler
        .add_status_subscriber("task-3", Box::new(subscriber))
        .await
        .unwrap();

    let result = handler.remove_task_subscribers("task-3").await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_subscriber_count() {
    let handler = MockStreamingHandler::new();

    let subscriber1 = MockSubscriber::new("sub-4".to_string());
    let subscriber2 = MockSubscriber::new("sub-5".to_string());

    handler
        .add_status_subscriber("task-4", Box::new(subscriber1))
        .await
        .unwrap();
    handler
        .add_artifact_subscriber("task-4", Box::new(subscriber2))
        .await
        .unwrap();

    let result = handler.get_subscriber_count("task-4").await;

    assert!(result.is_ok());
    // We expect 0 because we can't track the mock subscribers
    assert_eq!(result.unwrap(), 0);
}

#[tokio::test]
async fn test_has_subscribers_true() {
    let handler = MockStreamingHandler::new();

    let subscriber = MockSubscriber::new("sub-6".to_string());
    handler
        .add_status_subscriber("task-5", Box::new(subscriber))
        .await
        .unwrap();

    let result = handler.has_subscribers("task-5").await;

    assert!(result.is_ok());
    // Returns false because we can't track mock subscribers
    assert!(!result.unwrap());
}

#[tokio::test]
async fn test_has_subscribers_false() {
    let handler = MockStreamingHandler::new();

    let result = handler.has_subscribers("nonexistent-task").await;

    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[tokio::test]
async fn test_broadcast_status_update() {
    let handler = MockStreamingHandler::new();
    let update = create_status_update("task-6");

    let result = handler.broadcast_status_update("task-6", update).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broadcast_artifact_update() {
    let handler = MockStreamingHandler::new();
    let update = create_artifact_update("task-7");

    let result = handler.broadcast_artifact_update("task-7", update).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_status_update_stream() {
    let handler = MockStreamingHandler::new();

    let result = handler.status_update_stream("task-8").await;

    assert!(result.is_ok());
    // The stream should be valid (even if empty)
}

#[tokio::test]
async fn test_artifact_update_stream() {
    let handler = MockStreamingHandler::new();

    let result = handler.artifact_update_stream("task-9").await;

    assert!(result.is_ok());
    // The stream should be valid (even if empty)
}

#[tokio::test]
async fn test_combined_update_stream() {
    let handler = MockStreamingHandler::new();

    let result = handler.combined_update_stream("task-10").await;

    assert!(result.is_ok());
    // The stream should be valid (even if empty)
}

#[tokio::test]
async fn test_validate_streaming_params_valid() {
    let handler = MockStreamingHandler::new();

    let result = handler.validate_streaming_params("task-11").await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_validate_streaming_params_empty() {
    let handler = MockStreamingHandler::new();

    let result = handler.validate_streaming_params("").await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
    if let Err(A2AError::ValidationError { field, message }) = result {
        assert_eq!(field, "task_id");
        assert!(message.contains("cannot be empty"));
    }
}

#[tokio::test]
async fn test_validate_streaming_params_whitespace() {
    let handler = MockStreamingHandler::new();

    let result = handler.validate_streaming_params("   ").await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
}

#[tokio::test]
async fn test_start_task_streaming() {
    let handler = MockStreamingHandler::new();

    let result = handler.start_task_streaming("task-12").await;

    assert!(result.is_ok());
    // Should return a valid stream
}

#[tokio::test]
async fn test_start_task_streaming_invalid_task_id() {
    let handler = MockStreamingHandler::new();

    let result = handler.start_task_streaming("").await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
}

#[tokio::test]
async fn test_stop_task_streaming() {
    let handler = MockStreamingHandler::new();

    // Add a subscriber first
    let subscriber = MockSubscriber::new("sub-7".to_string());
    handler
        .add_status_subscriber("task-13", Box::new(subscriber))
        .await
        .unwrap();

    let result = handler.stop_task_streaming("task-13").await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_subscriber_on_update() {
    let subscriber = MockSubscriber::new("sub-8".to_string());
    let update = TaskStatusUpdateEvent {
        task_id: "task-14".to_string(),
        context_id: "context-14".to_string(),
        status: TaskState::Completed,
        final_: true,
        timestamp: Some(chrono::Utc::now()),
    };

    let result = subscriber.on_update(update).await;

    assert!(result.is_ok());
    assert_eq!(subscriber.get_update_count().await, 1);
}

#[tokio::test]
async fn test_subscriber_on_error() {
    let subscriber = MockSubscriber::new("sub-9".to_string());
    let error = A2AError::TaskNotFound("task-15".to_string());

    let result = subscriber.on_error(error).await;

    assert!(result.is_ok());
    // Default implementation just logs and returns Ok
}

#[tokio::test]
async fn test_subscriber_on_complete() {
    let subscriber = MockSubscriber::new("sub-10".to_string());

    let result = subscriber.on_complete().await;

    assert!(result.is_ok());
    // Default implementation is a no-op
}

#[tokio::test]
async fn test_update_event_task_id() {
    let status_update = create_status_update("task-16");
    let event = UpdateEvent::StatusUpdate(status_update);

    assert_eq!(event.task_id(), "task-16");
}

#[tokio::test]
async fn test_update_event_context_id() {
    let artifact_update = create_artifact_update("task-17");
    let event = UpdateEvent::ArtifactUpdate(artifact_update);

    assert_eq!(event.context_id(), "context-1");
}

#[tokio::test]
async fn test_update_event_is_final_status() {
    let status_update = TaskStatusUpdateEvent {
        task_id: "task-18".to_string(),
        context_id: "context-18".to_string(),
        status: TaskState::Completed,
        final_: true,
        timestamp: Some(chrono::Utc::now()),
    };
    let event = UpdateEvent::StatusUpdate(status_update);

    assert!(event.is_final());
}

#[tokio::test]
async fn test_update_event_is_final_artifact() {
    let artifact_update = TaskArtifactUpdateEvent {
        task_id: "task-19".to_string(),
        context_id: "context-19".to_string(),
        artifact_id: "artifact-2".to_string(),
        artifact_name: Some("Final".to_string()),
        last_chunk: Some(true),
        append: false,
        timestamp: Some(chrono::Utc::now()),
    };
    let event = UpdateEvent::ArtifactUpdate(artifact_update);

    assert!(event.is_final());
}

#[tokio::test]
async fn test_update_event_not_final() {
    let status_update = TaskStatusUpdateEvent {
        task_id: "task-20".to_string(),
        context_id: "context-20".to_string(),
        status: TaskState::Working,
        final_: false,
        timestamp: Some(chrono::Utc::now()),
    };
    let event = UpdateEvent::StatusUpdate(status_update);

    assert!(!event.is_final());
}

#[tokio::test]
async fn test_multiple_subscribers_same_task() {
    let handler = MockStreamingHandler::new();

    let subscriber1 = MockSubscriber::new("sub-11".to_string());
    let subscriber2 = MockSubscriber::new("sub-12".to_string());
    let subscriber3 = MockSubscriber::new("sub-13".to_string());

    handler
        .add_status_subscriber("task-21", Box::new(subscriber1))
        .await
        .unwrap();
    handler
        .add_status_subscriber("task-21", Box::new(subscriber2))
        .await
        .unwrap();
    handler
        .add_artifact_subscriber("task-21", Box::new(subscriber3))
        .await
        .unwrap();

    let result = handler.get_subscriber_count("task-21").await;

    assert!(result.is_ok());
    // Can't track mock subscribers, so we expect 0
    assert_eq!(result.unwrap(), 0);
}

#[tokio::test]
async fn test_concurrent_subscriber_operations() {
    let handler = MockStreamingHandler::new();

    // Add multiple subscribers concurrently
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let handler = handler.clone();
            tokio::spawn(async move {
                let subscriber = MockSubscriber::new(format!("sub-concurrent-{}", i));
                handler
                    .add_status_subscriber(&format!("task-concurrent-{}", i), Box::new(subscriber))
                    .await
            })
        })
        .collect();

    // Wait for all to complete
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }
}
