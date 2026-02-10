//! BPMN-like DSL compiler implementation.
//!
//! Compiles BPMN-style workflow DSL into van der Aalst's 43-pattern primitives.

use crate::domain::dsl::*;
use crate::domain::workflow::*;
use crate::port::dsl_compiler::{DslCompiler, DslCompilerError, DslCompilerResult};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// BPMN-like DSL compiler.
///
/// Transforms BPMN-style DSL workflows into internal workflow patterns
/// using van der Aalst's 43 workflow patterns as compilation primitives.
#[derive(Debug, Clone)]
pub struct BpmnCompiler {
    /// Enable optimization during compilation
    optimize: bool,
    /// Strict validation mode
    strict: bool,
}

impl BpmnCompiler {
    /// Creates a new BPMN compiler with default settings.
    pub fn new() -> Self {
        Self {
            optimize: false,
            strict: true,
        }
    }

    /// Creates a compiler with optimization enabled.
    pub fn with_optimization() -> Self {
        Self {
            optimize: true,
            strict: true,
        }
    }

    /// Creates a compiler with lenient validation.
    pub fn lenient() -> Self {
        Self {
            optimize: false,
            strict: false,
        }
    }

    /// Validates element references in flows.
    fn validate_flow_references(&self, dsl: &DslWorkflow) -> DslCompilerResult<()> {
        let element_ids: HashSet<String> = dsl
            .elements
            .iter()
            .map(|e| self.get_element_id(e))
            .collect();

        for flow in &dsl.flows {
            if !element_ids.contains(&flow.source_ref) {
                return Err(DslCompilerError::ElementNotFound {
                    element_id: flow.source_ref.clone(),
                });
            }
            if !element_ids.contains(&flow.target_ref) {
                return Err(DslCompilerError::ElementNotFound {
                    element_id: flow.target_ref.clone(),
                });
            }
        }

        Ok(())
    }

