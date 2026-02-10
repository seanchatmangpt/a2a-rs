# Audit Logging Implementation

## Overview

The audit logging system provides comprehensive logging for the Osiris compiler with:
- **Structured logging** with JSON serialization
- **W3C Trace Context** support for distributed tracing
- **Google Cloud Logging** integration
- **Local fallback buffer** when Cloud Logging is unavailable
- **User actions**, **state changes**, and **receipt events** logging

## Architecture

### Hexagonal Layers

```
Domain (domain/audit.rs)
  ↓
Port (port/audit_log.rs)
  ↓
Adapter (adapter/audit_logger.rs)
```

### Domain Layer: `src/domain/audit.rs`

Defines pure domain types for audit logging:

#### `AuditLogEntry`
Main audit log entry type containing:
- **id**: Unique UUID identifier
- **timestamp**: When the event occurred
- **event_type**: `AuditEventType` enum (20+ event types)
- **actor**: User or service that triggered the event
- **resource_id/resource_type**: What was operated on
- **action**: What action was performed
- **status**: `AuditStatus` (Success, Failure, Rejected, Pending, Cancelled)
- **details**: `AuditDetails` structured union type
- **trace_context**: W3C trace context for correlation
- **metadata**: Custom key-value pairs
- **severity**: `AuditSeverity` (Info, Warning, Error, Critical)

#### `AuditEventType` Enum
Comprehensive event types:
- `CompilationStarted`, `CompilationCompleted`, `CompilationFailed`
- `OperationCreated`, `OperationAccepted`, `OperationRefused`, `OperationStateChanged`
- `ReceiptCreated`, `ReceiptVerified`, `ReceiptVerificationFailed`, `ReceiptStored`
- `StateSnapshotCreated`, `GuardEvaluated`, `InvariantCheckPerformed`, `InvariantCheckFailed`
- `UserAuthenticated`, `AuthorizationFailed`, `ConfigurationChanged`, `SystemError`, `SecurityEvent`

#### `AuditDetails` Union Type
Structured audit details by event category:
- `UserAction` - User-initiated actions with expected vs actual state
- `StateChange` - State transitions with previous/new state and reason
- `ReceiptEvent` - Receipt operations with hashes and operation reference
- `GuardEvaluation` - Guard condition evaluation results
- `InvariantCheck` - Invariant check results with violation details
- `AuthEvent` - Authentication/authorization events with scopes and denials
- `ErrorDetails` - Error details with code and context
- `Unstructured` - Fallback for custom data

#### `TraceContext`
W3C Trace Context support:
- **trace_id**: Globally unique trace identifier
- **span_id**: Current operation span ID
- **trace_flags**: W3C sampling decision
- **parent_span_id**: Causal relationship tracking
- **trace_state**: Custom key-value trace state
- **request_id**: End-to-end request correlation ID

### Port Layer: `src/port/audit_log.rs`

Defines the `AuditLog` async trait that adapters must implement:

#### Core Methods
```rust
pub trait AuditLog: Send + Sync {
    async fn log(&self, entry: AuditLogEntry) -> Result<Uuid, AuditError>;
    async fn log_batch(&self, entries: Vec<AuditLogEntry>) -> Result<usize, AuditError>;

    // Query methods
    async fn get_logs_for_resource(&self, resource_id: Uuid) -> Result<Vec<AuditLogEntry>, AuditError>;
    async fn get_logs_by_actor(&self, actor: &str) -> Result<Vec<AuditLogEntry>, AuditError>;
    async fn get_logs_in_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<AuditLogEntry>, AuditError>;
    async fn get_log_entry(&self, entry_id: Uuid) -> Result<AuditLogEntry, AuditError>;
    async fn get_logs_by_event_type(&self, event_type: AuditEventType) -> Result<Vec<AuditLogEntry>, AuditError>;
    async fn get_logs_by_trace_id(&self, trace_id: &str) -> Result<Vec<AuditLogEntry>, AuditError>;

    // Health check
    async fn health_check(&self) -> Result<(), AuditError>;
}
```

### Adapter Layer: `src/adapter/audit_logger.rs`

Implements `AuditLog` port using Google Cloud Logging:

#### `CloudLoggingConfig`
Configuration for the audit logger:
```rust
pub struct CloudLoggingConfig {
    pub project_id: String,
    pub log_name: String,
    pub include_trace_context: bool,
    pub batch_size: usize,
    pub enable_local_fallback: bool,
    pub labels: HashMap<String, String>,
}
```

#### `CloudLoggingAuditLogger`
Implementation with:
- **Local buffer**: In-memory fallback (max 10k entries)
- **Cloud Logging**: Optional integration when feature enabled
- **Structured logging**: JSON serialization with trace context
- **Batch operations**: Efficient bulk writes
- **Health checks**: Connectivity verification

