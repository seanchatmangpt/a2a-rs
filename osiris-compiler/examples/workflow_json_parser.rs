//! JSON workflow parser and round-trip example.
//!
//! Demonstrates:
//! - Loading workflow DSL from JSON files
//! - Parsing and validation
//! - Compilation to workflow patterns
//! - Decompilation back to DSL (round-trip)
//! - Pattern analysis

use osiris_compiler::prelude::*;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Workflow JSON Parser Example ===\n");

    // Example workflow DSL as JSON string (simulating file load)
    let json_workflow = r#"
    {
        "id": "approval-workflow-v1",
        "name": "Purchase Approval Workflow",
        "description": "Multi-level approval workflow with compensation",
        "version": "1.0.0",
        "elements": [
            {
                "elementType": "startEvent",
                "id": "start",
                "name": "Purchase Request Received",
                "trigger": null
            },
            {
                "elementType": "task",
                "id": "validate",
                "name": "Validate Purchase Request",
                "taskType": {
                    "type": "serviceTask",
                    "implementation": {
                        "implementationType": "local",
                        "handler": "validate_purchase_request"
                    }
                },
                "properties": {}
            },
            {
                "elementType": "gateway",
                "id": "amount_check",
                "name": "Check Amount",
                "gatewayType": "exclusiveGateway",
                "conditions": [
                    {
                        "expression": "amount > 10000",
                        "targetRef": "cfo_approval",
                        "name": "Large purchase"
                    },
                    {
                        "expression": "amount > 1000",
                        "targetRef": "manager_approval",
                        "name": "Medium purchase"
                    },
                    {
                        "expression": "amount <= 1000",
                        "targetRef": "auto_approve",
                        "name": "Small purchase"
                    }
                ]
            },
            {
                "elementType": "task",
                "id": "cfo_approval",
                "name": "CFO Approval",
                "taskType": {
                    "type": "userTask",
                    "assignee": "cfo"
                },
                "properties": {}
            },
            {
                "elementType": "task",
                "id": "manager_approval",
                "name": "Manager Approval",
                "taskType": {
                    "type": "userTask",
                    "assignee": "manager"
                },
                "properties": {}
            },
            {
                "elementType": "task",
                "id": "auto_approve",
                "name": "Auto Approve",
                "taskType": {
                    "type": "task"
                },
                "properties": {}
            },
            {
                "elementType": "gateway",
                "id": "merge",
                "name": "Merge Approvals",
                "gatewayType": "exclusiveGateway",
                "conditions": []
            },
            {
                "elementType": "task",
                "id": "execute_purchase",
                "name": "Execute Purchase",
                "taskType": {
                    "type": "serviceTask",
                    "implementation": {
                        "implementationType": "http",
                        "url": "https://api.procurement.example.com/execute",
                        "method": "POST",
                        "headers": {
                            "Content-Type": "application/json"
                        }
                    }
                },
                "properties": {}
            },
            {
                "elementType": "endEvent",
                "id": "end_success",
                "name": "Purchase Completed",
                "result": null
            }
        ],
        "flows": [
            {
                "id": "f1",
                "sourceRef": "start",
                "targetRef": "validate",
                "name": null,
                "condition": null
            },
            {
                "id": "f2",
                "sourceRef": "validate",
                "targetRef": "amount_check",
                "name": null,
                "condition": null
            },
            {
                "id": "f3",
                "sourceRef": "amount_check",
                "targetRef": "cfo_approval",
                "name": "Large",
                "condition": "amount > 10000"
            },
            {
                "id": "f4",
                "sourceRef": "amount_check",
                "targetRef": "manager_approval",
                "name": "Medium",
                "condition": "amount > 1000 && amount <= 10000"
            },
            {
                "id": "f5",
                "sourceRef": "amount_check",
                "targetRef": "auto_approve",
                "name": "Small",
                "condition": "amount <= 1000"
            },
            {
                "id": "f6",
                "sourceRef": "cfo_approval",
                "targetRef": "merge",
                "name": null,
                "condition": null
            },
            {
                "id": "f7",
                "sourceRef": "manager_approval",
                "targetRef": "merge",
                "name": null,
                "condition": null
            },
            {
                "id": "f8",
                "sourceRef": "auto_approve",
                "targetRef": "merge",
                "name": null,
                "condition": null
            },
            {
                "id": "f9",
                "sourceRef": "merge",
                "targetRef": "execute_purchase",
                "name": null,
                "condition": null
            },
            {
                "id": "f10",
                "sourceRef": "execute_purchase",
                "targetRef": "end_success",
                "name": null,
                "condition": null
            }
        ],
        "variables": {
            "amount": {
                "name": "amount",
                "varType": "number",
                "defaultValue": 5000
            },
            "requestor": {
                "name": "requestor",
                "varType": "string",
                "defaultValue": "employee@example.com"
            }
        }
    }
    "#;

    println!("Step 1: Parse JSON to DSL");
    println!("-------------------------");

    let dsl: DslWorkflow = serde_json::from_str(json_workflow)?;
    println!("✓ Parsed workflow: {}", dsl.name);
    println!("  - ID: {}", dsl.id);
    println!("  - Version: {}", dsl.version.as_ref().unwrap());
    println!("  - Elements: {}", dsl.elements.len());
    println!("  - Flows: {}", dsl.flows.len());
    println!("  - Variables: {}\n", dsl.variables.len());

    // Analyze DSL structure
    analyze_dsl_structure(&dsl);

    println!("\nStep 2: Compile to Workflow Pattern");
    println!("------------------------------------");

    let compiler = BpmnCompiler::new();

    // Validate first
    println!("Validating workflow...");
    match compiler.validate(&dsl).await {
        Ok(()) => println!("✓ Validation passed"),
        Err(e) => {
            println!("✗ Validation failed: {}", e);
            return Err(Box::new(e));
        }
    }

    // Compile
    println!("\nCompiling to 43-pattern primitives...");
    let pattern = compiler.compile(dsl.clone()).await?;
    println!("✓ Compilation successful");
    println!("  - Workflow ID: {}", pattern.id.0);
    println!("  - Nodes: {}", pattern.nodes.len());
    println!("  - Edges: {}", pattern.edges.len());
    println!("  - Start nodes: {:?}", pattern.start_nodes);
    println!("  - End nodes: {:?}", pattern.end_nodes);

    // Analyze compiled pattern
    analyze_compiled_pattern(&pattern);

    println!("\nStep 3: Round-Trip Decompilation");
    println!("---------------------------------");

    println!("Decompiling pattern back to DSL...");
    let decompiled = compiler.decompile(&pattern).await?;
    println!("✓ Decompilation successful");
    println!("  - Workflow ID: {}", decompiled.id);
    println!("  - Elements: {}", decompiled.elements.len());
    println!("  - Flows: {}", decompiled.flows.len());

    // Compare original and decompiled
    println!("\nComparing original and decompiled:");
    println!(
        "  - Elements match: {}",
        dsl.elements.len() == decompiled.elements.len()
    );
    println!(
        "  - Flows match: {}",
        dsl.flows.len() == decompiled.flows.len()
    );
    println!("  - ID match: {}", dsl.id == decompiled.id);

    // Show decompiled JSON
    println!("\nDecompiled workflow (JSON):");
    println!("{}", serde_json::to_string_pretty(&decompiled)?);

    println!("\nStep 4: Execute Workflow Instance");
    println!("----------------------------------");

    let mut kernel = InMemoryWorkflowKernel::new();
    kernel.register_pattern(pattern.clone()).await?;

    let mut context = HashMap::new();
    context.insert("amount".to_string(), serde_json::json!(7500));
    context.insert(
        "requestor".to_string(),
        serde_json::json!("alice@example.com"),
    );

    let instance_id = kernel.start_instance(&pattern.id, context).await?;
    println!("✓ Started instance: {}", instance_id);

    // Execute a few steps
    for step in 0..5 {
        let instance = kernel.get_instance(&instance_id).await?;
        println!("\nStep {}: {:?}", step, instance.state);
        println!("  Active: {:?}", instance.active_nodes);

        if instance.state != InstanceState::Active {
            break;
        }

        let activated = kernel.execute_step(&instance_id).await?;
        if activated.is_empty() {
            println!("  (No more enabled nodes)");
            break;
        }
        println!("  Activated: {:?}", activated);
    }

    // Show final state
    let final_instance = kernel.get_instance(&instance_id).await?;
    println!("\nFinal State: {:?}", final_instance.state);
    println!("Execution History:");
    for (i, event) in final_instance.history.iter().enumerate() {
        println!("  {}. {:?}", i + 1, event.event_type);
    }

    println!("\n=== Complete ===");
    Ok(())
}

