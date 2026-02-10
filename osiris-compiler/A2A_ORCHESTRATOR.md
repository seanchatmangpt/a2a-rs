# A2A Orchestrator - CLM to Remote Agent Bridge

## Overview

This implementation bridges Osiris Compiler Lambda Manager (CLM) operations to remote A2A agents (like osiris-macos, osiris-windows) via the a2a-rs HTTP client. The orchestrator enables distributed compilation by delegating compiler operations to specialized agents on different platforms.

## Architecture

### Layered Design

```
domain/a2a_orchestration.rs  ← Pure types, zero external dependencies
         ↓
port/a2a_orchestrator.rs     ← Async port traits, domain types only
         ↓
adapter/a2a_orchestrator.rs  ← HTTP client implementation
         ↓
Application Layer            ← Wires orchestrator into handlers
```

## Domain Types (`domain/a2a_orchestration.rs`)

### Core Types

**`A2AOrchestrationTask`** - Represents a CLM operation bound to a remote A2A agent:
- Unique IDs and UUIDs for tracking
- Agent identification (ID and base URL)
- Remote task ID on the A2A agent
- Compilation context ID
- Current orchestration state
- Operation payload (what to compile/analyze/link)
- Artifacts produced (object files, binaries, etc.)
- Retry tracking (count, max attempts)
- Metadata for debugging

**`OrchestrationTaskState`** - Task lifecycle states:
- `Submitting` - Being sent to remote agent
- `Submitted` - Queued on remote agent
- `Executing` - Currently running
- `Paused` - Awaiting input
- `Completed` - Successfully finished
- `Canceled` - User cancellation
- `Failed` - Encountered error
- `Unknown` - Unable to determine state

**`OperationPayload`** - What to execute on the remote agent:
- `Compile` - Compilation with source, target, flags, optimization level
- `Link` - Linking object files into final binary
- `Analyze` - Type checking, invariant verification, etc.
- `Custom` - Arbitrary operations with JSON payload

**`ArtifactReference`** - Output files produced by operations:
- Artifact ID and name
- MIME type and URL
- Size, hash for integrity
- Creation timestamp
- Custom metadata

**`OrchestrationSnapshot`** - Point-in-time task state for monitoring:
- Task ID, current state
- Progress percentage (0-100)
- Status message
- Accumulated artifacts
- Timestamp

**`OrchestrationEvent`** - Streaming events for real-time monitoring:
- `StateChanged` - State transition with old/new state
- `ProgressUpdate` - Work progress
- `ArtifactProduced` - New output file
- `RetryScheduled` - Automatic retry scheduled
- `Completed` - Final completion with result

## Port Trait (`port/a2a_orchestrator.rs`)

### `A2AOrchestratorPort` Trait

Defines the async interface for remote agent orchestration:

**Core Operations:**
```rust
async fn submit_task(
    agent_id: &str,
    agent_url: &str,
    context_id: &str,
    operation: OperationPayload,
) -> OrchestrationResult<A2AOrchestrationTask>
```
- Submits a new compilation operation to a remote agent
- Returns task with assigned IDs and initial state

```rust
async fn get_task_status(&self, task: &A2AOrchestrationTask) -> OrchestrationResult<OrchestrationSnapshot>
```
- Fetches current status from remote agent
- Returns snapshot with state, progress, artifacts

```rust
async fn stream_task_updates(&self, task: &A2AOrchestrationTask) -> OrchestrationResult<OrchestrationEventStream>
```
- Returns a stream of real-time updates
- Polls remote agent periodically (configurable interval)
- Emits events until task reaches terminal state

```rust
async fn update_artifacts(&self, task: &mut A2AOrchestrationTask) -> OrchestrationResult<()>
```
- Fetches new artifacts from remote agent
- Adds them to the orchestration task

```rust
async fn cancel_task(&self, task: &mut A2AOrchestrationTask) -> OrchestrationResult<()>
```
- Sends cancellation to remote agent
- Updates task state to Canceled

```rust
async fn retry_task(&self, task: &mut A2AOrchestrationTask) -> OrchestrationResult<A2AOrchestrationTask>
```
- Re-submits a failed task
- Respects max retries
- Returns new task with incremented retry count

