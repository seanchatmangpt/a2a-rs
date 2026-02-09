//! SSE Stream Manager with resumable streaming support
//!
//! Implements MCP Streamable HTTP spec for Server-Sent Events with:
//! - Sequential event IDs for all events
//! - Last-Event-ID header support for resume cursor
//! - Configurable redelivery window policy
//! - Async streaming via tokio-stream

use crate::error::{Error, Result};
use chrono::{DateTime, Duration, Utc};
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

/// SSE event with unique ID for resumability
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SseEvent {
    /// Unique event ID for resumability
    pub id: String,
    /// Event type (e.g., "message", "status", "artifact")
    pub event: String,
    /// Event data as JSON
    pub data: serde_json::Value,
    /// Timestamp when event was created
    pub timestamp: DateTime<Utc>,
}

impl SseEvent {
    /// Create a new SSE event
    pub fn new(id: String, event: String, data: serde_json::Value) -> Self {
        Self {
            id,
            event,
            data,
            timestamp: Utc::now(),
        }
    }

    /// Format event for SSE protocol
    pub fn to_sse_string(&self) -> String {
        format!(
            "id: {}\nevent: {}\ndata: {}\n\n",
            self.id,
            self.event,
            serde_json::to_string(&self.data).unwrap_or_default()
        )
    }
}

/// Configuration for SSE stream manager
#[derive(Debug, Clone)]
pub struct SseManagerConfig {
    /// Maximum number of events to retain in redelivery window
    pub max_events: usize,
    /// Time-to-live for events in the redelivery window
    pub event_ttl: Duration,
    /// Broadcast channel capacity
    pub channel_capacity: usize,
}

impl Default for SseManagerConfig {
    fn default() -> Self {
        Self {
            max_events: 1000,
            event_ttl: Duration::hours(1),
            channel_capacity: 100,
        }
    }
}

/// Redelivery window for storing recent events
#[derive(Debug)]
struct RedeliveryWindow {
    /// Queue of events in chronological order
    events: VecDeque<SseEvent>,
    /// Maximum events to retain
    max_events: usize,
    /// TTL for events
    event_ttl: Duration,
}

impl RedeliveryWindow {
    fn new(max_events: usize, event_ttl: Duration) -> Self {
        Self {
            events: VecDeque::with_capacity(max_events),
            max_events,
            event_ttl,
        }
    }

    /// Add an event to the window
    fn push(&mut self, event: SseEvent) {
        // Remove expired events
        self.cleanup_expired();

        // Add new event
        self.events.push_back(event);

        // Enforce max events limit
        while self.events.len() > self.max_events {
            self.events.pop_front();
        }
    }

    /// Get events after a specific event ID
    fn get_after(&mut self, last_event_id: &str) -> Vec<SseEvent> {
        self.cleanup_expired();

        // Find the position of the last event ID
        let start_pos = self
            .events
            .iter()
            .position(|e| e.id == last_event_id)
            .map(|pos| pos + 1)
            .unwrap_or(0);

        // Return all events after that position
        self.events.iter().skip(start_pos).cloned().collect()
    }

    /// Remove expired events based on TTL
    fn cleanup_expired(&mut self) {
        let now = Utc::now();
        let cutoff = now - self.event_ttl;

        while let Some(event) = self.events.front() {
            if event.timestamp < cutoff {
                self.events.pop_front();
            } else {
                break;
            }
        }
    }

    /// Get all events in the window
    fn get_all(&mut self) -> Vec<SseEvent> {
        self.cleanup_expired();
        self.events.iter().cloned().collect()
    }
}

/// SSE stream manager with resumable streaming support
#[derive(Debug)]
pub struct SseManager {
    /// Configuration
    config: SseManagerConfig,
    /// Per-stream redelivery windows
    windows: Arc<RwLock<HashMap<String, RedeliveryWindow>>>,
    /// Per-stream broadcast channels
    senders: Arc<RwLock<HashMap<String, broadcast::Sender<SseEvent>>>>,
    /// Next event ID counter per stream
    event_counters: Arc<RwLock<HashMap<String, u64>>>,
}

