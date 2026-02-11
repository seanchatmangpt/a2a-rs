//! Kanban WIP gate implementation using tokio semaphore
//!
//! Provides deterministic admission control with:
//! - Hard WIP limit enforced by semaphore
//! - No queuing - immediate rejection when at capacity
//! - Bounded response times
//! - Thread-safe concurrent access

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::{
    domain::WipError,
    port::{AsyncWipGate, WipGate, WipPermit},
};

/// A permit representing one unit of WIP capacity backed by a semaphore permit
pub struct SemaphorePermit {
    #[allow(dead_code)]
    permit: tokio::sync::OwnedSemaphorePermit,
}

impl WipPermit for SemaphorePermit {
    fn release(self) {
        // Permit is automatically released when dropped
        drop(self);
    }
}

/// Kanban-style WIP gate using tokio semaphore
///
/// # Example
/// ```no_run
/// use osiris_edge::adapter::KanbanWipGate;
/// use osiris_edge::port::AsyncWipGate;
///
/// # async fn example() {
/// let gate = KanbanWipGate::new(5); // Allow max 5 concurrent work items
///
/// // Try to acquire a slot
/// match gate.try_acquire().await {
///     Ok(permit) => {
///         // Do work while holding permit
///         // Permit auto-released on drop
///     }
///     Err(e) => {
///         // WIP limit reached - emit refusal receipt
///         println!("Work rejected: {}", e);
///     }
/// }
/// # }
/// ```
#[derive(Debug)]
pub struct KanbanWipGate {
    semaphore: Arc<Semaphore>,
    limit: usize,
}

impl KanbanWipGate {
    /// Create a new Kanban WIP gate with the specified limit
    ///
    /// # Arguments
    /// * `limit` - Maximum number of concurrent work items (WIP limit)
    ///
    /// # Panics
    /// Panics if limit is 0
    pub fn new(limit: usize) -> Self {
        assert!(limit > 0, "WIP limit must be greater than 0");

        Self {
            semaphore: Arc::new(Semaphore::new(limit)),
            limit,
        }
    }

    /// Create a new gate with the specified limit, returning an error if invalid
    ///
    /// # Arguments
    /// * `limit` - Maximum number of concurrent work items (WIP limit)
    pub fn try_new(limit: usize) -> Result<Self, WipError> {
        if limit == 0 {
            return Err(WipError::ConfigurationError(
                "WIP limit must be greater than 0".to_string(),
            ));
        }

        Ok(Self {
            semaphore: Arc::new(Semaphore::new(limit)),
            limit,
        })
    }
}

impl Clone for KanbanWipGate {
    fn clone(&self) -> Self {
        Self {
            semaphore: Arc::clone(&self.semaphore),
            limit: self.limit,
        }
    }
}

impl WipGate for KanbanWipGate {
    type Permit = SemaphorePermit;

    fn try_acquire(&self) -> Result<Self::Permit, WipError> {
        // Try to acquire without blocking
        match Arc::clone(&self.semaphore).try_acquire_owned() {
            Ok(permit) => Ok(SemaphorePermit { permit }),
            Err(_) => Err(WipError::WipLimitReached {
                current: WipGate::current(self),
                limit: self.limit,
            }),
        }
    }

    fn limit(&self) -> usize {
        self.limit
    }

    fn current(&self) -> usize {
        self.limit - self.semaphore.available_permits()
    }
}

#[async_trait]
impl AsyncWipGate for KanbanWipGate {
    type Permit = SemaphorePermit;

    async fn try_acquire(&self) -> Result<Self::Permit, WipError> {
        // Use the sync version - no need to await since we're not blocking
        WipGate::try_acquire(self)
    }

    fn limit(&self) -> usize {
        self.limit
    }

