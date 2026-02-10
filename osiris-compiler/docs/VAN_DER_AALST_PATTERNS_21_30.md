# Van der Aalst Workflow Patterns 21-30: Multiple Instance and Advanced Control Flow

This document describes the implementation of van der Aalst patterns 21-30 in the osiris-compiler workflow kernel. Patterns 21-30 focus on multiple instance execution, dynamic control flow, and advanced loop/recursion constructs.

## Patterns 21-24: Multiple Instance (MI) Patterns

### Pattern 21: Multiple Instances Without Synchronization

**Definition**: Spawns multiple instances of an activity that execute independently without waiting for completion.

**Characteristics**:
- Fire-and-forget execution model
- No join synchronization required
- Each instance executes to completion independently
- Parent workflow continues without waiting

**Implementation**:
```rust
GatewayPattern::MultipleInstancesNoSync { config }
```

**Configuration**:
- `collection`: Variable containing array of items to process
- `item_variable`: Variable name for current item in iteration
- `activity_id`: Activity to execute for each item
- `asynchronous`: Whether to spawn async tasks (default: true)

**Use Cases**:
- Sending notifications to multiple recipients
- Triggering parallel background jobs
- Publishing events to multiple subscribers

**Example**:
```rust
let config = MultiInstanceWithoutSyncConfig {
    collection: "recipients".to_string(),
    item_variable: "current_recipient".to_string(),
    activity_id: NodeId::new("send_notification"),
    asynchronous: true,
};

kernel
    .execute_multiple_instances_no_sync(&instance_id, &config)
    .await?;
```

---

### Pattern 22: Multiple Instances with a Priori Design-Time Knowledge

**Definition**: Cardinality (number of instances) known at design time. Creates a fixed number of instances.

**Characteristics**:
- Number of instances defined in workflow definition
- No runtime determination needed
- Predictable execution pattern
- Synchronization point before proceeding

**Implementation**:
```rust
GatewayPattern::MultipleInstancesDesignTime {
    cardinality: 5,
    activity_id: NodeId::new("process_item"),
}
```

**Key Points**:
- Cardinality is a constant value (e.g., 5 instances)
- All instances created upfront
- Provides determinism in execution count

**Use Cases**:
- Processing a known set of parallel tasks
- Fan-out to fixed number of servers
- Fixed-size batch processing

**Example**:
```rust
kernel
    .execute_multiple_instances_design_time(&instance_id, 5, &activity_id)
    .await?;
```

---

### Pattern 23: Multiple Instances with a Priori Runtime Knowledge

**Definition**: Cardinality determined at runtime from context variables.

**Characteristics**:
- Cardinality evaluated at execution time
- Determined from context (e.g., array size, counter variable)
- Dynamic but known before activity execution
- More flexible than pattern 22

**Implementation**:
```rust
GatewayPattern::MultipleInstancesRuntime {
    cardinality_expression: "item_count".to_string(),
    activity_id: NodeId::new("process_item"),
}
```

**Expression Evaluation**:
- Direct variable lookup: `"item_count"` → gets value from context
- Numeric literals: `"5"` → parsed as constant
- Collection size: `"items"` → length of array in context

**Use Cases**:
- Processing variable-length collections
- Dynamic request fanout
- Adaptive parallelization based on input size

**Example**:
```rust
let mut context = HashMap::new();
context.insert("count".to_string(), serde_json::Value::Number(3.into()));

kernel
    .execute_multiple_instances_runtime(&instance_id, "count", &activity_id)
    .await?;
```

---

### Pattern 24: Multiple Instances with Synchronization

**Definition**: Spawns multiple instances and waits for all to complete (or meet a condition).

**Characteristics**:
- Creates multiple concurrent execution paths
- Synchronization point waits for completion
- Configurable merge strategies
- Join semantics for combining results

**Implementation**:
```rust
GatewayPattern::MultipleInstancesWithSync { config }
```

**Merge Strategies**:
1. `"all_complete"` - Wait for all instances to complete
2. `"one_complete"` - Proceed after first instance completes
3. `"threshold"` - Proceed when X% of instances complete

**Configuration**:
```rust
pub struct MultiInstanceWithSyncConfig {
    pub collection: String,
    pub item_variable: String,
    pub activity_id: NodeId,
    pub completion_condition: Option<String>,
    pub merge_strategy: String,
    pub completion_threshold: Option<u32>, // 0-100 for threshold strategy
}
```

**Use Cases**:
- Fan-out/fan-in patterns
- Parallel approval workflows
- Aggregation from multiple sources

**Example**:
```rust
let config = MultiInstanceWithSyncConfig {
    collection: "tasks".to_string(),
    item_variable: "current_task".to_string(),
    activity_id: NodeId::new("execute_task"),
    completion_condition: None,
    merge_strategy: "all_complete".to_string(),
    completion_threshold: None,
};

kernel
    .execute_multiple_instances_with_sync(&instance_id, &config)
    .await?;
```

