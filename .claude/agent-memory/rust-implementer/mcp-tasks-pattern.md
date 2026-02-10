# MCP Tasks Implementation Pattern

## Overview
Implemented MCP (Model Context Protocol) tasks primitive that bridges long-running operations to durable task IDs, with full A2A task integration.

## Key Learnings

### Object-Safe Traits
When creating port traits that need to work with `Arc<dyn Trait>`:
- Avoid generic type parameters in trait methods
- Use boxed futures: `Box<dyn FnOnce() -> Pin<Box<dyn Future>>>`
- Provide convenience methods on concrete implementations for ergonomic usage

Example:
```rust
// Port trait - object-safe
#[async_trait]
pub trait McpTaskManager: Send + Sync {
    async fn create_task_boxed(&self, operation: BoxedTaskOperation) -> Result<McpTask>;
}

// Adapter - ergonomic wrapper
impl TaskWrapper {
    pub async fn create_task<F, Fut>(&self, operation: F) -> Result<McpTask>
    where F: FnOnce() -> Fut + Send + 'static,
          Fut: Future<Output = Result<Value>> + Send + 'static
    {
        let boxed_op = Box::new(move || Box::pin(operation()));
        self.create_task_boxed(boxed_op).await
    }
}
```

### Background Task Execution
For long-running operations in tasks:
- Clone task_id before moving into async block
- Use `RwLock` for shared state (multiple readers, single writer)
- Store task handles in `Mutex<HashMap>` for cancellation support
- Always handle task completion/failure in the background task

### State Mapping Between Protocols
When bridging two task models:
- Create explicit mapping functions (mcp_to_a2a_state, a2a_to_mcp_state)
- Handle all enum variants, even if mapping is lossy
- Document the mapping decisions

### JSON-RPC Handlers
Pattern for request handlers:
- Parse params with helpful error messages
- Map domain errors to JSON-RPC error codes
- Include error context in error responses
- Test all endpoints including error cases

## Files Created
- Domain: `domain/mcp_task.rs`
- Port: `port/mcp_task_manager.rs`
- Adapter: `adapter/task_wrapper.rs`
- Application: `application/mcp_task_handlers.rs`
- Example: `examples/mcp_tasks_example.rs`
- Docs: `MCP_TASKS.md`, `IMPLEMENTATION_SUMMARY.md`

## Testing Strategy
- Unit tests for each layer
- Integration tests for JSON-RPC handlers
- Example code that demonstrates all features
- Test error cases, not just happy path

## Common Pitfalls Avoided
1. Making traits non-object-safe with generics
2. Moving values into async blocks without cloning
3. Forgetting to sync state changes to external systems (A2A)
4. Not handling task cleanup (memory leaks)
5. Calling futures twice (operation()() instead of operation().await)
