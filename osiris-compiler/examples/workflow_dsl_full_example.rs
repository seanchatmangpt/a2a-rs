//! Comprehensive workflow DSL example.
//!
//! Demonstrates the full workflow DSL pipeline:
//! 1. Define workflow in BPMN-like JSON DSL
//! 2. Parse and validate DSL
//! 3. Compile to 43-pattern primitives
//! 4. Execute workflow instance
//! 5. Monitor execution state

use osiris_compiler::prelude::*;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Osiris Workflow DSL Compiler Demo ===\n");

    // ============================================================================
    // Example 1: Simple Sequential Workflow
    // ============================================================================
    println!("Example 1: Simple Sequential Workflow");
    println!("--------------------------------------");

    let simple_workflow = create_simple_workflow();
    execute_workflow_demo(simple_workflow, "Simple Sequential").await?;

    println!("\n");

    // ============================================================================
    // Example 2: Parallel Workflow with AND Gateway
    // ============================================================================
    println!("Example 2: Parallel Workflow (Pattern 2 + 3)");
    println!("---------------------------------------------");

    let parallel_workflow = create_parallel_workflow();
    execute_workflow_demo(parallel_workflow, "Parallel Processing").await?;

    println!("\n");

    // ============================================================================
    // Example 3: Exclusive Choice with XOR Gateway
    // ============================================================================
    println!("Example 3: Exclusive Choice (Pattern 4)");
    println!("----------------------------------------");

    let choice_workflow = create_exclusive_choice_workflow();
    execute_workflow_demo(choice_workflow, "Exclusive Choice").await?;

    println!("\n");

    // ============================================================================
    // Example 4: Loop Pattern
    // ============================================================================
    println!("Example 4: Loop Pattern");
    println!("-----------------------");

    let loop_workflow = create_loop_workflow();
    execute_workflow_demo(loop_workflow, "Loop").await?;

    println!("\n");

    // ============================================================================
    // Example 5: Service Task with HTTP Integration
    // ============================================================================
    println!("Example 5: Service Task with HTTP");
    println!("----------------------------------");

    let service_workflow = create_service_task_workflow();
    execute_workflow_demo(service_workflow, "Service Task").await?;

    println!("\n");

    // ============================================================================
    // Example 6: Error Handling and Cancellation
    // ============================================================================
    println!("Example 6: Error Handling (Pattern 19)");
    println!("---------------------------------------");

    let error_workflow = create_error_handling_workflow();
    execute_workflow_demo(error_workflow, "Error Handling").await?;

    println!("\n=== Demo Complete ===");
    Ok(())
}

async fn execute_workflow_demo(
    dsl: DslWorkflow,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Step 1: Display DSL as JSON
    println!("DSL Definition (JSON):");
    let json = serde_json::to_string_pretty(&dsl)?;
    println!("{}\n", json);

    // Step 2: Compile DSL to workflow pattern
    let compiler = BpmnCompiler::new();

    println!("Compiling workflow...");
    let pattern = compiler.compile(dsl).await?;

    println!("✓ Compiled successfully");
    println!("  - Nodes: {}", pattern.nodes.len());
    println!("  - Edges: {}", pattern.edges.len());
    println!("  - Start nodes: {}", pattern.start_nodes.len());
    println!("  - End nodes: {}\n", pattern.end_nodes.len());

    // Step 3: Register pattern with workflow kernel
    let mut kernel = InMemoryWorkflowKernel::new();
    kernel.register_pattern(pattern.clone()).await?;
    println!("✓ Registered workflow pattern\n");

    // Step 4: Start workflow instance
    let mut context = HashMap::new();
    context.insert("amount".to_string(), serde_json::json!(1500));
    context.insert("iteration".to_string(), serde_json::json!(0));

    let instance_id = kernel.start_instance(&pattern.id, context).await?;
    println!("✓ Started workflow instance: {}\n", instance_id);

    // Step 5: Execute workflow steps
    println!("Executing workflow steps:");
    let mut step_count = 0;
    loop {
        let instance = kernel.get_instance(&instance_id).await?;

        println!("  Step {}: State = {:?}", step_count, instance.state);
        println!("           Active nodes: {:?}", instance.active_nodes);

        if instance.state != InstanceState::Active {
            println!("\n✓ Workflow completed with state: {:?}", instance.state);
            break;
        }

        // Execute next step
        let activated = kernel.execute_step(&instance_id).await?;
        if activated.is_empty() {
            println!("  No more enabled nodes");
            break;
        }

        step_count += 1;
        if step_count > 10 {
            println!("  (Stopping after 10 steps for demo)");
            break;
        }
    }

    // Step 6: Display execution history
    let history = kernel.get_history(&instance_id).await?;
    println!("\nExecution History:");
    for (i, event) in history.iter().enumerate() {
        println!("  {}. {:?} {:?}", i + 1, event.event_type, event.node_id);
    }

    Ok(())
}

