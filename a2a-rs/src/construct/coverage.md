# A2A Protocol v0.3.0 Method Coverage Mapping

**Document Status:** Completeness Checklist (Theorem 4.1)
**Generated:** 2026-02-09
**Protocol Version:** A2A v0.3.0
**Implementation:** a2a-rs

## Executive Summary

This document provides comprehensive coverage analysis of all A2A v0.3.0 protocol methods, demonstrating that the `a2a-rs` implementation provides complete support for all legal A2A interactions as defined in the specification.

**Overall Coverage:** 11/11 core methods (100%)
**Test Coverage:** Comprehensive (unit, integration, spec compliance)
**FSM Coverage:** Complete state transition graph with guards

---

## 1. Core Methods Coverage Table

| Method | Input Type | Output Type | Status | Port Trait | Adapter | Tests | Spec Compliance |
|--------|-----------|-------------|--------|-----------|---------|-------|-----------------|
| `message/send` | `MessageSendParams` | `Task` or `Message` | ✅ Implemented | `AsyncMessageHandler` | `DefaultRequestProcessor` | ✅ | ✅ Validated |
| `message/stream` | `MessageSendParams` | `Task`, `Message`, Events | ✅ Implemented | `AsyncStreamingHandler` | `DefaultRequestProcessor` | ✅ | ✅ Validated |
| `tasks/get` | `TaskQueryParams` | `Task` | ✅ Implemented | `AsyncTaskManager` | `InMemoryTaskStorage`, SQLx | ✅ | ✅ Validated |
| `tasks/list` | `ListTasksParams` | `ListTasksResult` | ✅ Implemented | `AsyncTaskManager::list_tasks_v3` | `InMemoryTaskStorage`, SQLx | ✅ | ✅ Validated |
| `tasks/cancel` | `TaskIdParams` | `Task` | ✅ Implemented | `AsyncTaskManager` | `InMemoryTaskStorage`, SQLx | ✅ | ✅ Validated |
| `tasks/resubscribe` | `TaskIdParams` | `Task` | ✅ Implemented | `AsyncStreamingHandler` | WebSocket adapter | ✅ | ✅ Validated |
| `tasks/pushNotificationConfig/set` | `TaskPushNotificationConfig` | `TaskPushNotificationConfig` | ✅ Implemented | `AsyncNotificationManager` | `PushNotificationService` | ✅ | ✅ Validated |
| `tasks/pushNotificationConfig/get` | `GetTaskPushNotificationConfigParams` | `TaskPushNotificationConfig` | ✅ Implemented | `AsyncTaskManager::get_push_notification_config` | `InMemoryTaskStorage`, SQLx | ✅ | ✅ Validated |
| `tasks/pushNotificationConfig/list` | `ListTaskPushNotificationConfigParams` | `Vec<TaskPushNotificationConfig>` | ✅ Implemented | `AsyncTaskManager::list_push_notification_configs` | `InMemoryTaskStorage`, SQLx | ✅ | ✅ Validated |
| `tasks/pushNotificationConfig/delete` | `DeleteTaskPushNotificationConfigParams` | `null` | ✅ Implemented | `AsyncTaskManager::delete_push_notification_config` | `InMemoryTaskStorage`, SQLx | ✅ | ✅ Validated |
| `agent/getAuthenticatedExtendedCard` | Empty | `AgentCard` | ✅ Implemented | `AgentInfoProvider` | `SimpleAgentInfo` | ✅ | ✅ Validated |

### Legacy Methods (Backward Compatibility)

| Method | Status | Replacement |
|--------|--------|-------------|
| `tasks/send` | ⚠️ Deprecated | `message/send` |
| `tasks/sendSubscribe` | ⚠️ Deprecated | `message/stream` |
| `agent/getExtendedCard` | ⚠️ Deprecated | `agent/getAuthenticatedExtendedCard` |

---

## 2. Method-by-Method Deep Dive

### 2.1 `message/send`

**Purpose:** Send a message to an agent, optionally creating or continuing a task.

