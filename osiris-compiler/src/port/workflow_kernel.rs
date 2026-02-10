//! WorkflowKernel port trait.
//!
//! Defines the interface for workflow pattern execution and orchestration.
//! Implementations provide the execution engine for van der Aalst's workflow patterns.

use crate::domain::workflow::{
    CancellationRegion, Edge, EscalationConfig, ExecutionEvent, InstanceState, MultiInstanceConfig,
    MultiInstanceWithSyncConfig, MultiInstanceWithoutSyncConfig, Node, NodeId, WorkflowId,
    WorkflowInstance, WorkflowPattern,
};
use async_trait::async_trait;
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during workflow execution.
#[derive(Debug, Error)]
pub enum WorkflowError {
    /// Workflow pattern not found
    #[error("Workflow not found: {workflow_id}")]
    WorkflowNotFound { workflow_id: String },

    /// Workflow instance not found
    #[error("Instance not found: {instance_id}")]
    InstanceNotFound { instance_id: String },

    /// Node not found in workflow
    #[error("Node not found: {node_id} in workflow {workflow_id}")]
    NodeNotFound {
        workflow_id: String,
        node_id: String,
    },

    /// Invalid workflow definition
    #[error("Invalid workflow: {reason}")]
    InvalidWorkflow { reason: String },

    /// Invalid state transition
    #[error("Invalid state transition from {from} to {to}: {reason}")]
    InvalidStateTransition {
        from: String,
        to: String,
        reason: String,
    },

    /// Gateway evaluation failed
    #[error("Gateway evaluation failed at node {node_id}: {reason}")]
    GatewayEvaluationFailed { node_id: String, reason: String },

    /// Activity execution failed
    #[error("Activity execution failed at node {node_id}: {reason}")]
    ActivityFailed { node_id: String, reason: String },

    /// Deadlock detected
    #[error("Deadlock detected in instance {instance_id}: {reason}")]
    Deadlock { instance_id: String, reason: String },

    /// Cancellation failed
    #[error("Cancellation failed for region {region_id}: {reason}")]
    CancellationFailed { region_id: String, reason: String },

    /// Escalation handling failed
    #[error("Escalation failed with code {escalation_code}: {reason}")]
    EscalationFailed {
        escalation_code: String,
        reason: String,
    },

    /// Cloud Workflows integration error
    #[error("Cloud Workflows error: {message}")]
    CloudWorkflowsError { message: String },

    /// Generic execution error
    #[error("Execution error: {message}")]
    ExecutionError { message: String },
}

/// Result type for workflow operations.
pub type WorkflowResult<T> = Result<T, WorkflowError>;

/// Port trait for workflow pattern execution.
///
/// This trait defines the core operations for managing and executing
/// workflow patterns based on van der Aalst's 43 patterns.
#[async_trait]
pub trait WorkflowKernel: Send + Sync {
    // -------------------------------------------------------------------------
    // Pattern Management
    // -------------------------------------------------------------------------

    /// Registers a workflow pattern definition.
    async fn register_pattern(&mut self, pattern: WorkflowPattern) -> WorkflowResult<()>;

    /// Retrieves a workflow pattern by ID.
    async fn get_pattern(&self, workflow_id: &WorkflowId) -> WorkflowResult<WorkflowPattern>;

    /// Lists all registered workflow patterns.
    async fn list_patterns(&self) -> WorkflowResult<Vec<WorkflowId>>;

    /// Removes a workflow pattern (fails if active instances exist).
    async fn unregister_pattern(&mut self, workflow_id: &WorkflowId) -> WorkflowResult<()>;

    /// Validates a workflow pattern for structural correctness.
    async fn validate_pattern(&self, pattern: &WorkflowPattern) -> WorkflowResult<()>;

    // -------------------------------------------------------------------------
    // Instance Lifecycle
    // -------------------------------------------------------------------------

    /// Starts a new workflow instance.
    async fn start_instance(
        &mut self,
        workflow_id: &WorkflowId,
        initial_context: HashMap<String, serde_json::Value>,
    ) -> WorkflowResult<String>;

    /// Retrieves a workflow instance by ID.
    async fn get_instance(&self, instance_id: &str) -> WorkflowResult<WorkflowInstance>;

    /// Lists all workflow instances (optionally filtered by state).
    async fn list_instances(
        &self,
        filter: Option<InstanceState>,
    ) -> WorkflowResult<Vec<WorkflowInstance>>;

    /// Suspends a running workflow instance.
    async fn suspend_instance(&mut self, instance_id: &str) -> WorkflowResult<()>;

