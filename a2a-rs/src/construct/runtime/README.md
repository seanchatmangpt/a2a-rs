# Runtime μ Function - Core Execution Engine

## Overview

The Runtime implements the μ(O) function - the total compiler/runtime function for the CONSTRUCT framework. It brings together all CONSTRUCT components into a unified execution pipeline.

## Architecture

```
A = μ(O) where:
- A: Actuation (output)
- μ: Runtime dispatcher/executor
- O: Ontology (input state)
```

### Components

The Runtime orchestrates:

1. **O (Ontology)**: State model (`OntologyState`)
2. **Λ (Lambda)**: Scheduler for ordered execution
3. **G (Guards)**: Admission control with typed refusal receipts
4. **Q (Invariants)**: State validation predicates
5. **Δ (Delta)**: State change execution
6. **R (Receipts)**: Cryptographic execution proofs

## Execution Pipeline

The Runtime executes operations through a 6-stage pipeline:

### Stage 1: Type Check
- Validates operation against ontology schema
- Checks bounded state limits
- Ensures referenced entities exist

### Stage 2: Admission Guard
- Evaluates guard predicates
- Returns typed `RefusalReceipt` on denial
- Deterministic, no LLM involved

### Stage 3: Apply Λ (Transformations)
- Submits tasks to scheduler
- Updates ontology state
- Manages work-in-progress limits

### Stage 4: Check Q (Invariants)
- Validates state machine rules
- Checks domain constraints
- Ensures semantic correctness

### Stage 5: Execute Δ (Deltas)
- Applies final state changes
- Emits runtime events
- Enforces bounded update semantics

### Stage 6: Emit Receipts
- Generates execution receipt
- Records completed stages
- Provides audit trail

## Usage

### Basic Runtime Creation

```rust
use a2a_rs::construct::{
    Runtime, OntologyState, Scheduler,
    InvariantRegistry, Guards
};

// Create default runtime
let runtime = Runtime::default_runtime();

// Or create with custom components
let ontology = OntologyState::new();
let scheduler = Scheduler::new(10); // 10 concurrent tasks
let guards = vec![];
let invariants = InvariantRegistry::new();

let runtime = Runtime::new(ontology, scheduler, guards, invariants)
    .with_policy_epoch(1)
    .with_update_limit(1000);
```

### Handling Operations

```rust
use a2a_rs::construct::{Operation, PriorityClass};
use a2a_rs::domain::{Task, TaskStatus};

let task = Task::builder()
    .id("task-1".to_string())
    .context_id("ctx-1".to_string())
    .status(TaskStatus::default())
    .build();

let operation = Operation::CreateTask {
    task,
    initial_message: None,
    priority: Some(PriorityClass::High),
};

let output = runtime.handle(operation)?;

// Check result
if output.receipt.success {
    println!("Execution succeeded!");
    println!("Stages: {:?}", output.receipt.stages_completed);
    println!("Duration: {}ms", output.receipt.duration_ms);
} else {
    println!("Execution failed: {:?}", output.errors);
}
```

### Working with Guards

```rust
use a2a_rs::construct::{TypeGuard, RangeGuard, Guard};
use std::sync::Arc;

// Create guards
let type_guard = Arc::new(TypeGuard::new("string".to_string()));
let range_guard = Arc::new(RangeGuard::new(Some(0.0), Some(100.0)));

let runtime = Runtime::new(
    ontology,
    scheduler,
    vec![type_guard, range_guard],
    invariants,
);

// Guards are checked automatically during operation handling
```

### Working with Invariants

```rust
use a2a_rs::construct::{InvariantRegistry, TaskStateInvariant};

let mut invariants = InvariantRegistry::new();
invariants.register("task_state", Box::new(TaskStateInvariant::new()));

let runtime = Runtime::new(ontology, scheduler, guards, invariants);

// Invariants are checked automatically for task operations
```

## Operation Types

### CreateTask
Creates a new task and schedules it for execution.

```rust
Operation::CreateTask {
    task: Task,
    initial_message: Option<Message>,
    priority: Option<PriorityClass>,
}
```

### SendMessage
Sends a message to an existing task.

```rust
Operation::SendMessage {
    task_id: String,
    message: Message,
}
```

### UpdateTaskState
Updates the state of a task (with invariant checking).

```rust
Operation::UpdateTaskState {
    task_id: String,
    state: TaskState,
}
```

### AddArtifact
Adds an artifact to a task.

```rust
Operation::AddArtifact {
    task_id: String,
    artifact: Artifact,
}
```

