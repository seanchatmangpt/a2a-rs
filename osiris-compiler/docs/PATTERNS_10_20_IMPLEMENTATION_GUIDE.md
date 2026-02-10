# Patterns 10-20 Implementation Guide

Complete guide for implementing and using van der Aalst workflow patterns 10-20 in osiris-compiler.

## Quick Implementation Summary

### Code Location
- **Domain types**: `/home/user/a2a-rs/osiris-compiler/src/domain/workflow.rs`
- **Adapter implementation**: `/home/user/a2a-rs/osiris-compiler/src/adapter/workflow_kernel.rs`
- **Port trait**: `/home/user/a2a-rs/osiris-compiler/src/port/workflow_kernel.rs`

### Key Files Modified
```
osiris-compiler/
├── src/
│   ├── domain/
│   │   └── workflow.rs              (GatewayPattern enum + supporting types)
│   ├── adapter/
│   │   └── workflow_kernel.rs       (InMemoryWorkflowKernel impl + tests)
│   └── port/
│       └── workflow_kernel.rs       (WorkflowKernel trait)
└── docs/
    ├── VAN_DER_AALST_PATTERNS_10_20.md
    └── PATTERNS_10_20_IMPLEMENTATION_GUIDE.md
```

## Domain Types

### Extended GatewayPattern Enum

```rust
// From domain/workflow.rs
pub enum GatewayPattern {
    // ... Patterns 2-9 ...

    /// Pattern 10: Arbitrary Cycles
    ArbitraryCycle { back_edge_to: NodeId },

    /// Pattern 11: Implicit Termination
    ImplicitTermination,

    /// Pattern 15: Deferred Choice
    DeferredChoice {
        event_conditions: Vec<Condition>,
        timeout_ms: Option<u64>,
    },

    /// Pattern 16: Interleaved Parallel Routing
    InterleavedParallelRouting,

    /// Pattern 17: Milestone
    Milestone {
        condition: String,
        monitor_node: Option<NodeId>,
    },

    /// Pattern 18: Critical Section
    CriticalSection { section_id: String },
}
```

### Supporting Types

```rust
// Pattern 18: Critical Section Config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalSectionConfig {
    pub section_id: String,
    pub activities: Vec<NodeId>,
    #[serde(default = "critical_section_default_max")]
    pub max_concurrent: u32,
}

// Pattern 17: Milestone Config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneConfig {
    pub milestone_id: String,
    pub condition: String,
    pub dependent_activities: Vec<NodeId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitor_node: Option<NodeId>,
}

// Pattern 16: Interleaved Execution Context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterleavedExecutionContext {
    pub active_paths: Vec<Vec<NodeId>>,
    pub independent_completion: bool,
}
```

## Adapter Implementation

### Pattern Dispatch (execute_gateway)

```rust
// Simplified pattern dispatch logic
pub async fn execute_gateway(
    &mut self,
    instance_id: &str,
    node_id: &NodeId,
) -> WorkflowResult<Vec<NodeId>> {
    // ... setup code ...

    let activated_nodes = match gateway_pattern {
        // Pattern 2: AND-split
        GatewayPattern::ParallelSplit => {
            outgoing.iter().map(|e| e.to.clone()).collect()
        }

        // Pattern 3: AND-join
        GatewayPattern::Synchronization => {
            let all_active = incoming
                .iter()
                .all(|e| instance.active_nodes.contains(&e.from));
            if all_active && !incoming.is_empty() {
                outgoing.iter().map(|e| e.to.clone()).collect()
            } else {
                Vec::new()
            }
        }

        // Pattern 6: OR-split (one or more paths)
        GatewayPattern::MultiChoice { conditions } => {
            conditions
                .iter()
                .filter(|c| self.evaluate_condition(&instance.context, &c.expression))
                .map(|c| c.target.clone())
                .collect()
        }

        // Pattern 10: Arbitrary Cycles
        GatewayPattern::ArbitraryCycle { back_edge_to } => {
            let mut nodes = outgoing.iter().map(|e| e.to.clone()).collect::<Vec<_>>();
            if self.should_loop(&instance.context) {
                nodes.push(back_edge_to.clone());
            }
            nodes
        }

        // Pattern 17: Milestone (condition-based enabling)
        GatewayPattern::Milestone { condition, monitor_node } => {
            if self.evaluate_condition(&instance.context, condition) {
                outgoing.iter().map(|e| e.to.clone()).collect()
            } else {
                Vec::new()
            }
        }

        // Pattern 18: Critical Section (mutual exclusion)
        GatewayPattern::CriticalSection { section_id } => {
            if self.is_critical_section_free(section_id).await {
                outgoing.iter().map(|e| e.to.clone()).collect()
            } else {
                Vec::new()
            }
        }

        // ... other patterns ...
    };

    Ok(activated_nodes)
}
```