    fn current(&self) -> usize {
        WipGate::current(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_gate() {
        let gate = KanbanWipGate::new(5);
        assert_eq!(gate.limit(), 5);
        assert_eq!(gate.current(), 0);
        assert_eq!(gate.available(), 5);
        assert!(!gate.is_at_capacity());
    }

    #[test]
    #[should_panic(expected = "WIP limit must be greater than 0")]
    fn test_new_gate_zero_limit() {
        KanbanWipGate::new(0);
    }

    #[test]
    fn test_try_new_gate_zero_limit() {
        let result = KanbanWipGate::try_new(0);
        assert!(result.is_err());
        match result {
            Err(WipError::ConfigurationError(msg)) => {
                assert!(msg.contains("WIP limit must be greater than 0"));
            }
            _ => panic!("Expected ConfigurationError"),
        }
    }

    #[test]
    fn test_acquire_and_release() {
        let gate = KanbanWipGate::new(2);

        // Acquire first permit
        let permit1 = gate.try_acquire().unwrap();
        assert_eq!(gate.current(), 1);
        assert_eq!(gate.available(), 1);
        assert!(!gate.is_at_capacity());

        // Acquire second permit
        let permit2 = gate.try_acquire().unwrap();
        assert_eq!(gate.current(), 2);
        assert_eq!(gate.available(), 0);
        assert!(gate.is_at_capacity());

        // Try to acquire third permit - should fail
        let result = gate.try_acquire();
        assert!(result.is_err());
        match result {
            Err(WipError::WipLimitReached { current, limit }) => {
                assert_eq!(current, 2);
                assert_eq!(limit, 2);
            }
            _ => panic!("Expected WipLimitReached error"),
        }

        // Release first permit
        drop(permit1);
        assert_eq!(gate.current(), 1);
        assert_eq!(gate.available(), 1);
        assert!(!gate.is_at_capacity());

        // Should be able to acquire again
        let _permit3 = gate.try_acquire().unwrap();
        assert_eq!(gate.current(), 2);
        assert!(gate.is_at_capacity());

        // Release all
        drop(permit2);
        drop(_permit3);
        assert_eq!(gate.current(), 0);
        assert_eq!(gate.available(), 2);
    }

    #[test]
    fn test_explicit_release() {
        let gate = KanbanWipGate::new(1);

        let permit = gate.try_acquire().unwrap();
        assert_eq!(gate.current(), 1);

        // Explicit release
        permit.release();
        assert_eq!(gate.current(), 0);

        // Should be able to acquire again
        let _permit2 = gate.try_acquire().unwrap();
        assert_eq!(gate.current(), 1);
    }

    #[tokio::test]
    async fn test_async_try_acquire() {
        let gate = KanbanWipGate::new(2);

        // Acquire permits
        let _permit1 = gate.try_acquire().await.unwrap();
        let _permit2 = gate.try_acquire().await.unwrap();

        // Third should fail
        let result = gate.try_acquire().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute() {
        let gate = KanbanWipGate::new(1);

        // Execute work within WIP limits
        let result = gate.execute(|| async { Ok::<i32, WipError>(42) }).await;
        assert_eq!(result.unwrap(), 42);

        // Gate should be available again
        assert_eq!(gate.current(), 0);
    }

    #[tokio::test]
    async fn test_execute_at_capacity() {
        let gate = KanbanWipGate::new(1);

        // Acquire the only permit
        let _permit = gate.try_acquire().await.unwrap();

        // Try to execute - should fail immediately
        let result = gate.execute(|| async { Ok::<i32, WipError>(42) }).await;

        assert!(result.is_err());
        match result {
            Err(WipError::WipLimitReached { current, limit }) => {
                assert_eq!(current, 1);
                assert_eq!(limit, 1);
            }
            _ => panic!("Expected WipLimitReached error"),
        }
    }

    #[tokio::test]
    async fn test_concurrent_work() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::time::{Duration, sleep};

        let gate = Arc::new(KanbanWipGate::new(2));
        let concurrent_count = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));

        let mut handles = vec![];

        // Spawn 5 tasks, but only 2 should run concurrently
        for _ in 0..5 {
            let gate = Arc::clone(&gate);
            let concurrent_count = Arc::clone(&concurrent_count);
            let max_concurrent = Arc::clone(&max_concurrent);

            let handle = tokio::spawn(async move {
                loop {
                    if let Ok(_permit) = gate.try_acquire().await {
                        // Increment concurrent count
                        let current = concurrent_count.fetch_add(1, Ordering::SeqCst) + 1;

                        // Update max
                        max_concurrent.fetch_max(current, Ordering::SeqCst);

                        // Simulate work
                        sleep(Duration::from_millis(10)).await;

                        // Decrement concurrent count
                        concurrent_count.fetch_sub(1, Ordering::SeqCst);

                        break;
                    } else {
                        // Wait a bit and retry
                        sleep(Duration::from_millis(1)).await;
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify we never exceeded the WIP limit
        let max = max_concurrent.load(Ordering::SeqCst);
        assert!(max <= 2, "Max concurrent work {} exceeded limit of 2", max);
    }

    #[test]
    fn test_clone() {
        let gate1 = KanbanWipGate::new(3);
        let gate2 = gate1.clone();

        // Both gates share the same semaphore
        let _permit1 = gate1.try_acquire().unwrap();
        assert_eq!(gate1.current(), 1);
        assert_eq!(gate2.current(), 1); // Same underlying semaphore

        let _permit2 = gate2.try_acquire().unwrap();
        assert_eq!(gate1.current(), 2);
        assert_eq!(gate2.current(), 2);
    }
}
