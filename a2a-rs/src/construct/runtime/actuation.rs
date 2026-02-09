//! Bounded Actuation system (Δ) for finite, deterministic state updates
//!
//! This module implements a bounded actuation system that ensures:
//! - Finite number of updates per transition
//! - Atomic batch execution with rollback on failure
//! - Invariant validation before committing
//! - Complete audit trail via update receipts
//!
//! # Theory
//!
//! The actuation system Δ provides a controlled mechanism for state modifications.
//! For a state S and a set of updates U = {u₁, u₂, ..., uₙ}, the actuation function
//! Δ: S × U → S' either succeeds atomically or fails completely, preserving the
//! invariant I(S) ⇒ I(S').
//!
//! # Bounds
//!
//! - Maximum updates per batch: configurable via `UpdateLimit`
//! - Each update must be idempotent and deterministic
//! - Rollback is guaranteed if any update fails or violates invariants
//!
//! # Example
//!
//! ```rust
//! use a2a_rs::construct::runtime::{BoundedActuator, StateUpdate, UpdateLimit};
//! use std::collections::HashMap;
//!
//! #[derive(Clone, Debug, PartialEq)]
//! struct Counter {
//!     value: i32,
//! }
//!
//! #[derive(Clone, Debug)]
//! enum CounterUpdate {
//!     Increment,
//!     Decrement,
//!     Set(i32),
//! }
//!
//! impl StateUpdate<Counter> for CounterUpdate {
//!     fn apply(&self, state: &mut Counter) -> Result<(), String> {
//!         match self {
//!             CounterUpdate::Increment => {
//!                 state.value = state.value.checked_add(1)
//!                     .ok_or_else(|| "overflow".to_string())?;
//!             }
//!             CounterUpdate::Decrement => {
//!                 state.value = state.value.checked_sub(1)
//!                     .ok_or_else(|| "underflow".to_string())?;
//!             }
//!             CounterUpdate::Set(v) => {
//!                 state.value = *v;
//!             }
//!         }
//!         Ok(())
//!     }
//!
//!     fn description(&self) -> String {
//!         match self {
//!             CounterUpdate::Increment => "increment".to_string(),
//!             CounterUpdate::Decrement => "decrement".to_string(),
//!             CounterUpdate::Set(v) => format!("set to {}", v),
//!         }
//!     }
//! }
//!
//! let mut actuator = BoundedActuator::new(
//!     Counter { value: 0 },
//!     UpdateLimit::default(),
//! );
//!
//! // Add updates to batch
//! actuator.stage_update(CounterUpdate::Increment);
//! actuator.stage_update(CounterUpdate::Increment);
//!
//! // Execute atomically
//! let receipt = actuator.commit(None).unwrap();
//! assert_eq!(receipt.updates_applied, 2);
//! assert_eq!(actuator.state().value, 2);
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use thiserror::Error;

/// Errors that can occur during actuation
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActuationError {
    /// Update batch exceeds maximum allowed size
    #[error("Update batch size {actual} exceeds limit {limit}")]
    UpdateLimitExceeded { actual: usize, limit: usize },

    /// Invariant validation failed after updates
    #[error("Invariant violation: {reason}")]
    InvariantViolation { reason: String },

    /// Update application failed
    #[error("Update failed: {message}")]
    UpdateFailed { message: String },

    /// No updates staged for commit
    #[error("No updates staged for commit")]
    NoUpdatesStaged,

    /// Rollback failed (critical error)
    #[error("Rollback failed: {reason}")]
    RollbackFailed { reason: String },
}

/// Configuration for update limits
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLimit {
    /// Maximum number of updates per batch
    pub max_updates_per_batch: usize,
}

impl Default for UpdateLimit {
    fn default() -> Self {
        Self {
            max_updates_per_batch: 100,
        }
    }
}

impl UpdateLimit {
    /// Create a new update limit with specified maximum
    pub fn new(max_updates_per_batch: usize) -> Self {
        Self {
            max_updates_per_batch,
        }
    }

    /// Unlimited updates (use with caution)
    pub fn unlimited() -> Self {
        Self {
            max_updates_per_batch: usize::MAX,
        }
    }

    /// Strict limit (single update per batch)
    pub fn strict() -> Self {
        Self {
            max_updates_per_batch: 1,
        }
    }
}

/// Trait for state updates that can be applied atomically
///
/// Implementations must be deterministic and idempotent when possible.
pub trait StateUpdate<S>: Debug + Clone {
    /// Apply this update to the given state
    ///
    /// Returns `Ok(())` if successful, or an error message on failure.
    /// The state should not be partially modified on failure.
    fn apply(&self, state: &mut S) -> Result<(), String>;

