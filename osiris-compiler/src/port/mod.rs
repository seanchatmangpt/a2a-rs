//! Port traits for the Osiris compiler.
//!
//! Defines the contracts (traits) that adapters must implement.
//! Ports depend only on domain types.

pub mod a2a_orchestrator;
pub mod artifact_publisher;
pub mod bounded_writer;
pub mod dsl_compiler;
pub mod guard_evaluator;
pub mod invariant_verifier;
pub mod merkle_storage;
pub mod orderer;
pub mod receipt_builder;
pub mod type_checker;
pub mod workflow_kernel;

pub use a2a_orchestrator::{
    A2AOrchestratorConfig, A2AOrchestratorPort, OrchestrationError, OrchestrationEventStream,
    OrchestrationResult, TaskLifecycleManager,
};
pub use artifact_publisher::ArtifactPublisher;
pub use bounded_writer::{BoundedWriter, CommitResult, WriteError};
pub use dsl_compiler::{
    CompilationStats, DslCompiler, DslCompilerError, DslCompilerResult, DslCompilerWithStats,
};
pub use guard_evaluator::GuardEvaluator;
pub use invariant_verifier::{InvariantVerificationError, InvariantVerifier};
pub use merkle_storage::{MerkleReceiptStorage, PersistentMerkleBackend};
pub use orderer::DeterministicOrderer;
pub use receipt_builder::{ReceiptBuilder, ReceiptStorage};
pub use type_checker::TypeChecker;
pub use workflow_kernel::{
    DeadlockReport, IssueSeverity, SoundnessIssue, SoundnessReport, WorkflowAnalyzer,
    WorkflowError, WorkflowKernel, WorkflowResult,
};

#[cfg(feature = "async")]
pub use orderer::AsyncDeterministicOrderer;