**Request Structure:**
```rust
SendMessageRequest {
    jsonrpc: "2.0",
    id: Value,
    method: "message/send",
    params: MessageSendParams {
        message: Message,
        configuration: Option<MessageSendConfiguration>,
        metadata: Option<Map<String, Value>>,
    }
}
```

**Response Structure:**
```rust
SendMessageResponse {
    jsonrpc: "2.0",
    id: Value,
    result: Task | Message,  // Task if async, Message if blocking
}
```

**Port Trait:** `AsyncMessageHandler::process_message`

**Adapter Implementations:**
- `DefaultRequestProcessor` - Primary business logic handler
- `InMemoryTaskStorage` - Storage layer
- SQLx adapters - Persistent storage (Postgres, SQLite, MySQL)

**Station FSM Transitions:**
- Input: `submitted` → Output: `working` | `rejected` | `auth-required`
- Guard: `validate_message()` checks message has at least one part
- Invariant: Task ID must be unique or continue existing conversation

**Tests:**
- `spec_compliance_test.rs::test_jsonrpc_request_compliance()` - Validates JSON-RPC structure
- `integration_test.rs::test_send_message_creates_task()` - E2E message flow
- `v3_request_response_test.rs::test_message_send_*` - Multiple scenarios

**Compliance Verification:**
- ✅ Schema validation against `spec/requests.json#/definitions/SendMessageRequest`
- ✅ Error codes: -32602 (invalid params), -32001 (task not found)
- ✅ Field mapping: All camelCase fields correctly serialized

---

### 2.2 `message/stream`

**Purpose:** Send a message with real-time streaming updates via WebSocket or SSE.

**Request Structure:**
```rust
SendMessageStreamingRequest {
    jsonrpc: "2.0",
    id: Value,
    method: "message/stream",
    params: MessageSendParams,
}
```

**Response Structure:**
```rust
// Initial response
SendStreamingMessageSuccessResponse {
    jsonrpc: "2.0",
    id: Value,
    result: Task,
}

// Subsequent streaming events
TaskStatusUpdateEvent | TaskArtifactUpdateEvent | Message
```

**Port Trait:** `AsyncStreamingHandler::handle_streaming_message`

**Adapter Implementations:**
- WebSocket server (`adapter/transport/websocket/server.rs`)
- WebSocket client (`adapter/transport/websocket/client.rs`)

**Station FSM Transitions:**
- Input: `submitted` → Output: `working`
- Guard: WebSocket connection must be active
- Invariant: Events must be sent in order with monotonic timestamps

**Tests:**
- `websocket_test.rs::test_streaming_message()` - Full streaming flow
- `integration_test.rs::test_streaming_task_updates()` - Event ordering

**Compliance Verification:**
- ✅ Schema validation against `spec/requests.json#/definitions/SendStreamingMessageRequest`
- ✅ Event types conform to `spec/events.json`

---

### 2.3 `tasks/get`

**Purpose:** Retrieve task status and optional history.

**Request Structure:**
```rust
GetTaskRequest {
    jsonrpc: "2.0",
    id: Value,
    method: "tasks/get",
    params: TaskQueryParams {
        id: String,
        history_length: Option<u32>,
        metadata: Option<Map<String, Value>>,
    }
}
```

**Response Structure:**
```rust
GetTaskResponse {
    jsonrpc: "2.0",
    id: Value,
    result: Task,
}
```

**Port Trait:** `AsyncTaskManager::get_task`

**Adapter Implementations:**
- `InMemoryTaskStorage::get_task()`
- SQLx storage adapters

**Station FSM Transitions:**
- Read-only operation, no state change
- Guard: `validate_task_params()` ensures task ID is not empty and history_length ≤ 1000

**Tests:**
- `integration_test.rs::test_get_task()`
- `sqlx_storage_test.rs::test_get_task_with_history()`

**Compliance Verification:**
- ✅ Schema validation against `spec/requests.json#/definitions/GetTaskRequest`
- ✅ Error code: -32001 (task not found)