### CompleteTask
Marks a task as complete and removes it from active execution.

```rust
Operation::CompleteTask {
    task_id: String,
    station_id: String,
}
```

### CancelTask
Cancels a pending task before it starts executing.

```rust
Operation::CancelTask {
    task_id: String,
}
```

## Output Structure

The `RuntimeOutput` contains:

```rust
pub struct RuntimeOutput {
    /// Tasks created or modified
    pub tasks: Vec<Task>,

    /// Events emitted during execution
    pub events: Vec<RuntimeEvent>,

    /// Artifacts generated
    pub artifacts: Vec<Artifact>,

    /// Errors encountered (non-fatal)
    pub errors: Vec<RuntimeError>,

    /// Execution receipt
    pub receipt: ExecutionReceipt,
}
```

## Events

Runtime events provide observability:

- `TaskCreated`: New task registered
- `TaskScheduled`: Task submitted to scheduler
- `TaskStateChanged`: State transition occurred
- `MessageProcessed`: Message added to task
- `ArtifactAdded`: Artifact attached to task
- `TransformationApplied`: Transformation executed
- `InvariantChecked`: Invariant validation result
- `GuardEvaluated`: Guard admission result

## Error Handling

The Runtime uses typed errors:

```rust
pub enum RuntimeError {
    TypeCheckFailed { message: String },
    AdmissionDenied { receipt: String },
    TransformationFailed { message: String },
    InvariantViolation { violation: String },
    ExecutionFailed { message: String },
    BoundedUpdateExceeded { limit: usize },
    SchedulerError { message: String },
    InvalidOperation { message: String },
}
```

Errors are returned in the `RuntimeOutput.errors` field, allowing partial success.

## Bounded Update Semantics

The Runtime enforces bounded updates to prevent unbounded state growth:

```rust
let runtime = Runtime::default_runtime()
    .with_update_limit(1000); // Maximum 1000 entities

// Operations that would exceed the limit are rejected
```

## Policy Epochs

Guards use policy epochs for versioning admission rules:

```rust
let runtime = Runtime::default_runtime()
    .with_policy_epoch(42);

// Guards check against policy epoch 42
// Refusal receipts record which epoch rejected them
```

## Determinism Guarantees

The Runtime provides deterministic execution:

1. **Scheduler**: Deterministic task ordering via (epoch, priority, station, task_id)
2. **Guards**: Pure predicates, no randomness
3. **Invariants**: Stateless checks
4. **Events**: Deterministically ordered
5. **State**: BTreeMap ensures consistent iteration

## Integration with A2A Protocol

The Runtime integrates seamlessly with A2A protocol operations:

```rust
// Handle A2A message
let message = Message::user_text("Hello".to_string(), "msg-1".to_string());
let operation = Operation::SendMessage {
    task_id: "task-123".to_string(),
    message,
};

let output = runtime.handle(operation)?;

// Handle A2A task creation
let task = Task::from_a2a_request(request);
let operation = Operation::CreateTask {
    task,
    initial_message: None,
    priority: Some(PriorityClass::Normal),
};

let output = runtime.handle(operation)?;
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_execution() {
        let mut runtime = Runtime::default_runtime();

        let task = Task::builder()
            .id("test-1".to_string())
            .context_id("ctx-1".to_string())
            .status(TaskStatus::default())
            .build();

        let operation = Operation::CreateTask {
            task,
            initial_message: None,
            priority: None,
        };

        let result = runtime.handle(operation);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.receipt.success);
        assert_eq!(output.tasks.len(), 1);
    }
}
```

## Future Enhancements

Potential extensions:

1. **Parallel execution**: Multiple stations executing concurrently
2. **Distributed runtime**: Multi-node execution
3. **Incremental state**: Efficient state updates
4. **Persistent receipts**: Long-term audit storage
5. **Guard composition**: Complex admission policies
6. **Custom transformations**: User-defined Λ functions

## Files

- `/home/user/a2a-rs/a2a-rs/src/construct/runtime/executor.rs` - Runtime μ implementation
- `/home/user/a2a-rs/a2a-rs/src/construct/runtime/scheduler.rs` - Λ scheduler
- `/home/user/a2a-rs/a2a-rs/src/construct/guards/mod.rs` - Guard system
- `/home/user/a2a-rs/a2a-rs/src/construct/invariants/mod.rs` - Invariant system
- `/home/user/a2a-rs/a2a-rs/src/construct/ontology/mod.rs` - Ontology state
