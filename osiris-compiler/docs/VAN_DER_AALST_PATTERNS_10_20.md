# van der Aalst Workflow Patterns 10-20 Implementation

## Overview

This document describes the implementation of workflow patterns 10-20 from van der Aalst's workflow patterns classification in the `osiris-compiler` workflow kernel. These patterns extend the basic control flow patterns (2-9) with advanced features for complex workflow scenarios.

**Reference**: van der Aalst, W. M. P. (2003). "Workflow Patterns" - https://www.workflowpatterns.com/

## Pattern Implementation Map

| Pattern | Name | Type | Implementation |
|---------|------|------|-----------------|
| 2 | Parallel Split (AND-split) | Control Flow | `GatewayPattern::ParallelSplit` |
| 3 | Synchronization (AND-join) | Control Flow | `GatewayPattern::Synchronization` |
| 4 | Exclusive Choice (XOR-split) | Control Flow | `GatewayPattern::ExclusiveChoice` |
| 5 | Simple Merge (XOR-join) | Control Flow | `GatewayPattern::SimpleMerge` |
| 6 | Multi-Choice (OR-split) | Control Flow | `GatewayPattern::MultiChoice` |
| 7 | Structured Synchronizing Merge (OR-join) | Control Flow | `GatewayPattern::StructuredSynchronizingMerge` |
| 8 | Multi-Merge | Control Flow | `GatewayPattern::MultiMerge` |
| 9 | Structured Discriminator | Control Flow | `GatewayPattern::StructuredDiscriminator` |
| 10 | Arbitrary Cycles | Advanced Flow | `GatewayPattern::ArbitraryCycle` |
| 11 | Implicit Termination | Advanced Flow | `GatewayPattern::ImplicitTermination` |
| 12-14 | Multiple Instance Patterns | Advanced Flow | `execute_multi_instance()` |
| 15 | Deferred Choice | Advanced Flow | `GatewayPattern::DeferredChoice` |
| 16 | Interleaved Parallel Routing | Advanced Flow | `GatewayPattern::InterleavedParallelRouting` |
| 17 | Milestone | Advanced Flow | `GatewayPattern::Milestone` |
| 18 | Critical Section | Advanced Flow | `GatewayPattern::CriticalSection` |
| 19 | Cancel Activity | Exception Handling | `execute_cancellation()` |
| 20 | Escalation | Exception Handling | `trigger_escalation()` |

## Detailed Pattern Descriptions

### Pattern 2-9: Basic Control Flow Patterns

All basic control flow patterns are implemented in the `execute_gateway()` method in `InMemoryWorkflowKernel`.

#### Pattern 2: Parallel Split (AND-split)
```rust
GatewayPattern::ParallelSplit
// Activates ALL outgoing paths concurrently
```

#### Pattern 3: Synchronization (AND-join)
```rust
GatewayPattern::Synchronization
// Waits for ALL incoming paths to complete before proceeding
```

#### Pattern 4: Exclusive Choice (XOR-split)
```rust
GatewayPattern::ExclusiveChoice { conditions: Vec<Condition> }
// Evaluates conditions and selects exactly ONE outgoing path
```

#### Pattern 5: Simple Merge (XOR-join)
```rust
GatewayPattern::SimpleMerge
// Waits for ANY ONE incoming path to complete
```

#### Pattern 6: Multi-Choice (OR-split)
```rust
GatewayPattern::MultiChoice { conditions: Vec<Condition> }
// Evaluates conditions and selects ONE OR MORE outgoing paths
```

#### Pattern 7: Structured Synchronizing Merge (OR-join)
```rust
GatewayPattern::StructuredSynchronizingMerge
// Waits for ALL ACTIVE incoming paths (those that were taken)
```

#### Pattern 8: Multi-Merge
```rust
GatewayPattern::MultiMerge
// Activates for EACH incoming path independently (no join semantics)
```

#### Pattern 9: Structured Discriminator
```rust
GatewayPattern::StructuredDiscriminator { reset_after: Option<NodeId> }
// Waits for first incoming path, ignores subsequent arrivals
// Optional reset node for clearing the "fired" state
```

