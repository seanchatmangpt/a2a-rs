//! Instrumented WIP gate with integrated analytics
//!
//! Wraps KanbanWipGate to automatically track work metrics and feed
//! the RealtimeAnalyticsEngine for live monitoring.

use async_trait::async_trait;
use std::future::Future;
use std::sync::Arc;
use uuid::Uuid;

use crate::adapter::{KanbanWipGate, RealtimeAnalyticsEngine};
use crate::domain::WipError;
use crate::port::{AnalyticsEngine, AsyncWipGate, WipGate, WipPermit};

/// Instrumented permit that tracks lifecycle automatically
pub struct InstrumentedPermit {
    work_id: Uuid,
    inner_permit: <KanbanWipGate as WipGate>::Permit,
    analytics: Arc<RealtimeAnalyticsEngine>,
}

impl WipPermit for InstrumentedPermit {
    fn release(self) {
        // Record completion before releasing
        let analytics = Arc::clone(&self.analytics);
        let work_id = self.work_id;
        tokio::spawn(async move {
            analytics.record_completion(work_id).await;
        });

        // Release inner permit
        self.inner_permit.release();
    }
}

impl Drop for InstrumentedPermit {
    fn drop(&mut self) {
        // Auto-release will also record completion
        let analytics = Arc::clone(&self.analytics);
        let work_id = self.work_id;
        tokio::spawn(async move {
            analytics.record_completion(work_id).await;
        });
    }
}

/// Instrumented WIP gate with automatic analytics tracking
///
/// Wraps KanbanWipGate and automatically:
/// - Records work arrivals on try_acquire attempts
/// - Records work starts on successful permit acquisition
/// - Records completions on permit release
/// - Records rejections on WIP limit reached
/// - Updates WIP state periodically
///
/// # Example
/// ```no_run
/// use osiris_edge::adapter::{InstrumentedWipGate, KanbanWipGate, RealtimeAnalyticsEngine};
/// use osiris_edge::port::{AnalyticsConfig, AsyncWipGate};
///
/// # async fn example() {
/// let gate = KanbanWipGate::new(5);
/// let analytics = RealtimeAnalyticsEngine::new(AnalyticsConfig::default());
/// let instrumented = InstrumentedWipGate::new(gate, analytics);
///
/// // All operations are automatically tracked
/// match instrumented.try_acquire_with_id(uuid::Uuid::new_v4(), "email").await {
///     Ok(permit) => {
///         // Work is now tracked
///         // Permit auto-releases and records completion on drop
///     }
///     Err(e) => {
///         // Rejection is tracked
///     }
/// }
/// # }
/// ```
#[derive(Clone)]
pub struct InstrumentedWipGate {
    inner: KanbanWipGate,
    analytics: Arc<RealtimeAnalyticsEngine>,
}

impl InstrumentedWipGate {
    /// Create a new instrumented WIP gate
    ///
    /// # Arguments
    /// * `gate` - The inner Kanban WIP gate
    /// * `analytics` - The analytics engine to report metrics to
    pub fn new(gate: KanbanWipGate, analytics: RealtimeAnalyticsEngine) -> Self {
        let instrumented = Self {
            inner: gate,
            analytics: Arc::new(analytics),
        };

        // Start periodic WIP state updates
        instrumented.start_wip_state_updater();

        instrumented
    }

    /// Try to acquire with explicit work ID and type (for tracking)
    ///
    /// # Arguments
    /// * `work_id` - Unique work identifier
    /// * `work_type` - Type of work for categorization
    pub async fn try_acquire_with_id(
        &self,
        work_id: Uuid,
        work_type: &str,
    ) -> Result<InstrumentedPermit, WipError> {
        // Record arrival
        self.analytics
            .record_arrival(work_id, work_type.to_string())
            .await;

        // Try to acquire from inner gate
        match self.inner.try_acquire() {
            Ok(permit) => {
                // Record start (permit acquired)
                self.analytics.record_start(work_id).await;

                Ok(InstrumentedPermit {
                    work_id,
                    inner_permit: permit,
                    analytics: Arc::clone(&self.analytics),
                })
            }
            Err(e) => {
                // Record rejection
                self.analytics.record_rejection(work_type.to_string()).await;
                Err(e)
            }
        }
    }

    /// Execute work with automatic tracking
    ///
    /// # Arguments
    /// * `work_id` - Unique work identifier
    /// * `work_type` - Type of work for categorization
    /// * `work` - The work function to execute
    pub async fn execute_with_id<F, Fut, T>(
        &self,
        work_id: Uuid,
        work_type: &str,
        work: F,
    ) -> Result<T, WipError>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<T, WipError>> + Send,
        T: Send,
    {
        let _permit = self.try_acquire_with_id(work_id, work_type).await?;
        work().await
        // Permit auto-released on drop
    }

    /// Start background task to periodically update WIP state
    fn start_wip_state_updater(&self) {
        let inner = self.inner.clone();
        let analytics = Arc::clone(&self.analytics);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

            loop {
                interval.tick().await;

                let current_wip = inner.current();
                let wip_limit = inner.limit();

                // For now, we don't track individual IDs in the gate
                // In production, you'd maintain a separate registry of in-progress work IDs
                let in_progress = vec![]; // TODO: Track actual in-progress IDs

                analytics
                    .update_wip_state(current_wip, wip_limit, in_progress)
                    .await;
            }
        });
    }

    /// Get reference to the analytics engine
    pub fn analytics(&self) -> &RealtimeAnalyticsEngine {
        &self.analytics
    }
}

