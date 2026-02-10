# CONSTRUCT Observability

## Overview

The observability module provides comprehensive tracing, metrics, and instrumentation for the CONSTRUCT runtime execution pipeline. It enables production monitoring, performance analysis, and debugging without sacrificing determinism or adding overhead when disabled.

## Architecture

The observability layer instruments four key subsystems:

1. **Runtime Executor** - μ(O) execution pipeline stages
2. **Scheduler** - Λ task ordering and execution decisions
3. **Guards** - Admission control and refusal tracking
4. **Invariants** - State validation and violation detection

## Components

### RuntimeMetrics

Thread-safe metrics registry using atomic counters:

```rust
use a2a_rs::construct::observability::RuntimeMetrics;

let metrics = RuntimeMetrics::new();

// Record operations
metrics.record_guard_admission("type_guard");
metrics.record_guard_rejection("range_guard", "value out of bounds");
metrics.record_invariant_passed("task_state");
metrics.record_task_submitted("task-1", "normal");

// Get snapshot
let snapshot = metrics.snapshot();
println!("Guard rejection rate: {:.2}%", snapshot.guard_rejection_rate() * 100.0);
```

### ObservabilityContext

Correlation context for distributed tracing:

```rust
use a2a_rs::construct::observability::ObservabilityContext;

let ctx = ObservabilityContext::new(
    "exec-123".to_string(),
    policy_epoch,
);

// Create tracing spans
let _span = ctx.runtime_span("create_task");
let _guard_span = ctx.guard_span("admission_guard");

// Track timing
let timing = ctx.create_timing("operation_name".to_string());
```

### InstrumentedGuard

Wrapper that adds observability to any `Guard`:

```rust
use a2a_rs::construct::{
    guards::{Guard, TypeGuard},
    observability::{InstrumentedGuard, RuntimeMetrics},
};

let metrics = RuntimeMetrics::new();
let guard = TypeGuard::new("string".to_string());
let instrumented = InstrumentedGuard::new(guard, metrics.clone());

// Now automatically tracks admissions/rejections
let result = instrumented.check(&input, "context", epoch);
```

### InstrumentedInvariant

Wrapper that adds observability to any `Invariant`:

```rust
use a2a_rs::construct::{
    invariants::{Invariant, TaskStateInvariant},
    observability::{InstrumentedInvariant, RuntimeMetrics},
};

let metrics = RuntimeMetrics::new();
let invariant = TaskStateInvariant::new();
let instrumented = InstrumentedInvariant::new(invariant, metrics.clone());

// Now automatically tracks passes/failures
let result = instrumented.check(&task);
```

## Metrics Tracked

### Guard Metrics
- `guard_admissions` - Total inputs admitted
- `guard_rejections` - Total inputs rejected
- `admission_errors` - Errors during guard evaluation

### Invariant Metrics
- `invariant_checks_passed` - Successful invariant checks
- `invariant_checks_failed` - Failed invariant checks
- `invariant_errors` - Errors during invariant checking

### Scheduler Metrics
- `tasks_submitted` - Tasks added to scheduler
- `tasks_completed` - Tasks successfully completed
- `tasks_cancelled` - Tasks cancelled before completion
- `scheduler_selections` - Scheduler decision points

### Runtime Metrics
- `type_check_errors` - Type validation failures
- `transformation_errors` - State transformation failures
- `execution_errors` - Execution stage failures

## Tracing Integration

When the `tracing` feature is enabled, all operations emit structured spans:

```rust
// Runtime execution span
tracing::info_span!("runtime_execution",
    execution_id = %exec_id,
    operation = "create_task",
    policy_epoch = 1
);

// Stage span
tracing::info_span!("runtime_stage",
    execution_id = %exec_id,
    stage = "type_check"
);

// Guard evaluation span
tracing::info_span!("guard_evaluation",
    execution_id = %exec_id,
    guard = "type_guard",
    policy_epoch = 1
);

// Invariant check span
tracing::info_span!("invariant_check",
    execution_id = %exec_id,
    invariant = "task_state"
);
```

## Performance Analysis

### Operation Timing

Track execution time across pipeline stages:

```rust
use a2a_rs::construct::observability::OperationTiming;

let mut timing = OperationTiming::new("create_task".to_string(), duration);
timing.add_stage("type_check".to_string(), 10);
timing.add_stage("admission_guard".to_string(), 20);
timing.add_stage("transformations".to_string(), 30);

// Find bottlenecks
if let Some((stage, duration_ms)) = timing.slowest_stage() {
    println!("Bottleneck: {} took {}ms", stage, duration_ms);
}
```

### Metrics Snapshot

Point-in-time metrics for analysis:

```rust
let snapshot = metrics.snapshot();

println!("Total errors: {}", snapshot.total_errors());
println!("Guard rejection rate: {:.2}%", snapshot.guard_rejection_rate() * 100.0);
println!("Invariant failure rate: {:.2}%", snapshot.invariant_failure_rate() * 100.0);
println!("Task completion rate: {:.2}%", snapshot.task_completion_rate() * 100.0);
```

## Feature Flags

All observability features are behind the `tracing` feature flag:

```toml
[dependencies]
a2a-rs = { version = "0.1", features = ["tracing"] }
```

When disabled, observability has **zero runtime cost** - all instrumentation compiles away.

## Usage Example

See [`examples/observability_demo.rs`](../../../examples/observability_demo.rs) for a complete demonstration.

```bash
RUST_LOG=debug cargo run --example observability_demo --features tracing
```

## Integration with Runtime

The observability layer is designed to integrate seamlessly with the runtime:

```rust
use a2a_rs::construct::{
    observability::{ObservabilityContext, RuntimeMetrics},
    runtime::Runtime,
};

// Create shared metrics
let metrics = RuntimeMetrics::new();

// Create runtime with instrumented guards/invariants
let runtime = Runtime::new(
    ontology,
    scheduler,
    vec![Box::new(InstrumentedGuard::new(guard, metrics.clone()))],
    registry_with_instrumented_invariants,
);

// Track execution
let ctx = ObservabilityContext::with_metrics(exec_id, epoch, metrics);
let _span = ctx.runtime_span("handle_operation");
let output = runtime.handle(operation)?;

// Analyze results
let snapshot = ctx.metrics.snapshot();
println!("Execution stats: {:?}", snapshot);
```

## Best Practices

1. **Share metrics across runtime instances** - Create one `RuntimeMetrics` instance and clone it for all components
2. **Use instrumented wrappers** - Wrap guards and invariants at registration time
3. **Create contexts per execution** - Each runtime operation gets its own `ObservabilityContext`
4. **Sample in production** - Use tracing subscriber filtering to control overhead
5. **Export snapshots** - Serialize `MetricsSnapshot` for external monitoring systems

## Determinism Guarantees

The observability layer is designed to maintain CONSTRUCT's determinism guarantees:

- Metrics use atomic counters (no locks)
- Tracing is side-effect free (writes to external subscriber)
- Instrumentation wrappers preserve deterministic behavior
- All timing is for observability only, never affects execution

## Future Extensions

Planned enhancements:

- [ ] Prometheus metrics exporter
- [ ] OpenTelemetry span propagation
- [ ] Histogram metrics for latency percentiles
- [ ] Custom metric collectors via trait
- [ ] Async-aware span propagation
- [ ] Metrics aggregation across stations