// ============================================================================
// Workflow Factory Functions
// ============================================================================

fn create_simple_workflow() -> DslWorkflow {
    DslWorkflow {
        id: "simple-seq-001".to_string(),
        name: "Simple Sequential Workflow".to_string(),
        description: Some("Pattern 1: Sequence - tasks execute one after another".to_string()),
        version: Some("1.0".to_string()),
        elements: vec![
            DslElement::StartEvent {
                id: "start".to_string(),
                name: "Start Process".to_string(),
                trigger: None,
            },
            DslElement::Task {
                id: "task1".to_string(),
                name: "Validate Request".to_string(),
                task_type: DslTaskType::Task,
                properties: HashMap::new(),
            },
            DslElement::Task {
                id: "task2".to_string(),
                name: "Process Data".to_string(),
                task_type: DslTaskType::Task,
                properties: HashMap::new(),
            },
            DslElement::Task {
                id: "task3".to_string(),
                name: "Send Confirmation".to_string(),
                task_type: DslTaskType::Task,
                properties: HashMap::new(),
            },
            DslElement::EndEvent {
                id: "end".to_string(),
                name: "End Process".to_string(),
                result: None,
            },
        ],
        flows: vec![
            DslFlow {
                id: "flow1".to_string(),
                source_ref: "start".to_string(),
                target_ref: "task1".to_string(),
                name: Some("to validation".to_string()),
                condition: None,
            },
            DslFlow {
                id: "flow2".to_string(),
                source_ref: "task1".to_string(),
                target_ref: "task2".to_string(),
                name: Some("to processing".to_string()),
                condition: None,
            },
            DslFlow {
                id: "flow3".to_string(),
                source_ref: "task2".to_string(),
                target_ref: "task3".to_string(),
                name: Some("to confirmation".to_string()),
                condition: None,
            },
            DslFlow {
                id: "flow4".to_string(),
                source_ref: "task3".to_string(),
                target_ref: "end".to_string(),
                name: Some("to end".to_string()),
                condition: None,
            },
        ],
        variables: HashMap::new(),
    }
}

