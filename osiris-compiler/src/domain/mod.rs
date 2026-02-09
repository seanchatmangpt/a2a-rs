//! Domain types for the Osiris compiler.
//!
//! This module contains pure domain types with no external dependencies
//! beyond serialization and basic utilities.

pub mod error;
pub mod invariants;
pub mod operation;
pub mod patch;
pub mod receipt;
pub mod triple;
pub mod types;
pub mod workflow;

pub use error::OrderingError;
pub use invariants::{
    Commit, CommitVerificationResult, ComparisonOperator, InvariantCheckResult, InvariantPredicate,
    InvariantSeverity, PreservationResult, QInvariant, StateSnapshot,
};
pub use operation::{Operation, OperationKind};
pub use patch::{MAX_MUTATION_UNITS, Patch, PatchError, PatchSet};
pub use receipt::{
    DependencyRelation, OperationResult, Receipt, ReceiptError, RefusalCategory, RefusalInfo,
    ReplayPointer,
};
pub use triple::{Triple, TriplePattern};
pub use types::*;
pub use workflow::*;
