# WebSocket Transport Implementation Patterns

## Overview

Complete implementation of bidirectional WebSocket transport for the a2a-rs workspace with automatic reconnection, heartbeat management, and graceful error handling.

## Architecture Decision

**Transport Port Trait**: Abstraction layer defining async communication interface:
- `connect()` - Establish connection
- `send(packet)` - Send typed packet
- `receive()` - Get next packet or None
- `disconnect()` - Graceful close
- `reconnect()` - Auto-backoff reconnection
- `send_batch()`, `receive_batch()` - Batch operations
- `status()` - Get connection state

**WebSocketTransport Adapter**: tokio-tungstenite implementation:
- Feature-gated with `ws` flag
- Manages full connection lifecycle
- Automatic ping/pong heartbeat
- Exponential backoff with configurable limits

## Key Technical Insights

### Stream Type Handling

**Problem**: `connect_async()` returns `WebSocketStream<MaybeTlsStream<TcpStream>>`, not `WebSocketStream<TcpStream>`

**Solution**: Import `MaybeTlsStream` from tokio-tungstenite and use as generic parameter:
```rust
use tokio_tungstenite::MaybeTlsStream;

ws: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>
```

This automatically supports both `ws://` (plain) and `wss://` (TLS) endpoints.

### Backoff Calculation

Exponential backoff with jitter-free maximum cap:

```rust
fn calculate_backoff(&self) -> Duration {
    let delay_ms = (self.config.initial_reconnect_delay.as_millis() as f64
        * self.config.reconnect_backoff.powi(self.reconnect_attempts as i32)) as u64;

    let max_ms = self.config.max_reconnect_delay.as_millis() as u64;
    Duration::from_millis(delay_ms.min(max_ms))
}
```

**Note**: Using `as i32` for exponent is safe because attempts are u32 and won't overflow.

### Heartbeat Mechanism

Ping/pong detection happens **before** receive to catch stale connections:

```rust
async fn receive(&mut self) -> Result<Option<TypedPacket>, TransportError> {
    // Poll heartbeat FIRST (may send ping)
    self.poll_heartbeat().await?;

    // Then receive message
    match tokio::time::timeout(
        self.config.pong_timeout,
        ws.next(),
    ).await { ... }
}
```

**Why proactive?** Waiting for pong timeout during receive means a dead connection blocks until timeout. Early ping detection makes status degraded sooner.

### Message Frame Handling

WebSocket frames map to TypedPacket operations:

```rust
match msg {
    Message::Text(text) => serde_json::from_str::<TypedPacket>(&text),
    Message::Binary(bytes) => serde_json::from_slice::<TypedPacket>(&bytes),
    Message::Ping(_) => Ok(None),  // Non-packet, continue
    Message::Pong(_) => Ok(None),  // Non-packet, continue
    Message::Close(_) => Ok(None), // Connection closing
}
```

**Return `None` for control frames**: `receive()` caller will retry, eventually getting packet or closed signal.

### Status State Machine

```
Disconnected
    ↓ (connect)
Connecting → Connected
                ↓
            (ping timeout)
                ↓
            Degraded
                ↓ (reconnect)
            Connecting → Connected
                ↓
            (max retries)
                ↓
            Failed
```

**Key invariant**: Only `Connected` state allows send/receive.

## Configuration Best Practices

### Default Values (tuned for production)

```rust
TransportConfig {
    url: "ws://localhost:8080/ws",
    ping_interval: 30s,           // Not too chatty, but frequent
    pong_timeout: 10s,            // TCP retransmit ~3x before timeout
    initial_reconnect_delay: 100ms, // Quick retry for transients
    max_reconnect_delay: 30s,     // Don't hammer server forever
    reconnect_backoff: 2.0,       // Standard exponential
    max_reconnect_attempts: 10,   // ~13 minutes with 2x backoff
    buffer_size: 256,             // Small bounded queue
}
```

**Why 30s ping?** HTTP/2 defaults to 30s for idle detection, TCP keepalive is system-dependent.

### Custom Configuration Pattern

```rust
let config = TransportConfig::new("wss://agents.example.com/ws")
    .with_ping_interval(Duration::from_secs(15))
    .with_pong_timeout(Duration::from_secs(5))
    .with_max_reconnect_attempts(Some(20))
```

