//! Server-Sent Events (SSE) streaming adapter
//!
//! Implements real-time streaming of task updates via SSE using axum.

use async_trait::async_trait;
use axum::response::sse::{Event, KeepAlive};
use futures::stream::{Stream, StreamExt};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::domain::{A2AError, TaskArtifactUpdateEvent, TaskStatusUpdateEvent};
use crate::port::{AsyncStreamingHandler, UpdateEvent};

/// Configuration for SSE streaming
#[derive(Debug, Clone)]
pub struct SseConfig {
    /// Channel buffer size for each task subscription
    pub buffer_size: usize,
    /// Heartbeat keepalive interval (default: 15 seconds)
    pub keepalive_interval: Duration,
    /// Maximum number of subscribers per task (default: 100)
    pub max_subscribers_per_task: usize,
}

impl Default for SseConfig {
    fn default() -> Self {
        Self {
            buffer_size: 100,
            keepalive_interval: Duration::from_secs(15),
            max_subscribers_per_task: 100,
        }
    }
}

/// Subscription info for a task
#[derive(Debug)]
struct TaskSubscription {
    status_tx: broadcast::Sender<TaskStatusUpdateEvent>,
    artifact_tx: broadcast::Sender<TaskArtifactUpdateEvent>,
    subscriber_count: usize,
}

/// SSE streaming handler implementation
pub struct SseStreamingHandler {
    config: SseConfig,
    /// Map of task_id -> TaskSubscription
    subscriptions: Arc<RwLock<HashMap<String, TaskSubscription>>>,
    /// Map of subscription_id -> task_id for cleanup
    subscription_map: Arc<RwLock<HashMap<String, String>>>,
}

impl SseStreamingHandler {
    /// Create a new SSE streaming handler with default config
    pub fn new() -> Self {
        Self::with_config(SseConfig::default())
    }

    /// Create a new SSE streaming handler with custom config
    pub fn with_config(config: SseConfig) -> Self {
        Self {
            config,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            subscription_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a subscription for a task
    async fn get_or_create_subscription(&self, task_id: &str) -> TaskSubscription {
        let mut subs = self.subscriptions.write().await;

        if let Some(sub) = subs.get(task_id) {
            return TaskSubscription {
                status_tx: sub.status_tx.clone(),
                artifact_tx: sub.artifact_tx.clone(),
                subscriber_count: sub.subscriber_count,
            };
        }

        // Create new subscription channels
        let (status_tx, _) = broadcast::channel(self.config.buffer_size);
        let (artifact_tx, _) = broadcast::channel(self.config.buffer_size);

        let sub = TaskSubscription {
            status_tx: status_tx.clone(),
            artifact_tx: artifact_tx.clone(),
            subscriber_count: 0,
        };

        subs.insert(task_id.to_string(), sub);

        TaskSubscription {
            status_tx,
            artifact_tx,
            subscriber_count: 0,
        }
    }

    /// Increment subscriber count for a task
    async fn increment_subscriber(&self, task_id: &str) -> Result<(), A2AError> {
        let mut subs = self.subscriptions.write().await;

        if let Some(sub) = subs.get_mut(task_id) {
            if sub.subscriber_count >= self.config.max_subscribers_per_task {
                return Err(A2AError::StreamingError {
                    message: format!(
                        "Maximum subscribers ({}) reached for task {}",
                        self.config.max_subscribers_per_task,
                        task_id
                    ),
                });
            }
            sub.subscriber_count += 1;
        }

        Ok(())
    }

    /// Decrement subscriber count for a task
    async fn decrement_subscriber(&self, task_id: &str) {
        let mut subs = self.subscriptions.write().await;

        if let Some(sub) = subs.get_mut(task_id) {
            sub.subscriber_count = sub.subscriber_count.saturating_sub(1);

            // Clean up if no more subscribers
            if sub.subscriber_count == 0 {
                subs.remove(task_id);
            }
        }
    }

    /// Create an SSE event from an update event
    fn create_sse_event(update: &UpdateEvent) -> Result<Event, A2AError> {
        let json = serde_json::to_string(update).map_err(|e| A2AError::SerializationError {
            message: format!("Failed to serialize update event: {}", e),
        })?;

        let event_type = match update {
            UpdateEvent::StatusUpdate(_) => "status-update",
            UpdateEvent::ArtifactUpdate(_) => "artifact-update",
        };

        Ok(Event::default()
            .event(event_type)
            .data(json))
    }

    /// Create a heartbeat SSE event
    fn create_heartbeat_event() -> Event {
        Event::default()
            .event("heartbeat")
            .data("ping")
    }
}

impl Default for SseStreamingHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AsyncStreamingHandler for SseStreamingHandler {
    async fn add_status_subscriber<'a>(
        &self,
        task_id: &'a str,
        _subscriber: Box<dyn crate::port::Subscriber<TaskStatusUpdateEvent> + Send + Sync>,
    ) -> Result<String, A2AError> {
        // For SSE, we manage subscriptions via streams, not individual subscribers
        // This method is kept for compatibility but delegates to stream creation
        let subscription_id = Uuid::new_v4().to_string();

        self.increment_subscriber(task_id).await?;

        let mut map = self.subscription_map.write().await;
        map.insert(subscription_id.clone(), task_id.to_string());

        Ok(subscription_id)
    }

    async fn add_artifact_subscriber<'a>(
        &self,
        task_id: &'a str,
        _subscriber: Box<dyn crate::port::Subscriber<TaskArtifactUpdateEvent> + Send + Sync>,
    ) -> Result<String, A2AError> {
        // For SSE, we manage subscriptions via streams, not individual subscribers
        let subscription_id = Uuid::new_v4().to_string();

        self.increment_subscriber(task_id).await?;

        let mut map = self.subscription_map.write().await;
        map.insert(subscription_id.clone(), task_id.to_string());

        Ok(subscription_id)
    }

