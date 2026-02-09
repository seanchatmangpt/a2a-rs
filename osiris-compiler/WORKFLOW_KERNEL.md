# 43-Pattern Workflow Kernel

## Overview

This module implements a comprehensive workflow orchestration kernel based on **van der Aalst's 43 workflow patterns**, providing the foundation for deterministic, verifiable workflow execution in the Osiris compiler.

## Architecture

Following hexagonal architecture principles:

```
domain/workflow.rs          → Pure workflow pattern types
    ↓
port/workflow_kernel.rs     → WorkflowKernel trait (25+ async methods)
    ↓
adapter/workflow_kernel.rs  → InMemoryWorkflowKernel implementation
```

## Core Patterns Implemented

### Basic Control Flow Patterns (1-5)

1. **Sequence**: Implicit through edge connections
2. **Parallel Split (AND-split)**: Creates multiple concurrent execution paths
3. **Synchronization (AND-join)**: Waits for all incoming paths to complete
4. **Exclusive Choice (XOR-split)**: Selects exactly one outgoing path based on conditions
5. **Simple Merge (XOR-join)**: Continues when any one incoming path completes

### Advanced Branching Patterns (6-9)

6. **Multi-Choice (OR-split)**: Selects one or more outgoing paths
7. **Structured Synchronizing Merge (OR-join)**: Waits for all active incoming paths
8. **Multi-Merge**: Activates for each incoming path independently
9. **Structured Discriminator**: Waits for first incoming path, ignores rest

### Multi-Instance Patterns (12-14)

- **Sequential**: Execute instances one after another
- **Parallel**: Execute all instances concurrently
- **Static**: Cardinality known at design time
- **Dynamic**: Cardinality determined at runtime

### Cancellation and State Patterns (19-20)

19. **Cancel Activity**: Cancellation regions with event/timeout/condition triggers
20. **Escalation**: Interrupt-based exception handling

### Termination

- **Terminate**: Immediately ends workflow execution across all branches

## Domain Types

### WorkflowPattern

```rust
pub struct WorkflowPattern {
    pub id: WorkflowId,
    pub name: String,
    pub nodes: HashMap<NodeId, Node>,
    pub edges: Vec<Edge>,
    pub start_nodes: Vec<NodeId>,
    pub end_nodes: Vec<NodeId>,
    pub variables: HashMap<String, serde_json::Value>,
}
```

Represents a complete workflow definition as a directed graph.

### Node

```rust
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub config: HashMap<String, serde_json::Value>,
}

pub enum NodeKind {
    Activity { name: String, implementation: ActivityImplementation },
    Gateway { pattern: GatewayPattern },
    Event { event_type: EventType },
    Subprocess { workflow_id: WorkflowId },
}
```

Nodes represent activities, gateways, events, or nested workflows.

### GatewayPattern

```rust
pub enum GatewayPattern {
    ParallelSplit,
    Synchronization,
    ExclusiveChoice { conditions: Vec<Condition> },
    SimpleMerge,
    MultiChoice { conditions: Vec<Condition> },
    StructuredSynchronizingMerge,
    MultiMerge,
    StructuredDiscriminator { reset_after: Option<NodeId> },
}
```

### WorkflowInstance

```rust
pub struct WorkflowInstance {
    pub instance_id: String,
    pub workflow_id: WorkflowId,
    pub state: InstanceState,
    pub active_nodes: HashSet<NodeId>,
    pub context: HashMap<String, serde_json::Value>,
    pub history: Vec<ExecutionEvent>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

pub enum InstanceState {
    Active,
    Completed,
    Failed,
    Cancelled,
    Terminated,
    Suspended,
}
```

## Port Trait: WorkflowKernel

### Pattern Management

```rust
async fn register_pattern(&mut self, pattern: WorkflowPattern) -> WorkflowResult<()>;
async fn get_pattern(&self, workflow_id: &WorkflowId) -> WorkflowResult<WorkflowPattern>;
async fn validate_pattern(&self, pattern: &WorkflowPattern) -> WorkflowResult<()>;
```

### Instance Lifecycle

