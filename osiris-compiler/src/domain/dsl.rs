//! Workflow DSL domain types.
//!
//! This module defines the external DSL representation that gets compiled
//! into the internal 43-pattern workflow primitives.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root DSL workflow definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DslWorkflow {
    /// Workflow identifier
    pub id: String,
    /// Workflow name
    pub name: String,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Workflow version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Process elements
    pub elements: Vec<DslElement>,
    /// Sequence flows connecting elements
    pub flows: Vec<DslFlow>,
    /// Global variables
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub variables: HashMap<String, DslVariable>,
}

/// A single element in the workflow DSL.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "elementType", rename_all = "camelCase")]
pub enum DslElement {
    /// Start event
    #[serde(rename_all = "camelCase")]
    StartEvent {
        id: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        trigger: Option<DslEventTrigger>,
    },

    /// End event
    #[serde(rename_all = "camelCase")]
    EndEvent {
        id: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<DslEndResult>,
    },

    /// Task/Activity
    #[serde(rename_all = "camelCase")]
    Task {
        id: String,
        name: String,
        task_type: DslTaskType,
        #[serde(skip_serializing_if = "HashMap::is_empty", default)]
        properties: HashMap<String, serde_json::Value>,
    },

    /// Gateway for control flow
    #[serde(rename_all = "camelCase")]
    Gateway {
        id: String,
        name: String,
        gateway_type: DslGatewayType,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        conditions: Vec<DslCondition>,
    },

    /// Subprocess
    #[serde(rename_all = "camelCase")]
    Subprocess {
        id: String,
        name: String,
        workflow_ref: String,
        #[serde(skip_serializing_if = "HashMap::is_empty", default)]
        input_mappings: HashMap<String, String>,
        #[serde(skip_serializing_if = "HashMap::is_empty", default)]
        output_mappings: HashMap<String, String>,
    },

    /// Intermediate event
    #[serde(rename_all = "camelCase")]
    IntermediateEvent {
        id: String,
        name: String,
        event_type: DslIntermediateEventType,
    },

    /// Boundary event (attached to task)
    #[serde(rename_all = "camelCase")]
    BoundaryEvent {
        id: String,
        name: String,
        attached_to: String,
        interrupting: bool,
        event_type: DslBoundaryEventType,
    },
}

/// DSL task types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DslTaskType {
    /// Manual user task
    #[serde(rename_all = "camelCase")]
    UserTask { assignee: Option<String> },

    /// Service/automated task
    #[serde(rename_all = "camelCase")]
    ServiceTask {
        implementation: DslServiceImplementation,
    },

    /// Script task
    #[serde(rename_all = "camelCase")]
    ScriptTask { language: String, script: String },

    /// Send task
    #[serde(rename_all = "camelCase")]
    SendTask { message_ref: String },

    /// Receive task
    #[serde(rename_all = "camelCase")]
    ReceiveTask { message_ref: String },

    /// Generic task
    #[serde(rename_all = "camelCase")]
    Task,
}

/// Service task implementation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "implementationType", rename_all = "camelCase")]
pub enum DslServiceImplementation {
    /// HTTP endpoint
    #[serde(rename_all = "camelCase")]
    Http {
        url: String,
        method: String,
        #[serde(skip_serializing_if = "HashMap::is_empty", default)]
        headers: HashMap<String, String>,
    },

    /// Cloud function
    #[serde(rename_all = "camelCase")]
    CloudFunction {
        project: String,
        region: String,
        function_name: String,
    },

    /// Cloud Workflow
    #[serde(rename_all = "camelCase")]
    CloudWorkflow {
        project: String,
        region: String,
        workflow_name: String,
    },

    /// Local handler
    #[serde(rename_all = "camelCase")]
    Local { handler: String },
}

/// DSL gateway types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DslGatewayType {
    /// Exclusive gateway (XOR)
    ExclusiveGateway,
    /// Parallel gateway (AND)
    ParallelGateway,
    /// Inclusive gateway (OR)
    InclusiveGateway,
    /// Event-based gateway
    EventBasedGateway,
    /// Complex gateway
    ComplexGateway,
}

/// DSL condition for gateway routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DslCondition {
    /// Condition expression
    pub expression: String,
    /// Target flow ID
    pub target_ref: String,
    /// Condition name/description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// DSL sequence flow connecting elements.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DslFlow {
    /// Flow identifier
    pub id: String,
    /// Source element ID
    pub source_ref: String,
    /// Target element ID
    pub target_ref: String,
    /// Optional flow name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Optional condition expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

/// DSL event trigger types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "triggerType", rename_all = "camelCase")]
pub enum DslEventTrigger {
    /// No trigger (default start)
    #[serde(rename_all = "camelCase")]
    None,
    /// Timer trigger
    #[serde(rename_all = "camelCase")]
    Timer { duration_ms: u64 },
    /// Message trigger
    #[serde(rename_all = "camelCase")]
    Message { message_ref: String },
    /// Signal trigger
    #[serde(rename_all = "camelCase")]
    Signal { signal_ref: String },
    /// Conditional trigger
    #[serde(rename_all = "camelCase")]
    Conditional { condition: String },
}