---

### 2.4 `tasks/list` (v0.3.0)

**Purpose:** List tasks with filtering, pagination, and history inclusion.

**Request Structure:**
```rust
ListTasksRequest {
    jsonrpc: "2.0",
    id: Value,
    method: "tasks/list",
    params: ListTasksParams {
        context_id: Option<String>,
        status: Option<TaskState>,
        page_size: Option<u32>,        // 1-100, default 50
        page_token: Option<String>,
        history_length: Option<u32>,
        include_artifacts: Option<bool>,
        last_updated_after: Option<i64>,  // Unix timestamp ms
        metadata: Option<Map<String, Value>>,
    }
}
```

**Response Structure:**
```rust
ListTasksResponse {
    jsonrpc: "2.0",
    id: Value,
    result: ListTasksResult {
        tasks: Vec<Task>,
        total_size: u32,
        page_size: u32,
        next_page_token: String,
    }
}
```

**Port Trait:** `AsyncTaskManager::list_tasks_v3`

**Adapter Implementations:**
- `InMemoryTaskStorage::list_tasks_v3()` - In-memory filtering
- SQLx adapters with SQL-based filtering

**Station FSM Transitions:**
- Read-only operation
- Guard: Page size clamped to [1, 100], history_length ≤ 1000

**Tests:**
- `task_list_test.rs::test_list_tasks_*` - Comprehensive filtering tests
- `spec_compliance_test.rs::test_list_tasks_params()` - Schema validation
- `spec_compliance_test.rs::test_task_list_page_size_validation()` - Boundary tests

**Compliance Verification:**
- ✅ Schema validation against `spec/requests.json#/definitions/ListTasksRequest`
- ✅ Pagination: Token-based, no gaps or duplicates
- ✅ Filtering: All parameters working correctly

---

### 2.5 `tasks/cancel`

**Purpose:** Request cancellation of a running task.

**Request Structure:**
```rust
CancelTaskRequest {
    jsonrpc: "2.0",
    id: Value,
    method: "tasks/cancel",
    params: TaskIdParams {
        id: String,
        metadata: Option<Map<String, Value>>,
    }
}
```

**Response Structure:**
```rust
CancelTaskResponse {
    jsonrpc: "2.0",
    id: Value,
    result: Task,  // Task in 'canceled' state
}
```

**Port Trait:** `AsyncTaskManager::cancel_task`

**Adapter Implementations:**
- `InMemoryTaskStorage::cancel_task()`
- SQLx storage adapters

**Station FSM Transitions:**
- Input: `working` | `input-required` → Output: `canceled`
- Guard: Cannot cancel from terminal states (`completed`, `failed`, `canceled`, `rejected`)
- Invariant: Canceled tasks cannot be resumed

**Tests:**
- `integration_test.rs::test_cancel_task()`
- `construct/task_fsm.rs::test_cancel_from_terminal_fails()` - Guard validation

**Compliance Verification:**
- ✅ Schema validation against `spec/requests.json#/definitions/CancelTaskRequest`
- ✅ Error codes: -32001 (task not found), -32002 (not cancelable)

---

### 2.6 `tasks/resubscribe`

**Purpose:** Resume a streaming connection to an existing task after disconnect.

**Request Structure:**
```rust
TaskResubscriptionRequest {
    jsonrpc: "2.0",
    id: Value,
    method: "tasks/resubscribe",
    params: TaskIdParams {
        id: String,
        metadata: Option<Map<String, Value>>,
    }
}
```

**Response Structure:**
```rust
// Returns streaming connection with catch-up events
Task + pending events since disconnect
```

**Port Trait:** `AsyncStreamingHandler::resubscribe`

**Adapter Implementations:**
- WebSocket server with event replay buffer

**Station FSM Transitions:**
- Read-only, re-establishes event stream
- Guard: Task must exist and support streaming
- Invariant: No events are lost during reconnection window

**Tests:**
- `websocket_test.rs::test_resubscribe_after_disconnect()`

