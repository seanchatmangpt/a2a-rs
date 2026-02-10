//! WIP gate port definitions
//!
//! Defines the interface for Work-in-Progress limiting using Kanban principles.
//! The gate enforces hard caps on concurrent work and rejects excess deterministically.

use async_trait::async_trait;
use std::future::Future;

use crate::domain::WipError;

/// A permit representing one unit of WIP capacity
///
/// The permit is held while work is in progress. When dropped,
/// the WIP slot is released back to the gate.
pub trait WipPermit: Send {
    /// Release the permit explicitly (also released on drop)
    fn release(self);
}

/// WIP (Work-in-Progress) gate for admission control
///
/// Enforces Kanban-style WIP limits with:
/// - Hard cap on concurrent in-flight work
/// - Deterministic rejection (no queuing)
/// - Bounded response times
/// - Semaphore-based concurrency control
pub trait WipGate {
    /// The type of permit returned by this gate
    type Permit: WipPermit;

    /// Try to acquire a WIP slot without blocking
    ///
    /// Returns a permit if capacity is available, or WipLimitReached if at capacity.
    /// This operation never blocks.
    fn try_acquire(&self) -> Result<Self::Permit, WipError>;

    /// Get the WIP limit (maximum concurrent work)
    fn limit(&self) -> usize;

    /// Get the current number of occupied WIP slots
    fn current(&self) -> usize;

    /// Get the number of available WIP slots
    fn available(&self) -> usize {
        self.limit().saturating_sub(self.current())
    }

    /// Check if the gate is at capacity
    fn is_at_capacity(&self) -> bool {
        self.current() >= self.limit()
    }
}

#[async_trait]
/// Async WIP gate for admission control
///
/// Async version of WipGate with support for async work execution.
pub trait AsyncWipGate: Send + Sync {
    /// The type of permit returned by this gate
    type Permit: WipPermit;

    /// Try to acquire a WIP slot without blocking
    ///
    /// Returns a permit if capacity is available, or WipLimitReached if at capacity.
    /// This operation never blocks.
    async fn try_acquire(&self) -> Result<Self::Permit, WipError>;

    /// Execute work within WIP limits
    ///
    /// Tries to acquire a permit, executes the work, and releases the permit.
    /// Returns WipLimitReached if no capacity is available.
    async fn execute<F, Fut, T>(&self, work: F) -> Result<T, WipError>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<T, WipError>> + Send,
        T: Send,
    {
        let _permit = self.try_acquire().await?;
        work().await
        // Permit auto-released on drop
    }

    /// Get the WIP limit (maximum concurrent work)
    fn limit(&self) -> usize;

    /// Get the current number of occupied WIP slots
    fn current(&self) -> usize;

    /// Get the number of available WIP slots
    fn available(&self) -> usize {
        self.limit().saturating_sub(self.current())
    }

    /// Check if the gate is at capacity
    fn is_at_capacity(&self) -> bool {
        self.current() >= self.limit()
    }
}
