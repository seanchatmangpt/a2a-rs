//! Deterministic Scheduler (Λ) for ordered execution
//!
//! This scheduler ensures deterministic, replay-safe execution by:
//! - Using stable sorting (epoch, priority class, station id, task id)
//! - Using only deterministic collections (BTreeMap, Vec)
//! - Enforcing WIP (work-in-progress) limits
//! - Fair scheduling across stations
//! - Deterministic concurrency ordering
//!
//! The scheduler guarantees that replaying the same sequence of events
//! produces identical observable sequences.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[cfg(feature = "tracing")]
use tracing::{debug, instrument};

/// Errors that can occur during scheduling operations
#[derive(Debug, Error, Clone)]
pub enum SchedulerError {
    #[error("WIP limit exceeded for station {station_id}: current={current}, limit={limit}")]
    WipLimitExceeded {
        station_id: String,
        current: usize,
        limit: usize,
    },

    #[error("Task not found: {task_id}")]
    TaskNotFound { task_id: String },

    #[error("Station not found: {station_id}")]
    StationNotFound { station_id: String },

    #[error("Invalid epoch: {epoch}")]
    InvalidEpoch { epoch: u64 },

    #[error("Duplicate task: {task_id}")]
    DuplicateTask { task_id: String },
}

/// Priority classes for task scheduling (ordered from highest to lowest)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PriorityClass {
    /// Critical system tasks (highest priority)
    Critical = 0,
    /// High priority user tasks
    High = 1,
    /// Normal priority tasks (default)
    Normal = 2,
    /// Low priority background tasks
    Low = 3,
    /// Lowest priority tasks
    Idle = 4,
}

impl Default for PriorityClass {
    fn default() -> Self {
        Self::Normal
    }
}

/// A scheduled task with all scheduling metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTask {
    /// Unique task identifier
    pub task_id: String,

    /// Station identifier where this task executes
    pub station_id: String,

    /// Logical clock epoch for ordering
    pub epoch: u64,

    /// Priority class for scheduling
    pub priority: PriorityClass,

    /// Optional context for the task
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,

    /// Task metadata (application-specific)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

impl ScheduledTask {
    /// Create a new scheduled task
    pub fn new(task_id: String, station_id: String, epoch: u64, priority: PriorityClass) -> Self {
        Self {
            task_id,
            station_id,
            epoch,
            priority,
            context_id: None,
            metadata: None,
        }
    }

    /// Create a new scheduled task with context
    pub fn with_context(
        task_id: String,
        station_id: String,
        epoch: u64,
        priority: PriorityClass,
        context_id: String,
    ) -> Self {
        Self {
            task_id,
            station_id,
            epoch,
            priority,
            context_id: Some(context_id),
            metadata: None,
        }
    }

    /// Get the scheduling key for deterministic ordering
    /// Order: (epoch, priority, station_id, task_id)
    fn scheduling_key(&self) -> (u64, PriorityClass, &str, &str) {
        (self.epoch, self.priority, &self.station_id, &self.task_id)
    }
}

/// Station-specific WIP tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StationState {
    /// Station identifier
    station_id: String,

    /// Maximum concurrent tasks for this station
    wip_limit: usize,

    /// Currently executing tasks (sorted for determinism)
    active_tasks: Vec<String>,
}

impl StationState {
    fn new(station_id: String, wip_limit: usize) -> Self {
        Self {
            station_id,
            wip_limit,
            active_tasks: Vec::new(),
        }
    }

    fn can_accept_task(&self) -> bool {
        self.active_tasks.len() < self.wip_limit
    }

    fn add_task(&mut self, task_id: String) -> Result<(), SchedulerError> {
        if !self.can_accept_task() {
            return Err(SchedulerError::WipLimitExceeded {
                station_id: self.station_id.clone(),
                current: self.active_tasks.len(),
                limit: self.wip_limit,
            });
        }
        self.active_tasks.push(task_id);
        // Keep sorted for determinism
        self.active_tasks.sort();
        Ok(())
    }

    fn remove_task(&mut self, task_id: &str) -> bool {
        if let Some(pos) = self.active_tasks.iter().position(|id| id == task_id) {
            self.active_tasks.remove(pos);
            true
        } else {
            false
        }
    }

    fn task_count(&self) -> usize {
        self.active_tasks.len()
    }

    fn available_slots(&self) -> usize {
        self.wip_limit.saturating_sub(self.active_tasks.len())
    }
}