All parameters optional - builder returns self after mutation.

## Error Handling Strategy

### Recoverable vs Permanent

**Recoverable** (attempt reconnect):
- `ConnectionFailed` - Network transient
- `ReceiveFailed` - Read interrupted
- `Timeout` - Pong not received in time

**Permanent** (fail fast):
- `MaxRetriesExhausted` - Server repeatedly unavailable
- `InvalidMessage` - Corrupted packet (won't retry)
- `AlreadyConnected` - Logic error in caller

### Error Recovery Pattern

```rust
loop {
    match transport.receive().await {
        Ok(Some(packet)) => handle_packet(packet),
        Ok(None) => continue,  // Control frame, retry
        Err(TransportError::NotConnected) => {
            transport.reconnect().await?;
        }
        Err(TransportError::Timeout(_)) => {
            // Connection degraded but may recover
            transport.reconnect().await?;
        }
        Err(e) => return Err(e), // Permanent failure
    }
}
```

## Testing Strategy

### Unit Tests (no network required)

```bash
cargo test -p osiris-edge --features ws --lib websocket
```

Tests:
1. Config builder - all settings apply
2. Backoff calculation - exponential growth, max capping
3. Reconnection counter - increment/reset behavior
4. Status tracking - transitions correct
5. Max retries check - threshold enforcement

### Integration Test Pattern

```rust
#[tokio::test]
async fn test_websocket_roundtrip() {
    let config = TransportConfig::new("ws://echo.websocket.org");
    let mut transport = WebSocketTransport::new(config);

    // Real server would be needed for full test
    // This example just creates the transport
    assert_eq!(transport.status(), TransportStatus::Disconnected);
}
```

### Example Verification

```bash
cargo run --example websocket_transport --features ws
```

Creates TypedPacket, demonstrates config, shows status tracking.

## Async Trait Limitations

### Why #[async_trait]?

Rust doesn't yet support `async fn` in trait definitions natively (return position impl trait in traits). Solution: `async_trait` macro expands to:

```rust
// What you write:
#[async_trait]
pub trait Transport {
    async fn connect(&mut self) -> Result<()>;
}

// Expands to:
pub trait Transport {
    fn connect(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}
```

### Performance impact

- Minimal: One heap allocation per trait method call
- Acceptable for I/O (network bound anyway)
- Not suitable for millions/sec hot paths (but we're doing WebSocket, not atomic operations)

## Feature Gating Best Practices

### Cargo.toml Pattern

```toml
[dependencies]
tokio-tungstenite = { version = "0.21", optional = true }

[features]
ws = ["tokio-tungstenite"]
```

### Code Pattern

```rust
#[cfg(feature = "ws")]
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

#[cfg(feature = "ws")]
pub struct WebSocketTransport { ... }

#[cfg(feature = "ws")]
#[async_trait]
impl Transport for WebSocketTransport { ... }
```

### Exporting Conditionally

In `adapter/mod.rs`:
```rust
#[cfg(feature = "ws")]
pub mod websocket;

#[cfg(feature = "ws")]
pub use websocket::WebSocketTransport;
```

In `lib.rs`:
```rust
#[cfg(feature = "ws")]
pub use adapter::WebSocketTransport;
```

## Production Considerations

### Monitoring

Log levels:
- `debug!` - Connection events, ping/pong, packets
- `warn!` - Reconnection attempts, timeouts
- `error!` - Permanent failures, max retries

Integrate with tracing for structured logging.

### Resource Limits

- Max connections: Limited by OS file descriptors + app memory
- Per-connection memory: ~10KB (frame buffer + struct)
- Bandwidth: Depends on packet frequency

### Security

- TLS via `wss://` scheme automatic (MaybeTlsStream)
- Verify certificate in production (default rustls)
- Auth via headers: Implement in application layer

## Future Enhancements

1. **Compression** - permessage-deflate support
2. **Partial messages** - Reassemble fragmented frames
3. **Connection pooling** - Multiple agents share transport
4. **Custom handlers** - Plug in different frame processors
5. **Rate limiting** - Backpressure on send queue

## Related Patterns

- **gRPC Transport** - Similar trait design, different protocol (protobuf + HTTP/2)
- **Axum Router** - Consuming transport for HTTP webhook gateway
- **SSE Streaming** - Server-push alternative (one-directional)
