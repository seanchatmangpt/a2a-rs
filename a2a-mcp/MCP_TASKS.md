# MCP Tasks Primitive

## Overview

This implementation provides MCP (Model Context Protocol) tasks primitive support, enabling long-running operations to be wrapped into durable task IDs with polling and result retrieval capabilities.

The implementation bridges MCP tasks to the A2A (Agent-to-Agent) task model, following hexagonal architecture principles.

## Architecture

### Hexagonal Architecture Layers

```
┌─────────────────────────────────────────────────────────┐
│                    Domain Layer                          │
│  • McpTask - MCP task representation                     │
│  • McpTaskState - Task state enumeration                 │
│  • McpTaskError - Task error representation              │
│  • McpTaskResult - Task result DTO                       │
└─────────────────────────────────────────────────────────┘
                           ▲
                           │
┌─────────────────────────────────────────────────────────┐
│                     Port Layer                           │
│  • McpTaskManager trait - Task management contract       │
│    - create_task()                                       │
│    - get_task()                                          │
│    - get_task_result()                                   │
│    - cancel_task()                                       │
│    - list_tasks()                                        │
│    - cleanup_old_tasks()                                 │
└─────────────────────────────────────────────────────────┘
                           ▲
                           │
┌─────────────────────────────────────────────────────────┐
│                   Adapter Layer                          │
│  • TaskWrapper - McpTaskManager implementation           │
│    - In-memory task registry                             │
│    - Background task execution                           │
│    - A2A task integration (optional)                     │
│    - Task lifecycle management                           │
└─────────────────────────────────────────────────────────┘
                           ▲
                           │
┌─────────────────────────────────────────────────────────┐
│                 Application Layer                        │
│  • McpTaskHandler - JSON-RPC request handler             │
│    - tasks/get endpoint                                  │
│    - tasks/result endpoint                               │
│    - tasks/list endpoint                                 │
│    - tasks/cancel endpoint                               │
└─────────────────────────────────────────────────────────┘
```

## Features

### 1. Durable Task IDs
- UUID-based task identification
- Tasks persist beyond the initial request
- Support for long-running operations

### 2. Task States
- **Pending**: Task created, waiting to execute
- **Running**: Task is executing
- **Completed**: Task finished successfully
- **Failed**: Task encountered an error
- **Cancelled**: Task was cancelled

### 3. JSON-RPC Methods

#### `tasks/get`
Retrieve the current state of a task.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tasks/get",
  "params": {
    "taskId": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "state": "running",
    "createdAt": "2026-02-09T12:00:00Z",
    "updatedAt": "2026-02-09T12:00:01Z"
  }
}
```

#### `tasks/result`
Retrieve the result of a completed task.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tasks/result",
  "params": {
    "taskId": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

**Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "taskId": "550e8400-e29b-41d4-a716-446655440000",
    "state": "completed",
    "result": {
      "status": "success",
      "data": "Processing complete"
    }
  }
}
```

**Response (Failure):**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "taskId": "550e8400-e29b-41d4-a716-446655440000",
    "state": "failed",
    "error": {
      "code": -32000,
      "message": "Task processing error"
    }
  }
}
```

#### `tasks/list`
List all tasks.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tasks/list"
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": [
    {
      "id": "task-1",
      "state": "completed",
      "createdAt": "2026-02-09T12:00:00Z",
      "updatedAt": "2026-02-09T12:00:05Z"
    },
    {
      "id": "task-2",
      "state": "running",
      "createdAt": "2026-02-09T12:00:10Z",
      "updatedAt": "2026-02-09T12:00:11Z"
    }
  ]
}
```

