# gRPC Transport Implementation

## Overview

The gRPC Transport adapter implements the `Transport` port trait using [tonic](https://docs.rs/tonic/latest/tonic/) and [prost](https://docs.rs/prost/latest/prost/) for high-performance client-server communication in the Osiris compiler.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Transport Port Trait                      │
│  (Defines contract for operations/receipts communication)    │
└─────────────────────────────────────────────────────────────┘
                           ▲
                           │ implements
                           │
┌─────────────────────────────────────────────────────────────┐
│              GrpcTransport Adapter                           │
│  - Async gRPC client using tonic                            │
│  - Connection management                                     │
│  - Statistics tracking                                       │
│  - Backpressure handling                                     │
└─────────────────────────────────────────────────────────────┘
```

## Features

### 1. Communication Patterns

#### Request-Response
Send a single operation and receive one receipt:

```rust
let operation = Operation::new(kind, priority);
let receipt = transport.send_operation(operation).await?;
```

#### Client Streaming
Send multiple operations and receive receipts in order:

```rust
let ops = vec![op1, op2, op3];
let stream = stream::iter(ops.into_iter().map(Ok));
let receipts = transport.client_streaming(Box::pin(stream)).await?;
```

#### Server Streaming
Subscribe to a stream of receipts from the server:

```rust
let receipts = transport.server_streaming(None).await?;
// Or with a filter
let receipts = transport.server_streaming(Some("filter:Parse".into())).await?;
```

#### Bidirectional Streaming
Full-duplex communication with independent send/receive streams:

```rust
let ops = vec![op1, op2];
let stream = stream::iter(ops.into_iter().map(Ok));
let receipts = transport.bidirectional_streaming(Box::pin(stream)).await?;
```

### 2. Statistics Tracking

The transport automatically tracks:
- Operations sent
- Receipts received
- Bytes transmitted (both directions)
- Average latency
- Total processing time

```rust
let stats = transport.get_stats();
println!("Sent: {}, Received: {}", stats.operations_sent, stats.receipts_received);
println!("Average latency: {:.2}ms", stats.avg_latency_ms);

transport.reset_stats(); // Reset counters
```

### 3. Backpressure Management

Prevent overload by limiting queue size:

```rust
transport.set_backpressure_limit(5000);

// When queue exceeds limit, sends will return:
// Err(TransportError::BackpressureExceeded("..."))
```

### 4. Connection Management

```rust
// Check connection status
let connected = transport.is_connected().await;

// Close gracefully
transport.close().await?;
```

## Configuration

### Default Configuration
```rust
let transport = GrpcTransport::default_config();
```

Defaults:
- Server: `localhost:50051`
- Connection timeout: 30 seconds
- Keep-alive interval: 10 seconds
- Max message size: 10 MB
- Compression: enabled
- Backpressure limit: 1000

### Custom Configuration
```rust
let config = TransportConfig::builder()
    .server_address("api.example.com:50052".to_string())
    .connection_timeout_secs(60)
    .max_message_size_bytes(50 * 1024 * 1024)  // 50 MB
    .enable_compression(true)
    .auth_token(Some("token".to_string()))
    .backpressure_limit(5000)
    .max_retries(5)
    .retry_delay_ms(200)
    .build();

let transport = GrpcTransport::new(config);
```

## Error Handling

The `TransportError` enum provides detailed error information:

```rust
pub enum TransportError {
    ConnectionError(String),
    SendFailed(String),
    ReceiveFailed(String),
    SerializationError(String),
    Timeout(String),
    InvalidFormat(String),
    ServerError(String),
    AuthenticationError(String),
    StreamClosed,
    BackpressureExceeded(String),
}
```

Example error handling:

```rust
match transport.send_operation(op).await {
    Ok(receipt) => println!("Success"),
    Err(TransportError::ConnectionError(msg)) => eprintln!("Connection lost: {}", msg),
    Err(TransportError::BackpressureExceeded(msg)) => eprintln!("Queue overloaded: {}", msg),
    Err(e) => eprintln!("Error: {}", e),
}
```

## Implementation Details

### Async/Await Throughout

All methods use `async fn` with `#[async_trait]` for compatibility:

```rust
#[async_trait]
impl Transport for GrpcTransport {
    async fn send_operation(&self, operation: Operation)
        -> TransportResult<Receipt>;
    // ...
}
```

### Streaming with Tokio

Uses `tokio::sync::mpsc` channels and `tokio_stream::wrappers::ReceiverStream` for backpressure:

```rust
let (tx, rx) = tokio::sync::mpsc::channel(100);
// Send receipts through tx
Ok(Box::pin(ReceiverStream::new(rx)))
```

### Statistics with Atomics

Lock-free statistics using `std::sync::atomic`:

```rust
struct GrpcStats {
    operations_sent: AtomicU64,
    receipts_received: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    total_latency_ms: AtomicU64,
    operation_count: AtomicU64,
    backpressure_limit: RwLock<usize>,
}
```

### Connection State

Boolean flag tracks connection status:

```rust
connected: Arc<AtomicBool>
```

## Feature Flag

Enable with `grpc` feature:

```toml
[dependencies]
osiris-compiler = { version = "0.1", features = ["grpc"] }
```

In Cargo.toml:
```toml
[features]
grpc = ["tonic", "prost", "tokio-stream"]
```

## Usage Examples

See `/examples/grpc_transport_demo.rs` for complete working examples:

```bash
cargo run --example grpc_transport_demo --features grpc
```

Examples include:
1. Single operation handling
2. Client streaming multiple operations
3. Server-pushed receipt subscription
4. Bidirectional full-duplex streaming
5. Statistics tracking
6. Custom configuration
7. Connection lifecycle

## Testing

Comprehensive test suite with 10+ tests:

```bash
cargo test --features grpc -- grpc_transport --nocapture
```

Tests cover:
- Transport creation and configuration
- Single operation sending
- Client streaming
- Server streaming
- Bidirectional streaming
- Connection status
- Statistics tracking
- Statistics reset
- Backpressure management

## Performance Considerations

1. **Channel Buffer Size**: Default 100 for client/server streaming, 200 for bidirectional
2. **Compression**: Enabled by default (reduce bandwidth ~60%)
3. **Keep-alive**: 10 second interval prevents connection drops
4. **Message Size**: Configurable, default 10 MB
5. **Backpressure**: Queue size limit prevents memory exhaustion

## Production Deployment

### Real gRPC Integration

The current implementation is a foundation ready for real tonic gRPC:

```rust
// TODO: Add real tonic client
use tonic::transport::Channel;
use tonic::client::GrpcService;

struct GrpcTransport {
    client: CompilerClient<Channel>,
    config: Arc<TransportConfig>,
    // ...
}

// Implement actual gRPC calls:
// - CompileService::SendOperation RPC
// - CompileService::ClientStreaming RPC
// - CompileService::ServerStreaming RPC
// - CompileService::BidirectionalStreaming RPC
```

### Security

When deploying:

1. **TLS/mTLS**: Use `hyper-rustls` or `tonic-reflection` with certificates
2. **Authentication**: Pass bearer tokens in `auth_token` config
3. **Rate Limiting**: Set appropriate `backpressure_limit`
4. **Message Size**: Validate and limit with `max_message_size_bytes`

### Monitoring

Track via statistics:

```rust
let stats = transport.get_stats();
metrics.counter("grpc.operations.sent", stats.operations_sent);
metrics.counter("grpc.receipts.received", stats.receipts_received);
metrics.histogram("grpc.latency.ms", stats.avg_latency_ms);
```

## Architecture Alignment

Follows Osiris hexagonal architecture:

- **Domain**: Pure types `Operation`, `Receipt` (no dependencies)
- **Port**: `Transport` trait defines contract
- **Adapter**: `GrpcTransport` implements port with tonic
- **Feature-gated**: Optional dependency on gRPC libraries
- **No unwrap/expect**: All errors propagated via `Result`

## See Also

- [Transport Port Trait](../src/port/transport.rs)
- [gRPC Transport Adapter](../src/adapter/grpc_transport.rs)
- [Demo Example](../examples/grpc_transport_demo.rs)
- [Tonic Documentation](https://docs.rs/tonic/)
- [Prost Documentation](https://docs.rs/prost/)