#### Key Features

**Log Entry Transformation**
- Converts `AuditLogEntry` to Cloud Logging JSON payload
- Includes structured audit details, trace context, and labels
- Adds W3C trace context fields for Cloud Trace correlation
- Generates human-readable log messages for console output

**Fallback Mechanism**
```
Cloud Logging Available? → Write to Cloud Logging
                        ↓
Cloud Logging Down → Write to Local Buffer
                        ↓
Local Buffer (Max 10k entries) → Trim to 5k if full
```

**Tracing Integration**
Uses `tracing` crate for structured logging:
```rust
tracing::info!(
    event_type = ?entry.event_type,
    resource_id = ?entry.resource_id,
    actor = ?entry.actor,
    trace_id = ?entry.trace_context.as_ref().map(|t| &t.trace_id),
    payload = %payload,
    "{}", message
);
```

## Usage Examples

### Basic Usage

```rust
use osiris_compiler::adapter::{CloudLoggingAuditLogger, CloudLoggingConfig};
use osiris_compiler::domain::{AuditLogEntry, AuditEventType, AuditStatus};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create logger with in-memory fallback
    let config = CloudLoggingConfig::default();
    let logger = CloudLoggingAuditLogger::in_memory(config);

    // Log a user action
    let resource_id = Uuid::new_v4();
    let entry = AuditLogEntry::user_action(
        "user@example.com".to_string(),
        "CompileModule".to_string(),
        resource_id,
    );

    let entry_id = logger.log(entry).await?;
    println!("Logged entry: {}", entry_id);

    // Query logs by resource
    let logs = logger.get_logs_for_resource(resource_id).await?;
    println!("Found {} log entries", logs.len());

    Ok(())
}
```

### With Trace Context

```rust
use osiris_compiler::domain::{TraceContext, AuditLogEntry, AuditEventType, AuditStatus};

// Create trace context
let trace = TraceContext {
    trace_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
    span_id: "6a3ff412-3ec3-430a-97c0-6407b34e5420".to_string(),
    trace_flags: Some("01".to_string()),
    parent_span_id: None,
    trace_state: HashMap::new(),
    request_id: Some("req-12345".to_string()),
};

// Log with trace context
let entry = AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success)
    .with_trace_context(trace)
    .with_severity(AuditSeverity::Info);

logger.log(entry).await?;
```

### State Change Tracking

```rust
// Log a state transition
let entry = AuditLogEntry::state_change(
    operation_id,
    "Pending".to_string(),
    "Completed".to_string(),
);

logger.log(entry).await?;
```

### Receipt Events

```rust
// Log a receipt creation
let entry = AuditLogEntry::receipt_event(
    receipt_id,
    operation_id,
    "Receipt created and signed with KMS".to_string(),
);

logger.log(entry).await?;
```

### Batch Logging

```rust
let entries = vec![
    AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success),
    AuditLogEntry::new(AuditEventType::CompilationCompleted, AuditStatus::Success),
    AuditLogEntry::new(AuditEventType::ReceiptCreated, AuditStatus::Success),
];

let count = logger.log_batch(entries).await?;
println!("Logged {} entries", count);
```

### Querying Logs

```rust
// Get logs by trace ID (distributed tracing)
let trace_logs = logger.get_logs_by_trace_id("550e8400-e29b-41d4-a716-446655440000").await?;

// Get logs by actor
let user_logs = logger.get_logs_by_actor("user@example.com").await?;

// Get logs in time range
let start = Utc::now() - Duration::hours(1);
let end = Utc::now();
let range_logs = logger.get_logs_in_range(start, end).await?;

// Get logs by event type
let compilation_logs = logger.get_logs_by_event_type(AuditEventType::CompilationStarted).await?;
```

### Cloud Logging Integration

Enable the `cloud-logging` feature in Cargo.toml:

```toml
[dependencies]
osiris-compiler = { path = ".", features = ["cloud-logging"] }
```

Then create with real Cloud Logging client:

```rust
let config = CloudLoggingConfig {
    project_id: "my-project".to_string(),
    log_name: "osiris-compiler-audit".to_string(),
    include_trace_context: true,
    batch_size: 100,
    enable_local_fallback: true,
    labels: HashMap::new(),
};

let logger = CloudLoggingAuditLogger::new(config).await?;
```

## Integration with HTTP Handlers

Example integration in an HTTP handler:

```rust
use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};

#[derive(Clone)]
pub struct AppState {
    audit_log: Arc<dyn AuditLog + Send + Sync>,
}

async fn compile_handler(
    State(state): State<AppState>,
    Json(request): Json<CompileRequest>,
) -> Result<Json<CompileResponse>, AppError> {
    // Log compilation started
    let entry = AuditLogEntry::new(
        AuditEventType::CompilationStarted,
        AuditStatus::Pending,
    );
    state.audit_log.log(entry).await.ok();

    // ... perform compilation ...

    // Log compilation completed
    let result_entry = AuditLogEntry::new(
        AuditEventType::CompilationCompleted,
        AuditStatus::Success,
    );
    state.audit_log.log(result_entry).await.ok();

    Ok(Json(compile_response))
}
```

## Cloud Logging JSON Payload Structure

Example log payload sent to Cloud Logging:

```json
{
  "auditEntry": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "timestamp": "2026-02-10T12:34:56.789Z",
    "eventType": "CompilationStarted",
    "status": "Success",
    "severity": "Info",
    "actor": "user@example.com",
    "resourceId": "6a3ff412-3ec3-430a-97c0-6407b34e5420",
    "resourceType": "Operation",
    "action": "CompileModule",
    "details": {
      "type": "UserAction",
      "actionDescription": "CompileModule",
      "expectedState": null,
      "actualState": null
    }
  },
  "traceContext": {
    "traceId": "550e8400-e29b-41d4-a716-446655440000",
    "spanId": "6a3ff412-3ec3-430a-97c0-6407b34e5420",
    "traceFlags": "01",
    "parentSpanId": null,
    "requestId": "req-12345",
    "traceState": {}
  },
  "trace": "projects/my-project/traces/550e8400-e29b-41d4-a716-446655440000",
  "spanId": "6a3ff412-3ec3-430a-97c0-6407b34e5420",
  "labels": {
    "resource_type": "Operation",
    "event_type": "CompilationStarted"
  }
}
```

## Error Handling

The `AuditError` enum covers all error cases:

```rust
pub enum AuditError {
    SerializationError(String),      // JSON serialization failed
    WriteError(String),              // Failed to write audit log
    FormatError(String),             // Log formatting error
    InvalidTraceContext(String),     // Invalid trace context
    ServiceError(String),            // Cloud Logging service error
    ConfigurationError(String),      // Configuration validation error
}
```

## Testing

The implementation includes comprehensive tests:

```bash
# Run all audit logger tests
cargo test --lib audit_logger --features "cloud-logging"

# Run specific test
cargo test --lib test_cloud_logging_config
```

Test coverage includes:
- Configuration defaults
- Entry creation and logging
- Batch operations
- Query methods (by resource, actor, event type, trace ID)
- Trace context handling
- Local buffer management
- Cloud Logging payload serialization
- Health checks

## Files Added/Modified

### New Files
- **`src/domain/audit.rs`** (16 KB) - Audit domain types
- **`src/port/audit_log.rs`** (5.9 KB) - AuditLog port trait
- **`src/adapter/audit_logger.rs`** (22 KB) - Cloud Logging adapter

### Modified Files
- **`src/domain/mod.rs`** - Added audit module and exports
- **`src/port/mod.rs`** - Added audit_log module and AuditLog trait export
- **`src/adapter/mod.rs`** - Added audit_logger module and CloudLoggingAuditLogger export
- **`Cargo.toml`** - Added google-cloud-logging dependency and cloud-logging feature

## Feature Flags

### cloud-logging (optional)
Enables Google Cloud Logging integration. When disabled, uses in-memory buffering:

```bash
cargo build --features "cloud-logging"
```

## Best Practices

1. **Always include trace context** for distributed tracing:
   ```rust
   let entry = AuditLogEntry::new(event_type, status)
       .with_trace_context(trace_context);
   ```

2. **Use semantic severity levels**:
   - `Info`: Normal operations
   - `Warning`: Unusual but handled conditions
   - `Error`: Operation failures
   - `Critical`: Security or system health issues

3. **Batch logs when possible**:
   ```rust
   logger.log_batch(entries).await?
   ```

4. **Query by trace ID for debugging**:
   ```rust
   let trace_logs = logger.get_logs_by_trace_id(trace_id).await?;
   ```

5. **Monitor health in readiness checks**:
   ```rust
   logger.health_check().await?
   ```

## Future Enhancements

Potential improvements for future versions:

1. **Async Cloud Logging Client**: Real integration with google-cloud-logging crate
2. **Log Rotation**: Configurable buffer size and rotation policies
3. **Filtering**: Query DSL for complex log filtering
4. **Signing**: Optional cryptographic signing of log entries
5. **Long-term Storage**: Archival to Cloud Storage or BigQuery
6. **Compliance**: SOC 2 / HIPAA logging patterns
7. **Performance Optimization**: Batching strategies and compression
8. **Custom Formatters**: Pluggable log format transformations
