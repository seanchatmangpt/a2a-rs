# MCP Session Management

This document describes the session management implementation for the Model Context Protocol (MCP) integration in `a2a-mcp`.

## Overview

The session management system provides thread-safe, concurrent handling of MCP sessions using the hexagonal architecture pattern. Sessions are identified by the `MCP-Session-Id` header and maintain state across multiple requests.

## Architecture

Following hexagonal architecture principles:

```
domain/session.rs          → Session domain type (pure data)
    ↓
port/session_manager.rs    → SessionManager trait (interface)
    ↓
adapter/session_manager.rs → InMemorySessionManager (implementation)
```

### Layer Responsibilities

1. **Domain Layer** (`domain/session.rs`)
   - Defines the `Session` type with no external dependencies
   - Contains business logic for session expiration, age calculation
   - Serializable with serde for persistence

2. **Port Layer** (`port/session_manager.rs`)
   - Defines the `SessionManager` trait
   - Specifies async interface for session operations
   - Independent of implementation details

3. **Adapter Layer** (`adapter/session_manager.rs`)
   - Implements `SessionManager` using `Arc<RwLock<HashMap>>`
   - Provides thread-safe concurrent access
   - Optimized read-write locking strategy

## Session Structure

```rust
pub struct Session {
    /// Unique session identifier (MCP-Session-Id header value)
    pub id: String,

    /// Timestamp when session was created
    pub created_at: DateTime<Utc>,

    /// Timestamp of last access
    pub last_accessed: DateTime<Utc>,

    /// Optional session metadata (e.g., client info)
    pub metadata: Option<Value>,

    /// Session-specific state data
    pub state: Option<Value>,

    /// Time-to-live in seconds (None = no expiration)
    pub ttl_seconds: Option<u64>,
}
```

## SessionManager Trait

The `SessionManager` trait provides the following async operations:

### Core Operations

- `create_session(session_id)` - Create a new session
- `get_session(session_id)` - Retrieve a session by ID
- `get_or_create_session(session_id)` - Atomic get-or-create operation
- `update_session(session)` - Update an existing session
- `touch_session(session_id)` - Update last accessed timestamp
- `delete_session(session_id)` - Remove a session

### Management Operations

- `cleanup_expired_sessions()` - Remove all expired sessions
- `list_sessions()` - Get all active session IDs
- `count_sessions()` - Get count of active sessions

## InMemorySessionManager

The default implementation uses `Arc<RwLock<HashMap<String, Session>>>` for:

- **Thread Safety**: Multiple threads can safely access sessions concurrently
- **Clone Efficiency**: `Arc` allows cheap cloning for sharing across tasks
- **Read Optimization**: Multiple readers can access simultaneously
- **Write Safety**: Exclusive write access when modifying sessions

### Concurrency Strategy

- Read operations use `.read().await` for concurrent access
- Write operations use `.write().await` for exclusive access
- `get_or_create_session` uses double-checked locking pattern for efficiency

## Usage Examples

### Basic Session Management

```rust
use a2a_mcp::{InMemorySessionManager, SessionManager};

let manager = InMemorySessionManager::new();

// Get or create session from MCP-Session-Id header
let session_id = "client-session-123";
let (session, created) = manager
    .get_or_create_session(session_id.to_string())
    .await?;

if created {
    println!("Created new session");
}
```

### Storing Session State

```rust
let mut session = manager.get_session(&session_id).await?.unwrap();

// Store conversation context
session.state = Some(serde_json::json!({
    "conversation": ["Hello", "How can I help?"],
    "tool_state": {
        "working_directory": "/home/user",
        "last_tool": "list_files"
    }
}));

manager.update_session(session).await?;
```

### HTTP Integration with Axum

```rust
use axum::{extract::State, http::HeaderMap};

async fn handler(
    State(manager): State<Arc<InMemorySessionManager>>,
    headers: HeaderMap,
) -> Result<String, StatusCode> {
    // Extract MCP-Session-Id header
    let session_id = headers
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Get or create session
    let (session, _) = manager
        .get_or_create_session(session_id.to_string())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Touch session to update last accessed
    manager.touch_session(&session.id).await.ok();

    Ok(format!("Session: {}", session.id))
}
```

### Background Cleanup

```rust
let cleanup_manager = manager.clone();
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        if let Ok(count) = cleanup_manager.cleanup_expired_sessions().await {
            if count > 0 {
                tracing::info!("Cleaned up {} expired sessions", count);
            }
        }
    }
});
```

## Session Lifecycle

```
1. Client sends request with MCP-Session-Id header
   ↓
2. Server extracts session ID from header
   ↓
3. SessionManager.get_or_create_session(id)
   ↓
4. If new: create Session with current timestamp
   If exists: return existing Session
   ↓
5. Process request, update session state if needed
   ↓
6. Touch session to update last_accessed
   ↓
7. Return response with same MCP-Session-Id header
```

## Error Handling

The session manager uses specific error types:

- `SessionNotFound` - Session doesn't exist
- `SessionAlreadyExists` - Attempting to create duplicate session
- `Session` - General session error

All operations return `Result<T, Error>` for proper error propagation.

## Testing

Comprehensive tests cover:

- Session creation and retrieval
- Concurrent access from multiple tasks
- Session expiration and cleanup
- State management and updates
- Edge cases (duplicate creation, missing sessions)

Run tests:
```bash
cargo test -p a2a-mcp session_manager
```

## Examples

Two example programs demonstrate the session management:

### 1. Session Management Demo
```bash
cargo run -p a2a-mcp --example session_management_demo
```

Demonstrates:
- Creating and managing sessions
- Storing state and metadata
- Concurrent access patterns
- Session cleanup

### 2. HTTP Session Handling
```bash
cargo run -p a2a-mcp --example http_session_handling
```

Demonstrates:
- Full HTTP server with MCP-Session-Id support
- RESTful API for session operations
- Background cleanup task
- Real-world integration patterns

Test with curl:
```bash
# Get or create session
curl -H 'MCP-Session-Id: my-session' http://localhost:3001/session

# Store data in session
curl -X POST \
  -H 'MCP-Session-Id: my-session' \
  -H 'Content-Type: application/json' \
  -d '{"key":"foo","value":"bar"}' \
  http://localhost:3001/session/data

# Retrieve session data
curl -H 'MCP-Session-Id: my-session' http://localhost:3001/session/data

# Delete session
curl -X DELETE -H 'MCP-Session-Id: my-session' http://localhost:3001/session

# List all sessions
curl http://localhost:3001/sessions
```

## Future Enhancements

Potential improvements for production use:

1. **Persistent Storage Adapter**
   - Redis-backed session manager
   - Database-backed session manager
   - Distributed session support

2. **Session Security**
   - Session token validation
   - CSRF protection
   - Secure session ID generation

3. **Advanced Features**
   - Session migration between servers
   - Session replication
   - Custom serialization strategies
   - Session event hooks (on create, update, delete)

4. **Monitoring**
   - Session metrics (active count, creation rate)
   - Expiration statistics
   - Memory usage tracking

## Performance Considerations

- **Read-heavy workloads**: `RwLock` provides excellent read concurrency
- **Write-heavy workloads**: Consider sharding or lock-free data structures
- **Memory**: Sessions are kept in memory; monitor total size
- **Cleanup frequency**: Adjust based on TTL patterns and memory constraints

## References

- [MCP Specification](https://spec.modelcontextprotocol.io/)
- [Hexagonal Architecture](../../.claude/rules/architecture.md)
- [Rust Async Patterns](https://rust-lang.github.io/async-book/)