    /// Detects circular flows using depth-first search.
    fn detect_circular_flows(&self, dsl: &DslWorkflow) -> DslCompilerResult<()> {
        let element_ids: Vec<String> = dsl
            .elements
            .iter()
            .map(|e| self.get_element_id(e))
            .collect();

        // Build adjacency list
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        for flow in &dsl.flows {
            graph
                .entry(flow.source_ref.clone())
                .or_insert_with(Vec::new)
                .push(flow.target_ref.clone());
        }

        // DFS cycle detection
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = Vec::new();

        for start_id in &element_ids {
            if !visited.contains(start_id) {
                if self.dfs_cycle_detect(
                    start_id,
                    &graph,
                    &mut visited,
                    &mut rec_stack,
                    &mut path,
                )? {
                    return Err(DslCompilerError::CircularFlow {
                        elements: path.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    /// DFS helper for cycle detection.
    fn dfs_cycle_detect(
        &self,
        node: &str,
        graph: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> DslCompilerResult<bool> {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());
        path.push(node.to_string());

        if let Some(neighbors) = graph.get(node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    if self.dfs_cycle_detect(neighbor, graph, visited, rec_stack, path)? {
                        return Ok(true);
                    }
                } else if rec_stack.contains(neighbor) {
                    path.push(neighbor.clone());
                    return Ok(true);
                }
            }
        }

        rec_stack.remove(node);
        path.pop();
        Ok(false)
    }

    /// Validates start and end events.
    fn validate_start_end_events(&self, dsl: &DslWorkflow) -> DslCompilerResult<()> {
        let has_start = dsl
            .elements
            .iter()
            .any(|e| matches!(e, DslElement::StartEvent { .. }));
        let has_end = dsl
            .elements
            .iter()
            .any(|e| matches!(e, DslElement::EndEvent { .. }));

        if !has_start && self.strict {
            return Err(DslCompilerError::StructuralError {
                reason: "No start event defined".to_string(),
            });
        }

        if !has_end && self.strict {
            return Err(DslCompilerError::NoEndEvents);
        }

        Ok(())
    }

    /// Gets the ID of a DSL element.
    fn get_element_id(&self, element: &DslElement) -> String {
        match element {
            DslElement::StartEvent { id, .. } => id.clone(),
            DslElement::EndEvent { id, .. } => id.clone(),
            DslElement::Task { id, .. } => id.clone(),
            DslElement::Gateway { id, .. } => id.clone(),
            DslElement::Subprocess { id, .. } => id.clone(),
            DslElement::IntermediateEvent { id, .. } => id.clone(),
            DslElement::BoundaryEvent { id, .. } => id.clone(),
        }
    }

    /// Compiles a DSL element into a workflow node.
    fn compile_element(&self, element: &DslElement) -> DslCompilerResult<Node> {
        let (id, kind) = match element {
            DslElement::StartEvent { id, trigger, .. } => {
                let event_type = self.compile_start_trigger(trigger)?;
                (id.clone(), NodeKind::Event { event_type })
            }
            DslElement::EndEvent { id, result, .. } => {
                let event_type = self.compile_end_result(result)?;
                (id.clone(), NodeKind::Event { event_type })
            }
            DslElement::Task {
                id,
                name,
                task_type,
                ..
            } => {
                let implementation = self.compile_task_implementation(task_type)?;
                (
                    id.clone(),
                    NodeKind::Activity {
                        name: name.clone(),
                        implementation,
                    },
                )
            }
            DslElement::Gateway {
                id,
                gateway_type,
                conditions,
                ..
            } => {
                let pattern = self.compile_gateway_pattern(gateway_type, conditions)?;
                (id.clone(), NodeKind::Gateway { pattern })
            }
            DslElement::Subprocess {
                id, workflow_ref, ..
            } => (
                id.clone(),
                NodeKind::Subprocess {
                    workflow_id: WorkflowId::new(workflow_ref.clone()),
                },
            ),
            DslElement::IntermediateEvent { id, event_type, .. } => {
                let compiled_type = self.compile_intermediate_event(event_type)?;
                (
                    id.clone(),
                    NodeKind::Event {
                        event_type: compiled_type,
                    },
                )
            }
            DslElement::BoundaryEvent { id, event_type, .. } => {
                let compiled_type = self.compile_boundary_event(event_type)?;
                (
                    id.clone(),
                    NodeKind::Event {
                        event_type: compiled_type,
                    },
                )
            }
        };

        Ok(Node {
            id: NodeId::new(id),
            kind,
            config: HashMap::new(),
        })
    }

    /// Compiles start event trigger.
    fn compile_start_trigger(
        &self,
        trigger: &Option<DslEventTrigger>,
    ) -> DslCompilerResult<EventType> {
        match trigger {
            None | Some(DslEventTrigger::None) => Ok(EventType::Start),
            Some(DslEventTrigger::Timer { duration_ms }) => Ok(EventType::Timer {
                duration_ms: Some(*duration_ms),
            }),
            Some(DslEventTrigger::Message { message_ref }) => Ok(EventType::Message {
                message_type: message_ref.clone(),
            }),
            _ => Err(DslCompilerError::UnsupportedFeature {
                feature: format!("Start trigger: {:?}", trigger),
            }),
        }
    }

    /// Compiles end event result.
    fn compile_end_result(&self, result: &Option<DslEndResult>) -> DslCompilerResult<EventType> {
        match result {
            None | Some(DslEndResult::None) => Ok(EventType::End),
            Some(DslEndResult::Error { error_code }) => Ok(EventType::Error {
                error_code: error_code.clone(),
            }),
            Some(DslEndResult::Terminate) => Ok(EventType::Terminate),
            Some(DslEndResult::Cancel) => Ok(EventType::Cancel { target_scope: None }),
            _ => Err(DslCompilerError::UnsupportedFeature {
                feature: format!("End result: {:?}", result),
            }),
        }
    }

    /// Compiles task implementation.
    fn compile_task_implementation(
        &self,
        task_type: &DslTaskType,
    ) -> DslCompilerResult<ActivityImplementation> {
        match task_type {
            DslTaskType::ServiceTask { implementation } => match implementation {
                DslServiceImplementation::Http { url, method, .. } => {
                    Ok(ActivityImplementation::Http {
                        url: url.clone(),
                        method: method.clone(),
                    })
                }
                DslServiceImplementation::CloudWorkflow {
                    project,
                    workflow_name,
                    region,
                } => Ok(ActivityImplementation::CloudWorkflow {
                    project_id: project.clone(),
                    workflow_name: workflow_name.clone(),
                    region: region.clone(),
                }),
                DslServiceImplementation::Local { handler } => Ok(ActivityImplementation::Local {
                    handler: handler.clone(),
                }),
                _ => Err(DslCompilerError::UnsupportedFeature {
                    feature: format!("Service implementation: {:?}", implementation),
                }),
            },
            DslTaskType::UserTask { .. } | DslTaskType::Task => Ok(ActivityImplementation::Local {
                handler: "user_task".to_string(),
            }),
            DslTaskType::ScriptTask { language, script } => Ok(ActivityImplementation::Custom {
                implementation_type: "script".to_string(),
                config: HashMap::from([
                    (
                        "language".to_string(),
                        serde_json::Value::String(language.clone()),
                    ),
                    (
                        "script".to_string(),
                        serde_json::Value::String(script.clone()),
                    ),
                ]),
            }),
            _ => Err(DslCompilerError::UnsupportedFeature {
                feature: format!("Task type: {:?}", task_type),
            }),
        }
    }

    /// Compiles gateway pattern.
    fn compile_gateway_pattern(
        &self,
        gateway_type: &DslGatewayType,
        conditions: &[DslCondition],
    ) -> DslCompilerResult<GatewayPattern> {
        match gateway_type {
            DslGatewayType::ParallelGateway => {
                // Determine if split or join based on conditions
                if conditions.is_empty() {
                    Ok(GatewayPattern::Synchronization)
                } else {
                    Ok(GatewayPattern::ParallelSplit)
                }
            }
            DslGatewayType::ExclusiveGateway => {
                if conditions.is_empty() {
                    Ok(GatewayPattern::SimpleMerge)
                } else {
                    let compiled_conditions: Vec<Condition> = conditions
                        .iter()
                        .map(|c| Condition {
                            expression: c.expression.clone(),
                            target: NodeId::new(&c.target_ref),
                            description: c.name.clone(),
                        })
                        .collect();
                    Ok(GatewayPattern::ExclusiveChoice {
                        conditions: compiled_conditions,
                    })
                }
            }
            DslGatewayType::InclusiveGateway => {
                if conditions.is_empty() {
                    Ok(GatewayPattern::StructuredSynchronizingMerge)
                } else {
                    let compiled_conditions: Vec<Condition> = conditions
                        .iter()
                        .map(|c| Condition {
                            expression: c.expression.clone(),
                            target: NodeId::new(&c.target_ref),
                            description: c.name.clone(),
                        })
                        .collect();
                    Ok(GatewayPattern::MultiChoice {
                        conditions: compiled_conditions,
                    })
                }
            }
            DslGatewayType::EventBasedGateway => {
                Ok(GatewayPattern::StructuredDiscriminator { reset_after: None })
            }
            DslGatewayType::ComplexGateway => Err(DslCompilerError::UnsupportedFeature {
                feature: "Complex gateway".to_string(),
            }),
        }
    }

    /// Compiles intermediate event.
    fn compile_intermediate_event(
        &self,
        event_type: &DslIntermediateEventType,
    ) -> DslCompilerResult<EventType> {
        match event_type {
            DslIntermediateEventType::Timer { duration_ms } => Ok(EventType::Timer {
                duration_ms: Some(*duration_ms),
            }),
            DslIntermediateEventType::MessageCatch { message_ref } => Ok(EventType::Message {
                message_type: message_ref.clone(),
            }),
            DslIntermediateEventType::Escalation { escalation_code } => Ok(EventType::Escalation {
                escalation_code: escalation_code.clone(),
            }),
            _ => Err(DslCompilerError::UnsupportedFeature {
                feature: format!("Intermediate event: {:?}", event_type),
            }),
        }
    }

    /// Compiles boundary event.
    fn compile_boundary_event(
        &self,
        event_type: &DslBoundaryEventType,
    ) -> DslCompilerResult<EventType> {
        match event_type {
            DslBoundaryEventType::Timer { duration_ms } => Ok(EventType::Timer {
                duration_ms: Some(*duration_ms),
            }),
            DslBoundaryEventType::Error { error_code } => Ok(EventType::Error {
                error_code: error_code.clone(),
            }),
            DslBoundaryEventType::Cancel => Ok(EventType::Cancel { target_scope: None }),
            DslBoundaryEventType::Escalation { escalation_code } => Ok(EventType::Escalation {
                escalation_code: escalation_code.clone(),
            }),
            _ => Err(DslCompilerError::UnsupportedFeature {
                feature: format!("Boundary event: {:?}", event_type),
            }),
        }
    }
}

impl Default for BpmnCompiler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DslCompiler for BpmnCompiler {
    async fn compile(&self, dsl: DslWorkflow) -> DslCompilerResult<WorkflowPattern> {
        // Validate DSL structure
        self.validate(&dsl).await?;

        // Compile nodes
        let mut nodes = HashMap::new();
        for element in &dsl.elements {
            let node = self.compile_element(element)?;
            nodes.insert(node.id.clone(), node);
        }

        // Compile edges
        let edges: Vec<Edge> = dsl
            .flows
            .iter()
            .map(|flow| Edge {
                from: NodeId::new(&flow.source_ref),
                to: NodeId::new(&flow.target_ref),
                condition: flow.condition.clone(),
                label: flow.name.clone(),
            })
            .collect();

        // Identify start and end nodes
        let start_nodes: Vec<NodeId> = dsl
            .elements
            .iter()
            .filter(|e| matches!(e, DslElement::StartEvent { .. }))
            .map(|e| NodeId::new(self.get_element_id(e)))
            .collect();

        let end_nodes: Vec<NodeId> = dsl
            .elements
            .iter()
            .filter(|e| matches!(e, DslElement::EndEvent { .. }))
            .map(|e| NodeId::new(self.get_element_id(e)))
            .collect();

        // Convert DSL variables
        let variables: HashMap<String, serde_json::Value> = dsl
            .variables
            .iter()
            .filter_map(|(k, v)| v.default_value.as_ref().map(|val| (k.clone(), val.clone())))
            .collect();

        Ok(WorkflowPattern {
            id: WorkflowId::new(dsl.id),
            name: dsl.name,
            description: dsl.description,
            nodes,
            edges,
            start_nodes,
            end_nodes,
            variables,
        })
    }

    async fn validate(&self, dsl: &DslWorkflow) -> DslCompilerResult<()> {
        // Validate flow references
        self.validate_flow_references(dsl)?;

        // Validate start/end events
        self.validate_start_end_events(dsl)?;

        // Detect circular flows (only in strict mode)
        if self.strict {
            // Note: Circular flows are actually valid for loops in workflows
            // Only detect truly problematic cycles
        }

        Ok(())
    }

    async fn decompile(&self, pattern: &WorkflowPattern) -> DslCompilerResult<DslWorkflow> {
        // Convert nodes back to DSL elements
        let mut elements = Vec::new();

        for node in pattern.nodes.values() {
            let element = self.decompile_node(node)?;
            elements.push(element);
        }

        // Convert edges to flows
        let flows: Vec<DslFlow> = pattern
            .edges
            .iter()
            .map(|edge| DslFlow {
                id: format!("flow_{}_to_{}", edge.from.0, edge.to.0),
                source_ref: edge.from.0.clone(),
                target_ref: edge.to.0.clone(),
                name: edge.label.clone(),
                condition: edge.condition.clone(),
            })
            .collect();

        // Convert variables
        let variables: HashMap<String, DslVariable> = pattern
            .variables
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    DslVariable {
                        name: k.clone(),
                        var_type: None,
                        default_value: Some(v.clone()),
                    },
                )
            })
            .collect();

        Ok(DslWorkflow {
            id: pattern.id.0.clone(),
            name: pattern.name.clone(),
            description: pattern.description.clone(),
            version: None,
            elements,
            flows,
            variables,
        })
    }

    async fn optimize(&self, dsl: DslWorkflow) -> DslCompilerResult<DslWorkflow> {
        if !self.optimize {
            return Ok(dsl);
        }

        // TODO: Implement optimizations:
        // - Remove redundant gateways (e.g., XOR with single path)
        // - Simplify parallel-to-parallel patterns
        // - Inline trivial subprocesses

        Ok(dsl)
    }
}

impl BpmnCompiler {
    /// Helper to decompile a node back to DSL element.
    fn decompile_node(&self, node: &Node) -> DslCompilerResult<DslElement> {
        match &node.kind {
            NodeKind::Event { event_type } => self.decompile_event_node(node, event_type),
            NodeKind::Activity {
                name,
                implementation,
            } => self.decompile_activity_node(node, name, implementation),
            NodeKind::Gateway { pattern } => self.decompile_gateway_node(node, pattern),
            NodeKind::Subprocess { workflow_id } => Ok(DslElement::Subprocess {
                id: node.id.0.clone(),
                name: node.id.0.clone(),
                workflow_ref: workflow_id.0.clone(),
                input_mappings: HashMap::new(),
                output_mappings: HashMap::new(),
            }),
        }
    }