/// Deterministic Scheduler (Λ)
///
/// Ensures deterministic task execution order through:
/// - Stable sorting by (epoch, priority, station_id, task_id)
/// - Deterministic iteration using BTreeMap and Vec
/// - WIP limits per station
/// - Fair round-robin scheduling across stations
/// - Reproducible concurrency ordering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scheduler {
    /// Pending tasks sorted deterministically
    /// Key: (epoch, priority, station_id, task_id)
    /// Using BTreeMap for deterministic iteration
    pending: BTreeMap<String, ScheduledTask>,

    /// Station states indexed by station_id (deterministic)
    stations: BTreeMap<String, StationState>,

    /// Default WIP limit for new stations
    default_wip_limit: usize,

    /// Global epoch counter
    current_epoch: u64,

    /// Last station scheduled (for fair round-robin)
    last_scheduled_station: Option<String>,
}

impl Scheduler {
    /// Create a new scheduler with default WIP limit
    pub fn new(default_wip_limit: usize) -> Self {
        Self {
            pending: BTreeMap::new(),
            stations: BTreeMap::new(),
            default_wip_limit,
            current_epoch: 0,
            last_scheduled_station: None,
        }
    }

    /// Register a station with a specific WIP limit
    #[cfg_attr(feature = "tracing", instrument(skip(self)))]
    pub fn register_station(&mut self, station_id: String, wip_limit: usize) {
        #[cfg(feature = "tracing")]
        debug!(
            "Registering station {} with WIP limit {}",
            station_id, wip_limit
        );

        self.stations
            .insert(station_id.clone(), StationState::new(station_id, wip_limit));
    }

    /// Ensure a station exists (creates with default WIP if not)
    fn ensure_station(&mut self, station_id: &str) {
        if !self.stations.contains_key(station_id) {
            self.stations.insert(
                station_id.to_string(),
                StationState::new(station_id.to_string(), self.default_wip_limit),
            );
        }
    }

    /// Submit a task to the scheduler
    #[cfg_attr(feature = "tracing", instrument(skip(self, task)))]
    pub fn submit(&mut self, task: ScheduledTask) -> Result<(), SchedulerError> {
        #[cfg(feature = "tracing")]
        debug!(
            "Submitting task {} to station {} with priority {:?}",
            task.task_id, task.station_id, task.priority
        );

        // Check for duplicate
        if self.pending.contains_key(&task.task_id) {
            return Err(SchedulerError::DuplicateTask {
                task_id: task.task_id.clone(),
            });
        }

        // Ensure station exists
        self.ensure_station(&task.station_id);

        // Add to pending queue with deterministic key
        self.pending.insert(task.task_id.clone(), task);

        Ok(())
    }

    /// Get the next task to execute (deterministic selection)
    ///
    /// Selection algorithm:
    /// 1. Sort all pending tasks by (epoch, priority, station_id, task_id)
    /// 2. Apply fair round-robin across stations (skip stations that were just scheduled)
    /// 3. Check WIP limits
    /// 4. Return first eligible task
    #[cfg_attr(feature = "tracing", instrument(skip(self)))]
    pub fn next(&mut self) -> Option<ScheduledTask> {
        if self.pending.is_empty() {
            return None;
        }

        // Collect and sort tasks deterministically
        let mut sorted_tasks: Vec<_> = self.pending.values().collect();
        sorted_tasks.sort_by_key(|t| t.scheduling_key());

        #[cfg(feature = "tracing")]
        debug!("Evaluating {} pending tasks", sorted_tasks.len());

        // Apply fair scheduling: prefer stations we haven't just scheduled
        let mut selected_task: Option<&ScheduledTask> = None;

        // First pass: try to find a task from a different station than last scheduled
        if let Some(ref last_station) = self.last_scheduled_station {
            for task in &sorted_tasks {
                if task.station_id != *last_station {
                    if let Some(station) = self.stations.get(&task.station_id) {
                        if station.can_accept_task() {
                            selected_task = Some(task);
                            break;
                        }
                    }
                }
            }
        }

        // Second pass: if no eligible task from different station, take first eligible
        if selected_task.is_none() {
            for task in &sorted_tasks {
                if let Some(station) = self.stations.get(&task.station_id) {
                    if station.can_accept_task() {
                        selected_task = Some(task);
                        break;
                    }
                }
            }
        }

        // Execute selected task
        if let Some(task) = selected_task {
            let task_id = task.task_id.clone();
            let station_id = task.station_id.clone();

            // Remove from pending
            if let Some(task) = self.pending.remove(&task_id) {
                // Add to station's active tasks
                if let Some(station) = self.stations.get_mut(&station_id) {
                    if station.add_task(task_id.clone()).is_ok() {
                        // Update last scheduled station for fairness
                        self.last_scheduled_station = Some(station_id.clone());

                        #[cfg(feature = "tracing")]
                        debug!(
                            "Scheduled task {} on station {} (epoch={}, priority={:?})",
                            task.task_id, task.station_id, task.epoch, task.priority
                        );

                        return Some(task);
                    }
                }

                // If we couldn't add to station, put back in pending
                self.pending.insert(task_id, task);
            }
        }

        None
    }

