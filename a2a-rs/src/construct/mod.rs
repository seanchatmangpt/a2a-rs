//! CONSTRUCT layer - Cryptographic primitives, proof systems, and artifact management.
//!
//! This module provides tools for creating verifiable proofs and audit trails
//! of agent behavior and state transitions, as well as immutable artifact storage,
//! ordered event streaming for observable transitions, and refusal determinism
//! through typed guards.
//!
//! ## Method Coverage Documentation
//!
//! For complete A2A protocol v0.3.0 method coverage analysis, see
//! [`coverage.md`](./coverage.md) - Theorem 4.1 completeness checklist.

pub mod artifacts;
pub mod coordination;
pub mod events;
pub mod guards;
pub mod invariants;
pub mod methods;
pub mod observability;
pub mod ontology;
pub mod replay;
pub mod runtime;
pub mod station;
pub mod task_fsm;
pub mod types;

#[cfg(feature = "receipts")]
pub mod receipts;

#[cfg(feature = "sqlx-storage")]
pub mod storage;

#[cfg(test)]
pub mod tests;

// Re-export artifact types
pub use artifacts::{
    ArtifactStore, ArtifactStoreError, ContentHash, InMemoryArtifactStore, StoredArtifact,
    TaskArtifacts,
};

// Re-export event types
pub use events::{
    ArtifactEvent, ErrorEvent, Event, EventError, EventKind, EventResult, TaskStatusEvent,
};

#[cfg(feature = "server")]
pub use events::EventStream;

// Re-export guard types for refusal determinism
pub use guards::{
    AllGuard, AnyGuard, EnumGuard, Guard, RangeGuard, RefusalCode, RefusalReceipt,
    RequiredFieldGuard, StateTransitionGuard, StringLengthGuard, TypeGuard,
};

// Re-export invariant types
pub use invariants::{
    ArtifactImmutabilityInvariant, ArtifactSnapshot, EventOrderingInvariant, EventSequence,
    Invariant, InvariantExpr, InvariantRegistry, InvariantResult, InvariantViolation,
    TaskStateInvariant, parse_invariant,
};

// Re-export method signature types (protocol realization)
pub use methods::{
    CancelTaskStation as MethodCancelTask, DeletePushNotificationConfigStation, EmptyParams,
    GetAuthenticatedExtendedCardStation as MethodGetAuthenticatedCard,
    GetPushNotificationConfigStation, GetTaskStation as MethodGetTask,
    ListPushNotificationConfigsStation, ListTasksStation as MethodListTasks, MessageSendResult,
    SendMessageStation as MethodSendMessage, SendStreamingMessageStation,
    SetPushNotificationConfigStation, Station as MethodStation,
    StationRegistry as MethodStationRegistry, StreamingMessageResult, TaskResubscribeStation,
};

// Re-export ontology types
pub use ontology::{
    DEFAULT_MAX_AGENTS, DEFAULT_MAX_MESSAGES_PER_TASK, DEFAULT_MAX_TASKS, OntologyState,
    OntologyStorage, StateBounds, StateStats,
};

#[cfg(feature = "receipts")]
pub use receipts::{Receipt, ReceiptChain, ReceiptError, compute_hash, compute_receipt_hash};

#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
pub use storage::{ReceiptStore, ReceiptStoreError};

#[cfg(feature = "sqlx-storage")]
pub use storage::{AsyncOntologyStorage, SqlxOntologyStore};

// Re-export typed packet system
pub use types::{
    CancelTaskRequest, CancelTaskResponse, DeleteTaskPushNotificationConfigRequest,
    DeleteTaskPushNotificationConfigResponse, GetAuthenticatedExtendedCardParams,
    GetAuthenticatedExtendedCardRequest, GetAuthenticatedExtendedCardResponse,
    GetExtendedCardParams, GetExtendedCardRequest, GetExtendedCardResponse,
    GetTaskPushNotificationConfigRequest, GetTaskPushNotificationConfigResponse,
    GetTaskPushNotificationRequest, GetTaskRequest, GetTaskResponse, JsonRpcError, JsonRpcId,
    ListTaskPushNotificationConfigRequest, ListTaskPushNotificationConfigResponse,
    ListTasksRequest, ListTasksResponse, Packet, PacketType, SendMessageRequest,
    SendMessageResponse, SendTaskRequest, SendTaskStreamingRequest, SetTaskPushNotificationRequest,
    SetTaskPushNotificationResponse, TaskResubscriptionRequest,
};

// Re-export runtime types
pub use runtime::{
    ActuationError, ActuationReceipt, BoundedActuator, ExecutionContext, ExecutionReceipt,
    Operation, PriorityClass, Runtime, RuntimeError, RuntimeEvent, RuntimeOutput, ScheduledTask,
    Scheduler, SchedulerError, StateUpdate, UpdateBatch, UpdateLimit,
};

// Re-export coordination types
pub use coordination::{
    CoordinationError, CoordinationResult, DependencyEdge, TaskGraph, TaskNode,
};

// Re-export task FSM types
pub use task_fsm::{
    StateTransition, StateTransitionError, TaskStateMachine, TransitionGuard, TransitionResult,
};

// Re-export observability types
pub use observability::{MetricsSnapshot, ObservabilityContext, OperationTiming, RuntimeMetrics};

#[cfg(feature = "tracing")]
pub use observability::{InstrumentedGuard, InstrumentedInvariant};

// Re-export station types (Note: station::RefusalReceipt is separate from guards::RefusalReceipt)
pub use station::{
    CancelTaskStation, GetExtendedCardStation, GetTaskStation, ListTasksStation, Ontology,
    SendMessageStation, Station, StationHandler, StationRegistry,
};

// Re-export replay types
pub use replay::{
    DebuggerConfig, DebuggerStatus, DifferenceKind, ExecutionRecorder, ExecutionReplayer,
    RecordedStep, ReplayDebugger, ReplayResult, SnapshotDiff, StateSnapshot, StepReport,
    StepResult, StepSummary,
};

#[cfg(feature = "receipts")]
pub use replay::ReceiptChainVerifier;