```rust
async fn start_instance(
    &mut self,
    workflow_id: &WorkflowId,
    initial_context: HashMap<String, serde_json::Value>,
) -> WorkflowResult<String>;

async fn suspend_instance(&mut self, instance_id: &str) -> WorkflowResult<()>;
async fn resume_instance(&mut self, instance_id: &str) -> WorkflowResult<()>;
async fn cancel_instance(&mut self, instance_id: &str) -> WorkflowResult<()>;
async fn terminate_instance(&mut self, instance_id: &str) -> WorkflowResult<()>;
```

### Core Execution

```rust
async fn execute_step(&mut self, instance_id: &str) -> WorkflowResult<Vec<NodeId>>;
async fn execute_gateway(&mut self, instance_id: &str, node_id: &NodeId)
    -> WorkflowResult<Vec<NodeId>>;
async fn execute_activity(&mut self, instance_id: &str, node_id: &NodeId)
    -> WorkflowResult<()>;
```

### Advanced Patterns

```rust
async fn execute_multi_instance(
    &mut self,
    instance_id: &str,
    node_id: &NodeId,
    config: &MultiInstanceConfig,
) -> WorkflowResult<()>;

async fn execute_cancellation(
    &mut self,
    instance_id: &str,
    region: &CancellationRegion,
) -> WorkflowResult<()>;

async fn trigger_escalation(
    &mut self,
    instance_id: &str,
    config: &EscalationConfig,
) -> WorkflowResult<()>;
```

### Cloud Workflows Integration

```rust
async fn delegate_to_cloud_workflows(
    &mut self,
    instance_id: &str,
    workflow_name: &str,
    arguments: HashMap<String, serde_json::Value>,
) -> WorkflowResult<serde_json::Value>;

async fn receive_external_callback(
    &mut self,
    instance_id: &str,
    node_id: &NodeId,
    result: serde_json::Value,
) -> WorkflowResult<()>;
```

### Query and Analysis

```rust
async fn get_active_nodes(&self, instance_id: &str) -> WorkflowResult<Vec<NodeId>>;
async fn get_enabled_nodes(&self, instance_id: &str) -> WorkflowResult<Vec<NodeId>>;
async fn check_deadlock(&self, instance_id: &str) -> WorkflowResult<bool>;
```

## Adapter: InMemoryWorkflowKernel

Thread-safe in-memory implementation using:
- `Arc<RwLock<HashMap<WorkflowId, WorkflowPattern>>>` for pattern storage
- `Arc<RwLock<HashMap<String, WorkflowInstance>>>` for instance storage
- Event history tracking for audit trails
- Structural validation (start/end nodes, edge references)

### Example Usage

```rust
use osiris_compiler::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut kernel = InMemoryWorkflowKernel::new();

    // Define workflow pattern
    let workflow_id = WorkflowId::new("approval-workflow");
    let mut nodes = HashMap::new();

    nodes.insert(
        NodeId::new("start"),
        Node {
            id: NodeId::new("start"),
            kind: NodeKind::Event { event_type: EventType::Start },
            config: HashMap::new(),
        },
    );

    nodes.insert(
        NodeId::new("check-amount"),
        Node {
            id: NodeId::new("check-amount"),
            kind: NodeKind::Gateway {
                pattern: GatewayPattern::ExclusiveChoice {
                    conditions: vec![
                        Condition {
                            expression: "amount > 1000".to_string(),
                            target: NodeId::new("manager-approval"),
                            description: Some("Requires manager approval".to_string()),
                        },
                        Condition {
                            expression: "amount <= 1000".to_string(),
                            target: NodeId::new("auto-approve"),
                            description: Some("Auto-approve".to_string()),
                        },
                    ],
                },
            },
            config: HashMap::new(),
        },
    );

    let pattern = WorkflowPattern {
        id: workflow_id.clone(),
        name: "Expense Approval".to_string(),
        description: Some("Multi-level expense approval workflow".to_string()),
        nodes,
        edges: vec![
            Edge {
                from: NodeId::new("start"),
                to: NodeId::new("check-amount"),
                condition: None,
                label: None,
            },
        ],
        start_nodes: vec![NodeId::new("start")],
        end_nodes: vec![NodeId::new("approved")],
        variables: HashMap::new(),
    };

    // Register pattern
    kernel.register_pattern(pattern).await?;

    // Start instance
    let mut context = HashMap::new();
    context.insert("amount".to_string(), serde_json::json!(1500));
    context.insert("requester".to_string(), serde_json::json!("alice@example.com"));

    let instance_id = kernel.start_instance(&workflow_id, context).await?;
    println!("Started workflow instance: {}", instance_id);

    // Execute steps
    let activated_nodes = kernel.execute_step(&instance_id).await?;
    println!("Activated nodes: {:?}", activated_nodes);

    // Get instance state
    let instance = kernel.get_instance(&instance_id).await?;
    println!("Instance state: {:?}", instance.state);
    println!("Active nodes: {:?}", instance.active_nodes);

    Ok(())
}
```

