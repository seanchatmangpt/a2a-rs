# MCP Streamable HTTP Server

## Overview

The `StreamableHttpServer` is an Axum-based HTTP server implementing the MCP Streamable HTTP transport specification with integrated middleware for origin guard and session management.

## Architecture

The server implements a layered middleware architecture:

```
Request
   ↓
Origin Guard Middleware ← Validates Origin header (DNS rebinding defense)
   ↓
Session Middleware ← Manages request-scoped sessions with MCP-Session-Id
   ↓
Route Handlers
   ├── POST /mcp    → JSON-RPC 2.0 request/response mode
   └── GET /mcp     → Server-Sent Events (SSE) streaming mode
```

## Features

### 1. Origin Guard Middleware
- **DNS Rebinding Defense**: Validates the `Origin` header against an allowlist
- **Flexible Configuration**: Support for localhost-only, custom allowlist, or wildcard (testing only)
- **Deterministic Rejection**: Returns 403 Forbidden with clear error messages

Example:
```rust
let origin_guard = Arc::new(OriginGuard::new(vec![
    "http://localhost:3000".to_string(),
    "https://example.com".to_string(),
]));
```

### 2. Session Middleware
- **Request Scoping**: Creates or retrieves sessions via `MCP-Session-Id` header
- **Access Tracking**: Automatically touches sessions on each request
- **Session State**: Stores arbitrary JSON state and metadata
- **TTL Support**: Optional session expiration via `ttl_seconds`

Example:
```rust
let session_manager = Arc::new(InMemorySessionManager::new());
let (session, is_new) = session_manager
    .get_or_create_session("session-123".to_string())
    .await?;
```

### 3. POST Request/Response Mode
- **JSON-RPC 2.0**: Full compliance with JSON-RPC 2.0 specification
- **Stateless**: Each POST request is independent
- **Session Binding**: Responses include `MCP-Session-Id` header

Example request:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tasks/get",
  "params": { "taskId": "task-123" }
}
```

### 4. GET Server-Sent Events (SSE) Mode
- **Streaming**: Bidirectional event streaming via HTTP long-polling
- **Resumability**: Supports `Last-Event-ID` header for stream resumption
- **Keep-Alive**: Automatic keep-alive events every 30 seconds
- **Session Lifecycle**: Maintains session state during streaming

Example curl request:
```bash
curl -N -H "Accept: text/event-stream" \
  -H "Origin: http://localhost:3000" \
  "http://localhost:3000/mcp"
```

## Usage

### Basic Setup

```rust
use std::sync::Arc;
use std::net::SocketAddr;
use a2a_mcp::{
    StreamableHttpServer, McpTaskHandler,
    InMemorySessionManager, TaskWrapper,
    OriginGuard,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create task handler
    let task_wrapper = Arc::new(TaskWrapper::new());
    let handler = Arc::new(McpTaskHandler::new(task_wrapper));

    // Create session manager
    let session_manager = Arc::new(InMemorySessionManager::new());

    // Create origin guard
    let origin_guard = Arc::new(OriginGuard::localhost_only());

    // Create and start server
    let addr: SocketAddr = "127.0.0.1:3000".parse()?;
    let server = StreamableHttpServer::new(
        addr,
        handler,
        origin_guard,
        session_manager,
    );

    server.start().await?;
    Ok(())
}
```

### Advanced Configuration

```rust
// Custom origins
let origin_guard = Arc::new(OriginGuard::new(vec![
    "https://example.com".to_string(),
    "https://app.example.com".to_string(),
]));

// Custom session manager (implement SessionManager trait)
let session_manager = Arc::new(MyCustomSessionManager::new());

// Create with custom configuration
let server = StreamableHttpServer::new(
    "0.0.0.0:8080".parse()?,
    handler,
    origin_guard,
    session_manager,
);
```

### Convenience Constructors

```rust
// Localhost-only with custom session manager
let server = StreamableHttpServer::localhost(
    addr,
    handler,
    session_manager,
);