---

## Pattern 25: Cancelling Multiple Instances

**Definition**: Cancels all active instances when a condition is met.

**Characteristics**:
- Condition-based cancellation trigger
- Can target specific activities
- Useful for handling errors or timeouts
- Prevents wasted resource usage

**Implementation**:
```rust
GatewayPattern::CancelMultipleInstances {
    cancel_condition: "error_occurred".to_string(),
    target_activities: vec![
        NodeId::new("activity_1"),
        NodeId::new("activity_2"),
    ],
}
```

**Context**:
```rust
let mut context = HashMap::new();
context.insert("error_occurred".to_string(), serde_json::Value::Bool(true));
kernel.update_context(&instance_id, context).await?;

kernel
    .execute_cancel_multiple_instances(
        &instance_id,
        "error_occurred",
        &target_activities,
    )
    .await?;
```

**Use Cases**:
- Halt parallel processing on critical failure
- Timeout handling
- User-initiated cancellation
- Resource cleanup on error

---

## Pattern 26: Dynamic Parallel Split

**Definition**: Routes to multiple nodes determined dynamically at runtime.

**Characteristics**:
- Routes not fixed at design time
- Determined by context expressions
- Flexible branching based on data
- Extension of pattern 2 (parallel split)

**Implementation**:
```rust
GatewayPattern::DynamicParallelSplit {
    routing_expression: "target_paths".to_string(),
}
```

**Context Format**:
```rust
context.insert(
    "target_paths".to_string(),
    serde_json::json!(vec!["path_a", "path_b", "path_c"]),
);
```

**Use Cases**:
- Content-based routing
- Service mesh with dynamic paths
- Multi-tenant workflow routing
- Conditional fan-out patterns

---

## Pattern 27: Structured Loop

**Definition**: Enables repeated execution of activities with explicit loop control.

**Characteristics**:
- Loop condition evaluated at each iteration
- Optional iteration limit (safeguard)
- Explicit loop variable tracking
- Clean loop semantics

**Implementation**:
```rust
GatewayPattern::StructuredLoop {
    loop_condition: "continue_processing".to_string(),
    loop_back_node: NodeId::new("process_item"),
    max_iterations: Some(100), // Safeguard against infinite loops
}
```

**Loop Context**:
```rust
// Automatically managed by kernel
context.insert("loop_iteration", Value::Number(n));
```

**Example**:
```rust
kernel
    .execute_structured_loop(
        &instance_id,
        "continue_processing",
        &NodeId::new("loop_body"),
        Some(10), // Max 10 iterations
    )
    .await?;
```

**Use Cases**:
- Batch processing with retry logic
- Iterative data processing
- State machine loops
- Cleanup iterations

---

## Pattern 28: Recursion

**Definition**: Allows recursive invocation of workflow subprocess with base case and recursive case.

**Characteristics**:
- Two-condition model: base case vs recursive case
- Optional recursion depth limit (safeguard)
- Supports nested workflow invocations
- Useful for hierarchical structures

**Implementation**:
```rust
GatewayPattern::Recursion {
    recursive_workflow_id: WorkflowId::new("process_tree"),
    base_condition: "is_leaf".to_string(),
    recursive_condition: "has_children".to_string(),
    max_depth: Some(10), // Safeguard
}
```

**Execution Semantics**:
1. Evaluate `base_condition`: if true, use base case (exit recursion)
2. Evaluate `recursive_condition`: if true and depth < max, recurse
3. Increment `recursion_depth` on recursive path
4. Set `recursion_status` to "base_case" or "recursive_case"

**Use Cases**:
- Tree traversal algorithms
- Hierarchical document processing
- Recursive approval chains
- Nested structure handling

**Example**:
```rust
kernel
    .execute_recursion(
        &instance_id,
        &WorkflowId::new("tree_processor"),
        "is_leaf",      // Base: if node is leaf
        "!is_leaf",     // Recursive: if not leaf
        Some(5),        // Max depth
    )
    .await?;
```

---

## Pattern 29: Termination Trigger

**Definition**: Immediately terminates the entire workflow instance when condition is met.

**Characteristics**:
- Abrupt termination (not graceful completion)
- Condition-based trigger
- No cleanup/compensation logic
- Highest priority control flow

**Implementation**:
```rust
GatewayPattern::TerminationTrigger {
    termination_condition: "fatal_error".to_string(),
}
```

**Context Update**:
```rust
let mut context = HashMap::new();
context.insert("fatal_error".to_string(), serde_json::Value::Bool(true));
kernel.update_context(&instance_id, context).await?;

kernel
    .execute_termination_trigger(&instance_id, "fatal_error")
    .await?;
```