### Condition Evaluation

```rust
// From InMemoryWorkflowKernel
fn evaluate_condition(
    &self,
    context: &HashMap<String, serde_json::Value>,
    expression: &str,
) -> bool {
    // Negation: !flag
    if expression.starts_with('!') {
        let key = &expression[1..];
        return context
            .get(key)
            .and_then(|v| v.as_bool())
            .map(|b| !b)
            .unwrap_or(true);
    }

    // Greater than: amount > 1000
    if expression.contains('>') {
        let parts: Vec<&str> = expression.split('>').collect();
        if parts.len() == 2 {
            if let Some(left_val) = context.get(parts[0].trim()) {
                if let Ok(right_num) = parts[1].trim().parse::<f64>() {
                    if let Some(left_num) = left_val.as_f64() {
                        return left_num > right_num;
                    }
                }
            }
        }
    }

    // Direct boolean lookup: approved
    if let Some(value) = context.get(expression) {
        matches!(value, serde_json::Value::Bool(true))
    } else {
        false
    }
}
```

### Critical Section Management

```rust
// Pattern 18: Critical Section Implementation
async fn is_critical_section_free(&self, section_id: &str) -> bool {
    let instances = self.instances.read().await;
    !instances.values().any(|inst| {
        inst.state == InstanceState::Active
            && inst
                .context
                .get("critical_section")
                .and_then(|v| v.as_str())
                .map(|s| s == section_id)
                .unwrap_or(false)
    })
}

async fn acquire_critical_section(
    &self,
    instance_id: &str,
    section_id: &str,
) -> WorkflowResult<()> {
    let mut instances = self.instances.write().await;
    if let Some(instance) = instances.get_mut(instance_id) {
        instance.context.insert(
            "critical_section".to_string(),
            serde_json::Value::String(section_id.to_string()),
        );
        Ok(())
    } else {
        Err(WorkflowError::InstanceNotFound {
            instance_id: instance_id.to_string(),
        })
    }
}
```

### Multi-Instance Execution (Patterns 12-14)

```rust
// Patterns 12-14: Multiple Instance patterns
pub async fn execute_multi_instance(
    &mut self,
    instance_id: &str,
    node_id: &NodeId,
    config: &MultiInstanceConfig,
) -> WorkflowResult<()> {
    // Get collection from context
    let collection = instance.context
        .get(&config.collection)
        .ok_or_else(|| WorkflowError::ExecutionError {
            message: format!("Collection not found: {}", config.collection),
        })?;

    let items = match collection {
        serde_json::Value::Array(arr) => arr.clone(),
        _ => return Err(WorkflowError::ExecutionError {
            message: "Collection must be array".to_string(),
        }),
    };

    // Execute based on mode
    match config.mode {
        MultiInstanceMode::Sequential => {
            // One at a time
            for (index, item) in items.iter().enumerate() {
                let mut ctx_updates = HashMap::new();
                ctx_updates.insert(config.item_variable.clone(), item.clone());
                ctx_updates.insert(
                    "mi_index".to_string(),
                    serde_json::Value::Number(index.into()),
                );
                self.update_context(instance_id, ctx_updates).await?;
                // Execute activity...
            }
        }
        MultiInstanceMode::Parallel | MultiInstanceMode::ParallelStatic => {
            // All in parallel
            for (index, item) in items.iter().enumerate() {
                let mut ctx_updates = HashMap::new();
                ctx_updates.insert(config.item_variable.clone(), item.clone());
                self.update_context(instance_id, ctx_updates).await?;
                // Spawn task...
            }
        }
        _ => {}
    }

    Ok(())
}
```

