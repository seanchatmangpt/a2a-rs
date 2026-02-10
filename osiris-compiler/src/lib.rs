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
    Artifact, ArtifactPublishError, Commit, CommitVerificationResult, ComparisonOperator,
    DependencyRelation, DslBoundaryEventType, DslCompensation, DslCondition, DslElement,
    DslEndResult, DslEventTrigger, DslFlow, DslGatewayType, DslIntermediateEventType,
    DslLoopConfig, DslLoopType, DslServiceImplementation, DslTaskType, DslVariable, DslWorkflow,
    GuardCondition, GuardEvaluationResult, HGuard, InvariantCheckResult, InvariantPredicate,
    InvariantSeverity, MAX_MUTATION_UNITS, MerkleError, MerkleNode, MerkleProof, MerkleRoot,
    MerkleTree, Operation, OperationKind, OperationResult, OrderingError, Packet, PacketType,
    Patch, PatchError, PatchSet, PreservationResult, ProofStep, PublishResult, QInvariant, Receipt,
    ReceiptError, RefusalCategory, RefusalInfo, RefusalReason, RefusalReceipt, ReplayPointer,
    Sigma, StateSnapshot, Triple, TriplePattern, TypeCheckResult, TypeSchema,
};
pub use port::{
    ArtifactPublisher, BoundedWriter, CommitResult, CompilationStats, DeadlockReport,
    DeterministicOrderer, DslCompiler, DslCompilerError, DslCompilerResult, DslCompilerWithStats,
    GuardEvaluator, InvariantVerificationError, InvariantVerifier, IssueSeverity,
    MerkleReceiptStorage, PersistentMerkleBackend, ReceiptBuilder, ReceiptStorage, SoundnessIssue,
    SoundnessReport, TypeChecker, WorkflowAnalyzer, WorkflowError, WorkflowKernel, WorkflowResult,
    WriteError,
};

pub use adapter::{
    BpmnCompiler, Construct8Writer, EvaluationContext, GuardEvaluationError,
    HGuardEvaluatorAdapter, InMemoryBackend, InMemoryMerkleStorage, InMemoryReceiptStorage,
    InMemoryWorkflowKernel, InMemoryWriter, LambdaOrderer, LambdaOrdererConfig, LocalSigner,
    PersistentMerkleStorage, QInvariantVerifier, SigmaTypeChecker, Signer, StandardReceiptBuilder,
    TypeCheckError,
};

pub use application::{
    AppError, CompileRequest, CompileResponse, Compiler, CompilerConfig, ErrorResponse,
    PipelineState, PipelineStats, compile, health_check,
};

#[cfg(feature = "kms")]
pub use adapter::{KmsConfig, KmsSigner};

#[cfg(feature = "storage")]
pub use adapter::{CloudStorageConfig, CloudStorageReceiptStorage};

#[cfg(feature = "gcs")]
pub use adapter::{GcsConfig, GcsReceiptStorage};

#[cfg(feature = "firestore")]
pub use adapter::FirestoreStateStore;

#[cfg(feature = "workspace-publisher")]
pub use adapter::{GoogleWorkspacePublisher, WorkspacePublisherConfig};

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::adapter::{
        BpmnCompiler, Construct8Writer, EvaluationContext, GuardEvaluationError,
        HGuardEvaluatorAdapter, InMemoryBackend, InMemoryMerkleStorage, InMemoryReceiptStorage,
        InMemoryWorkflowKernel, InMemoryWriter, LambdaOrderer, LambdaOrdererConfig, LocalSigner,
        PersistentMerkleStorage, QInvariantVerifier, SigmaTypeChecker, Signer,
        StandardReceiptBuilder, TypeCheckError,
    };
    pub use crate::domain::{
        Artifact, ArtifactPublishError, Commit, CommitVerificationResult, ComparisonOperator,
        DependencyRelation, DslBoundaryEventType, DslCompensation, DslCondition, DslElement,
        DslEndResult, DslEventTrigger, DslFlow, DslGatewayType, DslIntermediateEventType,
        DslLoopConfig, DslLoopType, DslServiceImplementation, DslTaskType, DslVariable,
        DslWorkflow, GuardCondition, GuardEvaluationResult, HGuard, InvariantCheckResult,
        InvariantPredicate, InvariantSeverity, MAX_MUTATION_UNITS, MerkleError, MerkleNode,
        MerkleProof, MerkleRoot, MerkleTree, Operation, OperationKind, OperationResult,
        OrderingError, Packet, PacketType, Patch, PatchError, PatchSet, PreservationResult,
        ProofStep, PublishResult, QInvariant, Receipt, ReceiptError, RefusalCategory, RefusalInfo,
        RefusalReason, RefusalReceipt, ReplayPointer, Sigma, StateSnapshot, Triple, TriplePattern,
        TypeCheckResult, TypeSchema,
    };
    pub use crate::port::{
        ArtifactPublisher, BoundedWriter, CommitResult, CompilationStats, DeadlockReport,
        DeterministicOrderer, DslCompiler, DslCompilerError, DslCompilerResult,
        DslCompilerWithStats, GuardEvaluator, InvariantVerificationError, InvariantVerifier,
        IssueSeverity, MerkleReceiptStorage, PersistentMerkleBackend, ReceiptBuilder,
        ReceiptStorage, SoundnessIssue, SoundnessReport, TypeChecker, WorkflowAnalyzer,
        WorkflowError, WorkflowKernel, WorkflowResult, WriteError,
    };

    #[cfg(feature = "kms")]
    pub use crate::adapter::{KmsConfig, KmsSigner};

    #[cfg(feature = "storage")]
    pub use crate::adapter::{CloudStorageConfig, CloudStorageReceiptStorage};

    #[cfg(feature = "gcs")]
    pub use crate::adapter::{GcsConfig, GcsReceiptStorage};

    #[cfg(feature = "firestore")]
    pub use crate::adapter::FirestoreStateStore;

    #[cfg(feature = "workspace-publisher")]
    pub use crate::adapter::{GoogleWorkspacePublisher, WorkspacePublisherConfig};
}