### Pattern 10: Arbitrary Cycles

**Purpose**: Allow loops and arbitrary cycle structures in workflows.

**Definition**: A workflow that can have arbitrary cycle structures without being restricted to structured loops.

**Implementation**:
```rust
GatewayPattern::ArbitraryCycle { back_edge_to: NodeId }
// Allows loop-back to a specified node based on continuation condition
```

**Execution Logic**:
1. Evaluate condition in context (e.g., `continue_loop` flag)
2. If true, add back-edge target to activated nodes
3. Create loop structure with proper token flow

**Example**:
```rust
// Setup: Activity -> Decision Gateway -> Activity (loop back to start)
//        |
//        +-> Exit Activity

// If "continue_loop" == true:
//   - Proceed to next activity AND loop back
// If "continue_loop" == false:
//   - Only proceed to exit activity
```

**Use Cases**:
- Retry logic with configurable attempts
- Batch processing loops
- Iterative refinement workflows

### Pattern 11: Implicit Termination

**Purpose**: Workflows terminate implicitly when no more enabled nodes exist.

**Definition**: No explicit end event required; workflow ends when all active paths reach completion.

**Implementation**:
```rust
GatewayPattern::ImplicitTermination
// Allows natural termination without explicit end nodes
```

**Execution Logic**:
1. Gateway allows tokens to flow naturally
2. When `execute_step()` finds no enabled nodes, instance completes
3. Transition from Active to Completed state

**Example**:
```rust
// Workflow with parallel paths, no explicit join or end
Activity_A -> (splits to) Activity_B and Activity_C
Activity_B completes -> no more enabled nodes
Activity_C completes -> no more enabled nodes
=> Workflow completes implicitly
```

### Patterns 12-14: Multiple Instance Patterns

**Purpose**: Execute an activity multiple times for each item in a collection.

**Implementation**: `execute_multi_instance()` method

#### Pattern 12: MI without Synchronization
```rust
MultiInstanceMode::Sequential
// Execute activities one after another
```

#### Pattern 13: MI with a priori Design Time Knowledge
```rust
MultiInstanceMode::ParallelStatic
// Cardinality known at design time
```

#### Pattern 14: MI with a priori Runtime Knowledge
```rust
MultiInstanceMode::ParallelDynamic or MultiInstanceMode::Parallel
// Cardinality known at runtime
```

**Configuration**:
```rust
MultiInstanceConfig {
    mode: MultiInstanceMode,
    collection: String,          // Context variable containing array
    item_variable: String,        // Variable name for current item
    completion_condition: Option<String>, // Optional early completion
}
```

**Execution**:
```rust
// For each item in collection:
//   1. Set current_item variable
//   2. Set mi_index variable
//   3. Execute activity
//   4. Check completion condition
```

**Example**:
```rust
// Process purchase orders
ctx["orders"] = [order1, order2, order3]
MultiInstanceConfig {
    mode: Sequential,
    collection: "orders",
    item_variable: "current_order",
    completion_condition: None,
}

// Executes:
// current_order = order1, mi_index = 0 -> Process
// current_order = order2, mi_index = 1 -> Process
// current_order = order3, mi_index = 2 -> Process
```

### Pattern 15: Deferred Choice

**Purpose**: Dynamic choice determined by which event occurs first (runtime selection).

**Definition**: Unlike exclusive choice, the decision is not made upfront but deferred until an event arrives.

**Implementation**:
```rust
GatewayPattern::DeferredChoice {
    event_conditions: Vec<Condition>,
    timeout_ms: Option<u64>,
}
```

**Execution Logic**:
1. Wait for first matching event condition
2. Activate corresponding path
3. If timeout specified and no event occurs, use default path
4. Cancel other potential paths

**Example**:
```rust
// Order fulfillment
GatewayPattern::DeferredChoice {
    event_conditions: vec![
        Condition { expression: "express_selected", target: NodeId("express_path") },
        Condition { expression: "standard_selected", target: NodeId("standard_path") },
    ],
    timeout_ms: Some(3600000), // 1 hour timeout
}

// Customer can choose at any time during the window
// First choice wins, activates corresponding path
```

