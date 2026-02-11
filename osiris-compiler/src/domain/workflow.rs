//! Workflow pattern domain types.
//!
//! Based on van der Aalst's 43 workflow patterns, this module defines
//! the core primitives for workflow orchestration and coordination.
//!
//! Reference: "Workflow Patterns: The Definitive Guide" by van der Aalst et al.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Unique identifier for workflow instances.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowId(pub String);

impl WorkflowId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Unique identifier for workflow nodes (activities, gateways, events).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Workflow pattern definition.
///
/// Represents a complete workflow as a directed graph of nodes and transitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPattern {
    /// Unique workflow identifier
    pub id: WorkflowId,
    /// Human-readable name
    pub name: String,
    /// Workflow description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Nodes in the workflow graph
    pub nodes: HashMap<NodeId, Node>,
    /// Edges connecting nodes
    pub edges: Vec<Edge>,
    /// Start node(s)
    pub start_nodes: Vec<NodeId>,
    /// End node(s)
    pub end_nodes: Vec<NodeId>,
    /// Workflow-level variables
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub variables: HashMap<String, serde_json::Value>,
}

/// A node in the workflow graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// Node identifier
    pub id: NodeId,
    /// Node type and behavior
    pub kind: NodeKind,
    /// Node-specific configuration
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub config: HashMap<String, serde_json::Value>,
}

/// Types of workflow nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NodeKind {
    /// Activity: unit of work
    #[serde(rename_all = "camelCase")]
    Activity {
        /// Activity name
        name: String,
        /// Activity implementation reference
        implementation: ActivityImplementation,
    },

    /// Gateway: control flow branching/merging
    #[serde(rename_all = "camelCase")]
    Gateway {
        /// Gateway pattern type
        pattern: GatewayPattern,
    },

    /// Event: external trigger or signal
    #[serde(rename_all = "camelCase")]
    Event {
        /// Event type
        event_type: EventType,
    },

    /// Subprocess: nested workflow
    #[serde(rename_all = "camelCase")]
    Subprocess {
        /// Nested workflow reference
        workflow_id: WorkflowId,
    },
}

/// Activity implementation reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ActivityImplementation {
    /// Local function/handler
    #[serde(rename_all = "camelCase")]
    Local { handler: String },
    /// HTTP endpoint
    #[serde(rename_all = "camelCase")]
    Http { url: String, method: String },
    /// Cloud Workflows integration
    #[serde(rename_all = "camelCase")]
    CloudWorkflow {
        project_id: String,
        workflow_name: String,
        region: String,
    },
    /// Custom implementation
    #[serde(rename_all = "camelCase")]
    Custom {
        implementation_type: String,
        config: HashMap<String, serde_json::Value>,
    },
}

/// Gateway patterns for control flow.
///
/// Based on van der Aalst's control flow patterns (1-20).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "patternType", rename_all = "camelCase")]
pub enum GatewayPattern {
    /// Pattern 2: Parallel Split (AND-split)
    /// Creates multiple concurrent execution paths
    #[serde(rename_all = "camelCase")]
    ParallelSplit,

    /// Pattern 3: Synchronization (AND-join)
    /// Waits for all incoming paths to complete
    #[serde(rename_all = "camelCase")]
    Synchronization,

    /// Pattern 4: Exclusive Choice (XOR-split)
    /// Selects exactly one outgoing path based on condition
    #[serde(rename_all = "camelCase")]
    ExclusiveChoice { conditions: Vec<Condition> },

    /// Pattern 5: Simple Merge (XOR-join)
    /// Waits for any one incoming path
    #[serde(rename_all = "camelCase")]
    SimpleMerge,

    /// Pattern 6: Multi-Choice (OR-split)
    /// Selects one or more outgoing paths
    #[serde(rename_all = "camelCase")]
    MultiChoice { conditions: Vec<Condition> },

    /// Pattern 7: Structured Synchronizing Merge (OR-join)
    /// Waits for all active incoming paths
    #[serde(rename_all = "camelCase")]
    StructuredSynchronizingMerge,

    /// Pattern 8: Multi-Merge
    /// Activates for each incoming path independently
    #[serde(rename_all = "camelCase")]
    MultiMerge,

    /// Pattern 9: Structured Discriminator
    /// Waits for first incoming path, ignores rest
    #[serde(rename_all = "camelCase")]
    StructuredDiscriminator { reset_after: Option<NodeId> },

    /// Pattern 10: Arbitrary Cycles
    /// Allows loops and arbitrary cycle structures
    #[serde(rename_all = "camelCase")]
    ArbitraryCycle { back_edge_to: NodeId },

    /// Pattern 11: Implicit Termination
    /// Terminates when no more enabled nodes exist
    #[serde(rename_all = "camelCase")]
    ImplicitTermination,