fn analyze_dsl_structure(dsl: &DslWorkflow) {
    println!("\nDSL Structure Analysis:");
    println!("----------------------");

    // Count element types
    let mut start_events = 0;
    let mut end_events = 0;
    let mut tasks = 0;
    let mut gateways = 0;
    let mut subprocesses = 0;
    let mut boundary_events = 0;

    for element in &dsl.elements {
        match element {
            DslElement::StartEvent { .. } => start_events += 1,
            DslElement::EndEvent { .. } => end_events += 1,
            DslElement::Task { .. } => tasks += 1,
            DslElement::Gateway { .. } => gateways += 1,
            DslElement::Subprocess { .. } => subprocesses += 1,
            DslElement::BoundaryEvent { .. } => boundary_events += 1,
            DslElement::IntermediateEvent { .. } => {}
        }
    }

    println!("  Element counts:");
    println!("    - Start events: {}", start_events);
    println!("    - End events: {}", end_events);
    println!("    - Tasks: {}", tasks);
    println!("    - Gateways: {}", gateways);
    println!("    - Subprocesses: {}", subprocesses);
    println!("    - Boundary events: {}", boundary_events);

    // Analyze gateway types
    let mut xor_gateways = 0;
    let mut and_gateways = 0;
    let mut or_gateways = 0;

    for element in &dsl.elements {
        if let DslElement::Gateway { gateway_type, .. } = element {
            match gateway_type {
                DslGatewayType::ExclusiveGateway => xor_gateways += 1,
                DslGatewayType::ParallelGateway => and_gateways += 1,
                DslGatewayType::InclusiveGateway => or_gateways += 1,
                _ => {}
            }
        }
    }

    println!("  Gateway breakdown:");
    println!("    - XOR (Exclusive): {}", xor_gateways);
    println!("    - AND (Parallel): {}", and_gateways);
    println!("    - OR (Inclusive): {}", or_gateways);
}