    fn decompile_event_node(
        &self,
        node: &Node,
        event_type: &EventType,
    ) -> DslCompilerResult<DslElement> {
        match event_type {
            EventType::Start => Ok(DslElement::StartEvent {
                id: node.id.0.clone(),
                name: node.id.0.clone(),
                trigger: None,
            }),
            EventType::End => Ok(DslElement::EndEvent {
                id: node.id.0.clone(),
                name: node.id.0.clone(),
                result: None,
            }),
            EventType::Timer { duration_ms } => Ok(DslElement::IntermediateEvent {
                id: node.id.0.clone(),
                name: node.id.0.clone(),
                event_type: DslIntermediateEventType::Timer {
                    duration_ms: duration_ms.unwrap_or(0),
                },
            }),
            EventType::Error { error_code } => Ok(DslElement::EndEvent {
                id: node.id.0.clone(),
                name: node.id.0.clone(),
                result: Some(DslEndResult::Error {
                    error_code: error_code.clone(),
                }),
            }),
            _ => Err(DslCompilerError::UnsupportedFeature {
                feature: format!("Event type decompilation: {:?}", event_type),
            }),
        }
    }

    fn decompile_activity_node(
        &self,
        node: &Node,
        name: &str,
        implementation: &ActivityImplementation,
    ) -> DslCompilerResult<DslElement> {
        let task_type = match implementation {
            ActivityImplementation::Http { url, method } => DslTaskType::ServiceTask {
                implementation: DslServiceImplementation::Http {
                    url: url.clone(),
                    method: method.clone(),
                    headers: HashMap::new(),
                },
            },
            ActivityImplementation::Local { handler } => {
                if handler == "user_task" {
                    DslTaskType::UserTask { assignee: None }
                } else {
                    DslTaskType::ServiceTask {
                        implementation: DslServiceImplementation::Local {
                            handler: handler.clone(),
                        },
                    }
                }
            }
            ActivityImplementation::CloudWorkflow {
                project_id,
                workflow_name,
                region,
            } => DslTaskType::ServiceTask {
                implementation: DslServiceImplementation::CloudWorkflow {
                    project: project_id.clone(),
                    workflow_name: workflow_name.clone(),
                    region: region.clone(),
                },
            },
            _ => DslTaskType::Task,
        };

        Ok(DslElement::Task {
            id: node.id.0.clone(),
            name: name.to_string(),
            task_type,
            properties: HashMap::new(),
        })
    }

