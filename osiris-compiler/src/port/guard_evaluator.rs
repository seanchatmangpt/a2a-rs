//! GuardEvaluator port trait.
//!
//! Defines the interface for evaluating H-guards (inadmissible-before constraints).

use crate::domain::{GuardEvaluationResult, HGuard, Packet};
use async_trait::async_trait;
use std::error::Error;

/// Port trait for evaluating H-guards.
///
/// Implementations MUST:
/// - Evaluate all registered H-guards for a packet
/// - Block packets that violate any guard
/// - Provide clear reasoning for guard violations
/// - Support temporal, state-based, and custom predicates
#[async_trait]
pub trait GuardEvaluator: Send + Sync {
    /// Registers an H-guard.
    ///
    /// The guard will be evaluated for all matching packets.
    async fn register_guard(&mut self, guard: HGuard) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Removes an H-guard by ID.
    async fn unregister_guard(
        &mut self,
        guard_id: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Evaluates all applicable H-guards for a packet.
    ///
    /// Returns a vector of evaluation results, one per applicable guard.
    /// If ANY guard returns `GuardEvaluationResult::Violated`, the packet MUST be rejected.
    async fn evaluate(
        &self,
        packet: &Packet,
    ) -> Result<Vec<GuardEvaluationResult>, Box<dyn Error + Send + Sync>>;

    /// Evaluates a specific H-guard for a packet.
    async fn evaluate_guard(
        &self,
        guard_id: &str,
        packet: &Packet,
    ) -> Result<GuardEvaluationResult, Box<dyn Error + Send + Sync>>;

    /// Lists all registered H-guards.
    async fn list_guards(&self) -> Result<Vec<HGuard>, Box<dyn Error + Send + Sync>>;
}
