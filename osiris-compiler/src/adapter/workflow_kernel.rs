//! WorkflowKernel adapter implementations.
//!
//! Provides concrete implementations of the WorkflowKernel port trait.

use crate::domain::workflow::{
    ActivityImplementation, CancellationRegion, Edge, EscalationConfig, EventType, ExecutionEvent,
    ExecutionEventType, GatewayPattern, InstanceState, MultiInstanceConfig, Node, NodeId, NodeKind,
    WorkflowId, WorkflowInstance, WorkflowPattern,
};
use crate::port::workflow_kernel::{WorkflowError, WorkflowKernel, WorkflowResult};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// In-memory implementation of WorkflowKernel.
///
/// Stores workflow patterns and instances in memory. Suitable for:
/// - Development and testing
/// - Single-node deployments
/// - Embedded workflow execution
///
/// For production use with persistence, implement WorkflowKernel using
/// a database backend (SQLx, etc.).
#[derive(Debug, Clone)]
pub struct InMemoryWorkflowKernel {
    patterns: Arc<RwLock<HashMap<WorkflowId, WorkflowPattern>>>,
    instances: Arc<RwLock<HashMap<String, WorkflowInstance>>>,
}

impl InMemoryWorkflowKernel {
    /// Creates a new in-memory workflow kernel.
    pub fn new() -> Self {
        Self {
            patterns: Arc::new(RwLock::new(HashMap::new())),
            instances: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Helper: generates a unique instance ID.
    fn generate_instance_id(&self) -> String {
        format!("inst-{}", uuid::Uuid::new_v4())
    }

    /// Helper: records an execution event in instance history.
    async fn record_event(
        &self,
        instance_id: &str,
        event_type: ExecutionEventType,
        node_id: Option<NodeId>,
        data: HashMap<String, serde_json::Value>,
    ) -> WorkflowResult<()> {
        let mut instances = self.instances.write().await;
        let instance =
            instances
                .get_mut(instance_id)
                .ok_or_else(|| WorkflowError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;

        let event = ExecutionEvent {
            event_type,
            node_id,
            #[cfg(feature = "timestamps")]
            timestamp: chrono::Utc::now(),
            #[cfg(not(feature = "timestamps"))]
            timestamp: "now".to_string(),
            data,
        };

        instance.history.push(event);
        Ok(())
    }

    /// Helper: validates workflow structure.
    fn validate_structure(&self, pattern: &WorkflowPattern) -> WorkflowResult<()> {
        // Check start nodes exist
        if pattern.start_nodes.is_empty() {
            return Err(WorkflowError::InvalidWorkflow {
                reason: "No start nodes defined".to_string(),
            });
        }

        // Check end nodes exist
        if pattern.end_nodes.is_empty() {
            return Err(WorkflowError::InvalidWorkflow {
                reason: "No end nodes defined".to_string(),
            });
        }

        // Validate all node references in edges exist
        for edge in &pattern.edges {
            if !pattern.nodes.contains_key(&edge.from) {
                return Err(WorkflowError::InvalidWorkflow {
                    reason: format!("Edge references non-existent node: {:?}", edge.from),
                });
            }
            if !pattern.nodes.contains_key(&edge.to) {
                return Err(WorkflowError::InvalidWorkflow {
                    reason: format!("Edge references non-existent node: {:?}", edge.to),
                });
            }
        }

        // Validate start and end nodes exist
        for node_id in &pattern.start_nodes {
            if !pattern.nodes.contains_key(node_id) {
                return Err(WorkflowError::InvalidWorkflow {
                    reason: format!("Start node does not exist: {:?}", node_id),
                });
            }
        }

        for node_id in &pattern.end_nodes {
            if !pattern.nodes.contains_key(node_id) {
                return Err(WorkflowError::InvalidWorkflow {
                    reason: format!("End node does not exist: {:?}", node_id),
                });
            }
        }

        Ok(())
    }

    /// Helper: finds enabled nodes (nodes ready to execute).
    async fn find_enabled_nodes_internal(&self, instance_id: &str) -> WorkflowResult<Vec<NodeId>> {
        let instances = self.instances.read().await;
        let instance =
            instances
                .get(instance_id)
                .ok_or_else(|| WorkflowError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;

        let patterns = self.patterns.read().await;
        let pattern =
            patterns
                .get(&instance.workflow_id)
                .ok_or_else(|| WorkflowError::WorkflowNotFound {
                    workflow_id: instance.workflow_id.0.clone(),
                })?;

        let mut enabled = Vec::new();

        // If no active nodes, start nodes are enabled
        if instance.active_nodes.is_empty() {
            return Ok(pattern.start_nodes.clone());
        }

        // Find nodes that have all prerequisites met
        for (node_id, _node) in &pattern.nodes {
            // Skip already active nodes
            if instance.active_nodes.contains(node_id) {
                continue;
            }

            // Check if all incoming edges are satisfied
            let incoming: Vec<&Edge> = pattern.edges.iter().filter(|e| &e.to == node_id).collect();

            if incoming.is_empty() {
                // No incoming edges - only start nodes should have this
                continue;
            }

            // For now, simple logic: node is enabled if any incoming edge's source is active
            // More sophisticated logic needed for proper gateway handling
            let any_incoming_active = incoming
                .iter()
                .any(|e| instance.active_nodes.contains(&e.from));

            if any_incoming_active {
                enabled.push(node_id.clone());
            }
        }

        Ok(enabled)
    }
}

impl Default for InMemoryWorkflowKernel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WorkflowKernel for InMemoryWorkflowKernel {
    // -------------------------------------------------------------------------
    // Pattern Management
    // -------------------------------------------------------------------------

    async fn register_pattern(&mut self, pattern: WorkflowPattern) -> WorkflowResult<()> {
        self.validate_structure(&pattern)?;

        let mut patterns = self.patterns.write().await;
        patterns.insert(pattern.id.clone(), pattern);
        Ok(())
    }

    async fn get_pattern(&self, workflow_id: &WorkflowId) -> WorkflowResult<WorkflowPattern> {
        let patterns = self.patterns.read().await;
        patterns
            .get(workflow_id)
            .cloned()
            .ok_or_else(|| WorkflowError::WorkflowNotFound {
                workflow_id: workflow_id.0.clone(),
            })
    }

    async fn list_patterns(&self) -> WorkflowResult<Vec<WorkflowId>> {
        let patterns = self.patterns.read().await;
        Ok(patterns.keys().cloned().collect())
    }

    async fn unregister_pattern(&mut self, workflow_id: &WorkflowId) -> WorkflowResult<()> {
        // Check if any active instances exist
        let instances = self.instances.read().await;
        let has_active = instances
            .values()
            .any(|i| &i.workflow_id == workflow_id && i.state == InstanceState::Active);

        if has_active {
            return Err(WorkflowError::InvalidWorkflow {
                reason: "Cannot unregister workflow with active instances".to_string(),
            });
        }

        let mut patterns = self.patterns.write().await;
        patterns.remove(workflow_id);
        Ok(())
    }

    async fn validate_pattern(&self, pattern: &WorkflowPattern) -> WorkflowResult<()> {
        self.validate_structure(pattern)
    }

    // -------------------------------------------------------------------------
    // Instance Lifecycle
    // -------------------------------------------------------------------------

    async fn start_instance(
        &mut self,
        workflow_id: &WorkflowId,
        initial_context: HashMap<String, serde_json::Value>,
    ) -> WorkflowResult<String> {
        // Verify workflow exists
        let patterns = self.patterns.read().await;
        let pattern = patterns
            .get(workflow_id)
            .ok_or_else(|| WorkflowError::WorkflowNotFound {
                workflow_id: workflow_id.0.clone(),
            })?;

        let instance_id = self.generate_instance_id();
        let instance = WorkflowInstance {
            instance_id: instance_id.clone(),
            workflow_id: workflow_id.clone(),
            state: InstanceState::Active,
            active_nodes: pattern.start_nodes.iter().cloned().collect(),
            context: initial_context,
            history: vec![],
            #[cfg(feature = "timestamps")]
            started_at: chrono::Utc::now(),
            #[cfg(not(feature = "timestamps"))]
            started_at: "now".to_string(),
            completed_at: None,
        };

        drop(patterns); // Release read lock

        let mut instances = self.instances.write().await;
        instances.insert(instance_id.clone(), instance);

        drop(instances); // Release write lock

        // Record start event
        self.record_event(
            &instance_id,
            ExecutionEventType::InstanceStarted,
            None,
            HashMap::new(),
        )
        .await?;

        Ok(instance_id)
    }

    async fn get_instance(&self, instance_id: &str) -> WorkflowResult<WorkflowInstance> {
        let instances = self.instances.read().await;
        instances
            .get(instance_id)
            .cloned()
            .ok_or_else(|| WorkflowError::InstanceNotFound {
                instance_id: instance_id.to_string(),
            })
    }

    async fn list_instances(
        &self,
        filter: Option<InstanceState>,
    ) -> WorkflowResult<Vec<WorkflowInstance>> {
        let instances = self.instances.read().await;
        let result: Vec<WorkflowInstance> = instances
            .values()
            .filter(|i| filter.is_none() || filter.as_ref() == Some(&i.state))
            .cloned()
            .collect();
        Ok(result)
    }

    async fn suspend_instance(&mut self, instance_id: &str) -> WorkflowResult<()> {
        let mut instances = self.instances.write().await;
        let instance =
            instances
                .get_mut(instance_id)
                .ok_or_else(|| WorkflowError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;

        if instance.state != InstanceState::Active {
            return Err(WorkflowError::InvalidStateTransition {
                from: format!("{:?}", instance.state),
                to: "Suspended".to_string(),
                reason: "Only active instances can be suspended".to_string(),
            });
        }

        instance.state = InstanceState::Suspended;
        Ok(())
    }

    async fn resume_instance(&mut self, instance_id: &str) -> WorkflowResult<()> {
        let mut instances = self.instances.write().await;
        let instance =
            instances
                .get_mut(instance_id)
                .ok_or_else(|| WorkflowError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;

        if instance.state != InstanceState::Suspended {
            return Err(WorkflowError::InvalidStateTransition {
                from: format!("{:?}", instance.state),
                to: "Active".to_string(),
                reason: "Only suspended instances can be resumed".to_string(),
            });
        }

        instance.state = InstanceState::Active;
        Ok(())
    }

    async fn cancel_instance(&mut self, instance_id: &str) -> WorkflowResult<()> {
        let mut instances = self.instances.write().await;
        let instance =
            instances
                .get_mut(instance_id)
                .ok_or_else(|| WorkflowError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;

        instance.state = InstanceState::Cancelled;
        instance.active_nodes.clear();
        #[cfg(feature = "timestamps")]
        {
            instance.completed_at = Some(chrono::Utc::now());
        }
        #[cfg(not(feature = "timestamps"))]
        {
            instance.completed_at = Some("now".to_string());
        }

        drop(instances); // Release write lock

        self.record_event(
            instance_id,
            ExecutionEventType::InstanceCancelled,
            None,
            HashMap::new(),
        )
        .await?;

        Ok(())
    }

    async fn terminate_instance(&mut self, instance_id: &str) -> WorkflowResult<()> {
        let mut instances = self.instances.write().await;
        let instance =
            instances
                .get_mut(instance_id)
                .ok_or_else(|| WorkflowError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;

        instance.state = InstanceState::Terminated;
        instance.active_nodes.clear();
        #[cfg(feature = "timestamps")]
        {
            instance.completed_at = Some(chrono::Utc::now());
        }
        #[cfg(not(feature = "timestamps"))]
        {
            instance.completed_at = Some("now".to_string());
        }

        drop(instances); // Release write lock

        self.record_event(
            instance_id,
            ExecutionEventType::InstanceTerminated,
            None,
            HashMap::new(),
        )
        .await?;

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Core Pattern Execution
    // -------------------------------------------------------------------------

    async fn execute_step(&mut self, instance_id: &str) -> WorkflowResult<Vec<NodeId>> {
        // Get enabled nodes
        let enabled_nodes = self.find_enabled_nodes_internal(instance_id).await?;

        if enabled_nodes.is_empty() {
            // Check if we've reached an end state
            let instances = self.instances.read().await;
            let instance =
                instances
                    .get(instance_id)
                    .ok_or_else(|| WorkflowError::InstanceNotFound {
                        instance_id: instance_id.to_string(),
                    })?;

            if instance.active_nodes.is_empty() {
                // No active nodes and no enabled nodes = completed
                drop(instances);
                let mut instances_mut = self.instances.write().await;
                if let Some(inst) = instances_mut.get_mut(instance_id) {
                    inst.state = InstanceState::Completed;
                    #[cfg(feature = "timestamps")]
                    {
                        inst.completed_at = Some(chrono::Utc::now());
                    }
                    #[cfg(not(feature = "timestamps"))]
                    {
                        inst.completed_at = Some("now".to_string());
                    }
                }
            }

            return Ok(Vec::new());
        }

        // Execute each enabled node
        // Note: In a full implementation, this would need more sophisticated
        // logic to handle different node types appropriately
        for node_id in &enabled_nodes {
            self.record_event(
                instance_id,
                ExecutionEventType::NodeActivated,
                Some(node_id.clone()),
                HashMap::new(),
            )
            .await?;
        }

        Ok(enabled_nodes)
    }

    async fn execute_gateway(
        &mut self,
        instance_id: &str,
        node_id: &NodeId,
    ) -> WorkflowResult<Vec<NodeId>> {
        let patterns = self.patterns.read().await;
        let instances = self.instances.read().await;

        let instance =
            instances
                .get(instance_id)
                .ok_or_else(|| WorkflowError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;

        let pattern =
            patterns
                .get(&instance.workflow_id)
                .ok_or_else(|| WorkflowError::WorkflowNotFound {
                    workflow_id: instance.workflow_id.0.clone(),
                })?;

        let node = pattern
            .nodes
            .get(node_id)
            .ok_or_else(|| WorkflowError::NodeNotFound {
                workflow_id: instance.workflow_id.0.clone(),
                node_id: node_id.0.clone(),
            })?;

        // Extract gateway pattern
        let gateway_pattern = match &node.kind {
            NodeKind::Gateway { pattern } => pattern,
            _ => {
                return Err(WorkflowError::GatewayEvaluationFailed {
                    node_id: node_id.0.clone(),
                    reason: "Node is not a gateway".to_string(),
                });
            }
        };

        // Find outgoing edges
        let outgoing: Vec<&Edge> = pattern
            .edges
            .iter()
            .filter(|e| &e.from == node_id)
            .collect();

        // Evaluate gateway pattern
        let activated_nodes = match gateway_pattern {
            GatewayPattern::ParallelSplit => {
                // Activate ALL outgoing paths
                outgoing.iter().map(|e| e.to.clone()).collect()
            }
            GatewayPattern::Synchronization => {
                // TODO: Wait for all incoming paths
                // For now, simple stub
                outgoing.iter().map(|e| e.to.clone()).collect()
            }
            GatewayPattern::ExclusiveChoice { conditions } => {
                // Evaluate conditions and select ONE path
                // TODO: Implement proper condition evaluation
                // For now, select first path
                if let Some(first) = outgoing.first() {
                    vec![first.to.clone()]
                } else {
                    Vec::new()
                }
            }
            GatewayPattern::SimpleMerge => {
                // Just pass through
                outgoing.iter().map(|e| e.to.clone()).collect()
            }
            _ => {
                // TODO: Implement other patterns
                return Err(WorkflowError::GatewayEvaluationFailed {
                    node_id: node_id.0.clone(),
                    reason: "Gateway pattern not yet implemented".to_string(),
                });
            }
        };

        drop(patterns);
        drop(instances);

        // Record event
        self.record_event(
            instance_id,
            ExecutionEventType::GatewayEvaluated,
            Some(node_id.clone()),
            HashMap::new(),
        )
        .await?;

        Ok(activated_nodes)
    }

    async fn execute_activity(
        &mut self,
        instance_id: &str,
        node_id: &NodeId,
    ) -> WorkflowResult<()> {
        // TODO: Implement actual activity execution
        // For now, just record the event
        self.record_event(
            instance_id,
            ExecutionEventType::NodeActivated,
            Some(node_id.clone()),
            HashMap::new(),
        )
        .await?;

        self.record_event(
            instance_id,
            ExecutionEventType::NodeCompleted,
            Some(node_id.clone()),
            HashMap::new(),
        )
        .await?;

        Ok(())
    }

    async fn handle_event(&mut self, instance_id: &str, node_id: &NodeId) -> WorkflowResult<()> {
        // TODO: Implement event handling logic
        self.record_event(
            instance_id,
            ExecutionEventType::EventTriggered,
            Some(node_id.clone()),
            HashMap::new(),
        )
        .await?;

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Advanced Patterns
    // -------------------------------------------------------------------------

    async fn execute_multi_instance(
        &mut self,
        instance_id: &str,
        node_id: &NodeId,
        config: &MultiInstanceConfig,
    ) -> WorkflowResult<()> {
        // TODO: Implement multi-instance pattern
        // This is a complex pattern requiring:
        // - Collection iteration
        // - Parallel/sequential execution management
        // - Completion condition evaluation
        Err(WorkflowError::ExecutionError {
            message: "Multi-instance pattern not yet implemented".to_string(),
        })
    }

    async fn execute_cancellation(
        &mut self,
        instance_id: &str,
        region: &CancellationRegion,
    ) -> WorkflowResult<()> {
        // TODO: Implement cancellation region logic
        Err(WorkflowError::ExecutionError {
            message: "Cancellation pattern not yet implemented".to_string(),
        })
    }

    async fn trigger_escalation(
        &mut self,
        instance_id: &str,
        config: &EscalationConfig,
    ) -> WorkflowResult<()> {
        // TODO: Implement escalation logic
        Err(WorkflowError::ExecutionError {
            message: "Escalation pattern not yet implemented".to_string(),
        })
    }

    // -------------------------------------------------------------------------
    // State Management
    // -------------------------------------------------------------------------

    async fn update_context(
        &mut self,
        instance_id: &str,
        updates: HashMap<String, serde_json::Value>,
    ) -> WorkflowResult<()> {
        let mut instances = self.instances.write().await;
        let instance =
            instances
                .get_mut(instance_id)
                .ok_or_else(|| WorkflowError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;

        for (key, value) in updates {
            instance.context.insert(key, value);
        }

        Ok(())
    }

    async fn get_context(
        &self,
        instance_id: &str,
    ) -> WorkflowResult<HashMap<String, serde_json::Value>> {
        let instances = self.instances.read().await;
        let instance =
            instances
                .get(instance_id)
                .ok_or_else(|| WorkflowError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;

        Ok(instance.context.clone())
    }

    async fn get_history(&self, instance_id: &str) -> WorkflowResult<Vec<ExecutionEvent>> {
        let instances = self.instances.read().await;
        let instance =
            instances
                .get(instance_id)
                .ok_or_else(|| WorkflowError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;

        Ok(instance.history.clone())
    }

    // -------------------------------------------------------------------------
    // Integration Points
    // -------------------------------------------------------------------------

    async fn delegate_to_cloud_workflows(
        &mut self,
        instance_id: &str,
        workflow_name: &str,
        arguments: HashMap<String, serde_json::Value>,
    ) -> WorkflowResult<serde_json::Value> {
        // TODO: Implement Cloud Workflows integration
        // This would use GCP client libraries to invoke workflows
        Err(WorkflowError::CloudWorkflowsError {
            message: "Cloud Workflows integration not yet implemented".to_string(),
        })
    }

    async fn receive_external_callback(
        &mut self,
        instance_id: &str,
        node_id: &NodeId,
        result: serde_json::Value,
    ) -> WorkflowResult<()> {
        // TODO: Implement callback handling
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Query and Analysis
    // -------------------------------------------------------------------------

    async fn get_active_nodes(&self, instance_id: &str) -> WorkflowResult<Vec<NodeId>> {
        let instances = self.instances.read().await;
        let instance =
            instances
                .get(instance_id)
                .ok_or_else(|| WorkflowError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;

        Ok(instance.active_nodes.iter().cloned().collect())
    }

    async fn check_deadlock(&self, instance_id: &str) -> WorkflowResult<bool> {
        // TODO: Implement deadlock detection
        // This requires analyzing the workflow graph and current state
        Ok(false)
    }

    async fn get_enabled_nodes(&self, instance_id: &str) -> WorkflowResult<Vec<NodeId>> {
        self.find_enabled_nodes_internal(instance_id).await
    }
}

// Add uuid to Cargo.toml dependencies
use uuid;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_get_pattern() {
        let mut kernel = InMemoryWorkflowKernel::new();
        let workflow_id = WorkflowId::new("test-wf-001");

        let pattern = WorkflowPattern {
            id: workflow_id.clone(),
            name: "Test Workflow".to_string(),
            description: Some("A test workflow".to_string()),
            nodes: HashMap::from([(
                NodeId::new("start"),
                Node {
                    id: NodeId::new("start"),
                    kind: NodeKind::Event {
                        event_type: EventType::Start,
                    },
                    config: HashMap::new(),
                },
            )]),
            edges: Vec::new(),
            start_nodes: vec![NodeId::new("start")],
            end_nodes: vec![NodeId::new("start")],
            variables: HashMap::new(),
        };

        kernel.register_pattern(pattern.clone()).await.unwrap();
        let retrieved = kernel.get_pattern(&workflow_id).await.unwrap();
        assert_eq!(retrieved.id, workflow_id);
    }

    #[tokio::test]
    async fn test_start_instance() {
        let mut kernel = InMemoryWorkflowKernel::new();
        let workflow_id = WorkflowId::new("test-wf-002");

        let pattern = WorkflowPattern {
            id: workflow_id.clone(),
            name: "Test Workflow".to_string(),
            description: None,
            nodes: HashMap::from([(
                NodeId::new("start"),
                Node {
                    id: NodeId::new("start"),
                    kind: NodeKind::Event {
                        event_type: EventType::Start,
                    },
                    config: HashMap::new(),
                },
            )]),
            edges: Vec::new(),
            start_nodes: vec![NodeId::new("start")],
            end_nodes: vec![NodeId::new("start")],
            variables: HashMap::new(),
        };

        kernel.register_pattern(pattern).await.unwrap();

        let instance_id = kernel
            .start_instance(&workflow_id, HashMap::new())
            .await
            .unwrap();

        let instance = kernel.get_instance(&instance_id).await.unwrap();
        assert_eq!(instance.state, InstanceState::Active);
        assert_eq!(instance.workflow_id, workflow_id);
    }

    #[tokio::test]
    async fn test_instance_lifecycle() {
        let mut kernel = InMemoryWorkflowKernel::new();
        let workflow_id = WorkflowId::new("test-wf-003");

        let pattern = WorkflowPattern {
            id: workflow_id.clone(),
            name: "Test Workflow".to_string(),
            description: None,
            nodes: HashMap::from([(
                NodeId::new("start"),
                Node {
                    id: NodeId::new("start"),
                    kind: NodeKind::Event {
                        event_type: EventType::Start,
                    },
                    config: HashMap::new(),
                },
            )]),
            edges: Vec::new(),
            start_nodes: vec![NodeId::new("start")],
            end_nodes: vec![NodeId::new("start")],
            variables: HashMap::new(),
        };

        kernel.register_pattern(pattern).await.unwrap();

        let instance_id = kernel
            .start_instance(&workflow_id, HashMap::new())
            .await
            .unwrap();

        // Suspend
        kernel.suspend_instance(&instance_id).await.unwrap();
        let instance = kernel.get_instance(&instance_id).await.unwrap();
        assert_eq!(instance.state, InstanceState::Suspended);

        // Resume
        kernel.resume_instance(&instance_id).await.unwrap();
        let instance = kernel.get_instance(&instance_id).await.unwrap();
        assert_eq!(instance.state, InstanceState::Active);

        // Cancel
        kernel.cancel_instance(&instance_id).await.unwrap();
        let instance = kernel.get_instance(&instance_id).await.unwrap();
        assert_eq!(instance.state, InstanceState::Cancelled);
    }
}
