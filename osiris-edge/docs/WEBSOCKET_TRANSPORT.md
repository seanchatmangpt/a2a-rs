# WebSocket Transport Adapter

Complete bidirectional transport for streaming TypedPacket messages with automatic reconnection and heartbeat management.

## Overview

The Transport port defines a trait for bidirectional communication with:
- Send/receive typed packets (JSON serialization)
- Ping/pong heartbeat mechanism
- Exponential backoff reconnection with retry limits
- Connection status tracking
- Batch operations

The WebSocketTransport adapter implements this using tokio-tungstenite:
- Feature-gated with `ws` flag
- Automatic connection lifecycle management
- Graceful shutdown support
- Comprehensive error handling

## Architecture

### Port Trait: `Transport`

Located in `src/port/transport.rs`:

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&mut self) -> Result<(), TransportError>;
    async fn send(&mut self, packet: TypedPacket) -> Result<(), TransportError>;
    async fn receive(&mut self) -> Result<Option<TypedPacket>, TransportError>;
    fn status(&self) -> TransportStatus;
    async fn disconnect(&mut self) -> Result<(), TransportError>;
    async fn reconnect(&mut self) -> Result<(), TransportError>;
    async fn send_batch(&mut self, packets: Vec<TypedPacket>) -> Result<(), TransportError>;
    async fn receive_batch(&mut self, timeout: Duration) -> Result<Vec<TypedPacket>, TransportError>;
}
```

### Adapter: `WebSocketTransport`

Located in `src/adapter/websocket.rs`:

- Wrapped in `#[cfg(feature = "ws")]`
- Implements Transport port with tokio-tungstenite
- Manages WebSocket connection lifecycle
- Handles ping/pong frames automatically
- Implements exponential backoff reconnection

## Configuration

### TransportConfig Builder

```rust
use osiris_edge::{TransportConfig, Transport};
use std::time::Duration;

let config = TransportConfig::new("ws://localhost:8080/ws")
    .with_ping_interval(Duration::from_secs(30))
    .with_pong_timeout(Duration::from_secs(10))
    .with_reconnect_config(
        Duration::from_millis(100),  // initial delay
        Duration::from_secs(30),     // max delay
        2.0,                          // backoff multiplier
    )
    .with_max_reconnect_attempts(Some(10))
    .with_buffer_size(256);
```

### Default Configuration

```rust
TransportConfig {
    url: "ws://localhost:8080/ws",
    ping_interval: 30s,
    pong_timeout: 10s,
    initial_reconnect_delay: 100ms,
    max_reconnect_delay: 30s,
    reconnect_backoff: 2.0,
    max_reconnect_attempts: Some(10),
    buffer_size: 256,
}
```

## Usage

### Basic Connection

```rust
use osiris_edge::{WebSocketTransport, TransportConfig, Transport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = TransportConfig::new("ws://server.example.com/ws");
    let mut transport = WebSocketTransport::new(config);

    // Connect to server
    transport.connect().await?;

    // Check status
    println!("Connected: {}", transport.is_connected());

    Ok(())
}
```

### Sending Packets

```rust
use osiris_edge::{domain::*, WebSocketTransport, Transport};

let mut transport = WebSocketTransport::new(config);
transport.connect().await?;

// Create a typed packet
let packet = TypedPacket::new(
    PacketSource::Gmail { /* ... */ },
    PacketPayload::Email { /* ... */ },
    PacketContext { /* ... */ },
);

// Send single packet
transport.send(packet).await?;

// Send batch of packets
let packets = vec![packet1, packet2, packet3];
transport.send_batch(packets).await?;

transport.disconnect().await?;
```

### Receiving Packets

```rust
// Receive single packet (blocks until message received or connection closed)
match transport.receive().await? {
    Some(packet) => println!("Received: {}", packet.id),
    None => println!("Connection closed"),
}

// Receive multiple packets with timeout
use std::time::Duration;

let packets = transport
    .receive_batch(Duration::from_secs(5))
    .await?;

println!("Received {} packets", packets.len());
```

### Reconnection Logic

```rust
// Automatic reconnection with exponential backoff
if !transport.is_connected() {
    transport.reconnect().await?;
}

// Check connection status
match transport.status() {
    TransportStatus::Connected => println!("Healthy"),
    TransportStatus::Degraded => println!("Reconnecting..."),
    TransportStatus::Failed => println!("Permanent failure"),
    TransportStatus::Disconnected => println!("Not connected"),
    TransportStatus::Connecting => println!("Connecting..."),
}
```

## Protocol Details

