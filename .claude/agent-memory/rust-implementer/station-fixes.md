# Station Trait Implementation - Type Corrections

## Key type structure differences found:

1. **Message**: Has `parts: Vec<Part>`, not a `content` field
   - Check with `input.params.message.parts.is_empty()`
   
2. **Task structure**:
   - Uses builder pattern: `Task::builder().id().context_id().status().build()`
   - Has `id`, `context_id`, and `status` (which contains `state`)
   - Not a simple `new(id, state, messages)` method

3. **TaskQuery/TaskIdParams**: Use `id` field, not `task_id`

4. **Response types**: All have `Option<T>` for result and `Option<JsonRpcError>` for error
   - Example: `SendMessageResponse { jsonrpc, id, result: Some(task), error: None }`

5. **TaskState**: Accessed via `task.status.state`, not `task.state`
   - Need to import `TaskStatus` as well

6. **ListTasksParams**: Is `Option<ListTasksParams>` in request, not required

7. **Task ID location**: Can be in `input.params.message.task_id` OR `input.params.task_id`

## Imports needed:
```rust
use crate::domain::{AgentCard, Task, TaskState, TaskStatus};
```