fn create_parallel_workflow() -> DslWorkflow {
    DslWorkflow {
        id: "parallel-001".to_string(),
        name: "Parallel Workflow".to_string(),
        description: Some("Pattern 2: Parallel Split + Pattern 3: Synchronization".to_string()),
        version: Some("1.0".to_string()),
        elements: vec![
            DslElement::StartEvent {
                id: "start".to_string(),
                name: "Start".to_string(),
                trigger: None,
            },
            DslElement::Gateway {
                id: "split".to_string(),
                name: "Parallel Split".to_string(),
                gateway_type: DslGatewayType::ParallelGateway,
                conditions: vec![],
            },
            DslElement::Task {
                id: "task_a".to_string(),
                name: "Process Branch A".to_string(),
                task_type: DslTaskType::Task,
                properties: HashMap::new(),
            },
            DslElement::Task {
                id: "task_b".to_string(),
                name: "Process Branch B".to_string(),
                task_type: DslTaskType::Task,
                properties: HashMap::new(),
            },
            DslElement::Task {
                id: "task_c".to_string(),
                name: "Process Branch C".to_string(),
                task_type: DslTaskType::Task,
                properties: HashMap::new(),
            },
            DslElement::Gateway {
                id: "join".to_string(),
                name: "Synchronization".to_string(),
                gateway_type: DslGatewayType::ParallelGateway,
                conditions: vec![],
            },
            DslElement::EndEvent {
                id: "end".to_string(),
                name: "End".to_string(),
                result: None,
            },
        ],
        flows: vec![
            DslFlow {
                id: "f1".to_string(),
                source_ref: "start".to_string(),
                target_ref: "split".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f2".to_string(),
                source_ref: "split".to_string(),
                target_ref: "task_a".to_string(),
                name: Some("Branch A".to_string()),
                condition: None,
            },
            DslFlow {
                id: "f3".to_string(),
                source_ref: "split".to_string(),
                target_ref: "task_b".to_string(),
                name: Some("Branch B".to_string()),
                condition: None,
            },
            DslFlow {
                id: "f4".to_string(),
                source_ref: "split".to_string(),
                target_ref: "task_c".to_string(),
                name: Some("Branch C".to_string()),
                condition: None,
            },
            DslFlow {
                id: "f5".to_string(),
                source_ref: "task_a".to_string(),
                target_ref: "join".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f6".to_string(),
                source_ref: "task_b".to_string(),
                target_ref: "join".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f7".to_string(),
                source_ref: "task_c".to_string(),
                target_ref: "join".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f8".to_string(),
                source_ref: "join".to_string(),
                target_ref: "end".to_string(),
                name: None,
                condition: None,
            },
        ],
        variables: HashMap::new(),
    }
}

fn create_exclusive_choice_workflow() -> DslWorkflow {
    DslWorkflow {
        id: "exclusive-choice-001".to_string(),
        name: "Exclusive Choice Workflow".to_string(),
        description: Some("Pattern 4: Exclusive Choice (XOR-split)".to_string()),
        version: Some("1.0".to_string()),
        elements: vec![
            DslElement::StartEvent {
                id: "start".to_string(),
                name: "Start".to_string(),
                trigger: None,
            },
            DslElement::Task {
                id: "evaluate".to_string(),
                name: "Evaluate Request".to_string(),
                task_type: DslTaskType::Task,
                properties: HashMap::new(),
            },
            DslElement::Gateway {
                id: "decision".to_string(),
                name: "Amount Decision".to_string(),
                gateway_type: DslGatewayType::ExclusiveGateway,
                conditions: vec![
                    DslCondition {
                        expression: "amount > 1000".to_string(),
                        target_ref: "manager_approval".to_string(),
                        name: Some("High amount".to_string()),
                    },
                    DslCondition {
                        expression: "amount <= 1000".to_string(),
                        target_ref: "auto_approve".to_string(),
                        name: Some("Low amount".to_string()),
                    },
                ],
            },
            DslElement::Task {
                id: "manager_approval".to_string(),
                name: "Manager Approval Required".to_string(),
                task_type: DslTaskType::UserTask {
                    assignee: Some("manager".to_string()),
                },
                properties: HashMap::new(),
            },
            DslElement::Task {
                id: "auto_approve".to_string(),
                name: "Auto Approve".to_string(),
                task_type: DslTaskType::Task,
                properties: HashMap::new(),
            },
            DslElement::Gateway {
                id: "merge".to_string(),
                name: "Merge".to_string(),
                gateway_type: DslGatewayType::ExclusiveGateway,
                conditions: vec![],
            },
            DslElement::EndEvent {
                id: "end".to_string(),
                name: "End".to_string(),
                result: None,
            },
        ],
        flows: vec![
            DslFlow {
                id: "f1".to_string(),
                source_ref: "start".to_string(),
                target_ref: "evaluate".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f2".to_string(),
                source_ref: "evaluate".to_string(),
                target_ref: "decision".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f3".to_string(),
                source_ref: "decision".to_string(),
                target_ref: "manager_approval".to_string(),
                name: Some("High amount".to_string()),
                condition: Some("amount > 1000".to_string()),
            },
            DslFlow {
                id: "f4".to_string(),
                source_ref: "decision".to_string(),
                target_ref: "auto_approve".to_string(),
                name: Some("Low amount".to_string()),
                condition: Some("amount <= 1000".to_string()),
            },
            DslFlow {
                id: "f5".to_string(),
                source_ref: "manager_approval".to_string(),
                target_ref: "merge".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f6".to_string(),
                source_ref: "auto_approve".to_string(),
                target_ref: "merge".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f7".to_string(),
                source_ref: "merge".to_string(),
                target_ref: "end".to_string(),
                name: None,
                condition: None,
            },
        ],
        variables: HashMap::from([(
            "amount".to_string(),
            DslVariable {
                name: "amount".to_string(),
                var_type: Some("number".to_string()),
                default_value: Some(serde_json::json!(500)),
            },
        )]),
    }
}