// Full defaults: localhost origin + in-memory sessions
let server = StreamableHttpServer::default_configured(
    addr,
    handler,
);
```

## Middleware Details

### Origin Guard Middleware

**Purpose**: Prevent DNS rebinding attacks by validating the Origin header

**Behavior**:
- Extracts `Origin` or `Referer` header
- Checks against allowlist
- Returns 403 Forbidden if invalid
- Stores validated origin in request extensions

**Security Note**: Always use explicit origin allowlists in production. Never use wildcard (`*`).

### Session Middleware

**Purpose**: Manage request-scoped sessions identified by `MCP-Session-Id`

**Behavior**:
- Extracts or generates session ID from `MCP-Session-Id` header
- Gets or creates session from SessionManager
- Touches session (updates `last_accessed`) on each request
- Injects RequestContext into route handlers
- Adds `MCP-Session-Id` to response headers

**RequestContext Structure**:
```rust
pub struct RequestContext {
    pub session_id: String,
    pub origin: Option<String>,
    pub session: Option<Session>,
}
```

## Route Handlers

### POST /mcp - Request/Response

**Handler**: `handle_mcp_post`

**Input**: JSON-RPC 2.0 request body

**Process**:
1. Validates Origin (origin_guard_middleware)
2. Gets/creates session (session_middleware)
3. Injects RequestContext
4. Calls McpTaskHandler.handle_request()
5. Returns JSON-RPC 2.0 response
6. Adds MCP-Session-Id header to response

**Status Codes**:
- `200 OK` - Request processed successfully
- `403 Forbidden` - Invalid origin
- `500 Internal Server Error` - Processing failed

### GET /mcp - SSE Streaming

**Handler**: `handle_mcp_sse`

**Input**: Optional `request` query parameter (JSON-RPC request as URL-encoded JSON)

**Process**:
1. Validates Origin (origin_guard_middleware)
2. Gets/creates session (session_middleware)
3. Injects RequestContext
4. Extracts Last-Event-ID for resumption
5. Creates SSE stream with broadcast channel
6. Spawns keep-alive task
7. Returns SSE response with event stream

**Query Parameters**:
- `request`: Optional JSON-RPC 2.0 request (URL-encoded)

**Events**:
- `mcp-response`: JSON-RPC response (if request provided)
- `keep-alive`: Empty event every 30 seconds
- `error`: Error event if streaming fails

**Headers**:
- `Content-Type: text/event-stream`
- `Cache-Control: no-cache`
- `Connection: keep-alive`
- `MCP-Session-Id`: Session identifier

## Error Handling

### Origin Validation Errors

```
403 Forbidden
{
  "jsonrpc": "2.0",
  "id": null,
  "error": {
    "code": -32000,
    "message": "Origin forbidden",
    "data": "Origin 'https://evil.com' is not in the allowlist"
  }
}
```

### JSON-RPC Processing Errors

```
500 Internal Server Error
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32603,
    "message": "Internal error",
    "data": "Error details..."
  }
}
```

## Integration with McpTaskHandler

The server wires JSON-RPC requests directly to `McpTaskHandler`:

```rust
// In handler
let response = state.handler.handle_request(request).await;
```

McpTaskHandler supports:
- `tasks/get` - Retrieve task by ID
- `tasks/result` - Get task result
- `tasks/list` - List all tasks
- `tasks/cancel` - Cancel task execution

See [MCP Task Handler Documentation](../src/application/mcp_task_handlers.rs) for details.

## Tracing and Observability

When the `tracing` feature is enabled, the server logs:

```rust
#[cfg(feature = "tracing")]
use tracing::{debug, error, info, instrument, warn};
```

**Log Levels**:
- `info!` - Server startup, major events
- `debug!` - Request details, session operations
- `warn!` - Origin validation failures
- `error!` - Processing failures

**Instrumented Functions**:
- `start()` - Server startup (address, origins)
- `origin_guard_middleware()` - Origin validation
- `session_middleware()` - Session management
- `handle_mcp_post()` - POST request handling
- `handle_mcp_sse()` - SSE streaming

Example log output:
```
Starting MCP Streamable HTTP server on 127.0.0.1:3000
MCP server listening on 127.0.0.1:3000
POST request: method=tasks/get, id=Some(Number(1))
Session ID: session-abc123
Origin validation passed
```

## Testing

Example curl tests:

**POST Request/Response**:
```bash
curl -X POST http://localhost:3000/mcp \
  -H 'Content-Type: application/json' \
  -H 'Origin: http://localhost:3000' \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tasks/list",
    "params": null
  }'
```

**SSE Streaming**:
```bash
curl -N -H 'Origin: http://localhost:3000' \
  http://localhost:3000/mcp
```

**With Session ID**:
```bash
curl -X POST http://localhost:3000/mcp \
  -H 'Content-Type: application/json' \
  -H 'Origin: http://localhost:3000' \
  -H 'MCP-Session-Id: session-abc123' \
  -d '{"jsonrpc": "2.0", "id": 1, "method": "tasks/list", "params": null}'
```

## Security Considerations

### 1. Origin Validation
- **Always** configure explicit origin allowlist
- Never use wildcard (`*`) in production
- Validate against both schemes (http/https)

### 2. Session Management
- Sessions are in-memory by default
- Production should use persistent storage
- Implement SessionManager trait for custom backends
- Configure TTL for session cleanup

### 3. HTTPS
- Use HTTPS in production to prevent origin header spoofing
- Configure proper TLS certificates
- Use `https://` origins in allowlist

### 4. Rate Limiting
- Implement rate limiting middleware if needed
- Consider connection limits
- Monitor for abuse patterns

## Performance

### Connection Handling
- Each GET request creates independent SSE connection
- Keep-alive events maintain connection
- Automatic cleanup on client disconnect

### Memory Usage
- SSE events buffered in bounded channel (100 events default)
- Sessions stored in memory (use persistent backend for scale)
- Each session ~500 bytes baseline

### Scalability
For production deployments:
1. Use persistent SessionManager implementation (Redis, database)
2. Run multiple server instances behind load balancer
3. Implement connection pooling
4. Monitor resource usage

## Troubleshooting

### "Origin forbidden" errors
- Check that Origin header is included in request
- Verify origin matches allowlist exactly
- Remember: origins are case-sensitive

### SSE connection drops
- Check keep-alive events are being sent
- Verify client supports long-polling
- Check browser dev tools for network issues

### Session not persisted
- Default InMemorySessionManager doesn't persist
- Implement custom SessionManager for persistence
- Check SessionManager::touch_session is called

## Files

- `src/server/mod.rs` - Module exports
- `src/server/streamable_http_server.rs` - Main server implementation
- `src/server/rmcp_a2a_server.rs` - Legacy RMCP A2A server
- `examples/streamable_http_server_demo.rs` - Complete example
