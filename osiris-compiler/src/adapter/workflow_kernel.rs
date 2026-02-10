//! WorkflowKernel adapter implementations.
//!
//! Provides concrete implementations of the WorkflowKernel port trait.

use crate::domain::workflow::{
    CancellationRegion, CancellationTrigger, Edge, EscalationConfig, ExecutionEvent,
    ExecutionEventType, GatewayPattern, InstanceState, MultiInstanceConfig, MultiInstanceMode,
    NodeId, NodeKind, WorkflowId, WorkflowInstance, WorkflowPattern,
};
use crate::port::workflow_kernel::{WorkflowError, WorkflowKernel, WorkflowResult};
use async_trait::async_trait;
use std::collections::HashMap;
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

    /// Helper: evaluates a condition expression against context
    fn evaluate_condition(
        &self,
        context: &HashMap<String, serde_json::Value>,
        expression: &str,
    ) -> bool {
        // Simple condition evaluator - checks if expression is in context and truthy
        // For production, implement proper expression evaluation (e.g., JsonLogic, CEL)
        if expression.starts_with('!') {
            // Negation
            let key = &expression[1..];
            if let Some(value) = context.get(key) {
                match value {
                    serde_json::Value::Bool(b) => !b,
                    serde_json::Value::Null => true,
                    _ => false,
                }
            } else {
                true // Missing = false, negated = true
            }
        } else if expression.contains('>') {
            // Greater than comparison (e.g., "amount > 1000")
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
            false
        } else if expression.contains('<') && !expression.contains("<=") {
            // Less than comparison
            let parts: Vec<&str> = expression.split('<').collect();
            if parts.len() == 2 {
                if let Some(left_val) = context.get(parts[0].trim()) {
                    if let Ok(right_num) = parts[1].trim().parse::<f64>() {
                        if let Some(left_num) = left_val.as_f64() {
                            return left_num < right_num;
                        }
                    }
                }
            }
            false
        } else if expression.contains("==") {
            // Equality comparison
            let parts: Vec<&str> = expression.split("==").collect();
            if parts.len() == 2 {
                if let Some(left_val) = context.get(parts[0].trim()) {
                    let right_str = parts[1].trim();
                    if right_str == "true" {
                        return left_val.is_boolean() && left_val.as_bool().unwrap_or(false);
                    } else if right_str == "false" {
                        return !left_val.as_bool().unwrap_or(true);
                    }
                }
            }
            false
        } else {
            // Direct boolean lookup
            if let Some(value) = context.get(expression) {
                match value {
                    serde_json::Value::Bool(b) => *b,
                    _ => false,
                }
            } else {
                false
            }
        }
    }

    /// Helper: determines if a loop should be executed
    fn should_loop(&self, context: &HashMap<String, serde_json::Value>) -> bool {
        // Check for a "continue_loop" or similar flag in context
        context
            .get("continue_loop")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Helper: checks if a critical section is free for execution
    /// Pattern 18: Critical Section mutual exclusion
    async fn is_critical_section_free(&self, section_id: &str) -> bool {
        let instances = self.instances.read().await;
        // Check if any active instance is in this critical section
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

    /// Helper: acquires a critical section lock
    /// Pattern 18: Critical Section
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

    /// Helper: releases a critical section lock
    /// Pattern 18: Critical Section
    async fn release_critical_section(&self, instance_id: &str) -> WorkflowResult<()> {
        let mut instances = self.instances.write().await;
        if let Some(instance) = instances.get_mut(instance_id) {
            instance.context.remove("critical_section");
            Ok(())
        } else {
            Err(WorkflowError::InstanceNotFound {
                instance_id: instance_id.to_string(),
            })
        }
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

        // Find incoming edges
        let incoming: Vec<&Edge> = pattern.edges.iter().filter(|e| &e.to == node_id).collect();

        // Evaluate gateway pattern
        let activated_nodes = match gateway_pattern {
            // Pattern 2: Parallel Split (AND-split)
            GatewayPattern::ParallelSplit => {
                // Activate ALL outgoing paths
                outgoing.iter().map(|e| e.to.clone()).collect()
            }

            // Pattern 3: Synchronization (AND-join)
            GatewayPattern::Synchronization => {
                // Check if all incoming paths are satisfied
                let all_active = incoming
                    .iter()
                    .all(|e| instance.active_nodes.contains(&e.from));
                if all_active && !incoming.is_empty() {
                    outgoing.iter().map(|e| e.to.clone()).collect()
                } else {
                    Vec::new() // Wait for all paths
                }
            }

            // Pattern 4: Exclusive Choice (XOR-split)
            GatewayPattern::ExclusiveChoice { conditions } => {
                // Evaluate conditions and select ONE path
                let mut selected = Vec::new();
                for condition in conditions {
                    if self.evaluate_condition(&instance.context, &condition.expression) {
                        selected.push(condition.target.clone());
                        break; // Only one path in exclusive choice
                    }
                }
                selected
            }

            // Pattern 5: Simple Merge (XOR-join)
            GatewayPattern::SimpleMerge => {
                // Only activate outgoing if exactly one incoming is active
                let active_incoming = incoming
                    .iter()
                    .filter(|e| instance.active_nodes.contains(&e.from))
                    .count();
                if active_incoming == 1 {
                    outgoing.iter().map(|e| e.to.clone()).collect()
                } else {
                    Vec::new()
                }
            }

            // Pattern 6: Multi-Choice (OR-split)
            GatewayPattern::MultiChoice { conditions } => {
                // Activate one or more paths based on conditions
                conditions
                    .iter()
                    .filter(|c| self.evaluate_condition(&instance.context, &c.expression))
                    .map(|c| c.target.clone())
                    .collect()
            }

            // Pattern 7: Structured Synchronizing Merge (OR-join)
            GatewayPattern::StructuredSynchronizingMerge => {
                // Wait for all ACTIVE incoming paths
                let all_active_arrived = incoming
                    .iter()
                    .all(|e| instance.active_nodes.contains(&e.from));
                if all_active_arrived && !incoming.is_empty() {
                    outgoing.iter().map(|e| e.to.clone()).collect()
                } else {
                    Vec::new()
                }
            }

            // Pattern 8: Multi-Merge
            GatewayPattern::MultiMerge => {
                // Activate for EACH incoming path independently (no join)
                // This creates multiple tokens, one per incoming edge
                outgoing.iter().map(|e| e.to.clone()).collect()
            }

            // Pattern 9: Structured Discriminator
            GatewayPattern::StructuredDiscriminator { reset_after } => {
                // Wait for first incoming, ignore rest
                let first_active = incoming
                    .iter()
                    .find(|e| instance.active_nodes.contains(&e.from));
                if first_active.is_some() {
                    outgoing.iter().map(|e| e.to.clone()).collect()
                } else {
                    Vec::new()
                }
            }

            // Pattern 10: Arbitrary Cycles
            GatewayPattern::ArbitraryCycle { back_edge_to } => {
                // Allow loop-back to specified node
                let mut nodes = outgoing.iter().map(|e| e.to.clone()).collect::<Vec<_>>();
                // Also add back-edge target if condition is met
                if self.should_loop(&instance.context) {
                    nodes.push(back_edge_to.clone());
                }
                nodes
            }

            // Pattern 11: Implicit Termination
            GatewayPattern::ImplicitTermination => {
                // Proceed when enabled, allows implicit termination
                outgoing.iter().map(|e| e.to.clone()).collect()
            }

            // Pattern 15: Deferred Choice
            GatewayPattern::DeferredChoice {
                event_conditions,
                timeout_ms,
            } => {
                // Dynamic choice based on which event occurs first
                let mut selected = Vec::new();
                for condition in event_conditions {
                    if self.evaluate_condition(&instance.context, &condition.expression) {
                        selected.push(condition.target.clone());
                        break; // Only first event wins
                    }
                }

                // If timeout is specified and no event fired, use default path
                if selected.is_empty() {
                    if let Some(first) = outgoing.first() {
                        selected.push(first.to.clone());
                    }
                }
                selected
            }

            // Pattern 16: Interleaved Parallel Routing
            GatewayPattern::InterleavedParallelRouting => {
                // Activate all paths but no mandatory join point
                outgoing.iter().map(|e| e.to.clone()).collect()
            }

            // Pattern 17: Milestone
            GatewayPattern::Milestone {
                condition,
                monitor_node,
            } => {
                // Only enable if condition is satisfied
                if self.evaluate_condition(&instance.context, condition) {
                    outgoing.iter().map(|e| e.to.clone()).collect()
                } else {
                    Vec::new() // Wait for condition
                }
            }

            // Pattern 18: Critical Section
            GatewayPattern::CriticalSection { section_id } => {
                // Check if critical section is free (no other instance executing)
                if self.is_critical_section_free(section_id).await {
                    outgoing.iter().map(|e| e.to.clone()).collect()
                } else {
                    Vec::new() // Wait for critical section to be free
                }
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
        // Patterns 12-14: Multiple Instance patterns
        // Handles:
        // - Pattern 12: MI without Synchronization
        // - Pattern 13: MI with a priori Design Time Knowledge
        // - Pattern 14: MI with a priori Runtime Knowledge

        let instances = self.instances.read().await;
        let instance =
            instances
                .get(instance_id)
                .ok_or_else(|| WorkflowError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;

        // Get the collection from context
        let collection_var = config.collection.clone();
        let collection =
            instance
                .context
                .get(&collection_var)
                .ok_or_else(|| WorkflowError::ExecutionError {
                    message: format!("Collection variable not found: {}", collection_var),
                })?;

        let items = match collection {
            serde_json::Value::Array(arr) => arr.clone(),
            _ => {
                return Err(WorkflowError::ExecutionError {
                    message: "Collection must be an array".to_string(),
                });
            }
        };

        drop(instances);

        // Record multi-instance start
        self.record_event(
            instance_id,
            ExecutionEventType::NodeActivated,
            Some(node_id.clone()),
            HashMap::from([(
                "multi_instance_count".to_string(),
                serde_json::Value::Number(items.len().into()),
            )]),
        )
        .await?;

        // Execute based on mode
        match config.mode {
            MultiInstanceMode::Sequential => {
                // Execute each instance sequentially
                for (index, item) in items.iter().enumerate() {
                    let mut ctx_updates = HashMap::new();
                    ctx_updates.insert(config.item_variable.clone(), item.clone());
                    ctx_updates.insert(
                        "mi_index".to_string(),
                        serde_json::Value::Number(index.into()),
                    );
                    self.update_context(instance_id, ctx_updates).await?;

                    // TODO: Execute activity for this item
                    self.record_event(
                        instance_id,
                        ExecutionEventType::NodeActivated,
                        Some(node_id.clone()),
                        HashMap::from([(
                            "mi_item_index".to_string(),
                            serde_json::Value::Number(index.into()),
                        )]),
                    )
                    .await?;
                }
            }
            MultiInstanceMode::Parallel
            | MultiInstanceMode::ParallelStatic
            | MultiInstanceMode::ParallelDynamic => {
                // Execute instances in parallel
                // In real implementation, would spawn tasks
                for (index, item) in items.iter().enumerate() {
                    let mut ctx_updates = HashMap::new();
                    ctx_updates.insert(config.item_variable.clone(), item.clone());
                    ctx_updates.insert(
                        "mi_index".to_string(),
                        serde_json::Value::Number(index.into()),
                    );
                    self.update_context(instance_id, ctx_updates).await?;

                    // TODO: Spawn parallel task
                    self.record_event(
                        instance_id,
                        ExecutionEventType::NodeActivated,
                        Some(node_id.clone()),
                        HashMap::from([(
                            "parallel_instance".to_string(),
                            serde_json::Value::Number(index.into()),
                        )]),
                    )
                    .await?;
                }
            }
        }

        // Evaluate completion condition
        if let Some(completion) = &config.completion_condition {
            if self.evaluate_condition(
                &self.get_context(instance_id).await.unwrap_or_default(),
                completion,
            ) {
                self.record_event(
                    instance_id,
                    ExecutionEventType::NodeCompleted,
                    Some(node_id.clone()),
                    HashMap::new(),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn execute_cancellation(
        &mut self,
        instance_id: &str,
        region: &CancellationRegion,
    ) -> WorkflowResult<()> {
        // Pattern 19: Cancel Activity
        // Cancels a region of the workflow based on trigger conditions

        let mut instances = self.instances.write().await;
        let instance =
            instances
                .get_mut(instance_id)
                .ok_or_else(|| WorkflowError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;

        // Evaluate cancellation trigger
        let should_cancel = match &region.trigger {
            CancellationTrigger::Event { event_code } => {
                // Check if event was triggered
                instance
                    .context
                    .get("triggered_event")
                    .and_then(|v| v.as_str())
                    .map(|e| e == event_code)
                    .unwrap_or(false)
            }
            CancellationTrigger::Timeout {
                #[cfg(feature = "timestamps")]
                    duration: _,
                #[cfg(not(feature = "timestamps"))]
                    duration_ms: _,
            } => {
                // In real implementation, check elapsed time
                instance
                    .context
                    .get("timeout_triggered")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            }
            CancellationTrigger::Condition { expression } => {
                // Evaluate condition
                self.evaluate_condition(&instance.context, expression)
            }
        };

        if should_cancel {
            // Cancel all nodes in the region
            for node_id in &region.nodes {
                instance.active_nodes.remove(node_id);
            }
            drop(instances);

            // Record cancellation event
            self.record_event(
                instance_id,
                ExecutionEventType::EventTriggered,
                Some(
                    region
                        .nodes
                        .first()
                        .cloned()
                        .unwrap_or(NodeId::new("unknown")),
                ),
                HashMap::from([(
                    "cancellation_region".to_string(),
                    serde_json::Value::String(region.region_id.clone()),
                )]),
            )
            .await?;
        }

        Ok(())
    }

    async fn trigger_escalation(
        &mut self,
        instance_id: &str,
        config: &EscalationConfig,
    ) -> WorkflowResult<()> {
        // Pattern 20: Escalation
        // Handles escalation events with optional interruption

        let mut instances = self.instances.write().await;
        let instance =
            instances
                .get_mut(instance_id)
                .ok_or_else(|| WorkflowError::InstanceNotFound {
                    instance_id: instance_id.to_string(),
                })?;

        // Record escalation trigger
        let mut event_data = HashMap::new();
        event_data.insert(
            "escalation_code".to_string(),
            serde_json::Value::String(config.escalation_code.clone()),
        );
        event_data.insert(
            "interrupting".to_string(),
            serde_json::Value::Bool(config.interrupting),
        );

        if config.interrupting {
            // Interrupting escalation: cancel current nodes
            instance.active_nodes.clear();
        }

        // Add handler node to active nodes
        instance.active_nodes.insert(config.handler_node.clone());

        drop(instances);

        // Record escalation event
        self.record_event(
            instance_id,
            ExecutionEventType::EventTriggered,
            Some(config.handler_node.clone()),
            event_data,
        )
        .await?;

        Ok(())
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
    use crate::domain::workflow::{ActivityImplementation, Condition, EventType, Node};

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

    #[tokio::test]
    async fn test_pattern_6_multi_choice() {
        let mut kernel = InMemoryWorkflowKernel::new();
        let workflow_id = WorkflowId::new("multi-choice-wf");

        // Create gateway with multi-choice pattern
        let pattern = WorkflowPattern {
            id: workflow_id.clone(),
            name: "Multi-Choice Pattern".to_string(),
            description: Some("Test Pattern 6: Multi-Choice".to_string()),
            nodes: HashMap::from([
                (
                    NodeId::new("gateway"),
                    Node {
                        id: NodeId::new("gateway"),
                        kind: NodeKind::Gateway {
                            pattern: GatewayPattern::MultiChoice {
                                conditions: vec![
                                    Condition {
                                        expression: "path_a".to_string(),
                                        target: NodeId::new("activity_a"),
                                        description: Some("Path A".to_string()),
                                    },
                                    Condition {
                                        expression: "path_b".to_string(),
                                        target: NodeId::new("activity_b"),
                                        description: Some("Path B".to_string()),
                                    },
                                ],
                            },
                        },
                        config: HashMap::new(),
                    },
                ),
                (
                    NodeId::new("activity_a"),
                    Node {
                        id: NodeId::new("activity_a"),
                        kind: NodeKind::Activity {
                            name: "Activity A".to_string(),
                            implementation: ActivityImplementation::Local {
                                handler: "handler_a".to_string(),
                            },
                        },
                        config: HashMap::new(),
                    },
                ),
                (
                    NodeId::new("activity_b"),
                    Node {
                        id: NodeId::new("activity_b"),
                        kind: NodeKind::Activity {
                            name: "Activity B".to_string(),
                            implementation: ActivityImplementation::Local {
                                handler: "handler_b".to_string(),
                            },
                        },
                        config: HashMap::new(),
                    },
                ),
            ]),
            edges: vec![
                Edge {
                    from: NodeId::new("gateway"),
                    to: NodeId::new("activity_a"),
                    condition: None,
                    label: None,
                },
                Edge {
                    from: NodeId::new("gateway"),
                    to: NodeId::new("activity_b"),
                    condition: None,
                    label: None,
                },
            ],
            start_nodes: vec![NodeId::new("gateway")],
            end_nodes: vec![NodeId::new("activity_a"), NodeId::new("activity_b")],
            variables: HashMap::new(),
        };

        kernel.register_pattern(pattern).await.unwrap();

        let mut context = HashMap::new();
        context.insert("path_a".to_string(), serde_json::Value::Bool(true));
        context.insert("path_b".to_string(), serde_json::Value::Bool(true));

        let instance_id = kernel.start_instance(&workflow_id, context).await.unwrap();

        let activated = kernel
            .execute_gateway(&instance_id, &NodeId::new("gateway"))
            .await
            .unwrap();

        // Both paths should be activated
        assert!(activated.contains(&NodeId::new("activity_a")));
        assert!(activated.contains(&NodeId::new("activity_b")));
    }

    #[tokio::test]
    async fn test_pattern_17_milestone() {
        let mut kernel = InMemoryWorkflowKernel::new();
        let workflow_id = WorkflowId::new("milestone-wf");

        let pattern = WorkflowPattern {
            id: workflow_id.clone(),
            name: "Milestone Pattern".to_string(),
            description: Some("Test Pattern 17: Milestone".to_string()),
            nodes: HashMap::from([
                (
                    NodeId::new("milestone"),
                    Node {
                        id: NodeId::new("milestone"),
                        kind: NodeKind::Gateway {
                            pattern: GatewayPattern::Milestone {
                                condition: "approval_given".to_string(),
                                monitor_node: Some(NodeId::new("approver")),
                            },
                        },
                        config: HashMap::new(),
                    },
                ),
                (
                    NodeId::new("next_activity"),
                    Node {
                        id: NodeId::new("next_activity"),
                        kind: NodeKind::Activity {
                            name: "Next Activity".to_string(),
                            implementation: ActivityImplementation::Local {
                                handler: "handler".to_string(),
                            },
                        },
                        config: HashMap::new(),
                    },
                ),
            ]),
            edges: vec![Edge {
                from: NodeId::new("milestone"),
                to: NodeId::new("next_activity"),
                condition: None,
                label: None,
            }],
            start_nodes: vec![NodeId::new("milestone")],
            end_nodes: vec![NodeId::new("next_activity")],
            variables: HashMap::new(),
        };

        kernel.register_pattern(pattern).await.unwrap();

        let mut context = HashMap::new();
        context.insert("approval_given".to_string(), serde_json::Value::Bool(false));

        let instance_id = kernel.start_instance(&workflow_id, context).await.unwrap();

        // Without approval, milestone should not activate
        let activated = kernel
            .execute_gateway(&instance_id, &NodeId::new("milestone"))
            .await
            .unwrap();
        assert!(activated.is_empty());

        // Update context with approval
        let mut updates = HashMap::new();
        updates.insert("approval_given".to_string(), serde_json::Value::Bool(true));
        kernel.update_context(&instance_id, updates).await.unwrap();

        // Now milestone should activate
        let activated = kernel
            .execute_gateway(&instance_id, &NodeId::new("milestone"))
            .await
            .unwrap();
        assert!(activated.contains(&NodeId::new("next_activity")));
    }

    #[tokio::test]
    async fn test_pattern_18_critical_section() {
        let mut kernel = InMemoryWorkflowKernel::new();
        let workflow_id = WorkflowId::new("critical-section-wf");

        let pattern = WorkflowPattern {
            id: workflow_id.clone(),
            name: "Critical Section Pattern".to_string(),
            description: Some("Test Pattern 18: Critical Section".to_string()),
            nodes: HashMap::from([
                (
                    NodeId::new("enter_critical"),
                    Node {
                        id: NodeId::new("enter_critical"),
                        kind: NodeKind::Gateway {
                            pattern: GatewayPattern::CriticalSection {
                                section_id: "section_1".to_string(),
                            },
                        },
                        config: HashMap::new(),
                    },
                ),
                (
                    NodeId::new("critical_activity"),
                    Node {
                        id: NodeId::new("critical_activity"),
                        kind: NodeKind::Activity {
                            name: "Critical Activity".to_string(),
                            implementation: ActivityImplementation::Local {
                                handler: "critical_handler".to_string(),
                            },
                        },
                        config: HashMap::new(),
                    },
                ),
            ]),
            edges: vec![Edge {
                from: NodeId::new("enter_critical"),
                to: NodeId::new("critical_activity"),
                condition: None,
                label: None,
            }],
            start_nodes: vec![NodeId::new("enter_critical")],
            end_nodes: vec![NodeId::new("critical_activity")],
            variables: HashMap::new(),
        };

        kernel.register_pattern(pattern).await.unwrap();

        let instance_id = kernel
            .start_instance(&workflow_id, HashMap::new())
            .await
            .unwrap();

        // Initially critical section should be free
        let is_free = kernel.is_critical_section_free("section_1").await;
        assert!(is_free);

        // Acquire section
        kernel
            .acquire_critical_section(&instance_id, "section_1")
            .await
            .unwrap();

        // Now section should not be free
        let is_free = kernel.is_critical_section_free("section_1").await;
        assert!(!is_free);

        // Release section
        kernel.release_critical_section(&instance_id).await.unwrap();

        // Now section should be free again
        let is_free = kernel.is_critical_section_free("section_1").await;
        assert!(is_free);
    }

    #[tokio::test]
    async fn test_pattern_19_cancel_activity() {
        let mut kernel = InMemoryWorkflowKernel::new();
        let workflow_id = WorkflowId::new("cancel-wf");

        let pattern = WorkflowPattern {
            id: workflow_id.clone(),
            name: "Cancel Activity Pattern".to_string(),
            description: Some("Test Pattern 19: Cancel Activity".to_string()),
            nodes: HashMap::from([
                (
                    NodeId::new("activity_1"),
                    Node {
                        id: NodeId::new("activity_1"),
                        kind: NodeKind::Activity {
                            name: "Activity 1".to_string(),
                            implementation: ActivityImplementation::Local {
                                handler: "handler_1".to_string(),
                            },
                        },
                        config: HashMap::new(),
                    },
                ),
                (
                    NodeId::new("activity_2"),
                    Node {
                        id: NodeId::new("activity_2"),
                        kind: NodeKind::Activity {
                            name: "Activity 2".to_string(),
                            implementation: ActivityImplementation::Local {
                                handler: "handler_2".to_string(),
                            },
                        },
                        config: HashMap::new(),
                    },
                ),
            ]),
            edges: Vec::new(),
            start_nodes: vec![NodeId::new("activity_1")],
            end_nodes: vec![NodeId::new("activity_2")],
            variables: HashMap::new(),
        };

        kernel.register_pattern(pattern).await.unwrap();

        let instance_id = kernel
            .start_instance(&workflow_id, HashMap::new())
            .await
            .unwrap();

        // Activate nodes
        {
            let mut instances = kernel.instances.write().await;
            if let Some(inst) = instances.get_mut(&instance_id) {
                inst.active_nodes.insert(NodeId::new("activity_1"));
                inst.active_nodes.insert(NodeId::new("activity_2"));
            }
        }

        // Execute cancellation
        let region = CancellationRegion {
            region_id: "region_1".to_string(),
            nodes: vec![NodeId::new("activity_1")],
            trigger: CancellationTrigger::Event {
                event_code: "cancel_event".to_string(),
            },
        };

        let mut context = HashMap::new();
        context.insert(
            "triggered_event".to_string(),
            serde_json::Value::String("cancel_event".to_string()),
        );
        kernel.update_context(&instance_id, context).await.unwrap();

        kernel
            .execute_cancellation(&instance_id, &region)
            .await
            .unwrap();

        // activity_1 should be cancelled
        let instance = kernel.get_instance(&instance_id).await.unwrap();
        assert!(!instance.active_nodes.contains(&NodeId::new("activity_1")));
        // activity_2 should still be active
        assert!(instance.active_nodes.contains(&NodeId::new("activity_2")));
    }

    #[tokio::test]
    async fn test_pattern_20_escalation() {
        let mut kernel = InMemoryWorkflowKernel::new();
        let workflow_id = WorkflowId::new("escalation-wf");

        let pattern = WorkflowPattern {
            id: workflow_id.clone(),
            name: "Escalation Pattern".to_string(),
            description: Some("Test Pattern 20: Escalation".to_string()),
            nodes: HashMap::from([
                (
                    NodeId::new("main_activity"),
                    Node {
                        id: NodeId::new("main_activity"),
                        kind: NodeKind::Activity {
                            name: "Main Activity".to_string(),
                            implementation: ActivityImplementation::Local {
                                handler: "main_handler".to_string(),
                            },
                        },
                        config: HashMap::new(),
                    },
                ),
                (
                    NodeId::new("escalation_handler"),
                    Node {
                        id: NodeId::new("escalation_handler"),
                        kind: NodeKind::Activity {
                            name: "Escalation Handler".to_string(),
                            implementation: ActivityImplementation::Local {
                                handler: "escalation_handler".to_string(),
                            },
                        },
                        config: HashMap::new(),
                    },
                ),
            ]),
            edges: Vec::new(),
            start_nodes: vec![NodeId::new("main_activity")],
            end_nodes: vec![NodeId::new("escalation_handler")],
            variables: HashMap::new(),
        };

        kernel.register_pattern(pattern).await.unwrap();

        let instance_id = kernel
            .start_instance(&workflow_id, HashMap::new())
            .await
            .unwrap();

        // Activate main activity
        {
            let mut instances = kernel.instances.write().await;
            if let Some(inst) = instances.get_mut(&instance_id) {
                inst.active_nodes.insert(NodeId::new("main_activity"));
            }
        }

        // Trigger escalation
        let config = EscalationConfig {
            escalation_code: "TIMEOUT".to_string(),
            handler_node: NodeId::new("escalation_handler"),
            interrupting: true,
        };

        kernel
            .trigger_escalation(&instance_id, &config)
            .await
            .unwrap();

        let instance = kernel.get_instance(&instance_id).await.unwrap();
        // Main activity should be interrupted
        assert!(
            !instance
                .active_nodes
                .contains(&NodeId::new("main_activity"))
        );
        // Escalation handler should be active
        assert!(
            instance
                .active_nodes
                .contains(&NodeId::new("escalation_handler"))
        );
    }

    #[tokio::test]
    async fn test_pattern_12_14_multi_instance() {
        let mut kernel = InMemoryWorkflowKernel::new();
        let workflow_id = WorkflowId::new("multi-instance-wf");

        let pattern = WorkflowPattern {
            id: workflow_id.clone(),
            name: "Multi-Instance Pattern".to_string(),
            description: Some("Test Patterns 12-14: Multiple Instance".to_string()),
            nodes: HashMap::from([(
                NodeId::new("mi_activity"),
                Node {
                    id: NodeId::new("mi_activity"),
                    kind: NodeKind::Activity {
                        name: "Multi-Instance Activity".to_string(),
                        implementation: ActivityImplementation::Local {
                            handler: "mi_handler".to_string(),
                        },
                    },
                    config: HashMap::new(),
                },
            )]),
            edges: Vec::new(),
            start_nodes: vec![NodeId::new("mi_activity")],
            end_nodes: vec![NodeId::new("mi_activity")],
            variables: HashMap::new(),
        };

        kernel.register_pattern(pattern).await.unwrap();

        let mut context = HashMap::new();
        context.insert(
            "items".to_string(),
            serde_json::json!(vec!["item1", "item2", "item3"]),
        );

        let instance_id = kernel.start_instance(&workflow_id, context).await.unwrap();

        let config = MultiInstanceConfig {
            mode: MultiInstanceMode::Sequential,
            collection: "items".to_string(),
            item_variable: "current_item".to_string(),
            completion_condition: None,
        };

        kernel
            .execute_multi_instance(&instance_id, &NodeId::new("mi_activity"), &config)
            .await
            .unwrap();

        let instance = kernel.get_instance(&instance_id).await.unwrap();
        assert!(instance.context.contains_key("mi_index"));
    }
}
