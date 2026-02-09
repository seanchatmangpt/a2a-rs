# Closure Property (Σ-Completeness) in Domain Types

## Overview

All domain types in `a2a-rs` enforce the **closure property** via `#[serde(deny_unknown_fields)]`. This ensures the input space is **Σ-complete**: only explicitly defined fields are accepted during deserialization.

## Mathematical Definition

For each domain type `T`, the deserialization function `deserialize: JSON → Result<T, Error>` must satisfy:

```
∀ json ∈ JSON:
  if json contains field f where f ∉ fields(T)
  then deserialize(json) = Error(unknown_field)
```

This property guarantees:
1. **Type safety**: No unexpected data can enter the system
2. **Security**: Prevents field injection attacks
3. **Specification compliance**: Rejects malformed protocol messages
4. **Fail-fast validation**: Invalid inputs are rejected at the boundary

## Implementation

Every struct in the domain layer includes `#[serde(deny_unknown_fields)]`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    // ... other fields
}
```

## Extensibility vs. Closure

The closure property applies at the **struct level**, not to the values within fields. This allows extensibility where the protocol requires it:

### ✅ Allowed: Arbitrary data in designated fields

```rust
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub role: Role,
    pub parts: Vec<Part>,
    // metadata can hold arbitrary key-value pairs
    pub metadata: Option<Map<String, Value>>,
}
```

```json
{
  "role": "user",
  "parts": [],
  "metadata": {
    "customKey": "allowed",
    "arbitraryData": {"nested": "ok"}
  }
}
```

### ❌ Rejected: Unknown fields on the struct

```json
{
  "role": "user",
  "parts": [],
  "unknownField": "REJECTED"
}
```

## Affected Types

All domain structs enforce this property:

### Core Types (`domain/core/`)
- **agent.rs**: `AgentCard`, `AgentInterface`, `AgentExtension`, `AgentProvider`, `SecurityScheme` variants, OAuth flow types, `AgentCapabilities`, `AgentSkill`, `PushNotificationConfig`
- **message.rs**: `FileContent`, `Message`, `Artifact`
- **task.rs**: `Task`, `TaskStatus`, `TaskIdParams`, `TaskQueryParams`, `MessageSendConfiguration`, `MessageSendParams`, `TaskSendParams`, `ListTasksParams`, `ListTasksResult`, all push notification config types

### Protocol Types (`domain/protocols/`)
- **json_rpc.rs**: `JSONRPCMessage`, `JSONRPCError`, `JSONRPCRequest`, `JSONRPCResponse`, `JSONRPCNotification`

### Event Types (`domain/events/`)
- **task_events.rs**: `TaskStatusUpdateEvent`, `TaskArtifactUpdateEvent`

## Test Coverage

Comprehensive tests in:
- `domain/core/tests_deny_unknown_fields.rs` - Tests for core types
- `domain/protocols/tests_deny_unknown_fields.rs` - Tests for JSON-RPC types
- `domain/events/tests_deny_unknown_fields.rs` - Tests for event types

Each test verifies:
1. Unknown fields are rejected (closure property)
2. Valid data still deserializes correctly
3. Designated extensibility points (like `metadata`) accept arbitrary data

## Rationale

### Security Benefits

1. **Prevents injection attacks**: Attackers cannot sneak data through unknown fields
2. **Enforces protocol boundaries**: Only spec-compliant messages are accepted
3. **Fail-fast principle**: Invalid data is caught at deserialization, not deep in business logic

### Protocol Evolution

The A2A protocol v0.3.0 specifies:
- Explicit fields for all data (see `spec/*.json`)
- `metadata` fields for extensibility
- `extensions` arrays for protocol extensions

This dual approach gives us:
- **Closed structs** for type safety and security
- **Open metadata/extensions** for future compatibility

### Example: Why Both Are Needed

Consider an `AgentCard`:

```rust
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]  // ✅ No unknown fields on struct
pub struct AgentCard {
    pub name: String,
    pub version: String,
    // ... spec-defined fields

    // ✅ But metadata can hold arbitrary data
    pub metadata: Option<Map<String, Value>>,
}
```

This allows:
```json
{
  "name": "Agent",
  "version": "1.0",
  "metadata": {
    "vendor": "Custom Corp",
    "internalId": "xyz-123"
  }
}
```

But rejects:
```json
{
  "name": "Agent",
  "version": "1.0",
  "customField": "INVALID - not in spec"
}
```

## JSON-RPC Special Cases

The JSON-RPC 2.0 spec (RFC 7230) defines certain fields as flexible:

1. **`id`**: Can be string, number, or null → `Option<Value>`
2. **`params`**: Can be any structured value → `Option<Value>`
3. **`result`**: Can be any value → `Option<Value>`
4. **`data`** (in error): Can be any value → `Option<Value>`

These use `serde_json::Value` by design (per the JSON-RPC spec), but the structs themselves still enforce `deny_unknown_fields`.

## Verification

To verify closure property enforcement:

```bash
# Run deny_unknown_fields tests
cargo test tests_deny_unknown_fields

# Grep for all deny_unknown_fields attributes
grep -r "deny_unknown_fields" a2a-rs/src/domain/
```

## Maintenance

When adding new domain types:

1. ✅ Add `#[serde(deny_unknown_fields)]` to all structs
2. ✅ Add tests verifying unknown field rejection
3. ✅ Document any extensibility points (metadata, params, etc.)
4. ✅ Update this document with the new type

## References

- Serde documentation: https://serde.rs/container-attrs.html#deny_unknown_fields
- A2A Protocol v0.3.0: `spec/*.json`
- JSON-RPC 2.0: https://www.jsonrpc.org/specification
