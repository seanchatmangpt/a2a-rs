# MCP Streamable HTTP Transport

Implementation of the Model Context Protocol (MCP) Streamable HTTP transport specification for the a2a-mcp crate.

## Overview

The Streamable HTTP transport provides two complementary modes for MCP JSON-RPC 2.0 communication:

1. **Request/Response Mode** (HTTP POST) - Traditional synchronous request/response
2. **Server-Sent Events Mode** (HTTP GET) - Streaming responses with resumable connections

## Architecture

Following hexagonal architecture principles:

- **Domain**: MCP JSON-RPC 2.0 protocol types (`McpRequest`, `McpResponse`, `McpError`)
- **Port**: `McpMessageHandler` trait for processing requests
- **Adapter**: `StreamableHttpServer` implementing HTTP/SSE transport

```
┌─────────────────────────────────────────────┐
│         Application Layer                   │
│   (implements McpMessageHandler)            │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│       StreamableHttpServer                  │
│  ┌──────────────┬────────────────────────┐  │
│  │ POST /mcp    │  GET /mcp/sse          │  │
│  │ (JSON-RPC)   │  (SSE Stream)          │  │
│  └──────────────┴────────────────────────┘  │
└─────────────────────────────────────────────┘
```

## Features

### Security

- **Origin Validation**: DNS rebinding attack prevention via Origin/Referer header checking
- **Session Management**: Session binding via `MCP-Session-Id` header
- **Resumable Streams**: Support for `Last-Event-ID` header to resume SSE connections

### Endpoints

#### POST /mcp

Request/response mode for synchronous operations.

**Request Headers**:
- `Content-Type: application/json`
- `Origin` (optional): Validated against allowed origins
- `MCP-Session-Id` (optional): Session identifier

**Request Body**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list",
  "params": {}
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "tools": [...]
  }
}
```

#### GET /mcp/sse

Server-Sent Events mode for streaming responses.

**Query Parameters**:
- `request` (optional): URL-encoded JSON-RPC request to process

**Request Headers**:
- `Origin` (optional): Validated against allowed origins
- `MCP-Session-Id` (optional): Session identifier
- `Last-Event-ID` (optional): Resume from this event sequence number

**Response**: SSE stream
```
event: message
data: {"jsonrpc":"2.0","id":1,"result":{"chunk":1}}

event: message
data: {"jsonrpc":"2.0","id":1,"result":{"chunk":2}}
```

## Usage

### 1. Implement the Message Handler

```rust
use a2a_mcp::transport::streamable_http::{
    McpMessageHandler, McpRequest, McpResponse,
};
use a2a_mcp::error::Result;
use tokio::sync::mpsc;

struct MyHandler;

#[async_trait::async_trait]
impl McpMessageHandler for MyHandler {
    async fn handle_request(&self, request: McpRequest) -> Result<McpResponse> {
        // Handle synchronous request
        Ok(McpResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::json!({"status": "ok"})),
            error: None,
        })
    }

    async fn handle_streaming_request(
        &self,
        request: McpRequest,
        tx: mpsc::Sender<McpResponse>,
    ) -> Result<()> {
        // Send multiple responses for streaming
        for i in 1..=5 {
            let response = McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                result: Some(serde_json::json!({"chunk": i})),
                error: None,
            };
            tx.send(response).await?;
        }
        Ok(())
    }
}
```

### 2. Configure the Server

```rust
use a2a_mcp::transport::streamable_http::{
    StreamableHttpServer, StreamableHttpConfig,
};
use std::time::Duration;

let config = StreamableHttpConfig {
    address: "127.0.0.1:3000".to_string(),
    allowed_origins: vec![
        "http://localhost:3000".to_string(),
        "https://myapp.example.com".to_string(),
    ],
    sse_keep_alive: true,
    sse_keep_alive_interval: Duration::from_secs(15),
    max_buffer_size: 100,
};
```

### 3. Start the Server

```rust
let handler = MyHandler;
let server = StreamableHttpServer::new(handler, config);
server.start().await?;
```

## Client Examples

### POST Request (cURL)

```bash
curl -X POST http://127.0.0.1:3000/mcp \
  -H "Content-Type: application/json" \
  -H "MCP-Session-Id: my-session-123" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/list",
    "params": {}
  }'
