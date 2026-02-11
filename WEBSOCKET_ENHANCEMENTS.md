# WebSocket Transport Enhancements

## Summary

The WebSocket transport implementation has been enhanced with production-ready features for reliability and resilience.

## What's New

### 1. Automatic Reconnection with Exponential Backoff

- Exponential backoff prevents overwhelming servers during outages
- Jitter prevents thundering herd problems
- Configurable retry limits and backoff parameters
- Automatic state restoration on reconnection

### 2. Session State Tracking

- Unique session IDs for connection lifecycle tracking
- Connection and activity timestamps
- Reconnection count monitoring
- Session expiration detection

### 3. Request Queue for Offline Periods

- Queues requests during disconnections
- Configurable queue size and age limits
- Automatic retry with configurable attempts
- Request age-based expiration

### 4. Heartbeat Mechanism

- Configurable ping/pong heartbeat
- Connection health monitoring
- Dead connection detection
- Automatic reconnection on heartbeat failure

### 5. Complete Error Recovery

- Graceful handling of connection failures
- Automatic retry with exponential backoff
- Comprehensive error types
- Clear error messages for debugging

## New Types

### `SessionState`
```rust
pub struct SessionState {
    pub session_id: String,
    pub connected_at: Instant,
    pub last_activity: Instant,
    pub reconnect_count: u32,
}
```

### `ConnectionStatus`
```rust
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Closed,
}
```

### Configuration Builders

#### `ReconnectConfig`
```rust
let config = ReconnectConfig::builder()
    .enabled(true)
    .max_attempts(10)
    .initial_backoff(Duration::from_millis(100))
    .max_backoff(Duration::from_secs(30))
    .backoff_multiplier(2.0)
    .jitter_factor(0.1)
    .build();
```

#### `HeartbeatConfig`
```rust
let config = HeartbeatConfig::builder()
    .enabled(true)
    .interval(Duration::from_secs(30))
    .timeout(Duration::from_secs(10))
    .build();
```

#### `QueueConfig`
```rust
let config = QueueConfig::builder()
    .enabled(true)
    .max_size(1000)
    .max_age(Duration::from_secs(300))
    .max_retries(3)
    .build();
```

## New Error Types

### `WebSocketClientError` additions

- `ReconnectionFailed { max_retries: u32 }` - Reconnection exhausted
- `SessionExpired` - Session timed out
- `HeartbeatTimeout { seconds: u64 }` - Heartbeat failed
- `QueueFull { current: usize, capacity: usize }` - Queue overflow

## API Additions

### WebSocketClient Methods

```rust
// Configuration
pub fn with_reconnect_config(self, config: ReconnectConfig) -> Self
pub fn with_heartbeat_config(self, config: HeartbeatConfig) -> Self
pub fn with_queue_config(self, config: QueueConfig) -> Self
pub fn with_session_timeout(self, timeout: Duration) -> Self

// Monitoring
pub async fn status(&self) -> ConnectionStatus
pub async fn session_state(&self) -> SessionState

// Lifecycle
pub async fn start_background_tasks(&self) -> Result<(), A2AError>
pub async fn close(&mut self) -> Result<(), A2AError>
```

## Files Modified

- `/Users/sac/a2a-rs/a2a-rs/src/adapter/error/client.rs` - Added new error types
- `/Users/sac/a2a-rs/a2a-rs/src/adapter/transport/websocket/client.rs` - Complete rewrite with production features

## Files Added

- `/Users/sac/a2a-rs/a2a-rs/tests/websocket_production_test.rs` - Comprehensive tests
- `/Users/sac/a2a-rs/a2a-rs/tests/websocket_quick_test.rs` - Quick compilation tests
- `/Users/sac/a2a-rs/a2a-rs/examples/websocket_production.rs` - Production usage example
- `/Users/sac/a2a-rs/docs/websocket-production-guide.md` - Complete documentation

## Usage Example

```rust
use a2a_rs::adapter::transport::websocket::client::{
    WebSocketClient, ReconnectConfig, HeartbeatConfig, QueueConfig
};
use std::time::Duration;

// Configure for production
let reconnect_config = ReconnectConfig::builder()
    .enabled(true)
    .max_attempts(10)
    .initial_backoff(Duration::from_millis(100))
    .max_backoff(Duration::from_secs(30))
    .build();

let heartbeat_config = HeartbeatConfig::builder()
    .enabled(true)
    .interval(Duration::from_secs(30))
    .timeout(Duration::from_secs(10))
    .build();

let queue_config = QueueConfig::builder()
    .enabled(true)
    .max_size(1000)
    .max_age(Duration::from_secs(300))
    .max_retries(3)
    .build();

// Create client
let client = WebSocketClient::new("ws://localhost:8080/ws".to_string())
    .with_reconnect_config(reconnect_config)
    .with_heartbeat_config(heartbeat_config)
    .with_queue_config(queue_config);

// Connect and start monitoring
let mut client_mut = client.clone();
client_mut.connect().await?;
client_mut.start_background_tasks().await?;

// Use the client
let task = client.get_task("task-id", None).await?;

// Monitor connection
let status = client.status().await;
let session = client.session_state().await;
println!("Status: {:?}, Reconnects: {}", status, session.reconnect_count);

// Clean shutdown
client_mut.close().await?;
```

## Running Examples

### Production Example
```bash
cargo run --example websocket_production --features "ws-client,ws-server,tracing"
```

### Tests
```bash
# Quick compilation test
cargo test --package a2a-rs --features "ws-client" websocket_quick_test

# Full production tests
cargo test --package a2a-rs --features "ws-client,ws-server" websocket_production_test
```

## Production Recommendations

1. **Always enable reconnection** in production with appropriate backoff settings
2. **Enable heartbeat** for dead connection detection (15-30s interval)
3. **Configure request queue** based on memory constraints and SLA requirements
4. **Monitor connection status** and session metrics
5. **Implement graceful shutdown** with `close()`
6. **Use appropriate timeouts** based on your use case
7. **Test failure scenarios** before deploying to production

## Documentation

See `/Users/sac/a2a-rs/docs/websocket-production-guide.md` for comprehensive production guidance including:

- Architecture details
- Configuration tuning guides
- Error handling patterns
- Performance considerations
- Monitoring and metrics
- Troubleshooting guide
- Complete production setup example

## Backward Compatibility

The enhanced WebSocket client maintains backward compatibility with the existing API. All new features are opt-in through configuration builders.

Default behavior:
- Reconnection: Enabled with conservative settings
- Heartbeat: Enabled with 30s interval
- Queue: Enabled with 1000 item limit
- Session timeout: 5 minutes

These defaults can be customized via the configuration builders.
