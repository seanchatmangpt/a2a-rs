# SSE Resumable Streaming Implementation

## Overview
Implemented SSE (Server-Sent Events) resumable streaming for a2a-mcp following MCP Streamable HTTP spec.

## Key Components

### 1. SseManager (`a2a-mcp/src/adapter/sse_manager.rs`)
Core SSE stream manager with:
- **Event IDs**: Sequential event IDs per stream (format: `{stream_id}-{counter}`)
- **Redelivery Window**: Configurable retention of recent events
- **Resume Support**: `Last-Event-ID` header handling for reconnection
- **Async Streaming**: Built on tokio-stream with broadcast channels

### 2. Configuration (`SseManagerConfig`)
```rust
pub struct SseManagerConfig {
    pub max_events: usize,        // Max events in redelivery window (default: 1000)
    pub event_ttl: Duration,       // Event time-to-live (default: 1 hour)
    pub channel_capacity: usize,   // Broadcast channel size (default: 100)
}
```

### 3. Core Operations
- `init_stream(stream_id)` - Initialize new SSE stream
- `publish(stream_id, event_type, data)` - Publish event, returns event ID
- `subscribe(stream_id, last_event_id)` - Subscribe with optional resume
- `close_stream(stream_id)` - Clean up stream resources
- `cleanup_all()` - Remove expired events across all streams

### 4. MCP Spec Compliance
✓ Event IDs assigned to all SSE events
✓ Last-Event-ID header support for resume cursor
✓ Redelivery window with configurable retention policy
✓ Async streaming via tokio-stream
✓ Automatic cleanup of expired events

## Integration in RmcpA2aServer

Updated `a2a-mcp/src/server.rs`:
- Added `SseManager` to `AppState` and `RmcpA2aServer`
- Implemented `handle_task_send_subscribe` with SSE streaming
- Created `process_task_with_streaming` for event publication during task execution
- Publishes events: `task.created`, `task.status`, `tool.response`, `task.completed`, `task.error`

## Key Patterns Discovered

### Pattern 1: Redelivery Window Design
Use `VecDeque` for efficient FIFO queue with:
- Automatic TTL-based expiration
- Position-based resume (find last event ID, return all after)
- Size-limited retention (configurable max events)

### Pattern 2: Broadcast + Missed Events
Combined stream pattern:
1. Check redelivery window for missed events since Last-Event-ID
2. Create stream from missed events (tokio_stream::iter)
3. Chain with live broadcast stream (BroadcastStream)
4. Result: seamless resume experience

### Pattern 3: Axum SSE Integration
Custom `AxumSseStream` adapter:
- Implements `Stream<Item = Result<axum::response::sse::Event>>`
- Converts `SseEvent` to `axum::response::sse::Event`
- Handles JSON serialization errors

### Pattern 4: Per-Stream State Management
Use HashMap of stream IDs to:
- Redelivery windows (Arc<RwLock<HashMap>>)
- Broadcast senders (Arc<RwLock<HashMap>>)
- Event counters (Arc<RwLock<HashMap>>)

## Dependencies Added
```toml
tokio-stream = { version = "0.1", features = ["sync"] }  # Required for BroadcastStream
chrono = { version = "0.4", features = ["serde"] }       # For event timestamps and TTL
axum = { version = "0.7", features = ["macros"] }        # For SSE response type
```

## Testing
Created comprehensive tests in `sse_manager.rs`:
- Event formatting and ID assignment
- Redelivery window behavior (max events, TTL)
- Resume functionality (`get_after` logic)
- Publish/subscribe integration
- Concurrent subscribers

Example usage in `examples/sse_streaming.rs`.

## Edge Cases Handled
1. **Stream not found**: Return Error::Server
2. **No active receivers**: Broadcast send failure is ignored (expected)
3. **Invalid Last-Event-ID**: Start from beginning (position not found = 0)
4. **Expired events**: Automatic cleanup on read operations
5. **Channel overflow**: Broadcast channel drops oldest if full (configurable capacity)

## Future Enhancements
- Persistent storage for redelivery window (currently in-memory)
- Stream lifecycle hooks (on_stream_created, on_stream_closed)
- Metrics/observability (event count, subscriber count, dropped messages)
- Backpressure handling for slow consumers
- Compression for large event payloads