    /// Mark a task as completed (removes from active set)
    #[cfg_attr(feature = "tracing", instrument(skip(self)))]
    pub fn complete(&mut self, task_id: &str, station_id: &str) -> Result<(), SchedulerError> {
        #[cfg(feature = "tracing")]
        debug!("Completing task {} on station {}", task_id, station_id);

        let station =
            self.stations
                .get_mut(station_id)
                .ok_or_else(|| SchedulerError::StationNotFound {
                    station_id: station_id.to_string(),
                })?;

        if station.remove_task(task_id) {
            Ok(())
        } else {
            Err(SchedulerError::TaskNotFound {
                task_id: task_id.to_string(),
            })
        }
    }

    /// Cancel a pending task (before it starts executing)
    #[cfg_attr(feature = "tracing", instrument(skip(self)))]
    pub fn cancel(&mut self, task_id: &str) -> Result<ScheduledTask, SchedulerError> {
        #[cfg(feature = "tracing")]
        debug!("Cancelling task {}", task_id);

        self.pending
            .remove(task_id)
            .ok_or_else(|| SchedulerError::TaskNotFound {
                task_id: task_id.to_string(),
            })
    }

    /// Get current pending task count
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Get active task count for a station
    pub fn station_active_count(&self, station_id: &str) -> Option<usize> {
        self.stations.get(station_id).map(|s| s.task_count())
    }

    /// Get available slots for a station
    pub fn station_available_slots(&self, station_id: &str) -> Option<usize> {
        self.stations.get(station_id).map(|s| s.available_slots())
    }

    /// Get list of all pending task IDs (deterministic order)
    pub fn pending_tasks(&self) -> Vec<String> {
        let mut tasks: Vec<_> = self.pending.values().collect();
        tasks.sort_by_key(|t| t.scheduling_key());
        tasks.iter().map(|t| t.task_id.clone()).collect()
    }

    /// Get list of active task IDs for a station (deterministic order)
    pub fn station_active_tasks(&self, station_id: &str) -> Option<Vec<String>> {
        self.stations
            .get(station_id)
            .map(|s| s.active_tasks.clone())
    }

    /// Advance to next epoch
    pub fn advance_epoch(&mut self) -> u64 {
        self.current_epoch += 1;
        self.current_epoch
    }

    /// Get current epoch
    pub fn current_epoch(&self) -> u64 {
        self.current_epoch
    }

    /// Get list of all registered station IDs (deterministic order)
    pub fn stations(&self) -> Vec<String> {
        self.stations.keys().cloned().collect()
    }

    /// Clear all state (useful for testing)
    pub fn clear(&mut self) {
        self.pending.clear();
        for station in self.stations.values_mut() {
            station.active_tasks.clear();
        }
        self.last_scheduled_station = None;
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_deterministic_ordering() {
        let mut scheduler = Scheduler::new(5);

        // Submit tasks in random order
        scheduler
            .submit(ScheduledTask::new(
                "task-3".to_string(),
                "station-a".to_string(),
                1,
                PriorityClass::Normal,
            ))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new(
                "task-1".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::High,
            ))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new(
                "task-2".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::Normal,
            ))
            .unwrap();

        // Should be sorted by (epoch, priority, station_id, task_id)
        let t1 = scheduler.next().unwrap();
        assert_eq!(t1.task_id, "task-1"); // epoch=0, priority=High

        let t2 = scheduler.next().unwrap();
        assert_eq!(t2.task_id, "task-2"); // epoch=0, priority=Normal

