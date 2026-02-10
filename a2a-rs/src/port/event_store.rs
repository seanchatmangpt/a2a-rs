//! Event store port definition for event sourcing
//!
//! Defines the contract for an append-only event log that stores task lifecycle events.

use async_trait::async_trait;

use crate::domain::{A2AError, TaskLifecycleEvent, TaskSnapshot};

/// Trait for an append-only event store
///
/// The event store is responsible for:
/// - Appending events to an immutable log
/// - Retrieving events by task ID
/// - Supporting optimistic concurrency via version numbers
/// - Managing snapshots for performance optimization
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Append an event to the store
    ///
    /// # Arguments
    /// * `event` - The event to append
    ///
    /// # Returns
    /// The version number assigned to this event
    ///
    /// # Errors
    /// Returns `A2AError::ConcurrencyConflict` if the version doesn't match (optimistic locking)
    async fn append_event(&self, event: TaskLifecycleEvent) -> Result<i64, A2AError>;

    /// Get all events for a task, optionally starting from a version
    ///
    /// # Arguments
    /// * `task_id` - The ID of the task
    /// * `from_version` - Optional version to start from (exclusive)
    ///
    /// # Returns
    /// Vector of events in chronological order
    async fn get_events(
        &self,
        task_id: &str,
        from_version: Option<i64>,
    ) -> Result<Vec<TaskLifecycleEvent>, A2AError>;

    /// Get the latest snapshot for a task
    ///
    /// # Arguments
    /// * `task_id` - The ID of the task
    ///
    /// # Returns
    /// The most recent snapshot, if one exists
    async fn get_snapshot(&self, task_id: &str) -> Result<Option<TaskSnapshot>, A2AError>;

    /// Save a snapshot of a task's state
    ///
    /// # Arguments
    /// * `snapshot` - The snapshot to save
    ///
    /// # Returns
    /// Unit on success
    async fn save_snapshot(&self, snapshot: TaskSnapshot) -> Result<(), A2AError>;

    /// Get the current version number for a task
    ///
    /// # Arguments
    /// * `task_id` - The ID of the task
    ///
    /// # Returns
    /// The latest version number, or 0 if no events exist
    async fn get_version(&self, task_id: &str) -> Result<i64, A2AError>;

    /// Check if a task exists (has any events)
    ///
    /// # Arguments
    /// * `task_id` - The ID of the task
    ///
    /// # Returns
    /// True if the task has at least one event
    async fn task_exists(&self, task_id: &str) -> Result<bool, A2AError>;
}
