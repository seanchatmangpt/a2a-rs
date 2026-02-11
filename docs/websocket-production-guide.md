# Production-Ready WebSocket Implementation Guide

## Overview

The enhanced WebSocket implementation provides production-grade reliability with:

1. **Automatic Reconnection** - Exponential backoff with jitter
2. **Session State Tracking** - Monitor connection lifecycle
3. **Request Queue** - Handle offline periods gracefully
4. **Heartbeat Mechanism** - Detect dead connections early
5. **Complete Error Recovery** - Bounce back from failures

## Architecture

### Core Components

#### SessionState
Tracks the connection lifecycle:
- `session_id`: Unique identifier for this session
- `connected_at`: When the connection was established
- `last_activity`: Last message send/receive timestamp
- `reconnect_count`: How many times we've reconnected

#### ConnectionStatus
Current state of the connection:
- `Disconnected`: Not connected
- `Connecting`: Attempting to connect
- `Connected`: Fully connected and operational
- `Reconnecting`: Attempting to reconnect
- `Closed`: Permanently closed

#### ReconnectConfig
Configure automatic reconnection:
- `enabled`: Enable/disable reconnection
- `max_attempts`: Maximum reconnection attempts
- `initial_backoff`: Starting backoff duration
- `max_backoff`: Maximum backoff cap
- `backoff_multiplier`: Exponential growth factor
- `jitter_factor`: Randomization to prevent thundering herd

#### HeartbeatConfig
Configure connection health monitoring:
- `enabled`: Enable/disable heartbeat
- `interval`: Ping interval
- `timeout`: No response = dead connection

#### QueueConfig
Configure offline request queue:
- `enabled`: Enable/disable queue
- `max_size`: Maximum queue size
- `max_age`: Request age limit
- `max_retries`: Retry attempts per request

## Usage

### Basic Usage

```rust
use a2a_rs::adapter::transport::websocket::client::WebSocketClient;
use std::time::Duration;

// Create client with default settings
let client = WebSocketClient::new("ws://localhost:8080/ws".to_string());

// Connect
let mut client_mut = client.clone();
client_mut.connect().await?;

// Use the client
let task = client_mut.get_task("task-id", None).await?;

// Clean shutdown
client_mut.close().await?;
```

### Custom Configuration

```rust
use a2a_rs::adapter::transport::websocket::client::{
    WebSocketClient, ReconnectConfig, HeartbeatConfig, QueueConfig
};
use std::time::Duration;

// Configure reconnection
let reconnect_config = ReconnectConfig::builder()
    .enabled(true)
    .max_attempts(10)
    .initial_backoff(Duration::from_millis(100))
    .max_backoff(Duration::from_secs(30))
    .backoff_multiplier(2.0)
    .jitter_factor(0.1)
    .build();

// Configure heartbeat
let heartbeat_config = HeartbeatConfig::builder()
    .enabled(true)
    .interval(Duration::from_secs(30))
    .timeout(Duration::from_secs(10))
    .build();

// Configure queue
let queue_config = QueueConfig::builder()
    .enabled(true)
    .max_size(1000)
    .max_age(Duration::from_secs(300))
    .max_retries(3)
    .build();

// Create client with custom configuration
let client = WebSocketClient::new("ws://localhost:8080/ws".to_string())
    .with_reconnect_config(reconnect_config)
    .with_heartbeat_config(heartbeat_config)
    .with_queue_config(queue_config)
    .with_timeout(60);

// Start background tasks (heartbeat, connection monitor)
let mut client_mut = client.clone();
client_mut.connect().await?;
client_mut.start_background_tasks().await?;

// Use the client...
```

### Monitoring Connection Health

```rust
// Check connection status
let status = client.status().await;
match status {
    ConnectionStatus::Connected => println!("Connected"),
    ConnectionStatus::Reconnecting => println!("Reconnecting..."),
    _ => println!("Disconnected"),
}

// Get session state
let session = client.session_state().await;
println!("Session ID: {}", session.session_id);
println!("Connected: {:?}", session.connected_at.elapsed());
println!("Reconnects: {}", session.reconnect_count);

// Check if session is expired
if session.is_expired(Duration::from_secs(300)) {
    println!("Session expired!");
}
```

### Streaming with Auto-Reconnection

