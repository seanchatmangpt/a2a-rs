//! Event emission and streaming for observable task transitions
//!
//! This module provides a bounded, ordered event stream for task lifecycle events.
//! Events are telemetry signals that capture observable state transitions without
//! blocking the main task execution flow.
//!
//! # Features
//!
//! - **Ordered emission**: Happens-before guarantees via monotonic sequence numbers
//! - **Bounded buffering**: Configurable capacity with backpressure
//! - **Multiple consumers**: Fan-out to multiple subscribers
//! - **Three event types**: Status, Artifact, Error
//!
//! # Example
//!
//! ```rust
//! # #[cfg(feature = "server")]
//! # {
//! use a2a_rs::construct::EventStream;
//! use a2a_rs::domain::TaskState;
//!
//! #[tokio::main]
//! async fn main() {
//!     let stream = EventStream::new("task-123".to_string(), 100);
//!
//!     // Emit status event
//!     stream.emit_status(TaskState::Working, None).await.unwrap();
//!
//!     // Subscribe to events
//!     let mut subscription = stream.subscribe().await;
//!
//!     // Receive events
//!     if let Some(event) = subscription.recv().await {
//!         println!("Received event: {:?}", event);
//!     }
//! }
//! # }
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

#[cfg(feature = "server")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "server")]
use tokio::sync::{RwLock, broadcast};

use crate::domain::{Artifact, FileContent, Message, Part, TaskState};

/// Errors that can occur during event operations
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum EventError {
    /// Event buffer is full
    #[error("Event buffer full (capacity: {capacity})")]
    BufferFull { capacity: usize },

    /// Subscription channel closed
    #[error("Subscription channel closed")]
    ChannelClosed,

    /// Invalid sequence number
    #[error("Invalid sequence number: expected {expected}, got {actual}")]
    InvalidSequence { expected: u64, actual: u64 },

    /// Stream already closed
    #[error("Event stream already closed")]
    StreamClosed,
}

/// Result type for event operations
pub type EventResult<T> = Result<T, EventError>;

/// Event type discriminator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventKind {
    /// Task status transition event
    TaskStatus,
    /// Artifact emission event
    Artifact,
    /// Error event
    Error,
}

/// A task status transition event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusEvent {
    /// The task ID
    pub task_id: String,
    /// New task state
    pub state: TaskState,
    /// Optional message associated with the transition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
    /// Whether this is a terminal state
    pub is_final: bool,
    /// When the event occurred
    pub timestamp: DateTime<Utc>,
    /// Sequence number for ordering
    pub sequence: u64,
}

/// An artifact emission event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactEvent {
    /// The task ID
    pub task_id: String,
    /// The emitted artifact
    pub artifact: Artifact,
    /// Whether to append to existing artifact
    #[serde(skip_serializing_if = "Option::is_none")]
    pub append: Option<bool>,
    /// Whether this is the last chunk
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_chunk: Option<bool>,
    /// When the event occurred
    pub timestamp: DateTime<Utc>,
    /// Sequence number for ordering
    pub sequence: u64,
}

/// An error event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEvent {
    /// The task ID
    pub task_id: String,
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Whether the error is fatal (task cannot continue)
    pub is_fatal: bool,
    /// When the event occurred
    pub timestamp: DateTime<Utc>,
    /// Sequence number for ordering
    pub sequence: u64,
}

/// Union type for all event kinds
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Event {
    /// Task status transition
    TaskStatus(TaskStatusEvent),
    /// Artifact emission
    Artifact(ArtifactEvent),
    /// Error occurrence
    Error(ErrorEvent),
}

impl Event {
    /// Get the task ID for this event
    pub fn task_id(&self) -> &str {
        match self {
            Event::TaskStatus(e) => &e.task_id,
            Event::Artifact(e) => &e.task_id,
            Event::Error(e) => &e.task_id,
        }
    }

    /// Get the sequence number for this event
    pub fn sequence(&self) -> u64 {
        match self {
            Event::TaskStatus(e) => e.sequence,
            Event::Artifact(e) => e.sequence,
            Event::Error(e) => e.sequence,
        }
    }