```

### SSE Stream (cURL)

```bash
# With initial request
curl -N "http://127.0.0.1:3000/mcp/sse?request=%7B%22jsonrpc%22%3A%222.0%22%2C%22id%22%3A1%2C%22method%22%3A%22stream%2Fdata%22%7D" \
  -H "MCP-Session-Id: my-session-123"

# Resume from event ID 42
curl -N http://127.0.0.1:3000/mcp/sse \
  -H "MCP-Session-Id: my-session-123" \
  -H "Last-Event-ID: 42"
```

### JavaScript Client

```javascript
// Request/response mode
async function mcpRequest(method, params) {
  const response = await fetch('http://127.0.0.1:3000/mcp', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'MCP-Session-Id': sessionId,
    },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method,
      params,
    }),
  });
  return response.json();
}

// SSE streaming mode
const eventSource = new EventSource(
  'http://127.0.0.1:3000/mcp/sse?request=' +
  encodeURIComponent(JSON.stringify({
    jsonrpc: '2.0',
    id: 2,
    method: 'stream/data',
  }))
);

eventSource.addEventListener('message', (event) => {
  const response = JSON.parse(event.data);
  console.log('Received:', response);
});
```

## Configuration

### StreamableHttpConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `address` | String | `"127.0.0.1:3000"` | Server bind address |
| `allowed_origins` | Vec\<String\> | `["http://localhost:3000"]` | Origins allowed for CORS |
| `sse_keep_alive` | bool | `true` | Enable SSE keep-alive pings |
| `sse_keep_alive_interval` | Duration | `15s` | Interval between keep-alive pings |
| `max_buffer_size` | usize | `100` | Maximum events buffered per session |

### Origin Validation

The server validates the `Origin` header against `allowed_origins`:

- If `allowed_origins` is empty, all origins are allowed (not recommended for production)
- If the request has no Origin header, it's considered same-origin and allowed
- If the Origin doesn't match, returns HTTP 403 Forbidden

This prevents DNS rebinding attacks where malicious sites try to communicate with localhost services.

## Error Handling

The transport returns standard JSON-RPC 2.0 errors:

| Code | Message | Description |
|------|---------|-------------|
| -32700 | Parse error | Invalid JSON in request |
| -32600 | Invalid request | Malformed JSON-RPC request |
| -32601 | Method not found | Unknown method |
| -32602 | Invalid params | Invalid method parameters |
| -32603 | Internal error | Server-side error |
| -32000 | Server error | Generic server error (including origin forbidden) |

## Session Management

Sessions are tracked via the `MCP-Session-Id` header:

- If not provided, a new UUID is generated
- Sessions are automatically cleaned up when SSE streams end
- Active sessions are stored in-memory with their event buffers

## Performance Considerations

- Each SSE connection maintains an in-memory buffer (`max_buffer_size`)
- Sessions are cleaned up 1 second after stream closure
- Keep-alive pings prevent connection timeout but add overhead
- For high-throughput scenarios, consider increasing `max_buffer_size`

## Testing

Run the included example:

```bash
cargo run -p a2a-mcp --example streamable_http_demo
```

Then test with:

```bash
# Test POST endpoint
curl -X POST http://127.0.0.1:3030/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"test/echo","params":{"message":"hello"}}'

# Test SSE endpoint
curl -N "http://127.0.0.1:3030/mcp/sse?request=%7B%22jsonrpc%22%3A%222.0%22%2C%22id%22%3A2%2C%22method%22%3A%22test%2Fstream%22%7D"
```

## Compliance

This implementation follows:

- JSON-RPC 2.0 Specification
- Model Context Protocol (MCP) Streamable HTTP Transport Specification
- Server-Sent Events (SSE) W3C Specification
- RFC 8615 Well-Known URIs (for future `.well-known/mcp` support)

## Future Enhancements

- [ ] Persistent session storage (Redis, etc.)
- [ ] Event replay buffer for missed events
- [ ] WebSocket transport as alternative to SSE
- [ ] Rate limiting and authentication middleware
- [ ] Metrics and observability hooks