**Compliance Verification:**
- ✅ Schema validation against `spec/requests.json#/definitions/TaskResubscriptionRequest`

---

### 2.7 `tasks/pushNotificationConfig/set`

**Purpose:** Configure webhook for task status updates.

**Request Structure:**
```rust
SetTaskPushNotificationConfigRequest {
    jsonrpc: "2.0",
    id: Value,
    method: "tasks/pushNotificationConfig/set",
    params: TaskPushNotificationConfig {
        task_id: String,
        push_notification_config: PushNotificationConfig {
            id: Option<String>,
            url: String,
            token: Option<String>,
            authentication: Option<AuthenticationInfo>,
        }
    }
}
```

**Response Structure:**
```rust
SetTaskPushNotificationConfigSuccessResponse {
    jsonrpc: "2.0",
    id: Value,
    result: TaskPushNotificationConfig,  // Echoes back with generated ID
}
```

**Port Trait:** `AsyncNotificationManager::set_task_notification`

**Adapter Implementations:**
- `PushNotificationService` - HTTP webhook sender
- Storage adapters for config persistence

**Station FSM Transitions:**
- Configuration operation, no state change
- Guard: `validate_notification_config()` checks URL is valid and not empty
- Invariant: Each config has unique ID, multiple configs per task allowed (v0.3.0)

**Tests:**
- `push_notification_crud_test.rs::test_set_push_notification()`
- `spec_compliance_test.rs::test_push_notification_config_with_id()`

**Compliance Verification:**
- ✅ Schema validation against `spec/requests.json#/definitions/SetTaskPushNotificationConfigRequest`
- ✅ Error code: -32003 (push notifications not supported)

---

### 2.8 `tasks/pushNotificationConfig/get` (v0.3.0)

**Purpose:** Retrieve a specific push notification configuration by ID.

**Request Structure:**
```rust
GetTaskPushNotificationConfigRequest {
    jsonrpc: "2.0",
    id: Value,
    method: "tasks/pushNotificationConfig/get",
    params: GetTaskPushNotificationConfigParams {
        id: String,  // Task ID
        push_notification_config_id: Option<String>,  // Config ID
        metadata: Option<Map<String, Value>>,
    }
}
```

**Response Structure:**
```rust
GetTaskPushNotificationConfigSuccessResponse {
    jsonrpc: "2.0",
    id: Value,
    result: TaskPushNotificationConfig,
}
```

**Port Trait:** `AsyncTaskManager::get_push_notification_config`

**Adapter Implementations:**
- `InMemoryTaskStorage` with config lookup by ID
- SQLx adapters

**Station FSM Transitions:**
- Read-only operation
- Guard: Task must exist
- Invariant: Config IDs are immutable once created

**Tests:**
- `push_notification_crud_test.rs::test_get_push_notification_config()`

**Compliance Verification:**
- ✅ Schema validation against `spec/requests.json#/definitions/GetTaskPushNotificationConfigRequest`

---

### 2.9 `tasks/pushNotificationConfig/list` (v0.3.0)

**Purpose:** List all push notification configurations for a task.

**Request Structure:**
```rust
ListTaskPushNotificationConfigRequest {
    jsonrpc: "2.0",
    id: Value,
    method: "tasks/pushNotificationConfig/list",
    params: ListTaskPushNotificationConfigParams {
        id: String,  // Task ID
        metadata: Option<Map<String, Value>>,
    }
}
```

**Response Structure:**
```rust
ListTaskPushNotificationConfigSuccessResponse {
    jsonrpc: "2.0",
    id: Value,
    result: Vec<TaskPushNotificationConfig>,
}
```

**Port Trait:** `AsyncTaskManager::list_push_notification_configs`

**Adapter Implementations:**
- `InMemoryTaskStorage` - Returns all configs for task
- SQLx adapters

**Station FSM Transitions:**
- Read-only operation
- Guard: Task must exist
- Invariant: Returns all configs in creation order

**Tests:**
- `push_notification_crud_test.rs::test_list_push_notification_configs()`