fn analyze_compiled_pattern(pattern: &WorkflowPattern) {
    println!("\nCompiled Pattern Analysis:");
    println!("-------------------------");

    // Count node types
    let mut activity_nodes = 0;
    let mut gateway_nodes = 0;
    let mut event_nodes = 0;
    let mut subprocess_nodes = 0;

    for node in pattern.nodes.values() {
        match &node.kind {
            NodeKind::Activity { .. } => activity_nodes += 1,
            NodeKind::Gateway { .. } => gateway_nodes += 1,
            NodeKind::Event { .. } => event_nodes += 1,
            NodeKind::Subprocess { .. } => subprocess_nodes += 1,
        }
    }

    println!("  Node types:");
    println!("    - Activities: {}", activity_nodes);
    println!("    - Gateways: {}", gateway_nodes);
    println!("    - Events: {}", event_nodes);
    println!("    - Subprocesses: {}", subprocess_nodes);

    // Analyze gateway patterns
    let mut parallel_split = 0;
    let mut synchronization = 0;
    let mut exclusive_choice = 0;
    let mut simple_merge = 0;
    let mut multi_choice = 0;

    for node in pattern.nodes.values() {
        if let NodeKind::Gateway {
            pattern: gw_pattern,
        } = &node.kind
        {
            match gw_pattern {
                GatewayPattern::ParallelSplit => parallel_split += 1,
                GatewayPattern::Synchronization => synchronization += 1,
                GatewayPattern::ExclusiveChoice { .. } => exclusive_choice += 1,
                GatewayPattern::SimpleMerge => simple_merge += 1,
                GatewayPattern::MultiChoice { .. } => multi_choice += 1,
                _ => {}
            }
        }
    }

    println!("  Workflow Patterns (van der Aalst):");
    println!("    - Pattern 2 (Parallel Split): {}", parallel_split);
    println!("    - Pattern 3 (Synchronization): {}", synchronization);
    println!("    - Pattern 4 (Exclusive Choice): {}", exclusive_choice);
    println!("    - Pattern 5 (Simple Merge): {}", simple_merge);
    println!("    - Pattern 6 (Multi-Choice): {}", multi_choice);

    // Calculate graph metrics
    let avg_degree = if pattern.nodes.is_empty() {
        0.0
    } else {
        (pattern.edges.len() * 2) as f64 / pattern.nodes.len() as f64
    };

    println!("  Graph metrics:");
    println!("    - Average node degree: {:.2}", avg_degree);
    println!("    - Total edges: {}", pattern.edges.len());
    println!("    - Density: {:.2}%", calculate_graph_density(pattern));
}

fn calculate_graph_density(pattern: &WorkflowPattern) -> f64 {
    let n = pattern.nodes.len();
    if n <= 1 {
        return 0.0;
    }
    let max_edges = n * (n - 1);
    let actual_edges = pattern.edges.len();
    (actual_edges as f64 / max_edges as f64) * 100.0
}