```rust
async fn wait_for_completion(
    &self,
    task: &mut A2AOrchestrationTask,
    timeout_secs: Option<u64>,
) -> OrchestrationResult<OrchestrationSnapshot>
```
- Blocks until task reaches terminal state
- Configurable timeout

**Administrative Operations:**
```rust
async fn list_tasks(
    agent_url: &str,
    context_id: &str,
) -> OrchestrationResult<Vec<A2AOrchestrationTask>>

async fn check_agent_health(&self, agent_url: &str) -> OrchestrationResult<bool>

async fn get_failure_details(&self, task: &A2AOrchestrationTask) -> OrchestrationResult<String>
```

### `A2AOrchestratorConfig` - Configuration

```rust
pub struct A2AOrchestratorConfig {
    pub timeout_secs: u64,              // Default: 300 (5 minutes)
    pub auto_retry: bool,               // Default: true
    pub max_retries: u32,               // Default: 3
    pub retry_delay_ms: u64,            // Default: 1000
    pub stream_updates: bool,           // Default: true
    pub poll_interval_ms: u64,          // Default: 500
    pub user_agent: String,             // "osiris-compiler/0.1.0"
}
```

### Error Types

**`OrchestrationError`**:
- `SubmissionFailed` - Task submission to remote agent failed
- `StatusFetchFailed` - Unable to get task status
- `ArtifactUpdateFailed` - Artifact retrieval failed
- `CancellationFailed` - Unable to cancel task
- `NetworkError` - Network communication error
- `RemoteAgentError` - Agent returned an error
- `TaskNotFound` - Task doesn't exist on remote agent
- `InvalidOperation` - Operation invalid or unsupported
- `Timeout` - Operation exceeded timeout
- `SerializationError` - JSON serialization/deserialization failed
- `MaxRetriesExceeded` - Task exhausted retry attempts

## Adapter Implementation (`adapter/a2a_orchestrator.rs`)

### `RemoteA2AOrchestratorAdapter`

Implements `A2AOrchestratorPort` using the a2a-rs `HttpClient`:

**Key Features:**
1. **HTTP Communication** - Uses a2a-rs HttpClient for all remote agent interaction
2. **Message Conversion** - Converts `OperationPayload` to A2A protocol messages
3. **State Mapping** - Maps A2A TaskState ↔ OrchestrationTaskState
4. **Polling** - Configurable polling interval for status updates
5. **Error Handling** - Comprehensive error mapping with tracing

**Helper Methods:**
```rust
fn extract_message_text(message: &Message) -> String
```
- Extracts text content from A2A message parts
- Used for status messages and error details

```rust
fn operation_to_message(&self, operation: &OperationPayload) -> OrchestrationResult<Message>
```
- Converts operation to JSON payload message for agent
- Includes source code, target platform, flags, etc.

```rust
fn remote_task_to_orchestration(...) -> A2AOrchestrationTask
```
- Maps A2A Task response to our domain type
- Preserves all relevant metadata

**Implementation Details:**
- Creates new HttpClient per request (avoids lifetime issues)
- Automatic user-agent header for tracking
- Configurable timeouts
- Comprehensive logging via tracing (when enabled)
- Polling-based event streaming (not SSE)
- Handles artifact extraction from A2A artifact types

## Integration Points

### Cargo.toml Changes

```toml
[dependencies]
a2a-rs = { path = "../a2a-rs", default-features = false,
           features = ["client", "http-client", "server", "http-server", "tracing"] }
futures = "0.3"
serde_bytes = "0.11"
```

### Module Structure

**domain/mod.rs:**
```rust
pub mod a2a_orchestration;
pub use a2a_orchestration::{
    A2AOrchestrationTask, ArtifactReference, OperationPayload,
    OrchestrationEvent, OrchestrationSnapshot, OrchestrationTaskState,
};
```

**port/mod.rs:**
```rust
pub mod a2a_orchestrator;
pub use a2a_orchestrator::{
    A2AOrchestratorConfig, A2AOrchestratorPort, OrchestrationError,
    OrchestrationEventStream, OrchestrationResult, TaskLifecycleManager,
};
```