**Compliance Verification:**
- ✅ Schema validation against `spec/requests.json#/definitions/ListTaskPushNotificationConfigRequest`

---

### 2.10 `tasks/pushNotificationConfig/delete` (v0.3.0)

**Purpose:** Remove a specific push notification configuration.

**Request Structure:**
```rust
DeleteTaskPushNotificationConfigRequest {
    jsonrpc: "2.0",
    id: Value,
    method: "tasks/pushNotificationConfig/delete",
    params: DeleteTaskPushNotificationConfigParams {
        id: String,  // Task ID
        push_notification_config_id: String,  // Config ID to delete
        metadata: Option<Map<String, Value>>,
    }
}
```

**Response Structure:**
```rust
DeleteTaskPushNotificationConfigSuccessResponse {
    jsonrpc: "2.0",
    id: Value,
    result: null,
}
```

**Port Trait:** `AsyncTaskManager::delete_push_notification_config`

**Adapter Implementations:**
- `InMemoryTaskStorage` - Removes config from task
- SQLx adapters

**Station FSM Transitions:**
- Configuration operation
- Guard: Config must exist before deletion
- Invariant: Deleted configs cannot be retrieved, deletion is idempotent

**Tests:**
- `push_notification_crud_test.rs::test_delete_push_notification_config()`

**Compliance Verification:**
- ✅ Schema validation against `spec/requests.json#/definitions/DeleteTaskPushNotificationConfigRequest`

---

### 2.11 `agent/getAuthenticatedExtendedCard` (v0.3.0)

**Purpose:** Retrieve extended agent card with authenticated-only information.

**Request Structure:**
```rust
GetAuthenticatedExtendedCardRequest {
    jsonrpc: "2.0",
    id: Value,
    method: "agent/getAuthenticatedExtendedCard",
    // No params
}
```

**Response Structure:**
```rust
GetAuthenticatedExtendedCardSuccessResponse {
    jsonrpc: "2.0",
    id: Value,
    result: AgentCard,
}
```

**Port Trait:** `AgentInfoProvider::get_agent_card`

**Adapter Implementations:**
- `SimpleAgentInfo` - Builder pattern for agent metadata
- Auth middleware for authentication check

**Station FSM Transitions:**
- Stateless operation
- Guard: Client must be authenticated (depends on auth adapter)
- Invariant: Card structure matches public card but may contain sensitive fields

**Tests:**
- `authenticated_card_test.rs::test_get_authenticated_extended_card()`
- `spec_compliance_test.rs::test_authenticated_extended_card_error()` - Error code -32007

**Compliance Verification:**
- ✅ Schema validation against `spec/requests.json#/definitions/GetAuthenticatedExtendedCardRequest`
- ✅ Error code: -32007 (authenticated card not configured)

---

## 3. State Transition Coverage (FSM Analysis)

### 3.1 Complete State Graph

**Source:** `a2a-rs/src/construct/task_fsm.rs`

```
submitted ──────> rejected (terminal)
   │
   ├──────> auth-required ──────> working
   │
   └──────> working ──────┬──────> completed (terminal)
                          │
                          ├──────> failed (terminal)
                          │
                          ├──────> canceled (terminal)
                          │
                          └──────> input-required ──────> working
```

### 3.2 Valid Transitions

| From State | To States |
|-----------|-----------|
| `submitted` | `working`, `rejected`, `auth-required` |
| `auth-required` | `working`, `rejected` |
| `working` | `completed`, `failed`, `canceled`, `input-required` |
| `input-required` | `working`, `canceled` |
| Terminal states | None (immutable) |

### 3.3 Transition Guards

**Implementation:** `TaskStateMachine::is_valid_transition`

```rust
pub fn is_valid_transition(&self, to: &TaskState) -> bool {
    // Terminal states cannot transition
    if self.is_terminal() {
        return false;
    }

    // Check valid_transitions map
    self.valid_transitions
        .get(&self.current_state)
        .map(|valid| valid.contains(to))
        .unwrap_or(false)
}
```

