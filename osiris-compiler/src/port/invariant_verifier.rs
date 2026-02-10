//! Port trait for Q invariant verification.
//!
//! This trait defines the interface for verifying invariants across state
//! transitions. Implementations must prove preserve(Q) before allowing commits.

use crate::domain::{
    Commit, CommitVerificationResult, InvariantCheckResult, PreservationResult, QInvariant,
    RefusalReceipt, StateSnapshot,
};
use async_trait::async_trait;
use thiserror::Error;

/// Errors that can occur during invariant verification.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum InvariantVerificationError {
    /// Invariant not found
    #[error("Invariant not found: {0}")]
    InvariantNotFound(String),

    /// State snapshot invalid or incomplete
    #[error("Invalid state snapshot: {0}")]
    InvalidStateSnapshot(String),

    /// Predicate evaluation failed
    #[error("Predicate evaluation failed: {0}")]
    PredicateEvaluationFailed(String),

    /// Missing required state field
    #[error("Missing required state field: {0}")]
    MissingStateField(String),

    /// Type mismatch in predicate evaluation
    #[error("Type mismatch in predicate: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    /// Custom expression evaluation error
    #[error("Custom expression error: {0}")]
    CustomExpressionError(String),

    /// Commit blocked by invariant violations
    #[error("Commit blocked: {violation_count} invariant(s) violated")]
    CommitBlocked { violation_count: usize },

    /// Internal verification error
    #[error("Internal verification error: {0}")]
    InternalError(String),
}

/// Port trait for Q invariant verification.
///
/// Implementations must:
/// 1. Evaluate invariant predicates over state snapshots
/// 2. Prove preserve(Q) for state transitions (commits)
/// 3. Block commits that violate invariants
/// 4. Emit refusal receipts on invariant violations
#[async_trait]
pub trait InvariantVerifier: Send + Sync {
    /// Register a Q invariant for verification.
    ///
    /// # Arguments
    /// * `invariant` - The invariant to register
    ///
    /// # Returns
    /// Ok(()) if registration succeeds, error otherwise
    async fn register_invariant(
        &mut self,
        invariant: QInvariant,
    ) -> Result<(), InvariantVerificationError>;

    /// Unregister an invariant by ID.
    ///
    /// # Arguments
    /// * `invariant_id` - ID of the invariant to remove
    ///
    /// # Returns
    /// Ok(()) if unregistration succeeds, error otherwise
    async fn unregister_invariant(
        &mut self,
        invariant_id: &str,
    ) -> Result<(), InvariantVerificationError>;

    /// Get a registered invariant by ID.
    ///
    /// # Arguments
    /// * `invariant_id` - ID of the invariant to retrieve
    ///
    /// # Returns
    /// The invariant if found, error otherwise
    async fn get_invariant(
        &self,
        invariant_id: &str,
    ) -> Result<QInvariant, InvariantVerificationError>;

    /// List all registered invariants.
    ///
    /// # Returns
    /// Vector of all registered invariants
    async fn list_invariants(&self) -> Vec<QInvariant>;

    /// Check if an invariant holds in a given state.
    ///
    /// # Arguments
    /// * `invariant_id` - ID of the invariant to check
    /// * `state` - State snapshot to evaluate against
    ///
    /// # Returns
    /// Result indicating whether the invariant holds
    async fn check_invariant(
        &self,
        invariant_id: &str,
        state: &StateSnapshot,
    ) -> Result<InvariantCheckResult, InvariantVerificationError>;

    /// Verify that an invariant is preserved across a state transition.
    ///
    /// This is the core preserve(Q) check: the invariant must hold in both
    /// the pre-state and post-state.
    ///
    /// # Arguments
    /// * `invariant_id` - ID of the invariant to verify
    /// * `pre_state` - State before the transition
    /// * `post_state` - State after the transition
    ///
    /// # Returns
    /// Result indicating whether preserve(Q) holds
    async fn verify_preservation(
        &self,
        invariant_id: &str,
        pre_state: &StateSnapshot,
        post_state: &StateSnapshot,
    ) -> Result<PreservationResult, InvariantVerificationError>;

    /// Verify all invariants for a commit.
    ///
    /// This is the jidoka "stop-the-line" mechanism. If any critical or error
    /// severity invariant is violated, the commit is blocked.
    ///
    /// # Arguments
    /// * `commit` - The commit to verify
    ///
    /// # Returns
    /// Verification result indicating whether the commit should be allowed
    async fn verify_commit(
        &self,
        commit: &Commit,
    ) -> Result<CommitVerificationResult, InvariantVerificationError>;

    /// Block a commit and emit a refusal receipt.
    ///
    /// Called when verify_commit indicates the commit should be blocked.
    ///
    /// # Arguments
    /// * `commit` - The commit being blocked
    /// * `verification_result` - Result from verify_commit
    ///
    /// # Returns
    /// Refusal receipt documenting the rejection
    async fn block_commit(
        &self,
        commit: &Commit,
        verification_result: &CommitVerificationResult,
    ) -> Result<RefusalReceipt, InvariantVerificationError>;

    /// Enable or disable an invariant.
    ///
    /// # Arguments
    /// * `invariant_id` - ID of the invariant
    /// * `enabled` - Whether to enable the invariant
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    async fn set_invariant_enabled(
        &mut self,
        invariant_id: &str,
        enabled: bool,
    ) -> Result<(), InvariantVerificationError>;
}
