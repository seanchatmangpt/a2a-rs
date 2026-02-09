//! Port traits for the Osiris compiler.
//!
//! Defines the contracts (traits) that adapters must implement.
//! Ports depend only on domain types.

pub mod bounded_writer;
pub mod guard_evaluator;
pub mod invariant_verifier;
pub mod orderer;
pub mod receipt_builder;
pub mod type_checker;
pub mod workflow_kernel;

pub use bounded_writer::{BoundedWriter, CommitResult, WriteError};
pub use guard_evaluator::GuardEvaluator;
pub use invariant_verifier::{InvariantVerificationError, InvariantVerifier};
pub use orderer::DeterministicOrderer;
pub use receipt_builder::{ReceiptBuilder, ReceiptStorage};
pub use type_checker::TypeChecker;
pub use workflow_kernel::{
    DeadlockReport, IssueSeverity, SoundnessIssue, SoundnessReport, WorkflowAnalyzer,
    WorkflowError, WorkflowKernel, WorkflowResult,
};

#[cfg(feature = "async")]
pub use orderer::AsyncDeterministicOrderer;