#### `tasks/cancel`
Cancel a running task.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tasks/cancel",
  "params": {
    "taskId": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": null
}
```

### 4. A2A Integration

The `TaskWrapper` can optionally integrate with A2A's `AsyncTaskManager`:

- MCP tasks are automatically synced to A2A tasks
- State mapping between MCP and A2A task states
- Unified task tracking across protocols

**State Mapping:**
| MCP State   | A2A State      |
|-------------|----------------|
| Pending     | Submitted      |
| Running     | Working        |
| Completed   | Completed      |
| Failed      | Failed         |
| Cancelled   | Canceled       |

## Usage

### Basic Usage

```rust
use a2a_mcp::{TaskWrapper, McpTaskManager};
use serde_json::json;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Create task wrapper
    let wrapper = Arc::new(TaskWrapper::new());

    // Create a task
    let task = wrapper.create_task(|| async {
        // Long-running operation
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        Ok(json!({"result": "success"}))
    }).await.unwrap();

    println!("Task created: {}", task.id);

    // Poll task status
    let status = wrapper.get_task(&task.id).await.unwrap();
    println!("Task state: {:?}", status.state);

    // Wait for completion
    tokio::time::sleep(tokio::time::Duration::from_secs(6)).await;

    // Get result
    let result = wrapper.get_task_result(&task.id).await.unwrap();
    println!("Task result: {:?}", result.result);
}
```

### With A2A Integration

```rust
use a2a_mcp::TaskWrapper;
use a2a_rs::port::task_manager::AsyncTaskManager;
use std::sync::Arc;

// Assuming you have an A2A task manager
let a2a_manager: Arc<dyn AsyncTaskManager> = // ... your implementation

// Create wrapper with A2A integration
let wrapper = TaskWrapper::with_a2a_manager(
    a2a_manager,
    "mcp-context".to_string()  // Default context ID for A2A tasks
);

// Tasks are now automatically synced to A2A
let task = wrapper.create_task(|| async {
    Ok(serde_json::json!({"status": "done"}))
}).await.unwrap();
```

### JSON-RPC Handler

```rust
use a2a_mcp::{McpTaskHandler, JsonRpcRequest};
use serde_json::json;
use std::sync::Arc;

let handler = McpTaskHandler::new(wrapper);

let request = JsonRpcRequest {
    jsonrpc: "2.0".to_string(),
    id: Some(json!(1)),
    method: "tasks/get".to_string(),
    params: Some(json!({"taskId": "task-123"})),
};

let response = handler.handle_request(request).await;
println!("Response: {:?}", response);
```

## Running the Example

```bash
cargo run --example mcp_tasks_example
```

The example demonstrates:
1. Creating long-running tasks
2. Polling task status
3. Retrieving results
4. Listing all tasks
5. Cancelling tasks
6. Error handling
7. Task cleanup

## Implementation Details

### Task Execution

Tasks are executed in background Tokio tasks:
- Each task gets a unique UUID
- Task state is tracked in a thread-safe `HashMap`
- Task handles are stored for cancellation support
- Completed/failed tasks can be cleaned up by age

### Concurrency

- `RwLock` for task registry (multiple readers, single writer)
- `Mutex` for task handle management
- Async-first design throughout

### Error Handling

- Tasks can fail with rich error information
- Error codes follow JSON-RPC conventions
- Task errors are preserved in `McpTaskError`

### Cleanup

```rust
// Clean up tasks older than 1 hour
let count = wrapper.cleanup_old_tasks(3600).await?;
println!("Cleaned up {} old tasks", count);
```

## Testing

Run tests with:

```bash
cargo test -p a2a-mcp
```

The implementation includes comprehensive tests for:
- Task creation and retrieval
- Task cancellation
- Task failure handling
- Task listing
- Task cleanup
- JSON-RPC handler methods

## Future Enhancements

Potential improvements for future versions:

1. **Persistence**: Store tasks in database for durability across restarts
2. **Pagination**: Add pagination support to `tasks/list`
3. **Filtering**: Filter tasks by state, creation time, etc.
4. **Streaming**: Support SSE/WebSocket for real-time task updates
5. **Priority**: Task priority and queue management
6. **Dependencies**: Task dependency graphs
7. **Timeouts**: Automatic timeout for long-running tasks
8. **Metrics**: Task execution metrics and monitoring

## References

- [A2A Protocol Specification](../../spec/)
- [Model Context Protocol (MCP)](https://modelcontextprotocol.io/)
- [Hexagonal Architecture](../../.claude/rules/architecture.md)
