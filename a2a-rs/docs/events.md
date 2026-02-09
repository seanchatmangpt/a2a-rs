# Event Emission and Streaming

## Overview

The `construct::events` module provides ordered, bounded event streaming for observable task transitions. Events are telemetry signals that capture state changes without blocking task execution.

## Features

- **Ordered emission**: Monotonic sequence numbers ensure happens-before guarantees
- **Bounded buffering**: Configurable capacity with backpressure (default: 1000 events)
- **Multiple consumers**: Fan-out to multiple subscribers via broadcast channels
- **Three event types**: TaskStatus, Artifact, Error

## Event Types

### TaskStatusEvent

Emitted when a task transitions to a new state.

```rust
pub struct TaskStatusEvent {
    pub task_id: String,
    pub state: TaskState,
    pub message: Option<Message>,
    pub is_final: bool,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
}
```

### ArtifactEvent

Emitted when a task produces an artifact.

```rust
pub struct ArtifactEvent {
    pub task_id: String,
    pub artifact: Artifact,
    pub append: Option<bool>,
    pub last_chunk: Option<bool>,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
}
```

### ErrorEvent

Emitted when an error occurs during task execution.

```rust
pub struct ErrorEvent {
    pub task_id: String,
    pub code: i32,
    pub message: String,
    pub is_fatal: bool,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
}
```

## Usage Examples

### Basic Event Emission

```rust
use a2a_rs::construct::EventStream;
use a2a_rs::domain::TaskState;

#[tokio::main]
async fn main() {
    // Create an event stream with 100-event buffer
    let stream = EventStream::new("task-123".to_string(), 100);

    // Emit a status event
    let seq = stream.emit_status(TaskState::Working, None)
        .await
        .unwrap();

    println!("Emitted event with sequence: {}", seq);
}
```

### Multiple Subscribers

```rust
use a2a_rs::construct::EventStream;
use a2a_rs::domain::TaskState;

#[tokio::main]
async fn main() {
    let stream = EventStream::new("task-123".to_string(), 100);

    // Create multiple subscribers
    let mut sub1 = stream.subscribe().await;
    let mut sub2 = stream.subscribe().await;

    // Emit event
    stream.emit_status(TaskState::Working, None).await.unwrap();

    // Both subscribers receive the same event
    let event1 = sub1.recv().await.unwrap();
    let event2 = sub2.recv().await.unwrap();

    assert_eq!(event1.sequence(), event2.sequence());
}
```

### Artifact Streaming

```rust
use a2a_rs::construct::EventStream;
use a2a_rs::domain::Artifact;

#[tokio::main]
async fn main() {
    let stream = EventStream::new("task-123".to_string(), 100);
    let mut sub = stream.subscribe().await;

    // Emit artifact chunks
    for i in 0..3 {
        let artifact = Artifact {
            artifact_id: format!("chunk-{}", i),
            name: Some(format!("output-{}.txt", i)),
            content: Some(format!("Chunk {} data", i)),
            // ... other fields
            kind: "artifact".to_string(),
            description: None,
            mime_type: None,
            uri: None,
            metadata: None,
        };

        let is_last = i == 2;
        stream.emit_artifact(
            artifact,
            Some(true),  // append
            Some(is_last)
        ).await.unwrap();
    }

    // Receive chunks in order
    while let Ok(event) = sub.recv().await {
        if let a2a_rs::construct::Event::Artifact(evt) = event {
            println!("Received: {:?}", evt.artifact.name);
            if evt.last_chunk == Some(true) {
                break;
            }
        }
    }
}
```

### Error Handling

```rust
use a2a_rs::construct::EventStream;

#[tokio::main]
async fn main() {
    let stream = EventStream::new("task-123".to_string(), 100);
    let mut sub = stream.subscribe().await;

    // Emit non-fatal error
    stream.emit_error(
        -32001,
        "Resource temporarily unavailable".to_string(),
        false  // non-fatal
    ).await.unwrap();

    // Continue processing...

    // Emit fatal error
    stream.emit_error(
        -32603,
        "Internal error".to_string(),
        true  // fatal
    ).await.unwrap();

    // Process events
    while let Ok(event) = sub.recv().await {
        if event.is_final() {
            println!("Fatal event received, stopping");
            break;
        }
    }
}
```

### Stream Lifecycle Management

```rust
use a2a_rs::construct::EventStream;
use a2a_rs::domain::TaskState;

#[tokio::main]
async fn main() {
    let stream = EventStream::new("task-123".to_string(), 100);

    // Check stream state
    assert!(!stream.is_closed().await);
    assert_eq!(stream.subscriber_count(), 0);

    let mut sub = stream.subscribe().await;
    assert_eq!(stream.subscriber_count(), 1);

    // Emit events
    stream.emit_status(TaskState::Working, None).await.unwrap();

    // Close stream
    stream.close().await.unwrap();
    assert!(stream.is_closed().await);

    // Cannot emit after close
    let result = stream.emit_status(TaskState::Completed, None).await;
    assert!(result.is_err());
}
```

## Ordering Guarantees

Events emitted by `EventStream` maintain strict happens-before ordering through monotonic sequence numbers:

1. Each event receives a unique, monotonically increasing sequence number
2. Sequence numbers are atomic and thread-safe
3. All subscribers receive events in the same order
4. Late subscribers only see events emitted after subscription

## Backpressure

When the buffer is full:

- `emit_*` methods return `EventError::BufferFull`
- The caller can retry after a delay or drop the event
- Subscribers must keep up to avoid missing events

## Performance Considerations

- Default capacity (1000) is suitable for most use cases
- Increase capacity for high-throughput scenarios
- Each subscriber adds minimal overhead (broadcast channel)
- Events are cloned for each subscriber

## Integration with Task State Machine

The `EventStream` pairs naturally with `TaskStateMachine` for comprehensive observability:

```rust
use a2a_rs::construct::{EventStream, TaskStateMachine};
use a2a_rs::domain::TaskState;

#[tokio::main]
async fn main() {
    let mut fsm = TaskStateMachine::new("task-123".to_string());
    let stream = EventStream::new("task-123".to_string(), 100);
    let mut sub = stream.subscribe().await;

    // Transition state and emit event
    let transition = fsm.start_working(None).unwrap();
    stream.emit_status(
        transition.to.clone(),
        transition.message.clone()
    ).await.unwrap();

    // Verify event
    let event = sub.recv().await.unwrap();
    assert_eq!(event.sequence(), 0);
}
```

## Thread Safety

- `EventStream` is `Clone + Send + Sync`
- All operations are async and non-blocking
- Sequence numbers use atomic operations
- Closed state uses `RwLock` for concurrent access

## Feature Flags

The `EventStream` type requires the `server` feature flag:

```toml
[dependencies]
a2a-rs = { version = "0.1", features = ["server"] }
```

Event types (`Event`, `TaskStatusEvent`, etc.) are available without feature flags for serialization purposes.