impl SseManager {
    /// Create a new SSE stream manager
    pub fn new(config: SseManagerConfig) -> Self {
        Self {
            config,
            windows: Arc::new(RwLock::new(HashMap::new())),
            senders: Arc::new(RwLock::new(HashMap::new())),
            event_counters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new SSE stream manager with default configuration
    pub fn default() -> Self {
        Self::new(SseManagerConfig::default())
    }

    /// Initialize a new stream for a given stream ID
    pub fn init_stream(&self, stream_id: &str) -> Result<()> {
        let mut windows = self.windows.write().unwrap();
        let mut senders = self.senders.write().unwrap();
        let mut counters = self.event_counters.write().unwrap();

        // Create redelivery window
        windows.insert(
            stream_id.to_string(),
            RedeliveryWindow::new(self.config.max_events, self.config.event_ttl),
        );

        // Create broadcast channel
        let (sender, _) = broadcast::channel(self.config.channel_capacity);
        senders.insert(stream_id.to_string(), sender);

        // Initialize event counter
        counters.insert(stream_id.to_string(), 0);

        Ok(())
    }

    /// Publish an event to a stream
    pub fn publish(
        &self,
        stream_id: &str,
        event_type: &str,
        data: serde_json::Value,
    ) -> Result<String> {
        // Get next event ID
        let event_id = {
            let mut counters = self.event_counters.write().unwrap();
            let counter = counters
                .get_mut(stream_id)
                .ok_or_else(|| Error::Server(format!("Stream not found: {}", stream_id)))?;
            *counter += 1;
            format!("{}-{}", stream_id, counter)
        };

        // Create event
        let event = SseEvent::new(event_id.clone(), event_type.to_string(), data);

        // Store in redelivery window
        {
            let mut windows = self.windows.write().unwrap();
            if let Some(window) = windows.get_mut(stream_id) {
                window.push(event.clone());
            }
        }

        // Broadcast to subscribers
        {
            let senders = self.senders.read().unwrap();
            if let Some(sender) = senders.get(stream_id) {
                // Ignore error if no receivers (just means no active subscriptions)
                let _ = sender.send(event);
            }
        }

        Ok(event_id)
    }

    /// Subscribe to a stream with optional resume from last event ID
    pub fn subscribe(
        &self,
        stream_id: &str,
        last_event_id: Option<&str>,
    ) -> Result<Pin<Box<dyn Stream<Item = SseEvent> + Send>>> {
        // Get the sender for this stream
        let receiver = {
            let senders = self.senders.read().unwrap();
            let sender = senders
                .get(stream_id)
                .ok_or_else(|| Error::Server(format!("Stream not found: {}", stream_id)))?;
            sender.subscribe()
        };

        // Get missed events if resuming
        let missed_events = if let Some(last_id) = last_event_id {
            let mut windows = self.windows.write().unwrap();
            if let Some(window) = windows.get_mut(stream_id) {
                window.get_after(last_id)
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Create stream that first delivers missed events, then live events
        let missed_stream = tokio_stream::iter(missed_events);
        let live_stream = BroadcastStream::new(receiver).filter_map(|result| result.ok());

        let combined_stream = missed_stream.chain(live_stream);

        Ok(Box::pin(combined_stream))
    }

    /// Get all events for a stream (for debugging/inspection)
    pub fn get_events(&self, stream_id: &str) -> Result<Vec<SseEvent>> {
        let mut windows = self.windows.write().unwrap();
        let window = windows
            .get_mut(stream_id)
            .ok_or_else(|| Error::Server(format!("Stream not found: {}", stream_id)))?;

        Ok(window.get_all())
    }

    /// Close a stream and clean up resources
    pub fn close_stream(&self, stream_id: &str) -> Result<()> {
        let mut windows = self.windows.write().unwrap();
        let mut senders = self.senders.write().unwrap();
        let mut counters = self.event_counters.write().unwrap();

        windows.remove(stream_id);
        senders.remove(stream_id);
        counters.remove(stream_id);

        Ok(())
    }

    /// Get active stream count
    pub fn active_stream_count(&self) -> usize {
        let windows = self.windows.read().unwrap();
        windows.len()
    }

    /// Cleanup expired events across all streams
    pub fn cleanup_all(&self) {
        let mut windows = self.windows.write().unwrap();
        for window in windows.values_mut() {
            window.cleanup_expired();
        }
    }
}

/// Axum SSE stream adapter
pub struct AxumSseStream {
    inner: Pin<Box<dyn Stream<Item = SseEvent> + Send>>,
}

impl AxumSseStream {
    pub fn new(stream: Pin<Box<dyn Stream<Item = SseEvent> + Send>>) -> Self {
        Self { inner: stream }
    }
}

impl Stream for AxumSseStream {
    type Item = Result<axum::response::sse::Event>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(event)) => {
                let sse_event = axum::response::sse::Event::default()
                    .id(event.id)
                    .event(event.event)
                    .json_data(&event.data)
                    .map_err(|e| Error::Server(format!("Failed to serialize SSE event: {}", e)));

                Poll::Ready(Some(sse_event))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_event_formatting() {
        let event = SseEvent::new(
            "test-1".to_string(),
            "message".to_string(),
            serde_json::json!({"text": "hello"}),
        );

        let formatted = event.to_sse_string();
        assert!(formatted.contains("id: test-1"));
        assert!(formatted.contains("event: message"));
        assert!(formatted.contains("data: "));
    }

    #[test]
    fn test_redelivery_window() {
        let mut window = RedeliveryWindow::new(3, Duration::hours(1));

        // Add events
        for i in 0..5 {
            let event = SseEvent::new(
                format!("event-{}", i),
                "test".to_string(),
                serde_json::json!({"index": i}),
            );
            window.push(event);
        }

        // Should only keep last 3 events due to max_events limit
        assert_eq!(window.events.len(), 3);
        assert_eq!(window.events[0].id, "event-2");
        assert_eq!(window.events[2].id, "event-4");
    }

    #[test]
    fn test_redelivery_window_get_after() {
        let mut window = RedeliveryWindow::new(10, Duration::hours(1));

        // Add events
        for i in 0..5 {
            let event = SseEvent::new(
                format!("event-{}", i),
                "test".to_string(),
                serde_json::json!({"index": i}),
            );
            window.push(event);
        }

        // Get events after event-2
        let after = window.get_after("event-2");
        assert_eq!(after.len(), 2);
        assert_eq!(after[0].id, "event-3");
        assert_eq!(after[1].id, "event-4");
    }

    #[tokio::test]
    async fn test_sse_manager_publish_subscribe() {
        let manager = SseManager::default();
        let stream_id = "test-stream";

        // Initialize stream
        manager.init_stream(stream_id).unwrap();

        // Subscribe to stream
        let mut stream = manager.subscribe(stream_id, None).unwrap();

        // Publish event
        manager
            .publish(stream_id, "test", serde_json::json!({"message": "hello"}))
            .unwrap();

        // Receive event
        let event = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            StreamExt::next(&mut stream),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(event.event, "test");
        assert_eq!(event.data["message"], "hello");
    }

    #[tokio::test]
    async fn test_sse_manager_resume() {
        let manager = SseManager::default();
        let stream_id = "test-stream";

        // Initialize stream
        manager.init_stream(stream_id).unwrap();

        // Publish some events
        let id1 = manager
            .publish(stream_id, "test", serde_json::json!({"index": 1}))
            .unwrap();
        let _id2 = manager
            .publish(stream_id, "test", serde_json::json!({"index": 2}))
            .unwrap();
        let _id3 = manager
            .publish(stream_id, "test", serde_json::json!({"index": 3}))
            .unwrap();

        // Subscribe with resume from first event
        let mut stream = manager.subscribe(stream_id, Some(&id1)).unwrap();

        // Should receive events 2 and 3
        let event1 = StreamExt::next(&mut stream).await.unwrap();
        assert_eq!(event1.data["index"], 2);

        let event2 = StreamExt::next(&mut stream).await.unwrap();
        assert_eq!(event2.data["index"], 3);
    }
}