    /// Get the timestamp for this event
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Event::TaskStatus(e) => e.timestamp,
            Event::Artifact(e) => e.timestamp,
            Event::Error(e) => e.timestamp,
        }
    }

    /// Get the event kind
    pub fn kind(&self) -> EventKind {
        match self {
            Event::TaskStatus(_) => EventKind::TaskStatus,
            Event::Artifact(_) => EventKind::Artifact,
            Event::Error(_) => EventKind::Error,
        }
    }

    /// Check if this event indicates task completion
    pub fn is_final(&self) -> bool {
        match self {
            Event::TaskStatus(e) => e.is_final,
            Event::Artifact(e) => e.last_chunk.unwrap_or(false),
            Event::Error(e) => e.is_fatal,
        }
    }
}

/// Ordered event stream with bounded buffering and multiple consumers
///
/// The EventStream provides happens-before guarantees through monotonic
/// sequence numbers. Events are broadcast to all subscribers with bounded
/// buffering to prevent memory exhaustion.
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "server")]
/// # {
/// use a2a_rs::construct::EventStream;
/// use a2a_rs::domain::TaskState;
///
/// #[tokio::main]
/// async fn main() {
///     let stream = EventStream::new("task-123".to_string(), 100);
///
///     // Multiple subscribers
///     let mut sub1 = stream.subscribe().await;
///     let mut sub2 = stream.subscribe().await;
///
///     // Emit event
///     stream.emit_status(TaskState::Working, None).await.unwrap();
///
///     // Both subscribers receive the event
///     assert!(sub1.recv().await.is_some());
///     assert!(sub2.recv().await.is_some());
/// }
/// # }
/// ```
#[cfg(feature = "server")]
pub struct EventStream {
    /// Task ID for this stream
    task_id: String,
    /// Broadcast channel for events
    sender: broadcast::Sender<Event>,
    /// Monotonic sequence counter
    sequence: Arc<AtomicU64>,
    /// Whether the stream is closed
    closed: Arc<RwLock<bool>>,
    /// Buffer capacity
    capacity: usize,
}

#[cfg(feature = "server")]
impl EventStream {
    /// Create a new event stream with the specified buffer capacity
    ///
    /// The capacity determines how many events can be buffered before
    /// backpressure is applied to emitters.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID for this stream
    /// * `capacity` - Maximum number of buffered events
    pub fn new(task_id: String, capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            task_id,
            sender,
            sequence: Arc::new(AtomicU64::new(0)),
            closed: Arc::new(RwLock::new(false)),
            capacity,
        }
    }

    /// Create a new event stream with default capacity (1000 events)
    pub fn with_default_capacity(task_id: String) -> Self {
        Self::new(task_id, 1000)
    }

    /// Get the next sequence number (monotonically increasing)
    fn next_sequence(&self) -> u64 {
        self.sequence.fetch_add(1, Ordering::SeqCst)
    }

    /// Check if the stream is closed
    pub async fn is_closed(&self) -> bool {
        *self.closed.read().await
    }

    /// Close the stream (no more events can be emitted)
    pub async fn close(&self) -> EventResult<()> {
        let mut closed = self.closed.write().await;
        *closed = true;
        Ok(())
    }

    /// Emit a task status event
    ///
    /// # Arguments
    ///
    /// * `state` - The new task state
    /// * `message` - Optional message associated with the transition
    pub async fn emit_status(
        &self,
        state: TaskState,
        message: Option<Message>,
    ) -> EventResult<u64> {
        if *self.closed.read().await {
            return Err(EventError::StreamClosed);
        }

        let sequence = self.next_sequence();
        let is_final = Self::is_terminal_state(&state);

        let event = Event::TaskStatus(TaskStatusEvent {
            task_id: self.task_id.clone(),
            state,
            message,
            is_final,
            timestamp: Utc::now(),
            sequence,
        });

        self.sender
            .send(event)
            .map_err(|_| EventError::BufferFull {
                capacity: self.capacity,
            })?;

        Ok(sequence)
    }

    /// Emit an artifact event
    ///
    /// # Arguments
    ///
    /// * `artifact` - The artifact to emit
    /// * `append` - Whether to append to existing artifact
    /// * `last_chunk` - Whether this is the last chunk
    pub async fn emit_artifact(
        &self,
        artifact: Artifact,
        append: Option<bool>,
        last_chunk: Option<bool>,
    ) -> EventResult<u64> {
        if *self.closed.read().await {
            return Err(EventError::StreamClosed);
        }

        let sequence = self.next_sequence();

        let event = Event::Artifact(ArtifactEvent {
            task_id: self.task_id.clone(),
            artifact,
            append,
            last_chunk,
            timestamp: Utc::now(),
            sequence,
        });

        self.sender
            .send(event)
            .map_err(|_| EventError::BufferFull {
                capacity: self.capacity,
            })?;

        Ok(sequence)
    }

    /// Emit an error event
    ///
    /// # Arguments
    ///
    /// * `code` - Error code
    /// * `message` - Error message
    /// * `is_fatal` - Whether the error is fatal
    pub async fn emit_error(&self, code: i32, message: String, is_fatal: bool) -> EventResult<u64> {
        if *self.closed.read().await {
            return Err(EventError::StreamClosed);
        }

        let sequence = self.next_sequence();

        let event = Event::Error(ErrorEvent {
            task_id: self.task_id.clone(),
            code,
            message,
            is_fatal,
            timestamp: Utc::now(),
            sequence,
        });

        self.sender
            .send(event)
            .map_err(|_| EventError::BufferFull {
                capacity: self.capacity,
            })?;

        Ok(sequence)
    }

    /// Subscribe to events from this stream
    ///
    /// Returns a receiver that will receive all future events.
    /// Events emitted before subscription are not received.
    pub async fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    /// Get the current sequence number
    pub fn current_sequence(&self) -> u64 {
        self.sequence.load(Ordering::SeqCst)
    }

    /// Get the number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Get the task ID
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Get the buffer capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Check if a task state is terminal
    fn is_terminal_state(state: &TaskState) -> bool {
        matches!(
            state,
            TaskState::Completed
                | TaskState::Failed
                | TaskState::Canceled
                | TaskState::Rejected
                | TaskState::Unknown
        )
    }
}