    /// Human-readable description of this update for audit trail
    fn description(&self) -> String;
}

/// Receipt for a committed batch of updates
///
/// Provides a complete audit trail of what was applied, when, and the resulting state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActuationReceipt {
    /// Unique identifier for this actuation
    pub id: String,

    /// When the updates were committed
    pub timestamp: DateTime<Utc>,

    /// Number of updates successfully applied
    pub updates_applied: usize,

    /// Descriptions of all applied updates
    pub update_descriptions: Vec<String>,

    /// Optional context provided during commit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,

    /// Whether rollback occurred
    pub rolled_back: bool,

    /// Optional error if rollback occurred
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Batch of staged updates ready for atomic application
#[derive(Debug, Clone)]
pub struct UpdateBatch<U> {
    /// Updates to apply in order
    updates: Vec<U>,
    /// Optional context for this batch
    context: Option<String>,
}

impl<U> UpdateBatch<U> {
    /// Create a new empty batch
    pub fn new() -> Self {
        Self {
            updates: Vec::new(),
            context: None,
        }
    }

    /// Add an update to this batch
    pub fn add(&mut self, update: U) {
        self.updates.push(update);
    }

    /// Set context for this batch
    pub fn with_context(mut self, context: String) -> Self {
        self.context = Some(context);
        self
    }

    /// Get the number of updates in this batch
    pub fn len(&self) -> usize {
        self.updates.len()
    }

    /// Check if this batch is empty
    pub fn is_empty(&self) -> bool {
        self.updates.is_empty()
    }

    /// Get the context for this batch
    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }
}

impl<U> Default for UpdateBatch<U> {
    fn default() -> Self {
        Self::new()
    }
}

/// Bounded actuator for finite, deterministic state updates
///
/// Manages state transitions with bounded updates, atomic commits, and rollback.
pub struct BoundedActuator<S, U>
where
    S: Clone,
    U: StateUpdate<S>,
{
    /// Current state
    state: S,
    /// Staged updates not yet committed
    staged_batch: UpdateBatch<U>,
    /// Update limits configuration
    limits: UpdateLimit,
    /// History of all committed receipts
    receipts: Vec<ActuationReceipt>,
    /// Optional invariant validator
    invariant: Option<Box<dyn Fn(&S) -> Result<(), String> + Send + Sync>>,
}

