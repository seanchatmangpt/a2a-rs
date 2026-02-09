//! TypeChecker port trait.
//!
//! Defines the interface for verifying packets against the closed type system Σ.

use crate::domain::{Packet, Sigma, TypeCheckResult};
use async_trait::async_trait;
use std::error::Error;

/// Port trait for type checking packets against Σ.
///
/// Implementations MUST:
/// - Reject any packet whose type is not in Σ
/// - Validate packet payload against registered schemas
/// - Return detailed error information for violations
#[async_trait]
pub trait TypeChecker: Send + Sync {
    /// Checks if a packet type is admissible in Σ.
    ///
    /// Returns `true` if the packet type is registered in Σ, `false` otherwise.
    async fn is_admissible(&self, packet: &Packet) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Validates a packet against Σ.
    ///
    /// Returns:
    /// - `TypeCheckResult::Valid` if packet type is in Σ and payload is valid
    /// - `TypeCheckResult::TypeNotInSigma` if packet type is not registered
    /// - `TypeCheckResult::SchemaViolation` if payload violates schema
    async fn check(&self, packet: &Packet)
    -> Result<TypeCheckResult, Box<dyn Error + Send + Sync>>;

    /// Returns the current type system Σ.
    async fn get_sigma(&self) -> Result<Sigma, Box<dyn Error + Send + Sync>>;

    /// Updates the type system Σ.
    ///
    /// This is a privileged operation and should be protected by authorization.
    async fn update_sigma(&mut self, sigma: Sigma) -> Result<(), Box<dyn Error + Send + Sync>>;
}