        let t3 = scheduler.next().unwrap();
        assert_eq!(t3.task_id, "task-3"); // epoch=1
    }

    #[test]
    fn test_wip_limits() {
        let mut scheduler = Scheduler::new(2);

        // Submit 3 tasks to same station
        scheduler
            .submit(ScheduledTask::new(
                "task-1".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::Normal,
            ))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new(
                "task-2".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::Normal,
            ))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new(
                "task-3".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::Normal,
            ))
            .unwrap();

        // Should get first two tasks
        let t1 = scheduler.next().unwrap();
        let t2 = scheduler.next().unwrap();

        // Third task should be blocked by WIP limit
        assert!(scheduler.next().is_none());

        // Complete one task
        scheduler.complete(&t1.task_id, &t1.station_id).unwrap();

        // Now third task should be available
        let t3 = scheduler.next().unwrap();
        assert_eq!(t3.task_id, "task-3");
    }

    #[test]
    fn test_fair_scheduling() {
        let mut scheduler = Scheduler::new(5);

        // Submit tasks to different stations with same priority
        scheduler
            .submit(ScheduledTask::new(
                "task-a1".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::Normal,
            ))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new(
                "task-b1".to_string(),
                "station-b".to_string(),
                0,
                PriorityClass::Normal,
            ))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new(
                "task-a2".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::Normal,
            ))
            .unwrap();

        // First task (alphabetically station-a)
        let t1 = scheduler.next().unwrap();
        assert_eq!(t1.station_id, "station-a");

        // Second task should prefer different station (station-b)
        let t2 = scheduler.next().unwrap();
        assert_eq!(t2.station_id, "station-b");

        // Third task back to station-a
        let t3 = scheduler.next().unwrap();
        assert_eq!(t3.station_id, "station-a");
    }

    #[test]
    fn test_priority_ordering() {
        let mut scheduler = Scheduler::new(5);

        scheduler
            .submit(ScheduledTask::new(
                "low".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::Low,
            ))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new(
                "critical".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::Critical,
            ))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new(
                "normal".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::Normal,
            ))
            .unwrap();

        // Should execute in priority order
        assert_eq!(scheduler.next().unwrap().task_id, "critical");
        assert_eq!(scheduler.next().unwrap().task_id, "normal");
        assert_eq!(scheduler.next().unwrap().task_id, "low");
    }

    #[test]
    fn test_epoch_ordering() {
        let mut scheduler = Scheduler::new(5);

        scheduler
            .submit(ScheduledTask::new(
                "epoch-2".to_string(),
                "station-a".to_string(),
                2,
                PriorityClass::Normal,
            ))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new(
                "epoch-0".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::Normal,
            ))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new(
                "epoch-1".to_string(),
                "station-a".to_string(),
                1,
                PriorityClass::Normal,
            ))
            .unwrap();

        // Should execute in epoch order
        assert_eq!(scheduler.next().unwrap().task_id, "epoch-0");
        assert_eq!(scheduler.next().unwrap().task_id, "epoch-1");
        assert_eq!(scheduler.next().unwrap().task_id, "epoch-2");
    }

    #[test]
    fn test_cancel_pending_task() {
        let mut scheduler = Scheduler::new(5);

        scheduler
            .submit(ScheduledTask::new(
                "task-1".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::Normal,
            ))
            .unwrap();

        assert_eq!(scheduler.pending_count(), 1);

        let cancelled = scheduler.cancel("task-1").unwrap();
        assert_eq!(cancelled.task_id, "task-1");
        assert_eq!(scheduler.pending_count(), 0);
    }

    #[test]
    fn test_duplicate_task_error() {
        let mut scheduler = Scheduler::new(5);

        scheduler
            .submit(ScheduledTask::new(
                "task-1".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::Normal,
            ))
            .unwrap();

        let result = scheduler.submit(ScheduledTask::new(
            "task-1".to_string(),
            "station-a".to_string(),
            0,
            PriorityClass::Normal,
        ));

        assert!(matches!(result, Err(SchedulerError::DuplicateTask { .. })));
    }

    #[test]
    fn test_station_metrics() {
        let mut scheduler = Scheduler::new(3);

        scheduler
            .submit(ScheduledTask::new(
                "task-1".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::Normal,
            ))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new(
                "task-2".to_string(),
                "station-a".to_string(),
                0,
                PriorityClass::Normal,
            ))
            .unwrap();

        scheduler.next(); // Start task-1
        scheduler.next(); // Start task-2

        assert_eq!(scheduler.station_active_count("station-a"), Some(2));
        assert_eq!(scheduler.station_available_slots("station-a"), Some(1));

        let tasks = scheduler.station_active_tasks("station-a").unwrap();
        assert_eq!(tasks.len(), 2);
        // Should be sorted
        assert!(tasks[0] < tasks[1]);
    }
}