    async fn remove_subscription<'a>(&self, subscription_id: &'a str) -> Result<(), A2AError> {
        let task_id = {
            let mut map = self.subscription_map.write().await;
            map.remove(subscription_id)
        };

        if let Some(task_id) = task_id {
            self.decrement_subscriber(&task_id).await;
        }

        Ok(())
    }

    async fn remove_task_subscribers<'a>(&self, task_id: &'a str) -> Result<(), A2AError> {
        let mut subs = self.subscriptions.write().await;
        subs.remove(task_id);

        // Clean up subscription map entries for this task
        let mut map = self.subscription_map.write().await;
        map.retain(|_, tid| tid != task_id);

        Ok(())
    }

    async fn get_subscriber_count<'a>(&self, task_id: &'a str) -> Result<usize, A2AError> {
        let subs = self.subscriptions.read().await;
        Ok(subs.get(task_id).map(|s| s.subscriber_count).unwrap_or(0))
    }

    async fn broadcast_status_update<'a>(
        &self,
        task_id: &'a str,
        update: TaskStatusUpdateEvent,
    ) -> Result<(), A2AError> {
        let subs = self.subscriptions.read().await;

        if let Some(sub) = subs.get(task_id) {
            // Ignore send errors - they just mean no active receivers
            let _ = sub.status_tx.send(update);
        }

        Ok(())
    }

    async fn broadcast_artifact_update<'a>(
        &self,
        task_id: &'a str,
        update: TaskArtifactUpdateEvent,
    ) -> Result<(), A2AError> {
        let subs = self.subscriptions.read().await;

        if let Some(sub) = subs.get(task_id) {
            // Ignore send errors - they just mean no active receivers
            let _ = sub.artifact_tx.send(update);
        }

        Ok(())
    }

    async fn status_update_stream<'a>(
        &self,
        task_id: &'a str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<TaskStatusUpdateEvent, A2AError>> + Send>>, A2AError> {
        let sub = self.get_or_create_subscription(task_id).await;
        self.increment_subscriber(task_id).await?;

        let mut rx = sub.status_tx.subscribe();
        let task_id = task_id.to_string();
        let handler = self.clone();

        let stream = async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(update) => yield Ok(update),
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        #[cfg(feature = "tracing")]
                        tracing::warn!("SSE stream lagged by {} messages for task {}", n, task_id);
                        continue;
                    }
                }
            }
            // Clean up on stream drop
            handler.decrement_subscriber(&task_id).await;
        };

        Ok(Box::pin(stream))
    }

    async fn artifact_update_stream<'a>(
        &self,
        task_id: &'a str,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<TaskArtifactUpdateEvent, A2AError>> + Send>>,
        A2AError,
    > {
        let sub = self.get_or_create_subscription(task_id).await;
        self.increment_subscriber(task_id).await?;

        let mut rx = sub.artifact_tx.subscribe();
        let task_id = task_id.to_string();
        let handler = self.clone();

        let stream = async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(update) => yield Ok(update),
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        #[cfg(feature = "tracing")]
                        tracing::warn!("SSE stream lagged by {} messages for task {}", n, task_id);
                        continue;
                    }
                }
            }
            // Clean up on stream drop
            handler.decrement_subscriber(&task_id).await;
        };

        Ok(Box::pin(stream))
    }

    async fn combined_update_stream<'a>(
        &self,
        task_id: &'a str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<UpdateEvent, A2AError>> + Send>>, A2AError> {
        let sub = self.get_or_create_subscription(task_id).await;
        self.increment_subscriber(task_id).await?;

        let mut status_rx = sub.status_tx.subscribe();
        let mut artifact_rx = sub.artifact_tx.subscribe();
        let task_id = task_id.to_string();
        let handler = self.clone();

        let stream = async_stream::stream! {
            loop {
                tokio::select! {
                    result = status_rx.recv() => {
                        match result {
                            Ok(update) => yield Ok(UpdateEvent::StatusUpdate(update)),
                            Err(broadcast::error::RecvError::Closed) => break,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                #[cfg(feature = "tracing")]
                                tracing::warn!("SSE status stream lagged by {} messages for task {}", n, task_id);
                                continue;
                            }
                        }
                    }
                    result = artifact_rx.recv() => {
                        match result {
                            Ok(update) => yield Ok(UpdateEvent::ArtifactUpdate(update)),
                            Err(broadcast::error::RecvError::Closed) => break,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                #[cfg(feature = "tracing")]
                                tracing::warn!("SSE artifact stream lagged by {} messages for task {}", n, task_id);
                                continue;
                            }
                        }
                    }
                }
            }
            // Clean up on stream drop
            handler.decrement_subscriber(&task_id).await;
        };

        Ok(Box::pin(stream))
    }
}

