# Task Finite-State Machine (FSM)

## Overview

The `TaskStateMachine` provides deterministic management of task lifecycle with explicit state transitions, guards, and artifact emission. Tasks are work orders with a well-defined path from submission to completion.

## Architecture

### State Transition Graph

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

### Terminal States

States from which no further transitions are possible:
- `Completed` - Task finished successfully
- `Failed` - Task encountered an error
- `Canceled` - Task was canceled before completion
- `Rejected` - Task was rejected (invalid, unauthorized, etc.)
- `Unknown` - Task state could not be determined

### Valid Transitions

| From State | To States |
|------------|-----------|
| `Submitted` | `Working`, `Rejected`, `AuthRequired` |
| `Working` | `InputRequired`, `Completed`, `Failed`, `Canceled` |
| `InputRequired` | `Working` |
| `AuthRequired` | `Working` |
| Terminal states | (none) |

## Components

### 1. TaskStateMachine

Main FSM implementation with:
- Current state tracking
- Complete transition history
- Configurable transition rules
- Optional custom guards

```rust
let mut fsm = TaskStateMachine::new("task-123".to_string());
assert_eq!(fsm.current_state(), &TaskState::Submitted);
```

### 2. StateTransition

Record of each state change with:
- Source and destination states
- Timestamp
- Optional message
- Artifacts emitted during transition

```rust
pub struct StateTransition {
    pub from: TaskState,
    pub to: TaskState,
    pub timestamp: DateTime<Utc>,
    pub message: Option<Message>,
    pub artifacts: Vec<Artifact>,
}
```

### 3. TransitionGuard

Custom validation function for transitions:

```rust
pub type TransitionGuard =
    Box<dyn Fn(&TaskState, &TaskState, Option<&Message>) -> TransitionResult<()> + Send + Sync>;
```

### 4. StateTransitionError

Comprehensive error types for failed transitions:
- `InvalidTransition` - Transition not allowed by FSM rules
- `TransitionFromTerminalState` - Attempted to leave terminal state
- `GuardRejected` - Custom guard blocked the transition
- `Custom` - User-defined error

## Usage Examples

### Basic Lifecycle

```rust
use a2a_rs::construct::TaskStateMachine;
use a2a_rs::domain::TaskState;

// Create FSM
let mut fsm = TaskStateMachine::new("task-1".to_string());

// Start work
fsm.start_working(None)?;

// Request user input
fsm.request_input(Some(message))?;

// Resume after input
fsm.start_working(None)?;

// Complete successfully
fsm.complete(Some(completion_msg), Some(artifacts))?;

assert!(fsm.is_terminal());
```

### With Artifacts

```rust
let artifact = Artifact {
    artifact_id: "result-001".to_string(),
    name: Some("output.json".to_string()),
    // ...
};

fsm.complete(
    Some(completion_message),
    Some(vec![artifact])
)?;
```

### Custom Guards

```rust
// Add a guard that requires a message for certain transitions
fsm.add_guard(
    TaskState::Working,
    TaskState::Failed,
    |message| {
        if message.is_none() {
            Err(StateTransitionError::GuardRejected {
                reason: "Failure must include explanation".to_string(),
            })
        } else {
            Ok(())
        }
    }
);

// This will be rejected by the guard
let result = fsm.fail(None, None);
assert!(result.is_err());

// This will succeed
let result = fsm.fail(Some(error_message), None);
assert!(result.is_ok());
```

### Transition History

```rust
// Execute several transitions
fsm.start_working(None)?;
fsm.request_input(None)?;
fsm.start_working(None)?;
fsm.complete(None, None)?;

// Review full history
for transition in fsm.history() {
    println!("{:?} -> {:?} at {}",
        transition.from,
        transition.to,
        transition.timestamp
    );
}

// Get most recent transition
if let Some(last) = fsm.last_transition() {
    println!("Last transition to: {:?}", last.to);
}
```

### Custom Transition Rules

```rust
use std::collections::HashMap;

// Define custom transitions
let mut transitions = HashMap::new();
transitions.insert(TaskState::Submitted, vec![TaskState::Working]);
transitions.insert(TaskState::Working, vec![TaskState::Completed]);

let mut fsm = TaskStateMachine::with_transitions(
    "task-1".to_string(),
    transitions
);

// Only submitted->working and working->completed are allowed
```

## Methods

### State Queries

- `current_state() -> &TaskState` - Get current state
- `is_terminal() -> bool` - Check if in terminal state
- `can_transition_to(&TaskState) -> bool` - Check if transition is possible
- `allowed_transitions() -> Vec<&TaskState>` - Get all valid next states
- `is_valid_transition(&TaskState, &TaskState) -> bool` - Check specific transition

### Transitions

- `transition_to(to, message, artifacts) -> Result<StateTransition, _>` - Generic transition
- `start_working(message) -> Result<_, _>` - Transition to Working
- `complete(message, artifacts) -> Result<_, _>` - Transition to Completed (terminal)
- `fail(message, artifacts) -> Result<_, _>` - Transition to Failed (terminal)
- `cancel(message, artifacts) -> Result<_, _>` - Transition to Canceled (terminal)
- `request_input(message) -> Result<_, _>` - Transition to InputRequired
- `reject(message) -> Result<_, _>` - Transition to Rejected (terminal)

### History

- `history() -> &[StateTransition]` - Get complete transition history
- `last_transition() -> Option<&StateTransition>` - Get most recent transition
- `transition_count() -> usize` - Count total transitions

### Guards

- `add_guard(from, to, guard_fn)` - Add custom transition guard

## Design Principles

### 1. Deterministic Behavior

The FSM ensures that:
- Same sequence of transitions always produces same result
- Transition validity is explicit and verifiable
- No hidden state or side effects

### 2. Complete Auditability

Every transition is recorded with:
- Source and destination states
- Precise timestamp
- Optional context (message, artifacts)

### 3. Type Safety

- Invalid transitions are compile-time or early-runtime errors
- Exhaustive error handling via Result types
- No panics in library code

### 4. Separation of Concerns

- Domain types (`TaskState`) are separate from FSM logic
- FSM is pure state management (no I/O, no storage)
- Adapters integrate FSM with storage, notifications, etc.

## Integration with Task Domain

The FSM is designed to work alongside `Task` domain types but remains independent:

```rust
use a2a_rs::domain::Task;
use a2a_rs::construct::TaskStateMachine;

// Domain task
let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());

// Parallel FSM for lifecycle management
let mut fsm = TaskStateMachine::new(task.id.clone());

// Transition FSM
fsm.start_working(None)?;

// Update task to match
task.update_status(TaskState::Working, None);
```

For tighter integration, consider implementing an adapter that keeps Task and FSM synchronized.

## Testing

The module includes comprehensive tests covering:
- Valid transitions for all states
- Invalid transition rejection
- Terminal state behavior
- Transition history tracking
- Artifact emission
- Custom guards
- Full lifecycle scenarios

Run tests:
```bash
cargo test -p a2a-rs construct::task_fsm
```

## Performance Characteristics

- State queries: O(1)
- Transition validation: O(1) for default rules
- History access: O(1) for last, O(n) for full history
- Guard evaluation: O(g) where g is number of guards for that transition

## Thread Safety

`TaskStateMachine` is not thread-safe by default (requires `&mut self` for transitions). For concurrent access, wrap in `Arc<Mutex<_>>` or similar synchronization primitive.

## Future Enhancements

Potential additions:
- Serialization/deserialization of FSM state
- Event emission system for reactive programming
- Visualization/graphviz export of transition graph
- Temporal constraints (e.g., minimum time in state)
- Conditional transitions based on task properties
