//! Domain types for the Osiris compiler.
//!
//! This module contains pure domain types with no external dependencies
//! beyond serialization and basic utilities.

pub mod a2a_orchestration;
pub mod artifact;
pub mod audit;
pub mod backup;
pub mod dsl;
pub mod error;
pub mod invariants;
pub mod job;
pub mod merkle_tree;
pub mod operation;
pub mod patch;
pub mod receipt;
pub mod triple;
pub mod types;
pub mod workflow;

pub use a2a_orchestration::{
    A2AOrchestrationTask, ArtifactReference, OperationPayload, OrchestrationEvent,
    OrchestrationSnapshot, OrchestrationTaskState,
};
pub use artifact::{
    Artifact, ArtifactPublishError, ArtifactResult, CalendarArtifact, DriveArtifact, EmailArtifact,
    PublishResult, SharingPermission,
};
pub use audit::{
    AuditDetails, AuditError, AuditEventType, AuditLogEntry, AuditSeverity, AuditStatus,
    TraceContext,
};
pub use backup::{
    BackupChain, BackupError, BackupRotationPolicy, BackupStats, BackupType, CompilerStateSnapshot,
    RecoveryRequest, RecoveryResult, VerificationResult,
};
pub use dsl::*;
pub use error::{CircuitBreakerError, OrderingError, QueueError};
pub use invariants::{
    Commit, CommitVerificationResult, ComparisonOperator, InvariantCheckResult, InvariantPredicate,
    InvariantSeverity, PreservationResult, QInvariant, StateSnapshot,
};
pub use job::{HttpMethod, Job, JobExecutionResult, JobStatus, OidcTokenConfig, RetryConfig};
pub use merkle_tree::{MerkleError, MerkleNode, MerkleProof, MerkleRoot, MerkleTree, ProofStep};
pub use operation::{Operation, OperationKind};
pub use patch::{Patch, PatchError, PatchSet, MAX_MUTATION_UNITS};
pub use receipt::{
    DependencyRelation, OperationResult, Receipt, ReceiptError, RefusalCategory, RefusalInfo,
    ReplayPointer,
};
pub use triple::{Triple, TriplePattern};
pub use types::*;
pub use workflow::*;