```rust
use futures::StreamExt;

let mut stream = client.subscribe_to_task("task-id", None).await?;

while let Some(item) = stream.next().await {
    match item {
        Ok(StreamItem::Task(task)) => {
            println!("Got task: {:?}", task.id);
        }
        Ok(StreamItem::StatusUpdate(update)) => {
            println!("Status update: {:?}", update.status);
        }
        Ok(StreamItem::ArtifactUpdate(update)) => {
            println!("Artifact update: {:?}", update.artifact.name);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            // Client will auto-reconnect on next message
        }
    }
}
```

## Production Best Practices

### 1. Reconnection Strategy

**Goal**: Balance speed vs. server load

```rust
// Conservative (for high-load servers)
let config = ReconnectConfig::builder()
    .initial_backoff(Duration::from_millis(500))
    .max_backoff(Duration::from_secs(60))
    .backoff_multiplier(2.0)
    .jitter_factor(0.2)
    .build();

// Aggressive (for low-latency requirements)
let config = ReconnectConfig::builder()
    .initial_backoff(Duration::from_millis(100))
    .max_backoff(Duration::from_secs(10))
    .backoff_multiplier(1.5)
    .jitter_factor(0.1)
    .build();
```

**Key Points**:
- Always enable jitter to prevent thundering herd
- Set max_attempts based on your SLA requirements
- Use longer backoffs for services under heavy load

### 2. Heartbeat Tuning

**Goal**: Detect failures quickly without excessive overhead

```rust
// For interactive applications (low latency)
let config = HeartbeatConfig::builder()
    .interval(Duration::from_secs(15))
    .timeout(Duration::from_secs(5))
    .build();

// For background processing (high throughput)
let config = HeartbeatConfig::builder()
    .interval(Duration::from_secs(60))
    .timeout(Duration::from_secs(20))
    .build();
```

**Key Points**:
- Timeout should be 2-3x interval
- Shorter intervals = faster failure detection but more overhead
- Account for network latency in timeout calculation

### 3. Queue Management

**Goal**: Handle temporary outages without data loss

```rust
// For critical operations (no data loss)
let config = QueueConfig::builder()
    .enabled(true)
    .max_size(10000)
    .max_age(Duration::from_secs(3600))
    .max_retries(5)
    .build();

// For best-effort operations
let config = QueueConfig::builder()
    .enabled(true)
    .max_size(100)
    .max_age(Duration::from_secs(60))
    .max_retries(1)
    .build();
```

**Key Points**:
- Set max_size based on memory constraints
- Set max_age based on request freshness requirements
- Set max_retries based on operation idempotency
- Monitor queue size in production

### 4. Error Handling

**Goal**: Graceful degradation and recovery

```rust
use a2a_rs::adapter::error::WebSocketClientError;

match client.send_task_message("task-id", &message, None, None).await {
    Ok(task) => println!("Sent: {:?}", task),
    Err(A2AError::Internal(msg)) if msg.contains("Reconnection failed") => {
        // Reconnection exhausted, implement circuit breaker
        eprintln!("Circuit breaker opened");
    }
    Err(A2AError::Internal(msg)) if msg.contains("Queue full") => {
        // Backpressure needed
        eprintln!("Request queue full, implement throttling");
    }
    Err(e) => {
        eprintln!("Temporary error: {}", e);
        // Client will retry automatically
    }
}
```

### 5. Monitoring and Metrics

**Essential metrics to track**:
- Connection status distribution
- Reconnection rate (reconnections/minute)
- Reconnection success rate
- Queue depth over time
- Message latency (p50, p95, p99)
- Heartbeat failures

**Example monitoring**:

```rust
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;

        let status = client.status().await;
        let session = client.session_state().await;

        metrics::gauge("websocket.status", status as i64);
        metrics::gauge("websocket.reconnects", session.reconnect_count);
        metrics::gauge("websocket.idle_time", session.last_activity.elapsed().as_secs());

        if !status.is_connected() {
            metrics::increment("websocket.disconnections");
        }
    }
});
```

### 6. Graceful Shutdown

**Always close connections properly**:

```rust
// Shutdown signal
tokio::select! {
    _ = shutdown_signal => {
        println!("Shutting down...");

        // Stop accepting new requests
        // ...

        // Close WebSocket connection
        client.close().await?;

        println!("Shutdown complete");
    }
    _ = tokio::signal::ctrl_c() => {
        println!("Ctrl-C received");
        client.close().await?;
    }
}
```

## Error Recovery

### Automatic Recovery

