//! Domain models for the A2A protocol

pub mod core;
pub mod error;
pub mod events;
pub mod hooks;
pub mod protocols;
pub mod queries;
#[cfg(test)]
mod tests;
pub mod validation;
pub mod workflow;

// Re-export key types for convenience
pub use core::{
    AdmissionDecision, AgentCapabilities, AgentCard, AgentCardSignature, AgentExtension,
    AgentInterface, AgentProvider, AgentSkill, Artifact, AuthorizationCodeOAuthFlow,
    ClientCredentialsOAuthFlow, DeleteTaskPushNotificationConfigParams, FileContent,
    GetTaskPushNotificationConfigParams, ImplicitOAuthFlow, IngressChannel, JidokaMode,
    ListTaskPushNotificationConfigParams, ListTasksParams, ListTasksResult, Message,
    MessageSendConfiguration, MessageSendParams, OAuthFlows, Part, PasswordOAuthFlow,
    PushNotificationAuthenticationInfo, PushNotificationConfig, RefusalReason, RefusalReceipt,
    Role, SecurityScheme, SupplierQuality, SystemHealth, Task, TaskIdParams,
    TaskPushNotificationConfig, TaskQueryParams, TaskSendParams, TaskState, TaskStatus,
    TransportProtocol, WorkConstraints, WorkPacket,
};
pub use error::A2AError;
pub use events::{TaskArtifactUpdateEvent, TaskStatusUpdateEvent};
pub use protocols::{
    JSONRPCError, JSONRPCMessage, JSONRPCNotification, JSONRPCRequest, JSONRPCResponse,
};
pub use queries::{
    AgentStatsView, GetAgentStats, GetMessagesByAgent, GetTasksByStatus, MessageListView,
    TaskListView,
};
pub use validation::{Validate, ValidationResult};
pub use workflow::{
    PatternCategory, StateType, WorkflowAnalysis, WorkflowError, WorkflowGraph, WorkflowPattern,
    WorkflowState, WorkflowTransition,
};