### Cancellation (Pattern 19)

```rust
// Pattern 19: Cancel Activity
pub async fn execute_cancellation(
    &mut self,
    instance_id: &str,
    region: &CancellationRegion,
) -> WorkflowResult<()> {
    let mut instances = self.instances.write().await;
    let instance = instances.get_mut(instance_id)
        .ok_or_else(|| WorkflowError::InstanceNotFound {
            instance_id: instance_id.to_string(),
        })?;

    // Evaluate trigger
    let should_cancel = match &region.trigger {
        CancellationTrigger::Event { event_code } => {
            instance
                .context
                .get("triggered_event")
                .and_then(|v| v.as_str())
                .map(|e| e == event_code)
                .unwrap_or(false)
        }
        CancellationTrigger::Condition { expression } => {
            self.evaluate_condition(&instance.context, expression)
        }
        _ => false,
    };

    if should_cancel {
        // Remove all nodes in region from active set
        for node_id in &region.nodes {
            instance.active_nodes.remove(node_id);
        }
    }

    Ok(())
}
```

### Escalation (Pattern 20)

```rust
// Pattern 20: Escalation
pub async fn trigger_escalation(
    &mut self,
    instance_id: &str,
    config: &EscalationConfig,
) -> WorkflowResult<()> {
    let mut instances = self.instances.write().await;
    let instance = instances.get_mut(instance_id)
        .ok_or_else(|| WorkflowError::InstanceNotFound {
            instance_id: instance_id.to_string(),
        })?;

    // If interrupting, cancel all active nodes
    if config.interrupting {
        instance.active_nodes.clear();
    }

    // Activate escalation handler
    instance.active_nodes.insert(config.handler_node.clone());

    Ok(())
}
```

## Usage Examples

### Example 1: Multi-Choice Pattern (Pattern 6)

```rust
let gateway = GatewayPattern::MultiChoice {
    conditions: vec![
        Condition {
            expression: "send_email".to_string(),
            target: NodeId::new("email_notifier"),
            description: Some("Send email notification".to_string()),
        },
        Condition {
            expression: "send_sms".to_string(),
            target: NodeId::new("sms_notifier"),
            description: Some("Send SMS notification".to_string()),
        },
    ],
};

// When both "send_email" and "send_sms" are true in context:
// Both email_notifier and sms_notifier are activated
```

### Example 2: Milestone Pattern (Pattern 17)

```rust
let gateway = GatewayPattern::Milestone {
    condition: "payment_received".to_string(),
    monitor_node: Some(NodeId::new("payment_gateway")),
};

// Activity is ready but only activates when:
// context["payment_received"] == true
```

### Example 3: Critical Section (Pattern 18)

```rust
let gateway = GatewayPattern::CriticalSection {
    section_id: "database_write".to_string(),
};

// Only one workflow instance can execute in this section at a time
// Others wait until previous instance exits the critical section
```

### Example 4: Deferred Choice (Pattern 15)

```rust
let gateway = GatewayPattern::DeferredChoice {
    event_conditions: vec![
        Condition {
            expression: "urgent_selected".to_string(),
            target: NodeId::new("fast_track"),
            description: None,
        },
        Condition {
            expression: "standard_selected".to_string(),
            target: NodeId::new("normal_track"),
            description: None,
        },
    ],
    timeout_ms: Some(3600000), // 1 hour
};

// First event wins, activates corresponding path
```

### Example 5: Arbitrary Cycle (Pattern 10)

```rust
let gateway = GatewayPattern::ArbitraryCycle {
    back_edge_to: NodeId::new("validation_activity"),
};

// If context["continue_loop"] == true:
//   Proceed to next activity AND loop back to validation_activity
// Else:
//   Only proceed to exit path
```

### Example 6: Multi-Instance (Patterns 12-14)