    /// Pattern 15: Deferred Choice (OR-join with external choice)
    /// Dynamic choice determined by which event occurs first
    #[serde(rename_all = "camelCase")]
    DeferredChoice {
        /// Event-based conditions for deferred choice
        event_conditions: Vec<Condition>,
        /// Timeout if no event occurs
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },

    /// Pattern 16: Interleaved Parallel Routing (Parallel without full join)
    /// Parallel paths with no mandatory synchronization
    #[serde(rename_all = "camelCase")]
    InterleavedParallelRouting,

    /// Pattern 17: Milestone
    /// Activity enabled only when a condition becomes true
    #[serde(rename_all = "camelCase")]
    Milestone {
        /// Condition that must be satisfied for enabling
        condition: String,
        /// Optional node to monitor for the condition
        #[serde(skip_serializing_if = "Option::is_none")]
        monitor_node: Option<NodeId>,
    },

    /// Pattern 18: Critical Section
    /// Only one instance/path can execute at a time (mutex-like)
    #[serde(rename_all = "camelCase")]
    CriticalSection {
        /// Unique identifier for the critical section
        section_id: String,
    },

    /// Pattern 21: Multiple Instances without Synchronization
    /// Spawns multiple instances that complete independently
    #[serde(rename_all = "camelCase")]
    MultipleInstancesNoSync {
        /// Configuration for multiple instances
        config: Box<MultiInstanceWithoutSyncConfig>,
    },

    /// Pattern 22: Multiple Instances with a Priori Design-Time Knowledge
    /// Cardinality known at design time (static)
    #[serde(rename_all = "camelCase")]
    MultipleInstancesDesignTime {
        /// Number of instances to create
        cardinality: u32,
        /// Activity to execute for each instance
        activity_id: NodeId,
    },

    /// Pattern 23: Multiple Instances with a Priori Runtime Knowledge
    /// Cardinality determined at runtime from context
    #[serde(rename_all = "camelCase")]
    MultipleInstancesRuntime {
        /// Expression to evaluate for cardinality
        cardinality_expression: String,
        /// Activity to execute for each instance
        activity_id: NodeId,
    },

    /// Pattern 24: Multiple Instances with Synchronization
    /// Spawns multiple instances and waits for all to complete
    #[serde(rename_all = "camelCase")]
    MultipleInstancesWithSync {
        /// Configuration for synchronized multiple instances
        config: Box<MultiInstanceWithSyncConfig>,
    },

    /// Pattern 25: Cancelling Multiple Instances
    /// Cancels all active instances when condition is met
    #[serde(rename_all = "camelCase")]
    CancelMultipleInstances {
        /// Condition triggering cancellation
        cancel_condition: String,
        /// Activities to cancel
        target_activities: Vec<NodeId>,
    },

    /// Pattern 26: Dynamic Parallel Split
    /// Routes to multiple nodes determined dynamically at runtime
    #[serde(rename_all = "camelCase")]
    DynamicParallelSplit {
        /// Expression to determine target nodes
        routing_expression: String,
    },

    /// Pattern 27: Structured Loop
    /// Enables repeated execution with explicit loop control
    #[serde(rename_all = "camelCase")]
    StructuredLoop {
        /// Loop condition (expression to evaluate)
        loop_condition: String,
        /// Node to loop back to
        loop_back_node: NodeId,
        /// Maximum iterations (optional safeguard)
        #[serde(skip_serializing_if = "Option::is_none")]
        max_iterations: Option<u32>,
    },

    /// Pattern 28: Recursion
    /// Allows recursive invocation of workflow subprocess
    #[serde(rename_all = "camelCase")]
    Recursion {
        /// Workflow to recursively invoke
        recursive_workflow_id: WorkflowId,
        /// Base condition (recursion termination)
        base_condition: String,
        /// Recursive condition
        recursive_condition: String,
        /// Maximum recursion depth (safeguard)
        #[serde(skip_serializing_if = "Option::is_none")]
        max_depth: Option<u32>,
    },

    /// Pattern 29: Termination Trigger
    /// Immediately terminates the entire workflow
    #[serde(rename_all = "camelCase")]
    TerminationTrigger {
        /// Condition that triggers termination
        termination_condition: String,
    },

    /// Pattern 30: Transient Trigger
    /// Triggers an activity based on a temporary condition
    #[serde(rename_all = "camelCase")]
    TransientTrigger {
        /// Transient condition that enables the activity
        trigger_condition: String,
        /// Activity to trigger
        triggered_activity: NodeId,
        /// Optional timeout for the trigger
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
}

/// Condition for gateway evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    /// Condition expression
    pub expression: String,
    /// Target node if condition is true
    pub target: NodeId,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "eventCategory", rename_all = "camelCase")]