**Result**:
- Instance state transitions to `Terminated`
- All active nodes cleared
- No further execution
- Timestamp recorded

**Use Cases**:
- Emergency stop/panic conditions
- Unrecoverable error handling
- Emergency shutdown
- System-level circuit breaker

---

## Pattern 30: Transient Trigger

**Definition**: Triggers an activity based on a temporary condition that may change during execution.

**Characteristics**:
- Condition-based activity activation
- Optional timeout for transient state
- Useful for event-driven execution
- Orthogonal to main flow

**Implementation**:
```rust
GatewayPattern::TransientTrigger {
    trigger_condition: "alert_received".to_string(),
    triggered_activity: NodeId::new("handle_alert"),
    timeout_ms: Some(5000), // Optional timeout
}
```

**Execution Semantics**:
1. Check `trigger_condition` against context
2. If true, activate `triggered_activity`
3. Optional timeout tracking for transient state

**Context**:
```rust
let mut context = HashMap::new();
context.insert("alert_received".to_string(), serde_json::Value::Bool(true));
kernel.update_context(&instance_id, context).await?;

kernel
    .execute_transient_trigger(
        &instance_id,
        "alert_received",
        &NodeId::new("handle_alert"),
        Some(5000),
    )
    .await?;
```

**Use Cases**:
- Exception handling without interrupting main flow
- Alert/notification handling
- Event-driven side activities
- Non-blocking exception management

---

## Context Variables and State Management

### Pattern-Specific Context Variables

| Pattern | Variables | Purpose |
|---------|-----------|---------|
| 21-24   | `mi_XX_index`, `mi_XX_total` | Instance tracking |
| 22      | `mi_22_cardinality` | Design-time count |
| 23      | `mi_23_cardinality` | Runtime-evaluated count |
| 24      | `mi_24_total`, `mi_24_index` | Sync tracking |
| 27      | `loop_iteration` | Current loop iteration number |
| 28      | `recursion_depth`, `recursion_status` | Recursion tracking |

### Automatic Updates
- `loop_iteration`: Incremented on each loop execution
- `recursion_depth`: Incremented on recursive invocation
- `mi_XX_index`: Set for each MI instance
- `recursion_status`: Set to "base_case" or "recursive_case"

---

## Implementation Details

### Adapter Methods

All patterns implement execute methods on `InMemoryWorkflowKernel`:

```rust
pub async fn execute_multiple_instances_no_sync(...) -> WorkflowResult<()>
pub async fn execute_multiple_instances_design_time(...) -> WorkflowResult<()>
pub async fn execute_multiple_instances_runtime(...) -> WorkflowResult<()>
pub async fn execute_multiple_instances_with_sync(...) -> WorkflowResult<()>
pub async fn execute_cancel_multiple_instances(...) -> WorkflowResult<()>
pub async fn execute_structured_loop(...) -> WorkflowResult<()>
pub async fn execute_recursion(...) -> WorkflowResult<()>
pub async fn execute_termination_trigger(...) -> WorkflowResult<()>
pub async fn execute_transient_trigger(...) -> WorkflowResult<()>
```

### Gateway Integration

All patterns are handled in `execute_gateway()` method:

```rust
pub async fn execute_gateway(
    &mut self,
    instance_id: &str,
    node_id: &NodeId,
) -> WorkflowResult<Vec<NodeId>>
```

Pattern evaluation returns activated nodes to proceed in the workflow.

---

## Thread Safety and Concurrency

- All MI patterns use `Arc<RwLock<HashMap>>` for instance storage
- Read-heavy operations use `read()` locks
- Write operations (context updates, state changes) use `write()` locks
- Lock releases with explicit `drop()` to minimize hold time

---

## Error Handling

All methods return `WorkflowResult<T>` with detailed errors:

- `InstanceNotFound`: Referenced instance doesn't exist
- `ExecutionError`: Invalid collection format, missing variables
- `GatewayEvaluationFailed`: Pattern execution failed

---

## Testing

10 comprehensive tests covering:
1. Pattern 21: Multiple instances without sync
2. Pattern 22: Design-time cardinality
3. Pattern 23: Runtime cardinality
4. Pattern 24: Synchronized multiple instances
5. Pattern 25: Cancelling multiple instances
6. Pattern 26: Dynamic parallel split
7. Pattern 27: Structured loop
8. Pattern 28: Recursion
9. Pattern 29: Termination trigger
10. Pattern 30: Transient trigger

Each test validates:
- Correct context variable updates
- Proper instance state transitions
- Event recording in execution history
- Condition evaluation

---

## References

- **van der Aalst et al.**: "Workflow Patterns: The Definitive Guide"
- **Patterns 21-24**: Multiple Instance patterns (MI)
- **Patterns 25-30**: Advanced control flow and exception handling