impl<S, U> BoundedActuator<S, U>
where
    S: Clone + Debug,
    U: StateUpdate<S>,
{
    /// Create a new bounded actuator with initial state and limits
    pub fn new(initial_state: S, limits: UpdateLimit) -> Self {
        Self {
            state: initial_state,
            staged_batch: UpdateBatch::new(),
            limits,
            receipts: Vec::new(),
            invariant: None,
        }
    }

    /// Set an invariant that must hold after every commit
    ///
    /// The invariant is checked before finalizing any commit. If it returns an error,
    /// the entire batch is rolled back.
    pub fn with_invariant<F>(mut self, invariant: F) -> Self
    where
        F: Fn(&S) -> Result<(), String> + Send + Sync + 'static,
    {
        self.invariant = Some(Box::new(invariant));
        self
    }

    /// Get a reference to the current state
    pub fn state(&self) -> &S {
        &self.state
    }

    /// Get the current update limits
    pub fn limits(&self) -> &UpdateLimit {
        &self.limits
    }

    /// Get all committed receipts
    pub fn receipts(&self) -> &[ActuationReceipt] {
        &self.receipts
    }

    /// Get the number of staged updates
    pub fn staged_count(&self) -> usize {
        self.staged_batch.len()
    }

    /// Stage an update for the next commit
    ///
    /// Returns error if staging would exceed the update limit.
    pub fn stage_update(&mut self, update: U) -> Result<(), ActuationError> {
        if self.staged_batch.len() >= self.limits.max_updates_per_batch {
            return Err(ActuationError::UpdateLimitExceeded {
                actual: self.staged_batch.len() + 1,
                limit: self.limits.max_updates_per_batch,
            });
        }

        self.staged_batch.add(update);
        Ok(())
    }

    /// Stage multiple updates at once
    pub fn stage_updates(&mut self, updates: Vec<U>) -> Result<(), ActuationError> {
        let new_total = self.staged_batch.len() + updates.len();
        if new_total > self.limits.max_updates_per_batch {
            return Err(ActuationError::UpdateLimitExceeded {
                actual: new_total,
                limit: self.limits.max_updates_per_batch,
            });
        }

        for update in updates {
            self.staged_batch.add(update);
        }
        Ok(())
    }

    /// Clear all staged updates without committing
    pub fn clear_staged(&mut self) {
        self.staged_batch = UpdateBatch::new();
    }

    /// Commit all staged updates atomically
    ///
    /// If any update fails or the invariant is violated, all changes are rolled back
    /// and the state is restored to its pre-commit value.
    ///
    /// # Arguments
    ///
    /// * `context` - Optional context string for the audit trail
    ///
    /// # Returns
    ///
    /// A receipt documenting the actuation, or an error if commit failed.
    pub fn commit(&mut self, context: Option<String>) -> Result<ActuationReceipt, ActuationError> {
        if self.staged_batch.is_empty() {
            return Err(ActuationError::NoUpdatesStaged);
        }

        // Save checkpoint for rollback
        let checkpoint = self.state.clone();

        // Apply all updates
        let mut applied_count = 0;
        let mut descriptions = Vec::new();

        for update in &self.staged_batch.updates {
            let desc = update.description();
            descriptions.push(desc.clone());

            match update.apply(&mut self.state) {
                Ok(()) => applied_count += 1,
                Err(e) => {
                    // Rollback on failure
                    self.state = checkpoint;
                    return Err(ActuationError::UpdateFailed { message: e });
                }
            }
        }

        // Validate invariant if present
        if let Some(ref invariant) = self.invariant {
            if let Err(reason) = invariant(&self.state) {
                // Rollback on invariant violation
                self.state = checkpoint;
                return Err(ActuationError::InvariantViolation { reason });
            }
        }

        // Success - create receipt
        let receipt = ActuationReceipt {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            updates_applied: applied_count,
            update_descriptions: descriptions,
            context: context.or_else(|| self.staged_batch.context.clone()),
            rolled_back: false,
            error: None,
        };

        self.receipts.push(receipt.clone());
        self.staged_batch = UpdateBatch::new();

        Ok(receipt)
    }

    /// Commit with automatic rollback on error, returning receipt even for failures
    ///
    /// Unlike `commit()`, this method always returns a receipt documenting what
    /// happened, including rollback information.
    pub fn try_commit(
        &mut self,
        context: Option<String>,
    ) -> Result<ActuationReceipt, ActuationReceipt> {
        match self.commit(context.clone()) {
            Ok(receipt) => Ok(receipt),
            Err(e) => {
                let receipt = ActuationReceipt {
                    id: uuid::Uuid::new_v4().to_string(),
                    timestamp: Utc::now(),
                    updates_applied: 0,
                    update_descriptions: self
                        .staged_batch
                        .updates
                        .iter()
                        .map(|u| u.description())
                        .collect(),
                    context,
                    rolled_back: true,
                    error: Some(e.to_string()),
                };

                self.receipts.push(receipt.clone());
                self.staged_batch = UpdateBatch::new();

                Err(receipt)
            }
        }
    }

    /// Replace the current state (use with caution)
    ///
    /// This bypasses the actuation system and directly replaces the state.
    /// No receipt is generated. Use only for initialization or recovery scenarios.
    pub fn reset_state(&mut self, new_state: S) {
        self.state = new_state;
        self.staged_batch = UpdateBatch::new();
    }

    /// Get the total number of updates ever committed
    pub fn total_updates_committed(&self) -> usize {
        self.receipts.iter().map(|r| r.updates_applied).sum()
    }
}