pub enum EventType {
    /// Start event
    #[serde(rename_all = "camelCase")]
    Start,
    /// End event
    #[serde(rename_all = "camelCase")]
    End,
    /// Timer event
    #[serde(rename_all = "camelCase")]
    Timer {
        #[cfg(feature = "timestamps")]
        duration: Option<chrono::Duration>,
        #[cfg(not(feature = "timestamps"))]
        duration_ms: Option<u64>,
    },
    /// Message event
    #[serde(rename_all = "camelCase")]
    Message { message_type: String },
    /// Error event
    #[serde(rename_all = "camelCase")]
    Error { error_code: String },
    /// Escalation event
    #[serde(rename_all = "camelCase")]
    Escalation { escalation_code: String },
    /// Cancellation event
    #[serde(rename_all = "camelCase")]
    Cancel { target_scope: Option<NodeId> },
    /// Termination event
    #[serde(rename_all = "camelCase")]
    Terminate,
}

/// Edge connecting two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    /// Source node
    pub from: NodeId,
    /// Target node
    pub to: NodeId,
    /// Optional condition for conditional edges
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// Optional label
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Workflow instance execution state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowInstance {
    /// Instance identifier
    pub instance_id: String,
    /// Reference to workflow pattern
    pub workflow_id: WorkflowId,
    /// Current execution state
    pub state: InstanceState,
    /// Currently active nodes
    pub active_nodes: HashSet<NodeId>,
    /// Execution context/variables
    pub context: HashMap<String, serde_json::Value>,
    /// Execution history
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub history: Vec<ExecutionEvent>,
    /// Start timestamp
    #[cfg(feature = "timestamps")]
    pub started_at: chrono::DateTime<chrono::Utc>,
    #[cfg(not(feature = "timestamps"))]
    pub started_at: String,
    /// End timestamp (if completed)
    #[cfg(feature = "timestamps")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[cfg(not(feature = "timestamps"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

/// Instance execution state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstanceState {
    /// Instance is running
    Active,
    /// Instance completed successfully
    Completed,
    /// Instance failed with error
    Failed,
    /// Instance was cancelled
    Cancelled,
    /// Instance was terminated
    Terminated,
    /// Instance is suspended/paused
    Suspended,
}

/// Execution event in workflow history.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionEvent {
    /// Event type
    pub event_type: ExecutionEventType,
    /// Node involved (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<NodeId>,
    /// Timestamp
    #[cfg(feature = "timestamps")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    #[cfg(not(feature = "timestamps"))]
    pub timestamp: String,
    /// Additional event data
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub data: HashMap<String, serde_json::Value>,
}

/// Types of execution events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionEventType {
    /// Instance started
    InstanceStarted,
    /// Node activated
    NodeActivated,
    /// Node completed
    NodeCompleted,
    /// Node failed
    NodeFailed,
    /// Gateway evaluated
    GatewayEvaluated,
    /// Event triggered
    EventTriggered,
    /// Instance completed
    InstanceCompleted,
    /// Instance failed
    InstanceFailed,
    /// Instance cancelled
    InstanceCancelled,
    /// Instance terminated
    InstanceTerminated,
    /// Variable updated
    VariableUpdated,
    /// External delegation (e.g., Cloud Workflows)
    ExternalDelegation,
}

/// Multi-instance pattern for repeated execution.
///
/// Pattern 12-14: Multiple instance patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiInstanceConfig {
    /// Type of multi-instance execution
    pub mode: MultiInstanceMode,
    /// Collection to iterate over
    pub collection: String,
    /// Variable name for current item
    pub item_variable: String,
    /// Completion condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_condition: Option<String>,
}

/// Multi-instance execution mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MultiInstanceMode {
    /// Execute instances sequentially
    Sequential,
    /// Execute instances in parallel
    Parallel,
    /// Parallel with static cardinality known at design time
    ParallelStatic,
    /// Parallel with dynamic cardinality known at runtime
    ParallelDynamic,
}

/// Cancellation region for Pattern 19: Cancel Activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancellationRegion {
    /// Region identifier
    pub region_id: String,
    /// Nodes in the cancellation scope
    pub nodes: Vec<NodeId>,
    /// Cancellation trigger condition
    pub trigger: CancellationTrigger,
}

/// Trigger for cancellation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "triggerType", rename_all = "camelCase")]
pub enum CancellationTrigger {
    /// Cancel on explicit event
    #[serde(rename_all = "camelCase")]
    Event { event_code: String },
    /// Cancel on timeout
    #[serde(rename_all = "camelCase")]
    Timeout {
        #[cfg(feature = "timestamps")]
        duration: chrono::Duration,
        #[cfg(not(feature = "timestamps"))]
        duration_ms: u64,
    },
    /// Cancel on condition
    #[serde(rename_all = "camelCase")]
    Condition { expression: String },
}