    /// Resumes a suspended workflow instance.
    async fn resume_instance(&mut self, instance_id: &str) -> WorkflowResult<()>;

    /// Cancels a workflow instance.
    async fn cancel_instance(&mut self, instance_id: &str) -> WorkflowResult<()>;

    /// Terminates a workflow instance immediately.
    async fn terminate_instance(&mut self, instance_id: &str) -> WorkflowResult<()>;

    // -------------------------------------------------------------------------
    // Core Pattern Execution (Basic Control Flow)
    // -------------------------------------------------------------------------

    /// Executes a single step in a workflow instance.
    ///
    /// Advances the workflow by executing the next enabled node(s).
    async fn execute_step(&mut self, instance_id: &str) -> WorkflowResult<Vec<NodeId>>;

    /// Evaluates and executes a gateway node.
    ///
    /// Returns the list of nodes to activate based on the gateway pattern.
    async fn execute_gateway(
        &mut self,
        instance_id: &str,
        node_id: &NodeId,
    ) -> WorkflowResult<Vec<NodeId>>;

    /// Executes an activity node.
    async fn execute_activity(&mut self, instance_id: &str, node_id: &NodeId)
        -> WorkflowResult<()>;

    /// Handles an event node.
    async fn handle_event(&mut self, instance_id: &str, node_id: &NodeId) -> WorkflowResult<()>;

    // -------------------------------------------------------------------------
    // Advanced Patterns
    // -------------------------------------------------------------------------

    /// Executes a multi-instance activity.
    ///
    /// Patterns 12-14: Multiple Instance patterns
    async fn execute_multi_instance(
        &mut self,
        instance_id: &str,
        node_id: &NodeId,
        config: &MultiInstanceConfig,
    ) -> WorkflowResult<()>;

    /// Cancels activities within a cancellation region.
    ///
    /// Pattern 19: Cancel Activity
    async fn execute_cancellation(
        &mut self,
        instance_id: &str,
        region: &CancellationRegion,
    ) -> WorkflowResult<()>;

    /// Triggers escalation handling.
    ///
    /// Pattern 20: Escalation
    async fn trigger_escalation(
        &mut self,
        instance_id: &str,
        config: &EscalationConfig,
    ) -> WorkflowResult<()>;

    /// Executes multiple instances without synchronization.
    ///
    /// Pattern 21: Multiple Instances without Synchronization
    async fn execute_multiple_instances_no_sync(
        &mut self,
        instance_id: &str,
        config: &MultiInstanceWithoutSyncConfig,
    ) -> WorkflowResult<()>;

    /// Executes multiple instances with design-time known cardinality.
    ///
    /// Pattern 22: Multiple Instances with a Priori Design-Time Knowledge
    async fn execute_multiple_instances_design_time(
        &mut self,
        instance_id: &str,
        cardinality: u32,
        activity_id: &NodeId,
    ) -> WorkflowResult<()>;

    /// Executes multiple instances with runtime-determined cardinality.
    ///
    /// Pattern 23: Multiple Instances with a Priori Runtime Knowledge
    async fn execute_multiple_instances_runtime(
        &mut self,
        instance_id: &str,
        cardinality_expression: &str,
        activity_id: &NodeId,
    ) -> WorkflowResult<()>;

    /// Executes multiple instances with synchronization.
    ///
    /// Pattern 24: Multiple Instances with Synchronization
    async fn execute_multiple_instances_with_sync(
        &mut self,
        instance_id: &str,
        config: &MultiInstanceWithSyncConfig,
    ) -> WorkflowResult<()>;

    /// Cancels multiple instances when condition is met.
    ///
    /// Pattern 25: Cancelling Multiple Instances
    async fn execute_cancel_multiple_instances(
        &mut self,
        instance_id: &str,
        cancel_condition: &str,
        target_activities: &[NodeId],
    ) -> WorkflowResult<()>;

    /// Executes a structured loop.
    ///
    /// Pattern 27: Structured Loop
    async fn execute_structured_loop(
        &mut self,
        instance_id: &str,
        loop_condition: &str,
        loop_back_node: &NodeId,
        max_iterations: Option<u32>,
    ) -> WorkflowResult<()>;

    /// Executes recursive workflow invocation.
    ///
    /// Pattern 28: Recursion
    async fn execute_recursion(
        &mut self,
        instance_id: &str,
        recursive_workflow_id: &WorkflowId,
        base_condition: &str,
        recursive_condition: &str,
        max_depth: Option<u32>,
    ) -> WorkflowResult<()>;

    /// Executes termination trigger.
    ///
    /// Pattern 29: Termination Trigger
    async fn execute_termination_trigger(
        &mut self,
        instance_id: &str,
        termination_condition: &str,
    ) -> WorkflowResult<()>;

