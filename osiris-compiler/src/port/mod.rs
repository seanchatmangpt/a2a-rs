//! Port traits for the Osiris compiler.
//!
//! Defines the contracts (traits) that adapters must implement.
//! Ports depend only on domain types.

pub mod a2a_orchestrator;
pub mod artifact_publisher;
pub mod audit_log;
pub mod backup;
pub mod bounded_writer;
pub mod circuit_breaker;
pub mod dsl_compiler;
pub mod guard_evaluator;
pub mod invariant_verifier;
pub mod merkle_storage;
pub mod orderer;
pub mod queue_adapter;
pub mod receipt_builder;
pub mod secret_store;
pub mod transport;
pub mod type_checker;
pub mod workflow_kernel;
pub mod workflow_store;

pub use a2a_orchestrator::{
    A2AOrchestratorConfig, A2AOrchestratorPort, OrchestrationError, OrchestrationEventStream,
    OrchestrationResult, TaskLifecycleManager,
};
pub use artifact_publisher::ArtifactPublisher;
pub use audit_log::AuditLog;
pub use backup::{BackupManager, GcsBackupConfig};
pub use bounded_writer::{BoundedWriter, CommitResult, WriteError};
pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerSnapshot, CircuitState,
};
pub use dsl_compiler::{
    CompilationStats, DslCompiler, DslCompilerError, DslCompilerResult, DslCompilerWithStats,
};
pub use guard_evaluator::GuardEvaluator;
pub use invariant_verifier::{InvariantVerificationError, InvariantVerifier};
pub use merkle_storage::{MerkleReceiptStorage, PersistentMerkleBackend};
pub use orderer::DeterministicOrderer;
pub use queue_adapter::{QueueAdapter, QueueConfig};
pub use receipt_builder::{ReceiptBuilder, ReceiptStorage};
pub use secret_store::{
    RotationPolicy, SecretMetadata, SecretStore, SecretStoreError, SecretVersion, VersionState,
};
pub use transport::{
    OperationResponse, OperationStream, ReceiptStream, StreamStats, Transport, TransportConfig,
    TransportConfigBuilder, TransportError, TransportResult,
};
pub use type_checker::TypeChecker;
pub use workflow_kernel::{
    DeadlockReport, IssueSeverity, SoundnessIssue, SoundnessReport, WorkflowAnalyzer,
    WorkflowError, WorkflowKernel, WorkflowResult,
};
pub use workflow_store::{
    Checkpoint, CheckpointMetadata, CheckpointQuery, RecoverySummary, WorkflowStore,
    WorkflowStoreError, WorkflowStoreResult,
};

#[cfg(feature = "async")]
pub use orderer::AsyncDeterministicOrderer;