/// Escalation configuration for Pattern 20.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EscalationConfig {
    /// Escalation code
    pub escalation_code: String,
    /// Target handler node
    pub handler_node: NodeId,
    /// Whether to interrupt the current scope
    pub interrupting: bool,
}

/// Cloud Workflows integration configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudWorkflowsConfig {
    /// GCP project ID
    pub project_id: String,
    /// Workflow location/region
    pub location: String,
    /// Authentication configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<CloudAuthConfig>,
}

/// Cloud authentication configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudAuthConfig {
    /// Service account email
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_account: Option<String>,
    /// Credentials JSON path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials_path: Option<String>,
}

/// Pattern 18: Critical Section configuration
/// Manages mutual exclusion in workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CriticalSectionConfig {
    /// Section identifier for grouping exclusive activities
    pub section_id: String,
    /// Activities in the critical section
    pub activities: Vec<NodeId>,
    /// Maximum concurrent instances (typically 1 for mutual exclusion)
    #[serde(default = "critical_section_default_max")]
    pub max_concurrent: u32,
}

fn critical_section_default_max() -> u32 {
    1
}

/// Pattern 17: Milestone configuration
/// Tracks and enables activities based on milestone conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MilestoneConfig {
    /// Milestone identifier
    pub milestone_id: String,
    /// Condition that enables the milestone
    pub condition: String,
    /// Activities that require this milestone to be active
    pub dependent_activities: Vec<NodeId>,
    /// Optional monitoring node that provides the condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitor_node: Option<NodeId>,
}

/// Pattern 16: Interleaved execution context
/// Tracks concurrent execution without full synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterleavedExecutionContext {
    /// Paths that are concurrently executing
    pub active_paths: Vec<Vec<NodeId>>,
    /// No mandatory join point - paths complete independently
    pub independent_completion: bool,
}

/// Pattern 21: Multiple Instances without Synchronization configuration
/// Each instance executes independently without waiting for others
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiInstanceWithoutSyncConfig {
    /// Collection variable containing items to process
    pub collection: String,
    /// Variable name for each item in iteration
    pub item_variable: String,
    /// Activity to execute for each instance
    pub activity_id: NodeId,
    /// Whether to spawn instances asynchronously (fire-and-forget)
    #[serde(default)]
    pub asynchronous: bool,
}

/// Pattern 24: Multiple Instances with Synchronization configuration
/// Spawns multiple instances and waits for all to complete
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiInstanceWithSyncConfig {
    /// Collection variable containing items to process
    pub collection: String,
    /// Variable name for each item in iteration
    pub item_variable: String,
    /// Activity to execute for each instance
    pub activity_id: NodeId,
    /// Completion condition (all instances must satisfy this)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_condition: Option<String>,
    /// Merge strategy: "all_complete", "one_complete", "threshold"
    #[serde(default)]
    pub merge_strategy: String,
    /// Threshold percentage for threshold-based completion (0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_threshold: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_id() {
        let id = WorkflowId::new("wf-001");
        assert_eq!(id.0, "wf-001");
    }

    #[test]
    fn test_gateway_pattern_serialization() {
        let pattern = GatewayPattern::ParallelSplit;
        let json = serde_json::to_string(&pattern).unwrap();
        assert!(json.contains("parallelSplit"));
    }

    #[test]
    fn test_exclusive_choice_gateway() {
        let gateway = GatewayPattern::ExclusiveChoice {
            conditions: vec![
                Condition {
                    expression: "amount > 1000".to_string(),
                    target: NodeId::new("approve-manager"),
                    description: Some("Manager approval required".to_string()),
                },
                Condition {
                    expression: "amount <= 1000".to_string(),
                    target: NodeId::new("auto-approve"),
                    description: Some("Auto-approve small amounts".to_string()),
                },
            ],
        };

        let json = serde_json::to_string(&gateway).unwrap();
        let deserialized: GatewayPattern = serde_json::from_str(&json).unwrap();

        match deserialized {
            GatewayPattern::ExclusiveChoice { conditions } => {
                assert_eq!(conditions.len(), 2);
            }
            _ => panic!("Wrong gateway type"),
        }
    }

    #[test]
    fn test_instance_state() {
        let state = InstanceState::Active;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"active\"");
    }

    #[test]
    fn test_multi_instance_config() {
        let config = MultiInstanceConfig {
            mode: MultiInstanceMode::Parallel,
            collection: "items".to_string(),
            item_variable: "item".to_string(),
            completion_condition: Some("all_completed".to_string()),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: MultiInstanceConfig = serde_json::from_str(&json).unwrap();

        matches!(deserialized.mode, MultiInstanceMode::Parallel);
    }
}