    /// Executes transient trigger.
    ///
    /// Pattern 30: Transient Trigger
    async fn execute_transient_trigger(
        &mut self,
        instance_id: &str,
        trigger_condition: &str,
        triggered_activity: &NodeId,
        timeout_ms: Option<u64>,
    ) -> WorkflowResult<()>;

    // -------------------------------------------------------------------------
    // State Management
    // -------------------------------------------------------------------------

    /// Updates the execution context/variables for an instance.
    async fn update_context(
        &mut self,
        instance_id: &str,
        updates: HashMap<String, serde_json::Value>,
    ) -> WorkflowResult<()>;

    /// Gets the current execution context for an instance.
    async fn get_context(
        &self,
        instance_id: &str,
    ) -> WorkflowResult<HashMap<String, serde_json::Value>>;

    /// Gets the execution history for an instance.
    async fn get_history(&self, instance_id: &str) -> WorkflowResult<Vec<ExecutionEvent>>;

    // -------------------------------------------------------------------------
    // Integration Points
    // -------------------------------------------------------------------------

    /// Integrates with external Cloud Workflows service.
    ///
    /// Delegates execution to GCP Cloud Workflows or similar orchestration engines.
    async fn delegate_to_cloud_workflows(
        &mut self,
        instance_id: &str,
        workflow_name: &str,
        arguments: HashMap<String, serde_json::Value>,
    ) -> WorkflowResult<serde_json::Value>;

    /// Receives callback from external workflow execution.
    async fn receive_external_callback(
        &mut self,
        instance_id: &str,
        node_id: &NodeId,
        result: serde_json::Value,
    ) -> WorkflowResult<()>;

    // -------------------------------------------------------------------------
    // Query and Analysis
    // -------------------------------------------------------------------------

    /// Gets currently active nodes for an instance.
    async fn get_active_nodes(&self, instance_id: &str) -> WorkflowResult<Vec<NodeId>>;

    /// Checks if an instance has reached a deadlock state.
    async fn check_deadlock(&self, instance_id: &str) -> WorkflowResult<bool>;

    /// Gets enabled nodes (ready to execute) for an instance.
    async fn get_enabled_nodes(&self, instance_id: &str) -> WorkflowResult<Vec<NodeId>>;
}

/// Optional trait for workflow visualization and analysis.
#[async_trait]
pub trait WorkflowAnalyzer: Send + Sync {
    /// Generates a DOT graph representation of a workflow.
    async fn generate_dot_graph(&self, workflow_id: &WorkflowId) -> WorkflowResult<String>;

    /// Analyzes a workflow for structural soundness.
    async fn analyze_soundness(&self, workflow_id: &WorkflowId) -> WorkflowResult<SoundnessReport>;

    /// Detects potential deadlocks in a workflow pattern.
    async fn detect_potential_deadlocks(
        &self,
        workflow_id: &WorkflowId,
    ) -> WorkflowResult<Vec<DeadlockReport>>;
}

/// Report on workflow soundness.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundnessReport {
    /// Whether the workflow is sound
    pub is_sound: bool,
    /// List of issues found
    pub issues: Vec<SoundnessIssue>,
}

/// A soundness issue in a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundnessIssue {
    /// Issue severity
    pub severity: IssueSeverity,
    /// Node(s) involved
    pub nodes: Vec<NodeId>,
    /// Issue description
    pub description: String,
}

/// Severity of a soundness issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueSeverity {
    /// Critical issue preventing execution
    Error,
    /// Non-critical issue that may cause problems
    Warning,
    /// Informational note
    Info,
}

/// Report on potential deadlock.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlockReport {
    /// Nodes involved in the potential deadlock
    pub nodes: Vec<NodeId>,
    /// Description of the deadlock scenario
    pub scenario: String,
    /// Suggested fix
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
}

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_error_display() {
        let err = WorkflowError::WorkflowNotFound {
            workflow_id: "wf-001".to_string(),
        };
        assert!(err.to_string().contains("wf-001"));
    }

    #[test]
    fn test_soundness_report_serialization() {
        let report = SoundnessReport {
            is_sound: false,
            issues: vec![SoundnessIssue {
                severity: IssueSeverity::Error,
                nodes: vec![NodeId::new("n1")],
                description: "Missing outgoing edge".to_string(),
            }],
        };

        let json = serde_json::to_string(&report).unwrap();
        let deserialized: SoundnessReport = serde_json::from_str(&json).unwrap();

        assert!(!deserialized.is_sound);
        assert_eq!(deserialized.issues.len(), 1);
    }
}
