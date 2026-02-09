//! Osiris Compiler: CONSTRUCT8 bounded writer for RDF-based state mutations.
//!
//! This library provides deterministic, bounded state mutations using SPARQL CONSTRUCT
//! semantics with a hard limit of 8 mutation units per commit.
//!
//! # Architecture
//!
//! - **Domain**: Pure types for patches, triples, and validation
//! - **Port**: Trait definitions for bounded writers
//! - **Adapter**: Implementations for in-memory, Firestore, and Spanner backends
//!
//! # Example
//!
//! ```rust,ignore
//! use osiris_compiler::prelude::*;
//!
//! // Create a writer with in-memory storage
//! let writer = InMemoryWriter::new();
//!
//! // Create a patch with some triples
//! let mut patch = Patch::new();
//! patch.add(Triple::new("subject", "predicate", "object"));
//!
//! // Commit the patch
//! let result = writer.commit_patch(patch).await?;
//! println!("Committed {} additions", result.additions_count);
//! ```
//!
//! # Features
//!
//! - `tokio-runtime` (default): Async runtime support
//! - `firestore-backend`: Firestore storage backend
//! - `spanner-backend`: Spanner storage backend
//! - `tracing`: Structured logging

pub mod adapter;
pub mod application;
pub mod domain;
pub mod port;

pub use domain::{
    Commit, CommitVerificationResult, ComparisonOperator, DependencyRelation, GuardCondition,
    GuardEvaluationResult, HGuard, InvariantCheckResult, InvariantPredicate, InvariantSeverity,
    MAX_MUTATION_UNITS, Operation, OperationKind, OperationResult, OrderingError, Packet,
    PacketType, Patch, PatchError, PatchSet, PreservationResult, QInvariant, Receipt, ReceiptError,
    RefusalCategory, RefusalInfo, RefusalReason, RefusalReceipt, ReplayPointer, Sigma,
    StateSnapshot, Triple, TriplePattern, TypeCheckResult, TypeSchema,
};
pub use port::{
    BoundedWriter, CommitResult, DeadlockReport, DeterministicOrderer, GuardEvaluator,
    InvariantVerificationError, InvariantVerifier, IssueSeverity, ReceiptBuilder, ReceiptStorage,
    SoundnessIssue, SoundnessReport, TypeChecker, WorkflowAnalyzer, WorkflowError, WorkflowKernel,
    WorkflowResult, WriteError,
};

pub use adapter::{
    Construct8Writer, EvaluationContext, GuardEvaluationError, HGuardEvaluatorAdapter,
    InMemoryReceiptStorage, InMemoryWorkflowKernel, InMemoryWriter, LambdaOrderer,
    LambdaOrdererConfig, LocalSigner, QInvariantVerifier, SigmaTypeChecker, Signer,
    StandardReceiptBuilder, TypeCheckError,
};

pub use application::{Compiler, CompilerConfig};

#[cfg(feature = "kms")]
pub use adapter::{KmsConfig, KmsSigner};

#[cfg(feature = "storage")]
pub use adapter::{CloudStorageConfig, CloudStorageReceiptStorage};

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::adapter::{
        Construct8Writer, EvaluationContext, GuardEvaluationError, HGuardEvaluatorAdapter,
        InMemoryReceiptStorage, InMemoryWorkflowKernel, InMemoryWriter, LambdaOrderer,
        LambdaOrdererConfig, LocalSigner, QInvariantVerifier, SigmaTypeChecker, Signer,
        StandardReceiptBuilder, TypeCheckError,
    };
    pub use crate::domain::{
        Commit, CommitVerificationResult, ComparisonOperator, DependencyRelation, GuardCondition,
        GuardEvaluationResult, HGuard, InvariantCheckResult, InvariantPredicate, InvariantSeverity,
        MAX_MUTATION_UNITS, Operation, OperationKind, OperationResult, OrderingError, Packet,
        PacketType, Patch, PatchError, PatchSet, PreservationResult, QInvariant, Receipt,
        ReceiptError, RefusalCategory, RefusalInfo, RefusalReason, RefusalReceipt, ReplayPointer,
        Sigma, StateSnapshot, Triple, TriplePattern, TypeCheckResult, TypeSchema,
    };
    pub use crate::port::{
        BoundedWriter, CommitResult, DeadlockReport, DeterministicOrderer, GuardEvaluator,
        InvariantVerificationError, InvariantVerifier, IssueSeverity, ReceiptBuilder,
        ReceiptStorage, SoundnessIssue, SoundnessReport, TypeChecker, WorkflowAnalyzer,
        WorkflowError, WorkflowKernel, WorkflowResult, WriteError,
    };

    #[cfg(feature = "kms")]
    pub use crate::adapter::{KmsConfig, KmsSigner};

    #[cfg(feature = "storage")]
    pub use crate::adapter::{CloudStorageConfig, CloudStorageReceiptStorage};
}