impl<S, U> Debug for BoundedActuator<S, U>
where
    S: Clone + Debug,
    U: StateUpdate<S>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundedActuator")
            .field("state", &self.state)
            .field("staged_batch", &self.staged_batch)
            .field("limits", &self.limits)
            .field("receipts_count", &self.receipts.len())
            .field(
                "invariant",
                &if self.invariant.is_some() {
                    "Some"
                } else {
                    "None"
                },
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct Counter {
        value: i32,
    }

    #[derive(Clone, Debug)]
    enum CounterUpdate {
        Increment,
        Decrement,
        Set(i32),
    }

    impl StateUpdate<Counter> for CounterUpdate {
        fn apply(&self, state: &mut Counter) -> Result<(), String> {
            match self {
                CounterUpdate::Increment => {
                    state.value = state
                        .value
                        .checked_add(1)
                        .ok_or_else(|| "overflow".to_string())?;
                }
                CounterUpdate::Decrement => {
                    state.value = state
                        .value
                        .checked_sub(1)
                        .ok_or_else(|| "underflow".to_string())?;
                }
                CounterUpdate::Set(v) => {
                    state.value = *v;
                }
            }
            Ok(())
        }

        fn description(&self) -> String {
            match self {
                CounterUpdate::Increment => "increment".to_string(),
                CounterUpdate::Decrement => "decrement".to_string(),
                CounterUpdate::Set(v) => format!("set to {}", v),
            }
        }
    }

    #[test]
    fn test_basic_actuation() {
        let mut actuator = BoundedActuator::new(Counter { value: 0 }, UpdateLimit::default());

        actuator.stage_update(CounterUpdate::Increment).unwrap();
        actuator.stage_update(CounterUpdate::Increment).unwrap();

        let receipt = actuator.commit(None).unwrap();

        assert_eq!(receipt.updates_applied, 2);
        assert_eq!(actuator.state().value, 2);
        assert_eq!(actuator.staged_count(), 0);
    }

    #[test]
    fn test_update_limit() {
        let mut actuator = BoundedActuator::new(Counter { value: 0 }, UpdateLimit::new(2));

        actuator.stage_update(CounterUpdate::Increment).unwrap();
        actuator.stage_update(CounterUpdate::Increment).unwrap();

        let result = actuator.stage_update(CounterUpdate::Increment);
        assert!(matches!(
            result,
            Err(ActuationError::UpdateLimitExceeded { .. })
        ));
    }

    #[test]
    fn test_rollback_on_update_failure() {
        let mut actuator =
            BoundedActuator::new(Counter { value: i32::MAX }, UpdateLimit::default());

        actuator.stage_update(CounterUpdate::Increment).unwrap();

        let result = actuator.commit(None);
        assert!(matches!(result, Err(ActuationError::UpdateFailed { .. })));
        assert_eq!(actuator.state().value, i32::MAX); // State unchanged
    }

    #[test]
    fn test_invariant_violation() {
        let actuator = BoundedActuator::new(Counter { value: 0 }, UpdateLimit::default())
            .with_invariant(|s| {
                if s.value < 0 {
                    Err("value must be non-negative".to_string())
                } else {
                    Ok(())
                }
            });

        let mut actuator = actuator;
        actuator.stage_update(CounterUpdate::Decrement).unwrap();

        let result = actuator.commit(None);
        assert!(matches!(
            result,
            Err(ActuationError::InvariantViolation { .. })
        ));
        assert_eq!(actuator.state().value, 0); // Rolled back
    }

    #[test]
    fn test_batch_operations() {
        let mut actuator = BoundedActuator::new(Counter { value: 0 }, UpdateLimit::default());

        let updates = vec![
            CounterUpdate::Increment,
            CounterUpdate::Increment,
            CounterUpdate::Set(10),
            CounterUpdate::Decrement,
        ];

        actuator.stage_updates(updates).unwrap();

        let receipt = actuator.commit(Some("batch test".to_string())).unwrap();

        assert_eq!(receipt.updates_applied, 4);
        assert_eq!(receipt.context, Some("batch test".to_string()));
        assert_eq!(actuator.state().value, 9);
    }

    #[test]
    fn test_try_commit() {
        let mut actuator =
            BoundedActuator::new(Counter { value: i32::MAX }, UpdateLimit::default());

        actuator.stage_update(CounterUpdate::Increment).unwrap();

        let result = actuator.try_commit(None);
        assert!(result.is_err());

        if let Err(receipt) = result {
            assert!(receipt.rolled_back);
            assert!(receipt.error.is_some());
        }
    }

    #[test]
    fn test_clear_staged() {
        let mut actuator = BoundedActuator::new(Counter { value: 0 }, UpdateLimit::default());

        actuator.stage_update(CounterUpdate::Increment).unwrap();
        assert_eq!(actuator.staged_count(), 1);

        actuator.clear_staged();
        assert_eq!(actuator.staged_count(), 0);

        let result = actuator.commit(None);
        assert!(matches!(result, Err(ActuationError::NoUpdatesStaged)));
    }

    #[test]
    fn test_receipt_audit_trail() {
        let mut actuator = BoundedActuator::new(Counter { value: 0 }, UpdateLimit::default());

        actuator.stage_update(CounterUpdate::Increment).unwrap();
        actuator.commit(Some("first".to_string())).unwrap();

        actuator.stage_update(CounterUpdate::Set(5)).unwrap();
        actuator.commit(Some("second".to_string())).unwrap();

        let receipts = actuator.receipts();
        assert_eq!(receipts.len(), 2);
        assert_eq!(receipts[0].context, Some("first".to_string()));
        assert_eq!(receipts[1].context, Some("second".to_string()));
        assert_eq!(actuator.total_updates_committed(), 2);
    }
}