### Packet Serialization

- TypedPacket serialized as JSON text frames
- Binary frames also supported
- Automatic serialization/deserialization

### Heartbeat Mechanism

- Ping sent every `ping_interval` (default: 30s)
- Waits `pong_timeout` (default: 10s) for pong response
- Timeout treated as connection failure
- Status transitions to Degraded on timeout

### Reconnection Strategy

**Exponential Backoff Formula:**
```
delay = min(initial_delay * backoff^attempts, max_delay)
```

**Default backoff:**
- 1st attempt: 100ms
- 2nd attempt: 200ms
- 3rd attempt: 400ms
- 4th attempt: 800ms
- ...
- Capped at 30s

**Max retries:** 10 (configurable)

### Status Transitions

```
Disconnected --connect--> Connecting --success--> Connected
                                          |
                                          v
                                       (ping/pong loop)
                                          |
                                  timeout/error
                                          |
                                          v
                                      Degraded
                                          |
                                   reconnect
                                          |
                                   attempt limit exceeded
                                          |
                                          v
                                       Failed
```

## Error Handling

### TransportError Types

```rust
pub enum TransportError {
    ConnectionFailed(String),
    SendFailed(String),
    ReceiveFailed(String),
    ConnectionClosed,
    Timeout(String),
    SerializationError(String),
    InvalidMessage(String),
    AlreadyConnected,
    NotConnected,
    MaxRetriesExhausted,
}
```

### Error Recovery Patterns

```rust
// Retry on transient errors
loop {
    match transport.send(packet).await {
        Ok(()) => break,
        Err(TransportError::NotConnected) => {
            transport.reconnect().await?;
        }
        Err(e) => return Err(e),
    }
}

// Handle permanent failures
match transport.reconnect().await {
    Err(TransportError::MaxRetriesExhausted) => {
        eprintln!("Permanent connection failure");
    }
    Err(e) => eprintln!("Reconnection error: {}", e),
    Ok(()) => println!("Reconnected"),
}
```

## Testing

### Unit Tests

```bash
cargo test -p osiris-edge --features ws --lib websocket
```

Includes:
- Configuration builder validation
- Backoff calculation
- Reconnection counter management
- Status transitions
- Serialization roundtrips

### Integration Example

Run the example:
```bash
cargo run --example websocket_transport --features ws
```

## Feature Gate

The WebSocket transport is feature-gated to avoid dependency on `tokio-tungstenite`:

```toml
[features]
ws = ["tokio-tungstenite"]
```

Without the `ws` feature:
- `WebSocketTransport` type not available
- Transport trait still accessible (for other implementations)
- No `tokio-tungstenite` dependency

## Implementation Notes

### Type Safety

- WebSocketStream uses `MaybeTlsStream<TcpStream>` to support both HTTP and HTTPS connections
- All async methods use `#[async_trait]` for trait definition
- Generic bounds: `Send + Sync` for thread safety with Tokio

### Memory Efficiency

- Bounded message buffer (configurable)
- Automatic cleanup on connection loss
- No unbounded queuing

### Tracing Integration

All operations logged at appropriate levels:
- `debug!`: Connection events, ping/pong, data transfers
- `warn!`: Reconnection attempts, timeouts
- `error!`: Connection failures, max retries exceeded

## Example: Multi-Agent Gateway

```rust
use osiris_edge::{WebSocketTransport, TransportConfig};
use std::time::Duration;

async fn gateway_server() -> Result<(), Box<dyn std::error::Error>> {
    // Multi-protocol gateway with WebSocket transport
    let ws_config = TransportConfig::new("ws://agents.example.com:8080")
        .with_ping_interval(Duration::from_secs(15));

    let mut transport = WebSocketTransport::new(ws_config);
    transport.connect().await?;

    // Receive packets from agents
    loop {
        match transport.receive().await? {
            Some(packet) => {
                println!("Received from agent: {}", packet.id);
                // Process packet...
            }
            None => {
                println!("Agent disconnected");
                transport.reconnect().await.ok();
            }
        }
    }
}
```

## Performance Characteristics

- Connection: ~1-2 TCP handshakes + WebSocket upgrade
- Send: One JSON serialization + one send_all call
- Receive: One timeout poll + one recv call
- Ping overhead: One frame every 30 seconds
- Memory: ~10KB per connection (stack + message buffer)

## Future Enhancements

- [ ] Compression support (permessage-deflate)
- [ ] TLS certificate validation options
- [ ] Partial message reassembly
- [ ] Custom message frame types
- [ ] Connection pooling for multiple agents