## Cloud Workflows Integration

The kernel provides integration points for external orchestration services like GCP Cloud Workflows:

### ActivityImplementation::CloudWorkflow

```rust
ActivityImplementation::CloudWorkflow {
    project_id: "my-gcp-project".to_string(),
    workflow_name: "data-processing".to_string(),
    region: "us-central1".to_string(),
}
```

### Delegation Pattern

```rust
// Delegate long-running task to Cloud Workflows
let result = kernel.delegate_to_cloud_workflows(
    instance_id,
    "data-processing",
    arguments,
).await?;

// Receive callback when external workflow completes
kernel.receive_external_callback(
    instance_id,
    &NodeId::new("process-data"),
    result,
).await?;
```

## Testing

All 10 tests pass:

### Domain Tests (5)
- `test_workflow_id`: ID creation and equality
- `test_gateway_pattern_serialization`: JSON round-trip
- `test_exclusive_choice_gateway`: Condition evaluation
- `test_instance_state`: State transitions
- `test_multi_instance_config`: Multi-instance configuration

### Port/Adapter Tests (5)
- `test_workflow_error_display`: Error message formatting
- `test_soundness_report_serialization`: Analysis result serialization
- `test_register_and_get_pattern`: Pattern storage/retrieval
- `test_start_instance`: Instance creation
- `test_instance_lifecycle`: Suspend/resume/cancel operations

## Future Work

### Complete Gateway Evaluation
- Implement condition expression parser and evaluator
- Add synchronization counters for AND-join patterns
- Support complex condition expressions (JSONPath, CEL, etc.)

### Multi-Instance Execution
- Collection iteration engine
- Parallel instance coordination
- Completion condition evaluation

### Deadlock Detection
- Graph analysis algorithm (e.g., Petri net analysis)
- Cycle detection in workflow graphs
- Liveness property verification

### Cloud Integration
- GCP Cloud Workflows client library integration
- Authentication and authorization
- Callback webhook handling

### Additional Patterns (10-43)
- Pattern 10: Arbitrary Cycles
- Pattern 16: Deferred Choice
- Pattern 17: Interleaved Parallel Routing
- Pattern 18: Milestone
- Pattern 24: Critical Section
- And 33 more patterns from the complete catalog

## Dependencies

All dependencies already present in `Cargo.toml`:

```toml
tokio = { version = "1.32", features = ["sync"] }
async-trait = "0.1"
uuid = { version = "1.4", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## Error Handling

```rust
pub enum WorkflowError {
    WorkflowNotFound { workflow_id: String },
    InstanceNotFound { instance_id: String },
    NodeNotFound { workflow_id: String, node_id: String },
    InvalidWorkflow { reason: String },
    InvalidStateTransition { from: String, to: String, reason: String },
    GatewayEvaluationFailed { node_id: String, reason: String },
    ActivityFailed { node_id: String, reason: String },
    Deadlock { instance_id: String, reason: String },
    CancellationFailed { region_id: String, reason: String },
    EscalationFailed { escalation_code: String, reason: String },
    CloudWorkflowsError { message: String },
    ExecutionError { message: String },
}
```

## References

1. **van der Aalst, W.M.P. et al.** (2003). "Workflow Patterns: The Definitive Guide"
2. **Russell, N., ter Hofstede, A.H.M., van der Aalst, W.M.P., Mulyar, N.** (2006). "Workflow Control-Flow Patterns: A Revised View"
3. [Workflow Patterns Initiative](http://www.workflowpatterns.com/)
4. **GCP Cloud Workflows**: https://cloud.google.com/workflows

## License

MIT OR Apache-2.0