### Pattern 16: Interleaved Parallel Routing

**Purpose**: Parallel execution without mandatory synchronization points.

**Definition**: Paths execute in parallel but can complete independently at different times.

**Implementation**:
```rust
GatewayPattern::InterleavedParallelRouting
// Parallel paths with no forced join point
```

**Execution Logic**:
1. Activate all outgoing paths (like parallel split)
2. No mandatory synchronization
3. Paths complete independently
4. Workflow continues as each path completes

**Example**:
```rust
// Order processing
Receive Order
  -> (parallel without mandatory join)
     -> Send Confirmation
     -> Update Inventory
     -> Process Payment
  -> Each completes independently
  -> Workflow may complete before all paths finish
```

**Use Cases**:
- Fire-and-forget notifications
- Asynchronous side effects
- Independent compensations

### Pattern 17: Milestone

**Purpose**: Enable an activity only when a specific condition becomes true.

**Definition**: An activity is in the "enabled" state but only activated when its milestone condition is satisfied.

**Implementation**:
```rust
GatewayPattern::Milestone {
    condition: String,
    monitor_node: Option<NodeId>,
}
```

**Execution Logic**:
1. Activity is ready but not enabled
2. Continuously check milestone condition
3. When condition becomes true, activate activity
4. Optionally monitor specific node for condition changes

**Example**:
```rust
GatewayPattern::Milestone {
    condition: "payment_received",
    monitor_node: Some(NodeId("payment_processor")),
}

// Activity "Ship Order" is available but only activates when:
// context["payment_received"] == true
// Condition is monitored during payment processing
```

**Use Cases**:
- Conditional approval workflows
- Waiting for external conditions
- Gate-based activity enablement

### Pattern 18: Critical Section

**Purpose**: Enforce mutual exclusion (mutex-like behavior) in workflow execution.

**Definition**: Only one instance or path can execute a critical section at a time.

**Implementation**:
```rust
GatewayPattern::CriticalSection { section_id: String }
// Mutex-like protection for exclusive activity execution
```

**Execution Logic**:
1. Check if critical section is free
2. If free, acquire lock: `instance.context["critical_section"] = section_id`
3. If busy, block until released
4. Release lock when exiting critical section

**Helper Methods**:
```rust
async fn is_critical_section_free(&self, section_id: &str) -> bool
async fn acquire_critical_section(&self, instance_id: &str, section_id: &str) -> Result<()>
async fn release_critical_section(&self, instance_id: &str) -> Result<()>
```

**Example**:
```rust
GatewayPattern::CriticalSection { section_id: "database_write" }

// Multiple instances may try to access
// Only one at a time can enter the critical section
// Others wait for release

// Thread 1: Acquire "database_write" -> Process -> Release
// Thread 2: Waiting for "database_write"...
// Thread 1: Releases -> Thread 2: Acquires
```

**Use Cases**:
- Database transaction serialization
- Shared resource protection
- Ensuring sequential execution of critical operations

### Pattern 19: Cancel Activity

**Purpose**: Cancel activities within a cancellation region based on trigger conditions.

**Definition**: Exception handling that cancels a set of activities when a cancellation event occurs.

**Implementation**:
```rust
async fn execute_cancellation(
    &mut self,
    instance_id: &str,
    region: &CancellationRegion,
) -> WorkflowResult<()>
```

**CancellationRegion Structure**:
```rust
struct CancellationRegion {
    region_id: String,
    nodes: Vec<NodeId>,  // Activities to cancel
    trigger: CancellationTrigger,
}

enum CancellationTrigger {
    Event { event_code: String },
    Timeout { duration_ms: u64 },
    Condition { expression: String },
}
```

**Execution Logic**:
1. Evaluate cancellation trigger
2. If triggered: remove all nodes in region from active set
3. Record cancellation event
4. Continue with remaining active paths

**Example**:
```rust
CancellationRegion {
    region_id: "approval_region",
    nodes: vec![NodeId("waiting_for_approval")],
    trigger: CancellationTrigger::Event {
        event_code: "approval_timeout"
    }
}

// When "approval_timeout" event is triggered:
// -> Cancel "waiting_for_approval" activity
// -> Continue with alternative path
```