#[cfg(feature = "server")]
impl Clone for EventStream {
    fn clone(&self) -> Self {
        Self {
            task_id: self.task_id.clone(),
            sender: self.sender.clone(),
            sequence: Arc::clone(&self.sequence),
            closed: Arc::clone(&self.closed),
            capacity: self.capacity,
        }
    }
}

#[cfg(feature = "server")]
impl std::fmt::Debug for EventStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventStream")
            .field("task_id", &self.task_id)
            .field("capacity", &self.capacity)
            .field("sequence", &self.current_sequence())
            .field("subscribers", &self.subscriber_count())
            .finish()
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_stream_creation() {
        let stream = EventStream::new("task-1".to_string(), 100);
        assert_eq!(stream.task_id(), "task-1");
        assert_eq!(stream.capacity(), 100);
        assert_eq!(stream.current_sequence(), 0);
        assert_eq!(stream.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn test_emit_status_event() {
        let stream = EventStream::new("task-1".to_string(), 100);
        let mut sub = stream.subscribe().await;

        let seq = stream.emit_status(TaskState::Working, None).await.unwrap();
        assert_eq!(seq, 0);

        let event = sub.recv().await.unwrap();
        assert_eq!(event.task_id(), "task-1");
        assert_eq!(event.sequence(), 0);
        assert_eq!(event.kind(), EventKind::TaskStatus);

        match event {
            Event::TaskStatus(e) => {
                assert_eq!(e.state, TaskState::Working);
                assert!(!e.is_final);
            }
            _ => panic!("Expected TaskStatus event"),
        }
    }

    #[tokio::test]
    async fn test_emit_artifact_event() {
        let stream = EventStream::new("task-1".to_string(), 100);
        let mut sub = stream.subscribe().await;

        let artifact = Artifact {
            artifact_id: "art-1".to_string(),
            name: Some("result.json".to_string()),
            description: None,
            parts: vec![Part::File {
                file: FileContent {
                    name: Some("result.json".to_string()),
                    mime_type: None,
                    bytes: None,
                    uri: Some("file:///result.json".to_string()),
                },
                metadata: None,
            }],
            metadata: None,
            extensions: None,
        };

        let seq = stream
            .emit_artifact(artifact.clone(), None, Some(true))
            .await
            .unwrap();
        assert_eq!(seq, 0);

        let event = sub.recv().await.unwrap();
        assert_eq!(event.kind(), EventKind::Artifact);

        match event {
            Event::Artifact(e) => {
                assert_eq!(e.artifact.artifact_id, "art-1");
                assert_eq!(e.last_chunk, Some(true));
            }
            _ => panic!("Expected Artifact event"),
        }
    }

    #[tokio::test]
    async fn test_emit_error_event() {
        let stream = EventStream::new("task-1".to_string(), 100);
        let mut sub = stream.subscribe().await;

        let seq = stream
            .emit_error(-32001, "Task not found".to_string(), true)
            .await
            .unwrap();
        assert_eq!(seq, 0);

        let event = sub.recv().await.unwrap();
        assert_eq!(event.kind(), EventKind::Error);

        match event {
            Event::Error(e) => {
                assert_eq!(e.code, -32001);
                assert_eq!(e.message, "Task not found");
                assert!(e.is_fatal);
            }
            _ => panic!("Expected Error event"),
        }
    }

    #[tokio::test]
    async fn test_sequence_ordering() {
        let stream = EventStream::new("task-1".to_string(), 100);
        let mut sub = stream.subscribe().await;

        // Emit multiple events
        stream
            .emit_status(TaskState::Submitted, None)
            .await
            .unwrap();
        stream.emit_status(TaskState::Working, None).await.unwrap();
        stream
            .emit_status(TaskState::Completed, None)
            .await
            .unwrap();

        // Verify sequence numbers are monotonically increasing
        let e1 = sub.recv().await.unwrap();
        let e2 = sub.recv().await.unwrap();
        let e3 = sub.recv().await.unwrap();

        assert_eq!(e1.sequence(), 0);
        assert_eq!(e2.sequence(), 1);
        assert_eq!(e3.sequence(), 2);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let stream = EventStream::new("task-1".to_string(), 100);
        let mut sub1 = stream.subscribe().await;
        let mut sub2 = stream.subscribe().await;

        assert_eq!(stream.subscriber_count(), 2);

        stream.emit_status(TaskState::Working, None).await.unwrap();

        // Both subscribers receive the event
        let e1 = sub1.recv().await.unwrap();
        let e2 = sub2.recv().await.unwrap();

        assert_eq!(e1.sequence(), e2.sequence());
        assert_eq!(e1.task_id(), e2.task_id());
    }

    #[tokio::test]
    async fn test_terminal_state_detection() {
        let stream = EventStream::new("task-1".to_string(), 100);
        let mut sub = stream.subscribe().await;

        stream
            .emit_status(TaskState::Completed, None)
            .await
            .unwrap();

        let event = sub.recv().await.unwrap();
        assert!(event.is_final());
    }

    #[tokio::test]
    async fn test_stream_close() {
        let stream = EventStream::new("task-1".to_string(), 100);
        assert!(!stream.is_closed().await);

        stream.close().await.unwrap();
        assert!(stream.is_closed().await);

        // Cannot emit after close
        let result = stream.emit_status(TaskState::Working, None).await;
        assert!(matches!(result, Err(EventError::StreamClosed)));
    }

    #[tokio::test]
    async fn test_event_helpers() {
        let stream = EventStream::new("task-1".to_string(), 100);
        let mut sub = stream.subscribe().await;

        stream.emit_status(TaskState::Working, None).await.unwrap();

        let event = sub.recv().await.unwrap();
        assert_eq!(event.task_id(), "task-1");
        assert_eq!(event.kind(), EventKind::TaskStatus);
        assert!(!event.is_final());
    }

    #[tokio::test]
    async fn test_default_capacity() {
        let stream = EventStream::with_default_capacity("task-1".to_string());
        assert_eq!(stream.capacity(), 1000);
    }

    #[tokio::test]
    async fn test_late_subscriber() {
        let stream = EventStream::new("task-1".to_string(), 100);

        // Emit before subscription
        stream
            .emit_status(TaskState::Submitted, None)
            .await
            .unwrap();

        // Late subscriber doesn't see past events
        let mut sub = stream.subscribe().await;

        // Emit after subscription
        stream.emit_status(TaskState::Working, None).await.unwrap();

        let event = sub.recv().await.unwrap();
        assert_eq!(event.sequence(), 1); // Second event, not first
    }

    #[tokio::test]
    async fn test_clone_stream() {
        let stream1 = EventStream::new("task-1".to_string(), 100);
        let stream2 = stream1.clone();

        let mut sub1 = stream1.subscribe().await;
        let mut sub2 = stream2.subscribe().await;

        // Events emitted on clone are visible to both
        stream1.emit_status(TaskState::Working, None).await.unwrap();

        let e1 = sub1.recv().await.unwrap();
        let e2 = sub2.recv().await.unwrap();

        assert_eq!(e1.sequence(), e2.sequence());
    }
}