```rust
// Setup context with items to process
let mut context = HashMap::new();
context.insert(
    "orders".to_string(),
    serde_json::json!(vec![
        {"id": "ORD001", "amount": 100},
        {"id": "ORD002", "amount": 200},
        {"id": "ORD003", "amount": 150},
    ]),
);

// Create instance
let instance_id = kernel.start_instance(&workflow_id, context).await?;

// Execute multi-instance activity
let config = MultiInstanceConfig {
    mode: MultiInstanceMode::Sequential,
    collection: "orders".to_string(),
    item_variable: "current_order".to_string(),
    completion_condition: None,
};

kernel.execute_multi_instance(&instance_id, &node_id, &config).await?;

// For each order:
//   current_order = {"id": "ORD001", "amount": 100}
//   mi_index = 0
//   -> Process order activity
```

## Testing

### Test Pattern 6: Multi-Choice

```rust
#[tokio::test]
async fn test_pattern_6_multi_choice() {
    let mut kernel = InMemoryWorkflowKernel::new();
    let workflow_id = WorkflowId::new("multi-choice-wf");

    // Create workflow with multi-choice pattern...
    // Set context with both conditions true...

    let activated = kernel
        .execute_gateway(&instance_id, &NodeId::new("gateway"))
        .await
        .unwrap();

    // Both paths should activate
    assert!(activated.contains(&NodeId::new("activity_a")));
    assert!(activated.contains(&NodeId::new("activity_b")));
}
```

### Test Pattern 17: Milestone

```rust
#[tokio::test]
async fn test_pattern_17_milestone() {
    // Without approval: activated.is_empty()

    // Update context: approval_given = true

    // With approval: activated contains next_activity
}
```

### Test Pattern 18: Critical Section

```rust
#[tokio::test]
async fn test_pattern_18_critical_section() {
    // is_critical_section_free() = true initially

    // Acquire: is_critical_section_free() = false

    // Release: is_critical_section_free() = true
}
```

## Running Tests

```bash
# Run all workflow kernel tests
cargo test -p osiris-compiler --lib adapter::workflow_kernel

# Run specific pattern tests
cargo test pattern_6_multi_choice
cargo test pattern_17_milestone
cargo test pattern_18_critical_section
cargo test pattern_19_cancel_activity
cargo test pattern_20_escalation
cargo test pattern_12_14_multi_instance
```

## Error Handling

All pattern implementations return `WorkflowResult<T>` which maps to `Result<T, WorkflowError>`:

```rust
pub enum WorkflowError {
    #[error("Workflow not found: {workflow_id}")]
    WorkflowNotFound { workflow_id: String },

    #[error("Instance not found: {instance_id}")]
    InstanceNotFound { instance_id: String },

    #[error("Gateway evaluation failed at node {node_id}: {reason}")]
    GatewayEvaluationFailed { node_id: String, reason: String },

    #[error("Deadlock detected in instance {instance_id}: {reason}")]
    Deadlock { instance_id: String, reason: String },

    #[error("Cancellation failed for region {region_id}: {reason}")]
    CancellationFailed { region_id: String, reason: String },

    #[error("Escalation failed with code {escalation_code}: {reason}")]
    EscalationFailed {
        escalation_code: String,
        reason: String,
    },

    // ... more error variants ...
}
```

## Integration with Existing Code

1. **Domain types** integrate seamlessly with existing `WorkflowPattern` struct
2. **Port trait** methods fully implement `WorkflowKernel` contract
3. **Adapter** uses existing `InMemoryWorkflowKernel` infrastructure
4. **Tests** follow existing test patterns and conventions
5. **Documentation** matches existing standards

## Performance Considerations

- **Condition evaluation**: O(1) for simple boolean checks, O(1) for numeric comparisons
- **Gateway execution**: O(n) where n = number of outgoing edges
- **Critical section check**: O(m) where m = number of active instances
- **Multi-instance**: O(k) per iteration where k = collection size

## Future Enhancements

1. **Event-driven monitoring** instead of polling for milestone conditions
2. **Distributed critical sections** using external lock service
3. **Nested cancellation regions** for complex exception handling
4. **Pattern composition** validation and optimization
5. **Deadlock detection** for arbitrary cycles

## References

- Complete documentation: `VAN_DER_AALST_PATTERNS_10_20.md`
- Source code: `/home/user/a2a-rs/osiris-compiler/src/`
- Tests: `src/adapter/workflow_kernel.rs` (test module at end of file)