/// DSL end event result types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "resultType", rename_all = "camelCase")]
pub enum DslEndResult {
    /// Normal completion
    #[serde(rename_all = "camelCase")]
    None,
    /// Throw error
    #[serde(rename_all = "camelCase")]
    Error { error_code: String },
    /// Send message
    #[serde(rename_all = "camelCase")]
    Message { message_ref: String },
    /// Signal
    #[serde(rename_all = "camelCase")]
    Signal { signal_ref: String },
    /// Terminate all instances
    #[serde(rename_all = "camelCase")]
    Terminate,
    /// Cancel
    #[serde(rename_all = "camelCase")]
    Cancel,
}

/// DSL intermediate event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "eventKind", rename_all = "camelCase")]
pub enum DslIntermediateEventType {
    /// Catch message
    #[serde(rename_all = "camelCase")]
    MessageCatch { message_ref: String },
    /// Throw message
    #[serde(rename_all = "camelCase")]
    MessageThrow { message_ref: String },
    /// Catch timer
    #[serde(rename_all = "camelCase")]
    Timer { duration_ms: u64 },
    /// Catch signal
    #[serde(rename_all = "camelCase")]
    SignalCatch { signal_ref: String },
    /// Throw signal
    #[serde(rename_all = "camelCase")]
    SignalThrow { signal_ref: String },
    /// Escalation
    #[serde(rename_all = "camelCase")]
    Escalation { escalation_code: String },
    /// Link (goto)
    #[serde(rename_all = "camelCase")]
    Link { target_ref: String },
}

/// DSL boundary event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "boundaryKind", rename_all = "camelCase")]
pub enum DslBoundaryEventType {
    /// Timer boundary
    #[serde(rename_all = "camelCase")]
    Timer { duration_ms: u64 },
    /// Error boundary
    #[serde(rename_all = "camelCase")]
    Error { error_code: String },
    /// Message boundary
    #[serde(rename_all = "camelCase")]
    Message { message_ref: String },
    /// Escalation boundary
    #[serde(rename_all = "camelCase")]
    Escalation { escalation_code: String },
    /// Cancellation boundary
    #[serde(rename_all = "camelCase")]
    Cancel,
    /// Compensation boundary
    #[serde(rename_all = "camelCase")]
    Compensation,
}

/// DSL variable definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DslVariable {
    /// Variable name
    pub name: String,
    /// Variable type hint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var_type: Option<String>,
    /// Default value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
}

/// DSL loop configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DslLoopConfig {
    /// Loop type
    pub loop_type: DslLoopType,
    /// Collection to iterate over (for multi-instance)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
    /// Item variable name (for multi-instance)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_variable: Option<String>,
    /// Completion condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_condition: Option<String>,
}

/// DSL loop types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DslLoopType {
    /// Standard loop (while/until)
    StandardLoop {
        condition: String,
        test_before: bool,
    },
    /// Sequential multi-instance
    SequentialMultiInstance,
    /// Parallel multi-instance
    ParallelMultiInstance,
}

/// DSL compensation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DslCompensation {
    /// Compensation handler task ID
    pub handler_ref: String,
    /// Whether compensation is triggered
    pub triggered: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dsl_workflow_serialization() {
        let workflow = DslWorkflow {
            id: "wf-001".to_string(),
            name: "Test Workflow".to_string(),
            description: Some("A test workflow".to_string()),
            version: Some("1.0".to_string()),
            elements: vec![],
            flows: vec![],
            variables: HashMap::new(),
        };

        let json = serde_json::to_string(&workflow).unwrap();
        let deserialized: DslWorkflow = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "wf-001");
    }

    #[test]
    fn test_start_event_serialization() {
        let element = DslElement::StartEvent {
            id: "start1".to_string(),
            name: "Start".to_string(),
            trigger: Some(DslEventTrigger::Timer { duration_ms: 5000 }),
        };

        let json = serde_json::to_string(&element).unwrap();
        assert!(json.contains("startEvent"));
        assert!(json.contains("5000"));
    }

    #[test]
    fn test_gateway_serialization() {
        let element = DslElement::Gateway {
            id: "gw1".to_string(),
            name: "Decide".to_string(),
            gateway_type: DslGatewayType::ExclusiveGateway,
            conditions: vec![DslCondition {
                expression: "amount > 1000".to_string(),
                target_ref: "flow1".to_string(),
                name: Some("High amount".to_string()),
            }],
        };

        let json = serde_json::to_string(&element).unwrap();
        let deserialized: DslElement = serde_json::from_str(&json).unwrap();

        match deserialized {
            DslElement::Gateway { conditions, .. } => {
                assert_eq!(conditions.len(), 1);
            }
            _ => panic!("Wrong element type"),
        }
    }

    #[test]
    fn test_service_task_serialization() {
        let element = DslElement::Task {
            id: "task1".to_string(),
            name: "Call API".to_string(),
            task_type: DslTaskType::ServiceTask {
                implementation: DslServiceImplementation::Http {
                    url: "https://api.example.com/endpoint".to_string(),
                    method: "POST".to_string(),
                    headers: HashMap::from([(
                        "Content-Type".to_string(),
                        "application/json".to_string(),
                    )]),
                },
            },
            properties: HashMap::new(),
        };

        let json = serde_json::to_string(&element).unwrap();
        let deserialized: DslElement = serde_json::from_str(&json).unwrap();

        match deserialized {
            DslElement::Task { task_type, .. } => match task_type {
                DslTaskType::ServiceTask { implementation } => match implementation {
                    DslServiceImplementation::Http { method, .. } => {
                        assert_eq!(method, "POST");
                    }
                    _ => panic!("Wrong implementation type"),
                },
                _ => panic!("Wrong task type"),
            },
            _ => panic!("Wrong element type"),
        }
    }
}