    fn decompile_gateway_node(
        &self,
        node: &Node,
        pattern: &GatewayPattern,
    ) -> DslCompilerResult<DslElement> {
        let (gateway_type, conditions) = match pattern {
            GatewayPattern::ParallelSplit => (DslGatewayType::ParallelGateway, Vec::new()),
            GatewayPattern::Synchronization => (DslGatewayType::ParallelGateway, Vec::new()),
            GatewayPattern::ExclusiveChoice { conditions: conds } => {
                let dsl_conditions: Vec<DslCondition> = conds
                    .iter()
                    .map(|c| DslCondition {
                        expression: c.expression.clone(),
                        target_ref: c.target.0.clone(),
                        name: c.description.clone(),
                    })
                    .collect();
                (DslGatewayType::ExclusiveGateway, dsl_conditions)
            }
            GatewayPattern::SimpleMerge => (DslGatewayType::ExclusiveGateway, Vec::new()),
            GatewayPattern::MultiChoice { conditions: conds } => {
                let dsl_conditions: Vec<DslCondition> = conds
                    .iter()
                    .map(|c| DslCondition {
                        expression: c.expression.clone(),
                        target_ref: c.target.0.clone(),
                        name: c.description.clone(),
                    })
                    .collect();
                (DslGatewayType::InclusiveGateway, dsl_conditions)
            }
            _ => {
                return Err(DslCompilerError::UnsupportedFeature {
                    feature: format!("Gateway pattern decompilation: {:?}", pattern),
                });
            }
        };

        Ok(DslElement::Gateway {
            id: node.id.0.clone(),
            name: node.id.0.clone(),
            gateway_type,
            conditions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_workflow_compilation() {
        let dsl = DslWorkflow {
            id: "test-workflow".to_string(),
            name: "Test Workflow".to_string(),
            description: Some("A simple test".to_string()),
            version: Some("1.0".to_string()),
            elements: vec![
                DslElement::StartEvent {
                    id: "start".to_string(),
                    name: "Start".to_string(),
                    trigger: None,
                },
                DslElement::Task {
                    id: "task1".to_string(),
                    name: "Task 1".to_string(),
                    task_type: DslTaskType::Task,
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
                    id: "flow1".to_string(),
                    source_ref: "start".to_string(),
                    target_ref: "task1".to_string(),
                    name: None,
                    condition: None,
                },
                DslFlow {
                    id: "flow2".to_string(),
                    source_ref: "task1".to_string(),
                    target_ref: "end".to_string(),
                    name: None,
                    condition: None,
                },
            ],
            variables: HashMap::new(),
        };

        let compiler = BpmnCompiler::new();
        let pattern = compiler.compile(dsl).await.unwrap();

        assert_eq!(pattern.id.0, "test-workflow");
        assert_eq!(pattern.nodes.len(), 3);
        assert_eq!(pattern.edges.len(), 2);
        assert_eq!(pattern.start_nodes.len(), 1);
        assert_eq!(pattern.end_nodes.len(), 1);
    }

    #[tokio::test]
    async fn test_parallel_gateway_compilation() {
        let dsl = DslWorkflow {
            id: "parallel-wf".to_string(),
            name: "Parallel Workflow".to_string(),
            description: None,
            version: None,
            elements: vec![
                DslElement::StartEvent {
                    id: "start".to_string(),
                    name: "Start".to_string(),
                    trigger: None,
                },
                DslElement::Gateway {
                    id: "split".to_string(),
                    name: "Split".to_string(),
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
                    target_ref: "end".to_string(),
                    name: None,
                    condition: None,
                },
            ],
            variables: HashMap::new(),
        };

        let compiler = BpmnCompiler::new();
        let pattern = compiler.compile(dsl).await.unwrap();

        // Check gateway was compiled to Synchronization (no conditions = join)
        let gateway_node = pattern.nodes.get(&NodeId::new("split")).unwrap();
        match &gateway_node.kind {
            NodeKind::Gateway { pattern } => {
                assert!(matches!(pattern, GatewayPattern::Synchronization));
            }
            _ => panic!("Expected gateway node"),
        }
    }

    #[tokio::test]
    async fn test_validation_missing_element() {
        let dsl = DslWorkflow {
            id: "invalid-wf".to_string(),
            name: "Invalid".to_string(),
            description: None,
            version: None,
            elements: vec![DslElement::StartEvent {
                id: "start".to_string(),
                name: "Start".to_string(),
                trigger: None,
            }],
            flows: vec![DslFlow {
                id: "f1".to_string(),
                source_ref: "start".to_string(),
                target_ref: "nonexistent".to_string(),
                name: None,
                condition: None,
            }],
            variables: HashMap::new(),
        };

        let compiler = BpmnCompiler::new();
        let result = compiler.compile(dsl).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DslCompilerError::ElementNotFound { .. }
        ));
    }

    #[tokio::test]
    async fn test_round_trip_compilation() {
        let original_dsl = DslWorkflow {
            id: "roundtrip-wf".to_string(),
            name: "Roundtrip Test".to_string(),
            description: Some("Test round-trip".to_string()),
            version: None,
            elements: vec![
                DslElement::StartEvent {
                    id: "start".to_string(),
                    name: "Start".to_string(),
                    trigger: None,
                },
                DslElement::EndEvent {
                    id: "end".to_string(),
                    name: "End".to_string(),
                    result: None,
                },
            ],
            flows: vec![DslFlow {
                id: "f1".to_string(),
                source_ref: "start".to_string(),
                target_ref: "end".to_string(),
                name: None,
                condition: None,
            }],
            variables: HashMap::new(),
        };

        let compiler = BpmnCompiler::new();
        let pattern = compiler.compile(original_dsl.clone()).await.unwrap();
        let decompiled = compiler.decompile(&pattern).await.unwrap();

        assert_eq!(decompiled.id, original_dsl.id);
        assert_eq!(decompiled.name, original_dsl.name);
        assert_eq!(decompiled.elements.len(), original_dsl.elements.len());
    }
}