impl WipGate for InstrumentedWipGate {
    type Permit = InstrumentedPermit;

    fn try_acquire(&self) -> Result<Self::Permit, WipError> {
        // Generate a work ID since caller didn't provide one
        let work_id = Uuid::new_v4();

        // We can't use async in sync context, so we spawn
        let analytics = Arc::clone(&self.analytics);
        tokio::spawn(async move {
            analytics
                .record_arrival(work_id, "unknown".to_string())
                .await;
        });

        match self.inner.try_acquire() {
            Ok(permit) => {
                let analytics = Arc::clone(&self.analytics);
                tokio::spawn(async move {
                    analytics.record_start(work_id).await;
                });

                Ok(InstrumentedPermit {
                    work_id,
                    inner_permit: permit,
                    analytics: Arc::clone(&self.analytics),
                })
            }
            Err(e) => {
                let analytics = Arc::clone(&self.analytics);
                tokio::spawn(async move {
                    analytics.record_rejection("unknown".to_string()).await;
                });
                Err(e)
            }
        }
    }

    fn limit(&self) -> usize {
        self.inner.limit()
    }

    fn current(&self) -> usize {
        self.inner.current()
    }
}

#[async_trait]
impl AsyncWipGate for InstrumentedWipGate {
    type Permit = InstrumentedPermit;

    async fn try_acquire(&self) -> Result<Self::Permit, WipError> {
        // Generate a work ID since caller didn't provide one
        let work_id = Uuid::new_v4();

        // Use async version
        self.try_acquire_with_id(work_id, "unknown").await
    }

    fn limit(&self) -> usize {
        self.inner.limit()
    }

    fn current(&self) -> usize {
        self.inner.current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port::AnalyticsConfig;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn test_instrumented_acquire_and_release() {
        let gate = KanbanWipGate::new(2);
        let analytics = RealtimeAnalyticsEngine::new(AnalyticsConfig::default());
        let instrumented = InstrumentedWipGate::new(gate, analytics.clone());

        let work_id = Uuid::new_v4();

        // Acquire permit
        let permit = instrumented
            .try_acquire_with_id(work_id, "test_work")
            .await
            .unwrap();

        // Wait a bit for metrics to be recorded
        sleep(Duration::from_millis(50)).await;

        // Check metrics were recorded
        let metrics = instrumented
            .analytics()
            .get_work_metrics(&work_id)
            .await
            .unwrap();
        assert!(metrics.started_at.is_some());

        // Release permit
        drop(permit);

        // Wait for completion to be recorded
        sleep(Duration::from_millis(50)).await;

        let metrics = instrumented
            .analytics()
            .get_work_metrics(&work_id)
            .await
            .unwrap();
        assert!(metrics.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_instrumented_rejection_tracking() {
        let gate = KanbanWipGate::new(1);
        let analytics = RealtimeAnalyticsEngine::new(AnalyticsConfig::default());
        let instrumented = InstrumentedWipGate::new(gate, analytics.clone());

        // Fill capacity
        let _permit1 = instrumented
            .try_acquire_with_id(Uuid::new_v4(), "work1")
            .await
            .unwrap();

        // Try to acquire when at capacity
        let result = instrumented
            .try_acquire_with_id(Uuid::new_v4(), "work2")
            .await;

        assert!(result.is_err());

        // Wait for rejection to be recorded
        sleep(Duration::from_millis(50)).await;

        let snapshot = instrumented.analytics().get_snapshot().await;
        assert_eq!(snapshot.total_rejections, 1);
    }

    #[tokio::test]
    async fn test_instrumented_execute() {
        let gate = KanbanWipGate::new(5);
        let analytics = RealtimeAnalyticsEngine::new(AnalyticsConfig::default());
        let instrumented = InstrumentedWipGate::new(gate, analytics.clone());

        let work_id = Uuid::new_v4();

        // Execute work
        let result = instrumented
            .execute_with_id(work_id, "computation", || async {
                sleep(Duration::from_millis(10)).await;
                Ok::<i32, WipError>(42)
            })
            .await;

        assert_eq!(result.unwrap(), 42);

        // Wait for metrics
        sleep(Duration::from_millis(50)).await;

        let metrics = instrumented
            .analytics()
            .get_work_metrics(&work_id)
            .await
            .unwrap();
        assert!(metrics.completed_at.is_some());
        assert!(metrics.cycle_time_ms.unwrap() >= 10);
    }

    #[tokio::test]
    async fn test_wip_state_updates() {
        let gate = KanbanWipGate::new(5);
        let analytics = RealtimeAnalyticsEngine::new(AnalyticsConfig::default());
        let instrumented = InstrumentedWipGate::new(gate, analytics.clone());

        // Acquire some permits
        let _p1 = instrumented
            .try_acquire_with_id(Uuid::new_v4(), "work1")
            .await
            .unwrap();
        let _p2 = instrumented
            .try_acquire_with_id(Uuid::new_v4(), "work2")
            .await
            .unwrap();

        // Wait for WIP state update (happens every 1 second)
        sleep(Duration::from_millis(1200)).await;

        let snapshot = instrumented.analytics().get_snapshot().await;
        assert_eq!(snapshot.wip_snapshot.current_wip, 2);
        assert_eq!(snapshot.wip_snapshot.wip_limit, 5);
    }
}