The client automatically handles:
- Network interruptions (auto-reconnect)
- Server restarts (reconnect with backoff)
- Temporary failures (request queue)
- Dead connections (heartbeat timeout)

### Manual Recovery

For critical failures:

```rust
// If reconnection fails
match client.send_request(&request).await {
    Err(A2AError::Internal(msg)) if msg.contains("Reconnection failed") => {
        // Wait before manual retry
        tokio::time::sleep(Duration::from_secs(30)).await;

        // Create new client
        let new_client = WebSocketClient::new(url);
        new_client.connect().await?;
    }
    _ => {}
}
```

## Performance Considerations

### Memory Usage

Each client maintains:
- Connection buffer: ~16KB
- Request queue: up to `max_size * message_size`
- Session state: <1KB
- Background tasks: 2-3 tasks

**For high connection counts**:
- Use connection pooling
- Limit queue size per connection
- Monitor memory usage

### CPU Usage

Factors affecting CPU:
- Heartbeat frequency (linear)
- Reconnection attempts (exponential backoff helps)
- Message serialization/deserialization

**Optimization tips**:
- Tune heartbeat interval
- Use larger batch sizes
- Enable compression for large payloads

### Network Usage

Factors affecting bandwidth:
- Heartbeat pings (negligible)
- Message payloads
- Reconnection attempts

**Optimization tips**:
- Use appropriate message batching
- Compress large payloads
- Monitor bandwidth usage

## Testing

### Unit Tests

```bash
cargo test --package a2a-rs --features "ws-client" websocket
```

### Integration Tests

```bash
cargo test --package a2a-rs --features "ws-client,ws-server" --test websocket_production_test
```

### Manual Testing

Run the production example:

```bash
cargo run --example websocket_production --features "ws-client,ws-server,tracing"
```

## Troubleshooting

### Issue: Frequent Disconnections

**Symptoms**: High reconnect count, connection status flapping

**Solutions**:
1. Check network stability
2. Increase heartbeat interval
3. Adjust session timeout
4. Check server load

### Issue: Reconnection Failures

**Symptoms**: ReconnectionFailed error after max attempts

**Solutions**:
1. Verify server is running
2. Check firewall rules
3. Increase max_attempts
4. Adjust backoff parameters

### Issue: Queue Overflow

**Symptoms**: QueueFull error

**Solutions**:
1. Increase max_size
2. Implement backpressure
3. Reduce request rate
4. Improve server throughput

### Issue: Heartbeat Timeouts

**Symptoms**: Session expired, unexpected reconnections

**Solutions**:
1. Increase heartbeat timeout
2. Check network latency
3. Reduce heartbeat interval
4. Verify server is responsive

## Example: Complete Production Setup

```rust
use a2a_rs::adapter::transport::websocket::client::{
    WebSocketClient, ReconnectConfig, HeartbeatConfig, QueueConfig
};
use std::time::Duration;

// Production configuration
let reconnect_config = ReconnectConfig::builder()
    .enabled(true)
    .max_attempts(10)
    .initial_backoff(Duration::from_millis(200))
    .max_backoff(Duration::from_secs(30))
    .backoff_multiplier(2.0)
    .jitter_factor(0.15)
    .build();

let heartbeat_config = HeartbeatConfig::builder()
    .enabled(true)
    .interval(Duration::from_secs(30))
    .timeout(Duration::from_secs(15))
    .build();

let queue_config = QueueConfig::builder()
    .enabled(true)
    .max_size(1000)
    .max_age(Duration::from_secs(300))
    .max_retries(3)
    .build();

let client = WebSocketClient::new("ws://api.example.com/ws".to_string())
    .with_reconnect_config(reconnect_config)
    .with_heartbeat_config(heartbeat_config)
    .with_queue_config(queue_config)
    .with_session_timeout(Duration::from_secs(300))
    .with_timeout(30);

// Connect and start background tasks
let mut client_mut = client.clone();
client_mut.connect().await?;
client_mut.start_background_tasks().await?;

// Monitor connection
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        let status = client.status().await;
        let session = client.session_state().await;
        tracing::info!(
            "Status: {:?}, Reconnects: {}, Idle: {:?}",
            status,
            session.reconnect_count,
            session.last_activity.elapsed()
        );
    }
});

// Use the client...
```

## Additional Resources

- Example: `examples/websocket_production.rs`
- Tests: `tests/websocket_production_test.rs`
- Error types: `src/adapter/error/client.rs`
- Implementation: `src/adapter/transport/websocket/client.rs`
