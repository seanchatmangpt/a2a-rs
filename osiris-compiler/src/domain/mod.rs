//! Domain types for the Osiris compiler.
//!
//! This module contains pure domain types with no external dependencies
//! beyond serialization and basic utilities.

pub mod a2a_orchestration;
pub mod artifact;
pub mod dsl;
pub mod error;
pub mod invariants;
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
pub use dsl::*;
pub use error::OrderingError;
pub use invariants::{
    Commit, CommitVerificationResult, ComparisonOperator, InvariantCheckResult, InvariantPredicate,
    InvariantSeverity, PreservationResult, QInvariant, StateSnapshot,
};
pub use merkle_tree::{MerkleError, MerkleNode, MerkleProof, MerkleRoot, MerkleTree, ProofStep};
pub use operation::{Operation, OperationKind};
pub use patch::{MAX_MUTATION_UNITS, Patch, PatchError, PatchSet};
pub use receipt::{
    DependencyRelation, OperationResult, Receipt, ReceiptError, RefusalCategory, RefusalInfo,
    ReplayPointer,
};
pub use triple::{Triple, TriplePattern};
pub use types::*;
pub use workflow::*;