fn create_loop_workflow() -> DslWorkflow {
    DslWorkflow {
        id: "loop-001".to_string(),
        name: "Loop Workflow".to_string(),
        description: Some("Pattern 21: Structured Loop".to_string()),
        version: Some("1.0".to_string()),
        elements: vec![
            DslElement::StartEvent {
                id: "start".to_string(),
                name: "Start".to_string(),
                trigger: None,
            },
            DslElement::Task {
                id: "init".to_string(),
                name: "Initialize Counter".to_string(),
                task_type: DslTaskType::Task,
                properties: HashMap::new(),
            },
            DslElement::Task {
                id: "process".to_string(),
                name: "Process Item".to_string(),
                task_type: DslTaskType::Task,
                properties: HashMap::new(),
            },
            DslElement::Gateway {
                id: "check_continue".to_string(),
                name: "Check Continue".to_string(),
                gateway_type: DslGatewayType::ExclusiveGateway,
                conditions: vec![
                    DslCondition {
                        expression: "iteration < 5".to_string(),
                        target_ref: "process".to_string(),
                        name: Some("Continue loop".to_string()),
                    },
                    DslCondition {
                        expression: "iteration >= 5".to_string(),
                        target_ref: "end".to_string(),
                        name: Some("Exit loop".to_string()),
                    },
                ],
            },
            DslElement::EndEvent {
                id: "end".to_string(),
                name: "End".to_string(),
                result: None,
            },
        ],
        flows: vec![
            DslFlow {
                id: "f1".to_string(),
                source_ref: "start".to_string(),
                target_ref: "init".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f2".to_string(),
                source_ref: "init".to_string(),
                target_ref: "process".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f3".to_string(),
                source_ref: "process".to_string(),
                target_ref: "check_continue".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f4_loop".to_string(),
                source_ref: "check_continue".to_string(),
                target_ref: "process".to_string(),
                name: Some("Loop back".to_string()),
                condition: Some("iteration < 5".to_string()),
            },
            DslFlow {
                id: "f5_exit".to_string(),
                source_ref: "check_continue".to_string(),
                target_ref: "end".to_string(),
                name: Some("Exit".to_string()),
                condition: Some("iteration >= 5".to_string()),
            },
        ],
        variables: HashMap::from([(
            "iteration".to_string(),
            DslVariable {
                name: "iteration".to_string(),
                var_type: Some("number".to_string()),
                default_value: Some(serde_json::json!(0)),
            },
        )]),
    }
}