**Use Cases**:
- Request timeout cancellation
- User-initiated cancellation
- Conditional activity cancellation

### Pattern 20: Escalation

**Purpose**: Handle escalation events with optional interruption of current activities.

**Definition**: When escalation triggers, optionally cancel active activities and activate an escalation handler.

**Implementation**:
```rust
async fn trigger_escalation(
    &mut self,
    instance_id: &str,
    config: &EscalationConfig,
) -> WorkflowResult<()>
```

**EscalationConfig Structure**:
```rust
struct EscalationConfig {
    escalation_code: String,
    handler_node: NodeId,
    interrupting: bool,  // If true, cancels active activities
}
```

**Execution Logic**:
1. Evaluate escalation condition
2. If `interrupting == true`: clear all active nodes
3. Activate escalation handler node
4. Record escalation event with metadata

**Example**:
```rust
EscalationConfig {
    escalation_code: "MANAGER_APPROVAL_TIMEOUT",
    handler_node: NodeId("escalate_to_director"),
    interrupting: true,
}

// When escalation triggers:
// -> Cancel all current activities
// -> Activate "escalate_to_director" activity
// -> Send escalation notification
```

**Use Cases**:
- SLA violation handling
- Manager escalation workflows
- Priority-based re-routing

## Condition Evaluation

The `evaluate_condition()` helper method supports simple expression evaluation:

```rust
fn evaluate_condition(&self, context: &HashMap<String, Value>, expression: &str) -> bool
```

**Supported Expressions**:
- Direct boolean: `"approved"` -> checks `context["approved"]`
- Negation: `"!rejected"` -> inverts boolean
- Comparison: `"amount > 1000"`, `"count < 5"`
- Equality: `"status == approved"`

**Example**:
```rust
ctx["amount"] = 1500
ctx["approved"] = true
ctx["status"] = "pending"

evaluate_condition(ctx, "amount > 1000")        // true
evaluate_condition(ctx, "!rejected")            // true (no "rejected" key)
evaluate_condition(ctx, "approved")             // true
evaluate_condition(ctx, "status == pending")    // true
```

## Integration Points

### Context Variables

Workflow execution uses a context map for condition evaluation and state tracking:

```rust
let mut context = HashMap::new();
context.insert("approval_given".to_string(), serde_json::Value::Bool(true));
context.insert("amount".to_string(), serde_json::json!(1500));
context.insert("items".to_string(), serde_json::json!(["item1", "item2"]));
```

### Pattern Composition

Patterns can be composed:

```
[Start] -> [Parallel Split]
          -> [Milestone] -> [Activity] -> [OR-join]
          -> [Deferred Choice] -> [Activity] -> [OR-join]
          -> [OR-join] -> [Critical Section] -> [End]
```

## Testing

Comprehensive tests are provided for each pattern:

```bash
# Run workflow kernel tests
cargo test -p osiris-compiler --lib adapter::workflow_kernel

# Test specific pattern
cargo test pattern_17_milestone
cargo test pattern_18_critical_section
cargo test pattern_19_cancel_activity
cargo test pattern_20_escalation
cargo test pattern_12_14_multi_instance
cargo test pattern_6_multi_choice
```

## Future Enhancements

1. **Advanced Condition Evaluation**
   - Support JsonLogic or CEL expressions
   - XPath expressions for XML documents

2. **Pattern Optimization**
   - Deadlock detection for arbitrary cycles
   - Structural soundness analysis

3. **Performance Improvements**
   - Event-based condition monitoring instead of polling
   - Lazy evaluation of conditions

4. **Error Handling**
   - Pattern-specific error recovery
   - Nested cancellation regions

5. **Monitoring and Analytics**
   - Pattern execution metrics
   - Performance analysis by pattern type

## References

- van der Aalst Workflow Patterns: https://www.workflowpatterns.com/
- Paper: "Workflow Patterns: The Definitive Guide"
- BPMN 2.0 Specification for pattern mapping