// Implement Clone manually since Arc fields are cloneable
impl Clone for SseStreamingHandler {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            subscriptions: Arc::clone(&self.subscriptions),
            subscription_map: Arc::clone(&self.subscription_map),
        }
    }
}

/// Helper function to create an SSE stream from an UpdateEvent stream
///
/// This converts a stream of UpdateEvent into a stream of axum SSE events
/// with automatic heartbeat keepalive.
pub fn create_sse_stream(
    update_stream: Pin<Box<dyn Stream<Item = Result<UpdateEvent, A2AError>> + Send>>,
    keepalive: Duration,
) -> impl Stream<Item = Result<Event, axum::Error>> {
    let event_stream = update_stream.map(|result| {
        result
            .and_then(|update| SseStreamingHandler::create_sse_event(&update))
            .map_err(|e| {
                axum::Error::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })
    });

    // Add heartbeat keepalive
    let heartbeat_interval = tokio::time::interval(keepalive);
    let heartbeat_stream = tokio_stream::wrappers::IntervalStream::new(heartbeat_interval)
        .map(|_| Ok(SseStreamingHandler::create_heartbeat_event()));

    // Merge the two streams
    futures::stream::select(event_stream, heartbeat_stream)
}

/// Helper function to create an SSE response for a task
///
/// This is a convenience function for creating SSE responses in axum handlers.
///
/// # Example
///
/// ```rust,no_run
/// use axum::{Router, routing::get, extract::{State, Path}};
/// use a2a_rs::adapter::streaming::sse::{SseStreamingHandler, task_sse_stream};
/// use std::sync::Arc;
///
/// async fn stream_task_updates(
///     State(handler): State<Arc<SseStreamingHandler>>,
///     Path(task_id): Path<String>,
/// ) -> axum::response::Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, axum::Error>>> {
///     task_sse_stream(handler, &task_id).await
/// }
/// ```
pub async fn task_sse_stream(
    handler: Arc<SseStreamingHandler>,
    task_id: &str,
) -> axum::response::Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let keepalive = handler.config.keepalive_interval;

    let stream = match handler.combined_update_stream(task_id).await {
        Ok(update_stream) => create_sse_stream(update_stream, keepalive),
        Err(e) => {
            // Return an error stream
            let error_stream = futures::stream::once(async move {
                Err(axum::Error::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )))
            });
            return axum::response::Sse::new(error_stream)
                .keep_alive(KeepAlive::new().interval(keepalive));
        }
    };

    axum::response::Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(keepalive))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_increment_subscriber() {
        let handler = SseStreamingHandler::new();
        let task_id = "test-task-1";

        // Initially no subscribers
        assert_eq!(handler.get_subscriber_count(task_id).await.unwrap(), 0);

        // Create subscription
        handler.get_or_create_subscription(task_id).await;
        handler.increment_subscriber(task_id).await.unwrap();

        // Should have 1 subscriber
        assert_eq!(handler.get_subscriber_count(task_id).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_broadcast_status_update() {
        let handler = SseStreamingHandler::new();
        let task_id = "test-task-2";

        // Create a subscription and get a receiver
        let stream = handler.status_update_stream(task_id).await.unwrap();

        // Broadcast an update
        let update = TaskStatusUpdateEvent {
            task_id: task_id.to_string(),
            context_id: "ctx-1".to_string(),
            kind: "status-update".to_string(),
            status: crate::domain::TaskStatus::InProgress,
            final_: false,
            metadata: None,
        };

        handler.broadcast_status_update(task_id, update.clone()).await.unwrap();

        // Stream should receive the update
        let mut stream = stream;
        let received = tokio::time::timeout(
            Duration::from_secs(1),
            stream.next(),
        ).await;

        assert!(received.is_ok());
        let item = received.unwrap();
        assert!(item.is_some());
        let result = item.unwrap();
        assert!(result.is_ok());
        let received_update = result.unwrap();
        assert_eq!(received_update.task_id, task_id);
    }

    #[tokio::test]
    async fn test_max_subscribers_limit() {
        let config = SseConfig {
            buffer_size: 10,
            keepalive_interval: Duration::from_secs(15),
            max_subscribers_per_task: 2,
        };
        let handler = SseStreamingHandler::with_config(config);
        let task_id = "test-task-3";

        // Add first subscriber - should succeed
        handler.increment_subscriber(task_id).await.unwrap();

        // Add second subscriber - should succeed
        handler.increment_subscriber(task_id).await.unwrap();

        // Add third subscriber - should fail
        let result = handler.increment_subscriber(task_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remove_task_subscribers() {
        let handler = SseStreamingHandler::new();
        let task_id = "test-task-4";

        // Add some subscribers
        handler.increment_subscriber(task_id).await.unwrap();
        handler.increment_subscriber(task_id).await.unwrap();

        assert_eq!(handler.get_subscriber_count(task_id).await.unwrap(), 2);

        // Remove all subscribers
        handler.remove_task_subscribers(task_id).await.unwrap();

        assert_eq!(handler.get_subscriber_count(task_id).await.unwrap(), 0);
    }
}