fn create_service_task_workflow() -> DslWorkflow {
    DslWorkflow {
        id: "service-task-001".to_string(),
        name: "Service Task Workflow".to_string(),
        description: Some("Demonstrates HTTP service task integration".to_string()),
        version: Some("1.0".to_string()),
        elements: vec![
            DslElement::StartEvent {
                id: "start".to_string(),
                name: "Start".to_string(),
                trigger: None,
            },
            DslElement::Task {
                id: "call_api".to_string(),
                name: "Call External API".to_string(),
                task_type: DslTaskType::ServiceTask {
                    implementation: DslServiceImplementation::Http {
                        url: "https://api.example.com/process".to_string(),
                        method: "POST".to_string(),
                        headers: HashMap::from([
                            ("Content-Type".to_string(), "application/json".to_string()),
                            ("Authorization".to_string(), "Bearer token123".to_string()),
                        ]),
                    },
                },
                properties: HashMap::new(),
            },
            DslElement::Task {
                id: "transform_response".to_string(),
                name: "Transform Response".to_string(),
                task_type: DslTaskType::ScriptTask {
                    language: "javascript".to_string(),
                    script: "return data.result.toUpperCase();".to_string(),
                },
                properties: HashMap::new(),
            },
            DslElement::EndEvent {
                id: "end".to_string(),
                name: "End".to_string(),
                result: None,
            },
        ],
        flows: vec![
            DslFlow {
                id: "f1".to_string(),
                source_ref: "start".to_string(),
                target_ref: "call_api".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f2".to_string(),
                source_ref: "call_api".to_string(),
                target_ref: "transform_response".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f3".to_string(),
                source_ref: "transform_response".to_string(),
                target_ref: "end".to_string(),
                name: None,
                condition: None,
            },
        ],
        variables: HashMap::new(),
    }
}

fn create_error_handling_workflow() -> DslWorkflow {
    DslWorkflow {
        id: "error-handling-001".to_string(),
        name: "Error Handling Workflow".to_string(),
        description: Some("Pattern 19: Cancel Activity + Error Events".to_string()),
        version: Some("1.0".to_string()),
        elements: vec![
            DslElement::StartEvent {
                id: "start".to_string(),
                name: "Start".to_string(),
                trigger: None,
            },
            DslElement::Task {
                id: "risky_task".to_string(),
                name: "Risky Operation".to_string(),
                task_type: DslTaskType::Task,
                properties: HashMap::new(),
            },
            DslElement::BoundaryEvent {
                id: "timeout".to_string(),
                name: "Timeout".to_string(),
                attached_to: "risky_task".to_string(),
                interrupting: true,
                event_type: DslBoundaryEventType::Timer { duration_ms: 5000 },
            },
            DslElement::BoundaryEvent {
                id: "error_catch".to_string(),
                name: "Error Handler".to_string(),
                attached_to: "risky_task".to_string(),
                interrupting: true,
                event_type: DslBoundaryEventType::Error {
                    error_code: "PROCESSING_ERROR".to_string(),
                },
            },
            DslElement::Task {
                id: "handle_timeout".to_string(),
                name: "Handle Timeout".to_string(),
                task_type: DslTaskType::Task,
                properties: HashMap::new(),
            },
            DslElement::Task {
                id: "handle_error".to_string(),
                name: "Handle Error".to_string(),
                task_type: DslTaskType::Task,
                properties: HashMap::new(),
            },
            DslElement::EndEvent {
                id: "end_success".to_string(),
                name: "Success".to_string(),
                result: None,
            },
            DslElement::EndEvent {
                id: "end_error".to_string(),
                name: "Error".to_string(),
                result: Some(DslEndResult::Error {
                    error_code: "WORKFLOW_ERROR".to_string(),
                }),
            },
        ],
        flows: vec![
            DslFlow {
                id: "f1".to_string(),
                source_ref: "start".to_string(),
                target_ref: "risky_task".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f2".to_string(),
                source_ref: "risky_task".to_string(),
                target_ref: "end_success".to_string(),
                name: Some("Success path".to_string()),
                condition: None,
            },
            DslFlow {
                id: "f3".to_string(),
                source_ref: "timeout".to_string(),
                target_ref: "handle_timeout".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f4".to_string(),
                source_ref: "error_catch".to_string(),
                target_ref: "handle_error".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f5".to_string(),
                source_ref: "handle_timeout".to_string(),
                target_ref: "end_error".to_string(),
                name: None,
                condition: None,
            },
            DslFlow {
                id: "f6".to_string(),
                source_ref: "handle_error".to_string(),
                target_ref: "end_error".to_string(),
                name: None,
                condition: None,
            },
        ],
        variables: HashMap::new(),
    }
}