**adapter/mod.rs:**
```rust
pub mod a2a_orchestrator;
pub use a2a_orchestrator::RemoteA2AOrchestratorAdapter;
```

## Usage Example

```rust
use osiris_compiler::adapter::RemoteA2AOrchestratorAdapter;
use osiris_compiler::domain::OperationPayload;
use osiris_compiler::port::{A2AOrchestratorConfig, A2AOrchestratorPort};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create orchestrator with custom config
    let config = A2AOrchestratorConfig {
        timeout_secs: 600,
        auto_retry: true,
        max_retries: 3,
        ..Default::default()
    };
    let orchestrator = RemoteA2AOrchestratorAdapter::new(config);

    // Submit a compilation operation
    let operation = OperationPayload::Compile {
        source: "fn main() {}".to_string(),
        target: "aarch64-apple-darwin".to_string(),
        flags: Some(vec!["-O2".to_string()]),
        opt_level: 2,
    };

    let mut task = orchestrator.submit_task(
        "osiris-macos",
        "https://macos-agent.local/api",
        "ctx-123",
        operation,
    ).await?;

    // Stream updates
    let mut updates = orchestrator.stream_task_updates(&task).await?;
    while let Some(event) = updates.next().await {
        println!("Event: {:?}", event);
    }

    // Wait for completion
    let snapshot = orchestrator.wait_for_completion(
        &mut task,
        Some(300),
    ).await?;

    println!("Task state: {:?}", snapshot.state);
    println!("Artifacts: {:?}", snapshot.artifacts);

    Ok(())
}
```

## Testing

Unit tests included in each module:

**domain/a2a_orchestration.rs:**
- Task creation
- State transitions
- Artifact addition
- Retry logic
- Event serialization

**port/a2a_orchestrator.rs:**
- Error display
- Config defaults
- Serialization error conversion

**adapter/a2a_orchestrator.rs:**
- Adapter creation
- Operation to message conversion
- Default configuration

Run tests:
```bash
cargo test -p osiris-compiler --lib a2a_orchestrator
cargo test -p osiris-compiler --lib a2a_orchestration
```

## Future Enhancements

1. **SSE-based streaming** - Replace polling with Server-Sent Events
2. **Task caching** - Cache HTTP clients per agent URL
3. **Metrics collection** - Track operation latencies, success rates
4. **Batch operations** - Submit multiple operations atomically
5. **Artifact streaming** - Stream large artifacts instead of loading in memory
6. **Circuit breaker** - Fail fast for unhealthy agents
7. **Task persistence** - Save task state to database
8. **WebSocket support** - Alternative to HTTP for real-time updates

## Files Created/Modified

### Created Files
- `src/domain/a2a_orchestration.rs` (430 lines) - Domain types
- `src/port/a2a_orchestrator.rs` (260 lines) - Port trait
- `src/adapter/a2a_orchestrator.rs` (590 lines) - HTTP client adapter
- `A2A_ORCHESTRATOR.md` (this file)

### Modified Files
- `src/domain/mod.rs` - Added module imports
- `src/port/mod.rs` - Added module imports and exports
- `src/adapter/mod.rs` - Added module imports and exports
- `src/application/http_handlers.rs` - Added BoundedWriter import
- `Cargo.toml` - Added a2a-rs, futures, serde_bytes dependencies

## Architecture Compliance

✅ **Hexagonal Architecture** - Port trait defines contract, adapter implements
✅ **Dependency Direction** - domain ← port ← adapter ← application
✅ **Feature Gates** - Tracing conditionally compiled
✅ **Error Handling** - thiserror types, no unwrap()
✅ **Type Safety** - All public types derive Debug, Clone, Serialize, Deserialize
✅ **Async-First** - #[async_trait] for all port methods
✅ **Zero Panic** - Comprehensive error handling

## Conventions Applied

- Edition 2024, MSRV 1.85
- `serde(rename_all = "camelCase")` for JSON
- Builder patterns not needed (3- field types using defaults)
- Comprehensive unit tests
- Detailed documentation comments