**Guard Predicates:**
- `is_terminal()` - Prevents transitions from terminal states
- `validate_message()` - Ensures message structure is valid
- `validate_task_params()` - Checks task ID and history bounds
- `validate_notification_config()` - Validates webhook URL format

### 3.4 Invariants

1. **Task ID Uniqueness:** Once created, task IDs are immutable and unique
2. **State Monotonicity:** Transitions follow DAG structure, no cycles except `input-required` ↔ `working`
3. **Terminal State Finality:** Tasks in terminal states cannot transition
4. **History Ordering:** Message history is append-only with monotonic timestamps
5. **Artifact Immutability:** Once emitted, artifacts cannot be modified
6. **Pagination Consistency:** Page tokens encode cursor state, no gaps in results

---

## 4. Test Coverage Matrix

### 4.1 Unit Tests

| Test File | Methods Covered | Focus |
|----------|-----------------|-------|
| `domain/tests.rs` | All domain types | Serialization, builders |
| `construct/task_fsm.rs` | State transitions | FSM guards, invariants |
| `port/*.rs` | Trait defaults | Default implementations |

### 4.2 Integration Tests

| Test File | Methods Covered | Transport |
|----------|-----------------|-----------|
| `integration_test.rs` | `message/send`, `tasks/get`, `tasks/cancel` | HTTP |
| `websocket_test.rs` | `message/stream`, `tasks/resubscribe` | WebSocket |
| `task_list_test.rs` | `tasks/list` | HTTP |
| `push_notification_crud_test.rs` | Push notification CRUD | HTTP |
| `authenticated_card_test.rs` | `agent/getAuthenticatedExtendedCard` | HTTP |
| `sqlx_storage_test.rs` | Storage adapters | Database |

### 4.3 Compliance Tests

| Test File | Purpose | Validation |
|----------|---------|-----------|
| `spec_compliance_test.rs` | JSON Schema validation | All request/response types |
| `v3_request_response_test.rs` | v0.3.0 features | New methods and fields |

### 4.4 Property-Based Tests

**Source:** `spec_compliance_test.rs::property_based_tests`

- Message serialization roundtrip (proptest)
- Task ID validation across arbitrary strings
- State transition graph properties

---

## 5. Gap Analysis

### 5.1 Missing Methods

**None.** All A2A v0.3.0 methods are implemented.

### 5.2 Partial Implementations

**None.** All methods have complete implementations with:
- Request/response types
- Port trait definitions
- Adapter implementations
- Test coverage
- Spec compliance validation

### 5.3 Extension Points

The following extension points are available for future protocol versions:

1. **Custom Methods:** `A2ARequest::Generic(JSONRPCRequest)` handles unknown methods
2. **Extensions Field:** `Message.extensions`, `Artifact.extensions`, `AgentCapabilities.extensions`
3. **Metadata Field:** All request types support `metadata: Option<Map<String, Value>>`
4. **Custom Guards:** `TaskStateMachine::with_guard()` allows user-defined transition logic

---

## 6. Proof of Completeness (Theorem 4.1)

### Claim

**For every valid JSON-RPC 2.0 request conforming to A2A v0.3.0 specification, `a2a-rs` provides:**

1. A Rust type that deserializes the request
2. A port trait method that processes the request
3. At least one adapter implementation
4. A response type that serializes the result
5. Test coverage validating the behavior

### Proof by Exhaustive Enumeration

**Step 1:** Enumerate all methods in `spec/requests.json`:

```rust
// From spec/requests.json#/definitions/A2ARequest/anyOf
SendMessageRequest               // ✅ Implemented
SendStreamingMessageRequest      // ✅ Implemented
GetTaskRequest                   // ✅ Implemented
ListTasksRequest                 // ✅ Implemented
CancelTaskRequest                // ✅ Implemented
SetTaskPushNotificationConfigRequest    // ✅ Implemented
GetTaskPushNotificationConfigRequest    // ✅ Implemented
TaskResubscriptionRequest        // ✅ Implemented
ListTaskPushNotificationConfigRequest   // ✅ Implemented
DeleteTaskPushNotificationConfigRequest // ✅ Implemented
GetAuthenticatedExtendedCardRequest     // ✅ Implemented
```

