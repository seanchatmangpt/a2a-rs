# Replay Debugger

Interactive debugger for stepping through receipt chains and inspecting state at each step.

## Overview

The replay debugger provides an interactive interface for analyzing recorded execution sequences. It allows:

- **Step Forward/Backward** - Navigate through the execution timeline
- **Breakpoints** - Pause execution at specific steps
- **State Inspection** - View complete state at any point
- **State Diffing** - Compare states between steps
- **Receipt Navigation** - Jump to specific receipts by hash

## Components

### Core Types (`core.rs`)

- `StateSnapshot` - BTreeMap-based deterministic state representation
- `RecordedStep` - Complete capture of a single execution step
- `ExecutionRecorder` - Records execution for replay
- `ExecutionReplayer` - Replays recorded sequences
- `ReceiptChainVerifier` - Verifies cryptographic integrity (with `receipts` feature)

### Debugger (`debugger.rs`)

- `ReplayDebugger` - Interactive debugger with navigation and inspection
- `DebuggerConfig` - Display and behavior configuration
- `StepResult` - Result of navigation operations
- `StepReport` - Detailed report of a single step

## Usage Example

```rust
use a2a_rs::construct::replay::{ExecutionRecorder, ReplayDebugger, StateSnapshot};
use a2a_rs::construct::runtime::{Operation, Runtime};

// Record execution
let mut runtime = Runtime::default_runtime();
let mut recorder = ExecutionRecorder::new();

// Execute operations and record
for op in operations {
    let state_before = StateSnapshot::empty();
    let output = runtime.handle(op.clone()).unwrap();
    let state_after = StateSnapshot::empty();

    recorder.record_step(op, state_before, state_after, output.receipt);
}

// Create debugger from recording
let mut debugger = ReplayDebugger::from_recorder(&recorder);

// Navigate through execution
debugger.step_forward();
debugger.step_forward();

// Inspect current state
if let Some(report) = debugger.inspect_current() {
    println!("Step {}: {}", report.step_number, report.operation);
    println!("Success: {}", report.execution_success);
}

// Set breakpoint and continue
debugger.add_breakpoint(5);
debugger.continue_execution();

// Compare states
if let Some(diff) = debugger.diff_states(0, 5) {
    if !diff.is_identical {
        println!("States differ: {:?}", diff.differences);
    }
}

// Jump to specific step
debugger.goto_step(10);

// Jump to receipt (with receipts feature)
#[cfg(feature = "receipts")]
debugger.goto_receipt("abc123...");

// Get debugger status
let status = debugger.status();
println!("At step {:?} of {}", status.current_position, status.total_steps);
```

## Debugger Commands

| Method | Description |
|--------|-------------|
| `step_forward()` | Move to next step |
| `step_back()` | Move to previous step |
| `goto_step(n)` | Jump to step n |
| `goto_receipt(hash)` | Jump to receipt by hash |
| `reset()` | Return to beginning |
| `goto_end()` | Jump to last step |
| `add_breakpoint(n)` | Add breakpoint at step n |
| `remove_breakpoint(n)` | Remove breakpoint |
| `clear_breakpoints()` | Remove all breakpoints |
| `continue_execution()` | Run until next breakpoint or end |
| `current_step()` | Get current RecordedStep |
| `current_state()` | Get current StateSnapshot |
| `inspect_current()` | Get detailed StepReport |
| `diff_states(a, b)` | Compare two steps |
| `diff_with_current(n)` | Compare step n with current |
| `diff_with_previous()` | Compare current with previous |
| `search(predicate)` | Find steps matching condition |
| `search_by_operation(type)` | Find steps by operation type |
| `list_all_steps()` | Get summary of all steps |
| `export_state()` | Serialize debugger state to JSON |

## Breakpoints

Breakpoints pause execution when stepping through a recording:

```rust
// Add breakpoint at step 10
debugger.add_breakpoint(10);

// Continue until breakpoint
match debugger.continue_execution() {
    StepResult::BreakpointHit { at } => println!("Hit breakpoint at {}", at),
    StepResult::AtBoundary => println!("Reached end"),
    _ => {}
}

// List all breakpoints
for bp in debugger.list_breakpoints() {
    println!("Breakpoint at step {}", bp);
}

// Clear all
debugger.clear_breakpoints();
```

## State Diffing

Compare states to detect non-determinism:

```rust
// Compare two specific steps
if let Some(diff) = debugger.diff_states(0, 10) {
    if !diff.is_identical {
        for difference in diff.differences {
            match difference {
                DifferenceKind::TasksMismatch { left_count, right_count } => {
                    println!("Task count: {} vs {}", left_count, right_count);
                }
                DifferenceKind::MessagesMismatch { .. } => {
                    println!("Message mismatch detected");
                }
                _ => {}
            }
        }
    }
}

// Compare current with previous
if let Some(diff) = debugger.diff_with_previous() {
    // Inspect what changed in this step
}
```

## Search

Find steps matching specific criteria:

```rust
// Find all CreateTask operations
let create_steps = debugger.search_by_operation("CreateTask");

// Find steps with custom predicate
let failing_steps = debugger.search(|step| {
    !step.execution_receipt.success
});

// Find steps that modified task count
let state_changes = debugger.search(|step| {
    step.state_before.tasks.len() != step.state_after.tasks.len()
});
```

## Configuration

Customize debugger display and behavior:

```rust
let config = DebuggerConfig {
    verbose_receipts: true,    // Show full receipt details
    verbose_state: true,       // Show full state snapshots
    max_diff_lines: 100,       // Lines to show in diffs
    break_on_diff: true,       // Auto-break on state differences
};

debugger.configure(config);
```

## Integration with Testing

Use the debugger in tests to diagnose determinism failures:

```rust
#[test]
fn test_deterministic_execution() {
    // Record two executions of the same operations
    let recording1 = record_execution(&ops);
    let recording2 = record_execution(&ops);

    // Create debuggers
    let mut debugger1 = ReplayDebugger::new(recording1);
    let mut debugger2 = ReplayDebugger::new(recording2);

    // Step through both and compare
    for _ in 0..ops.len() {
        debugger1.step_forward();
        debugger2.step_forward();

        let state1 = debugger1.current_state().unwrap();
        let state2 = debugger2.current_state().unwrap();

        let diff = state1.diff(state2);
        assert!(diff.is_identical, "Non-determinism at step {}",
                debugger1.current_position().unwrap());
    }
}
```

## Receipt Chain Verification

With the `receipts` feature enabled, verify cryptographic integrity:

```rust
#[cfg(feature = "receipts")]
{
    use a2a_rs::construct::replay::ReceiptChainVerifier;

    let chain = recorder.receipt_chain();

    // Verify chain integrity
    ReceiptChainVerifier::verify_chain(&chain)
        .expect("Receipt chain should be valid");

    // Compare two executions
    let chain1 = recorder1.receipt_chain();
    let chain2 = recorder2.receipt_chain();

    assert!(ReceiptChainVerifier::verify_identical_chains(&chain1, &chain2),
            "Receipt chains should match for deterministic execution");
}
```

## Architecture Notes

### BTreeMap for Determinism

All state snapshots use `BTreeMap` instead of `HashMap` to ensure:
- Deterministic serialization order
- Consistent hashing across runs
- Reproducible comparisons

### Separation from Test Module

The replay module is a production component, not just for testing:
- Core types in `construct/replay/core.rs`
- Debugger in `construct/replay/debugger.rs`
- Tests import from replay module (not vice versa)

This enables:
- Using the debugger in production for diagnostics
- Recording and analyzing real execution sequences
- Debugging non-determinism in deployed systems