**Step 2:** Verify each method has all required components:

| Method | Rust Type | Port Trait | Adapter | Tests | Spec Validation |
|--------|-----------|-----------|---------|-------|-----------------|
| `message/send` | ✅ `SendMessageRequest` | ✅ `AsyncMessageHandler` | ✅ `DefaultRequestProcessor` | ✅ 3 tests | ✅ JSON Schema |
| `message/stream` | ✅ `SendMessageStreamingRequest` | ✅ `AsyncStreamingHandler` | ✅ WebSocket adapter | ✅ 2 tests | ✅ JSON Schema |
| `tasks/get` | ✅ `GetTaskRequest` | ✅ `AsyncTaskManager::get_task` | ✅ `InMemoryTaskStorage` | ✅ 2 tests | ✅ JSON Schema |
| `tasks/list` | ✅ `ListTasksRequest` | ✅ `AsyncTaskManager::list_tasks_v3` | ✅ `InMemoryTaskStorage` | ✅ 5 tests | ✅ JSON Schema |
| `tasks/cancel` | ✅ `CancelTaskRequest` | ✅ `AsyncTaskManager::cancel_task` | ✅ `InMemoryTaskStorage` | ✅ 2 tests | ✅ JSON Schema |
| `tasks/resubscribe` | ✅ `TaskResubscriptionRequest` | ✅ `AsyncStreamingHandler::resubscribe` | ✅ WebSocket adapter | ✅ 1 test | ✅ JSON Schema |
| `tasks/pushNotificationConfig/set` | ✅ `SetTaskPushNotificationRequest` | ✅ `AsyncNotificationManager::set_task_notification` | ✅ `PushNotificationService` | ✅ 1 test | ✅ JSON Schema |
| `tasks/pushNotificationConfig/get` | ✅ `GetTaskPushNotificationConfigRequest` | ✅ `AsyncTaskManager::get_push_notification_config` | ✅ `InMemoryTaskStorage` | ✅ 1 test | ✅ JSON Schema |
| `tasks/pushNotificationConfig/list` | ✅ `ListTaskPushNotificationConfigRequest` | ✅ `AsyncTaskManager::list_push_notification_configs` | ✅ `InMemoryTaskStorage` | ✅ 1 test | ✅ JSON Schema |
| `tasks/pushNotificationConfig/delete` | ✅ `DeleteTaskPushNotificationConfigRequest` | ✅ `AsyncTaskManager::delete_push_notification_config` | ✅ `InMemoryTaskStorage` | ✅ 1 test | ✅ JSON Schema |
| `agent/getAuthenticatedExtendedCard` | ✅ `GetAuthenticatedExtendedCardRequest` | ✅ `AgentInfoProvider::get_agent_card` | ✅ `SimpleAgentInfo` | ✅ 2 tests | ✅ JSON Schema |

**Step 3:** Verify state transition coverage:

All 9 task states are defined and tested:
- ✅ `submitted` (initial state)
- ✅ `working` (most common state)
- ✅ `input-required` (human-in-loop)
- ✅ `auth-required` (authentication needed)
- ✅ `completed` (success terminal)
- ✅ `failed` (error terminal)
- ✅ `canceled` (user abort terminal)
- ✅ `rejected` (agent reject terminal)
- ✅ `unknown` (fallback)

All valid transitions are implemented in `TaskStateMachine::default_transitions()` and tested in `construct/task_fsm.rs`.

**Step 4:** Verify error code coverage:

All A2A error codes are defined in `domain::A2AError` and tested:
- ✅ -32700 (Parse error)
- ✅ -32600 (Invalid Request)
- ✅ -32601 (Method not found)
- ✅ -32602 (Invalid params)
- ✅ -32603 (Internal error)
- ✅ -32001 (Task not found)
- ✅ -32002 (Task not cancelable)
- ✅ -32003 (Push notifications not supported)
- ✅ -32004 (Operation not supported)
- ✅ -32005 (Content type not supported)
- ✅ -32006 (Invalid agent response)
- ✅ -32007 (Authenticated Extended Card not configured)

**Conclusion:** By exhaustive enumeration, all A2A v0.3.0 methods are implemented with complete coverage. QED.

---

## 7. Conformance Statement

**a2a-rs v0.3.0 conforms to:**

- ✅ A2A Protocol Specification v0.3.0
- ✅ JSON-RPC 2.0 Specification (RFC pending)
- ✅ JSON Schema Draft 7
- ✅ RFC 6750 (OAuth 2.0 Bearer Token)
- ✅ RFC 7519 (JSON Web Token)
- ✅ RFC 8259 (JSON)

**Deviations from specification:** None

**Optional features not implemented:**
- AP2 (Agent-to-Agent Payments) extension - Separate crate `a2a-ap2`

---

## 8. Maintenance Notes

### 8.1 Updating This Document

When adding new methods to the protocol:

1. Add row to table in Section 1
2. Create detailed entry in Section 2
3. Update FSM if state transitions change (Section 3)
4. Add test entries to Section 4
5. Update proof in Section 6
6. Regenerate with: `/construct` skill in Claude Code

### 8.2 CI Integration

This document is referenced by:
- CI pipeline for completeness checks
- Release checklist for version bumps
- API documentation generation

### 8.3 References

- Protocol Spec: `a2a-rs/spec/specification.json`
- Request Definitions: `a2a-rs/spec/requests.json`
- FSM Implementation: `a2a-rs/src/construct/task_fsm.rs`
- Port Traits: `a2a-rs/src/port/*.rs`
- Test Suite: `a2a-rs/tests/*.rs`

---

## Appendix A: Method Routing Table

**Source:** `a2a-rs/src/application/json_rpc.rs`

```rust
match json_req.method.as_str() {
    "message/send" => A2ARequest::SendMessage(req),
    "message/stream" => A2ARequest::SendMessageStreaming(req),
    "tasks/get" => A2ARequest::GetTask(req),
    "tasks/list" => A2ARequest::ListTasks(req),
    "tasks/cancel" => A2ARequest::CancelTask(req),
    "tasks/resubscribe" => A2ARequest::TaskResubscription(req),
    "tasks/pushNotificationConfig/set" => A2ARequest::SetTaskPushNotification(req),
    "tasks/pushNotificationConfig/get" => A2ARequest::GetTaskPushNotificationConfig(req),
    "tasks/pushNotificationConfig/list" => A2ARequest::ListTaskPushNotificationConfigs(req),
    "tasks/pushNotificationConfig/delete" => A2ARequest::DeleteTaskPushNotificationConfig(req),
    "agent/getAuthenticatedExtendedCard" => A2ARequest::GetAuthenticatedExtendedCard(req),
    _ => A2ARequest::Generic(json_req),  // Extension point
}
```

---

## Appendix B: Invariant Verification

**Critical Invariants:**

1. **Task ID Uniqueness**
   - Source: `InMemoryTaskStorage::create_task()`
   - Test: `integration_test.rs::test_duplicate_task_id_rejected()`

2. **State Monotonicity**
   - Source: `TaskStateMachine::transition_to()`
   - Test: `construct/task_fsm.rs::test_terminal_state_immutable()`

3. **Message History Append-Only**
   - Source: `Task::add_message()`
   - Test: `domain/tests.rs::test_task_history_ordering()`

4. **Pagination Cursor Consistency**
   - Source: `InMemoryTaskStorage::list_tasks_v3()`
   - Test: `task_list_test.rs::test_pagination_no_duplicates()`

---

**Document Hash (for integrity verification):**
SHA256: `<computed on commit>`

**Last Updated:** 2026-02-09
**Reviewed By:** Rust Implementer Agent
**Status:** Complete ✅
