//! Comprehensive tests for the TPS (Toyota Production System) Coordinator Service
//!
//! Tests Lean manufacturing patterns:
//! - Kanban (WIP limits, pull-based scheduling)
//! - Andon (real-time status monitoring with GREEN/YELLOW/RED)
//! - Jidoka (automatic stopping on abnormalities)
//! - Heijunka (level loading for smooth workflow)
//! - Takt Time (rhythm-based scheduling)

#![cfg(feature = "server")]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use a2a_rs::domain::{A2AError, Task, TaskState};
use a2a_rs::port::AsyncTaskManager;
use a2a_rs::services::coordinator::{
    AndonStatus, CoordinatorConfig, HeijunkaScheduler, JidokaGate, Station, TaktTime,
    TpsCoordinator,
};
use async_trait::async_trait;
use tokio::sync::RwLock;

// =============================================================================
// Mock Task Manager for Testing
// =============================================================================

/// In-memory mock task manager for isolated testing
#[derive(Debug, Clone)]
struct MockTaskManager {
    tasks: Arc<RwLock<HashMap<String, Task>>>,
    task_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl MockTaskManager {
    fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            task_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Add a task directly to storage (for test setup)
    async fn add_task(&self, task: Task) {
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.id.clone(), task);
    }

    /// Get the number of tasks in storage
    async fn task_count(&self) -> usize {
        self.task_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Get the task count as a cloneable Arc reference
    fn get_task_count_counter(&self) -> Arc<std::sync::atomic::AtomicUsize> {
        Arc::clone(&self.task_count)
    }

    /// Check if a task with given ID exists
    async fn has_task(&self, task_id: &str) -> bool {
        let tasks = self.tasks.read().await;
        tasks.contains_key(task_id)
    }
}

impl Default for MockTaskManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AsyncTaskManager for MockTaskManager {
    async fn create_task<'a>(
        &self,
        task_id: &'a str,
        context_id: &'a str,
    ) -> Result<Task, A2AError> {
        let task = Task::builder()
            .id(task_id.to_string())
            .context_id(context_id.to_string())
            .status(a2a_rs::domain::TaskStatus {
                state: TaskState::Submitted,
                message: None,
                timestamp: Some(chrono::Utc::now()),
            })
            .build();

        let mut tasks = self.tasks.write().await;
        tasks.insert(task_id.to_string(), task.clone());

        // Increment task count
        self.task_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        Ok(task)
    }

    async fn get_task<'a>(
        &self,
        task_id: &'a str,
        _history_length: Option<u32>,
    ) -> Result<Task, A2AError> {
        let tasks = self.tasks.read().await;
        tasks
            .get(task_id)
            .cloned()
            .ok_or_else(|| A2AError::TaskNotFound(format!("Task not found: {}", task_id)))
    }

    async fn update_task_status<'a>(
        &self,
        task_id: &'a str,
        state: TaskState,
        _message: Option<a2a_rs::domain::Message>,
    ) -> Result<Task, A2AError> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| A2AError::TaskNotFound(format!("Task not found: {}", task_id)))?;

        task.status.state = state.clone();
        task.status.timestamp = Some(chrono::Utc::now());

        Ok(task.clone())
    }

    async fn cancel_task<'a>(&self, task_id: &'a str) -> Result<Task, A2AError> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| A2AError::TaskNotFound(format!("Task not found: {}", task_id)))?;

        task.status.state = TaskState::Canceled;
        task.status.timestamp = Some(chrono::Utc::now());

        Ok(task.clone())
    }

    async fn task_exists<'a>(&self, task_id: &'a str) -> Result<bool, A2AError> {
        let tasks = self.tasks.read().await;
        Ok(tasks.contains_key(task_id))
    }
}

// =============================================================================
// Test Helper Functions
// =============================================================================

/// Create a default coordinator config for testing
fn create_test_config() -> CoordinatorConfig {
    CoordinatorConfig::builder()
        .stations(vec![
            Station::new("submitted", 5),
            Station::new("working", 3),
            Station::new("review", 2),
        ])
        .andon_yellow_threshold(0.7)
        .andon_red_threshold(0.9)
        .takt_time_seconds(60.0)
        .enable_jidoka(true)
        .jidoka_defect_threshold(0.1)
        .heijunka_period_seconds(300.0)
        .heijunka_target_throughput(10)
        .max_queue_size(1000)
        .build()
}

/// Create a minimal coordinator config for faster tests
fn create_minimal_config() -> CoordinatorConfig {
    CoordinatorConfig::builder()
        .stations(vec![Station::new("submitted", 2), Station::new("working", 1)])
        .andon_yellow_threshold(0.7)
        .andon_red_threshold(0.9)
        .heijunka_period_seconds(60.0) // Short period for testing
        .heijunka_target_throughput(5)
        .max_queue_size(100)
        .metrics_interval_seconds(10.0)
        .build()
}

// =============================================================================
// Initialization Tests
// =============================================================================

#[tokio::test]
async fn test_coordinator_creation() {
    let config = create_test_config();
    let task_manager = MockTaskManager::new();

    let coordinator = TpsCoordinator::new(config, task_manager);

    // Verify coordinator was created successfully
    let metrics = coordinator.get_metrics().await.unwrap();
    assert_eq!(metrics.total_processed, 0);
    assert_eq!(metrics.current_wip, 0);
    assert_eq!(metrics.andon_status, AndonStatus::Green);

    // Verify jidoka gate is not halted
    let jidoka = coordinator.get_jidoka_gate().await.unwrap();
    assert!(!jidoka.is_halted);

    // Verify andon status is green
    let andon = coordinator.get_andon_status().await.unwrap();
    assert_eq!(andon, AndonStatus::Green);
}

#[tokio::test]
async fn test_station_registration() {
    let config = create_test_config();
    let task_manager = MockTaskManager::new();
    let coordinator = TpsCoordinator::new(config, task_manager);

    // Start the coordinator to initialize state
    coordinator.start().await.unwrap();

    // Verify stations are registered by checking metrics
    let metrics = coordinator.get_metrics().await.unwrap();

    // Should have metrics for 3 stations
    assert_eq!(metrics.station_metrics.len(), 3);

    // Verify each station has correct WIP limits
    let submitted_metrics = metrics.station_metrics.get("submitted").unwrap();
    assert_eq!(submitted_metrics.wip_limit, 5);
    assert_eq!(submitted_metrics.current_wip, 0);
    assert_eq!(submitted_metrics.utilization, 0.0);

    let working_metrics = metrics.station_metrics.get("working").unwrap();
    assert_eq!(working_metrics.wip_limit, 3);
    assert_eq!(working_metrics.current_wip, 0);

    let review_metrics = metrics.station_metrics.get("review").unwrap();
    assert_eq!(review_metrics.wip_limit, 2);
    assert_eq!(review_metrics.current_wip, 0);

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_coordinator_start_stop() {
    let config = create_minimal_config();
    let task_manager = MockTaskManager::new();
    let coordinator = TpsCoordinator::new(config, task_manager);

    // Start the coordinator
    coordinator.start().await.unwrap();

    // Give background tasks time to spawn
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Stop the coordinator
    coordinator.stop().await.unwrap();

    // Verify coordinator is still functional after restart
    coordinator.start().await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    coordinator.stop().await.unwrap();
}

// =============================================================================
// Work Packet Admission Tests
// =============================================================================

#[tokio::test]
async fn test_work_packet_admission() {
    let config = create_minimal_config();
    let task_manager = MockTaskManager::new();
    let task_counter = task_manager.get_task_count_counter();
    let coordinator = TpsCoordinator::new(config.clone(), task_manager);

    coordinator.start().await.unwrap();

    // Submit a task - should be admitted to first station
    let task = coordinator
        .submit_task("task-1", "context-1", 0)
        .await
        .unwrap();

    assert_eq!(task.id, "task-1");
    assert_eq!(task.context_id, "context-1");
    assert_eq!(task.status.state, TaskState::Submitted);

    // Submit multiple tasks
    for i in 2..=5 {
        coordinator
            .submit_task(&format!("task-{}", i), "context-1", 0)
            .await
            .unwrap();
    }

    // Verify all tasks were created via counter
    assert_eq!(task_counter.load(std::sync::atomic::Ordering::SeqCst), 5);

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_wip_limit_enforcement() {
    let config = CoordinatorConfig::builder()
        .stations(vec![Station::new("station1", 2), Station::new("station2", 1)])
        .heijunka_period_seconds(60.0)
        .heijunka_target_throughput(10)
        .max_queue_size(100)
        .build();

    let task_manager = MockTaskManager::new();
    let coordinator = TpsCoordinator::new(config, task_manager);

    coordinator.start().await.unwrap();

    // Submit 3 tasks to the first station (WIP limit = 2, but queue should accept them)
    coordinator
        .submit_task("task-1", "context-1", 0)
        .await
        .unwrap();
    coordinator
        .submit_task("task-2", "context-1", 0)
        .await
        .unwrap();
    coordinator
        .submit_task("task-3", "context-1", 0)
        .await
        .unwrap();

    // Pull tasks from first station up to WIP limit
    let task1 = coordinator.pull_task("station1").await.unwrap().unwrap();
    assert_eq!(task1.id, "task-1");

    let task2 = coordinator.pull_task("station1").await.unwrap().unwrap();
    assert_eq!(task2.id, "task-2");

    // Third pull should return None because WIP limit is reached
    let task3 = coordinator.pull_task("station1").await.unwrap();
    assert!(task3.is_none(), "Should not pull task beyond WIP limit");

    // Complete one task to free up capacity
    coordinator
        .complete_task_at_station("task-1", "station1", true)
        .await
        .unwrap();

    // Now we should be able to pull the third task
    let task3 = coordinator.pull_task("station1").await.unwrap().unwrap();
    assert_eq!(task3.id, "task-3");

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_jidoka_mode_transitions() {
    let config = CoordinatorConfig::builder()
        .stations(vec![Station::new("station1", 5)])
        .enable_jidoka(true)
        .jidoka_defect_threshold(0.1)
        .heijunka_period_seconds(60.0)
        .heijunka_target_throughput(10)
        .build();

    let task_manager = MockTaskManager::new();
    let coordinator = TpsCoordinator::new(config, task_manager);

    coordinator.start().await.unwrap();

    // Initially, jidoka gate should not be halted
    let jidoka = coordinator.get_jidoka_gate().await.unwrap();
    assert!(!jidoka.is_halted);

    // Submit and complete some successful tasks
    coordinator
        .submit_task("task-1", "context-1", 0)
        .await
        .unwrap();
    coordinator
        .complete_task_at_station("task-1", "station1", true)
        .await
        .unwrap();

    // Still not halted
    let jidoka = coordinator.get_jidoka_gate().await.unwrap();
    assert!(!jidoka.is_halted);

    // Trigger manual jidoka halt
    coordinator
        .trigger_jidoka_halt("Manual test halt".to_string())
        .await
        .unwrap();

    let jidoka = coordinator.get_jidoka_gate().await.unwrap();
    assert!(jidoka.is_halted);
    assert_eq!(jidoka.halt_reason, Some("Manual test halt".to_string()));

    // Try to submit a task while halted - should fail
    let result = coordinator.submit_task("task-2", "context-1", 0).await;
    assert!(result.is_err());

    // Resume from jidoka
    coordinator.resume_from_jidoka().await.unwrap();

    let jidoka = coordinator.get_jidoka_gate().await.unwrap();
    assert!(!jidoka.is_halted);
    assert!(jidoka.halt_reason.is_none());

    // Now submission should work again
    coordinator
        .submit_task("task-2", "context-1", 0)
        .await
        .unwrap();

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_jidoka_defect_rate_trigger() {
    let config = CoordinatorConfig::builder()
        .stations(vec![Station::new("station1", 10), Station::new("station2", 10)])
        .enable_jidoka(true)
        .jidoka_defect_threshold(0.2) // 20% defect threshold
        .heijunka_period_seconds(60.0)
        .heijunka_target_throughput(10)
        .metrics_interval_seconds(0.5)
        .build();

    let task_manager = MockTaskManager::new();
    let coordinator = TpsCoordinator::new(config, task_manager);

    coordinator.start().await.unwrap();

    // Submit 10 tasks
    for i in 1..=10 {
        coordinator
            .submit_task(&format!("task-{}", i), "context-1", 0)
            .await
            .unwrap();
    }

    // Pull and complete 7 successful tasks
    for _i in 1..=7 {
        let task = coordinator.pull_task("station1").await.unwrap().unwrap();
        coordinator
            .complete_task_at_station(&task.id, "station1", true)
            .await
            .unwrap();
    }

    // Pull and fail 3 tasks (30% defect rate)
    for _i in 8..=10 {
        let task = coordinator.pull_task("station1").await.unwrap().unwrap();
        coordinator
            .complete_task_at_station(&task.id, "station1", false)
            .await
            .unwrap();
    }

    // Wait for metrics to update
    tokio::time::sleep(Duration::from_secs(11)).await;

    // Check metrics - defect rate should be 3/10 = 30%
    let metrics = coordinator.get_metrics().await.unwrap();
    assert_eq!(metrics.total_processed, 10);
    assert_eq!(metrics.total_failed, 3);

    // The check_jidoka is only called during task completion
    // To trigger jidoka based on the updated metrics, we need to fail another task
    // Let's submit and fail another task to trigger the check again
    coordinator
        .submit_task("task-11", "context-1", 0)
        .await
        .unwrap();
    let task = coordinator.pull_task("station1").await.unwrap().unwrap();
    coordinator
        .complete_task_at_station(&task.id, "station1", false)
        .await
        .unwrap();

    // Now check jidoka gate - should be halted due to defect rate
    let jidoka = coordinator.get_jidoka_gate().await.unwrap();
    assert!(jidoka.is_halted);
    assert!(jidoka
        .halt_reason
        .as_ref()
        .unwrap()
        .contains("Defect rate"));

    coordinator.stop().await.unwrap();
}

// =============================================================================
// Andon System Tests
// =============================================================================

#[tokio::test]
async fn test_andon_status_calculation() {
    let config = CoordinatorConfig::builder()
        .stations(vec![
            Station::new("station1", 10),
            Station::new("station2", 10),
        ])
        .andon_yellow_threshold(0.7)
        .andon_red_threshold(0.9)
        .heijunka_period_seconds(60.0)
        .heijunka_target_throughput(10)
        .metrics_interval_seconds(0.5) // Update frequently for testing
        .build();

    let task_manager = MockTaskManager::new();
    let coordinator = TpsCoordinator::new(config, task_manager);

    coordinator.start().await.unwrap();

    // Initial status should be Green
    let andon = coordinator.get_andon_status().await.unwrap();
    assert_eq!(andon, AndonStatus::Green);

    // Submit and pull tasks to increase utilization
    for i in 1..=5 {
        coordinator
            .submit_task(&format!("task-{}", i), "context-1", 0)
            .await
            .unwrap();
        coordinator.pull_task("station1").await.unwrap();
    }

    // Wait for andon monitor to run (it runs every 10 seconds)
    tokio::time::sleep(Duration::from_secs(11)).await;

    // At 50% utilization (5/10), should still be Green
    let andon = coordinator.get_andon_status().await.unwrap();
    assert_eq!(andon, AndonStatus::Green);

    // Pull more tasks to reach Yellow threshold (70%)
    for i in 6..=8 {
        coordinator
            .submit_task(&format!("task-{}", i), "context-1", 0)
            .await
            .unwrap();
        coordinator.pull_task("station1").await.unwrap();
    }

    tokio::time::sleep(Duration::from_secs(11)).await;

    // At 80% utilization (8/10), should be Yellow
    let andon = coordinator.get_andon_status().await.unwrap();
    assert_eq!(andon, AndonStatus::Yellow);

    // Pull more to reach Red threshold (90%)
    coordinator
        .submit_task("task-9", "context-1", 0)
        .await
        .unwrap();
    coordinator.pull_task("station1").await.unwrap();

    tokio::time::sleep(Duration::from_secs(11)).await;

    // At 90% utilization (9/10), should be Red
    let andon = coordinator.get_andon_status().await.unwrap();
    assert_eq!(andon, AndonStatus::Red);

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_threshold_violations() {
    let config = CoordinatorConfig::builder()
        .stations(vec![Station::new("station1", 5)])
        .andon_yellow_threshold(0.6)
        .andon_red_threshold(0.8)
        .heijunka_period_seconds(60.0)
        .heijunka_target_throughput(10)
        .metrics_interval_seconds(0.5)
        .build();

    let task_manager = MockTaskManager::new();
    let coordinator = TpsCoordinator::new(config, task_manager);

    coordinator.start().await.unwrap();

    // Start with Green status
    assert_eq!(
        coordinator.get_andon_status().await.unwrap(),
        AndonStatus::Green
    );

    // Violate yellow threshold (60%)
    coordinator
        .submit_task("task-1", "context-1", 0)
        .await
        .unwrap();
    coordinator.pull_task("station1").await.unwrap();
    coordinator
        .submit_task("task-2", "context-1", 0)
        .await
        .unwrap();
    coordinator.pull_task("station1").await.unwrap();
    coordinator
        .submit_task("task-3", "context-1", 0)
        .await
        .unwrap();
    coordinator.pull_task("station1").await.unwrap();

    tokio::time::sleep(Duration::from_secs(11)).await;

    let metrics = coordinator.get_metrics().await.unwrap();
    let incident_count_after_yellow = metrics.andon_incidents;

    // Should have triggered yellow - new incident
    assert!(incident_count_after_yellow > 0);
    assert_eq!(
        coordinator.get_andon_status().await.unwrap(),
        AndonStatus::Yellow
    );

    // Violate red threshold (80%)
    coordinator
        .submit_task("task-4", "context-1", 0)
        .await
        .unwrap();
    coordinator.pull_task("station1").await.unwrap();

    tokio::time::sleep(Duration::from_secs(11)).await;

    let metrics = coordinator.get_metrics().await.unwrap();
    let incident_count_after_red = metrics.andon_incidents;

    // Should have triggered red - another new incident
    assert!(incident_count_after_red >= incident_count_after_yellow);
    assert_eq!(
        coordinator.get_andon_status().await.unwrap(),
        AndonStatus::Red
    );

    // Complete tasks to reduce utilization
    coordinator
        .complete_task_at_station("task-1", "station1", true)
        .await
        .unwrap();
    coordinator
        .complete_task_at_station("task-2", "station1", true)
        .await
        .unwrap();
    coordinator
        .complete_task_at_station("task-3", "station1", true)
        .await
        .unwrap();
    coordinator
        .complete_task_at_station("task-4", "station1", true)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(11)).await;

    // Should return to Green
    assert_eq!(
        coordinator.get_andon_status().await.unwrap(),
        AndonStatus::Green
    );

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_system_health_reporting() {
    let config = CoordinatorConfig::builder()
        .stations(vec![
            Station::new("submitted", 10),
            Station::new("working", 10),
            Station::new("review", 10),
        ])
        .heijunka_period_seconds(60.0)
        .heijunka_target_throughput(10)
        .metrics_interval_seconds(0.5)
        .build();

    let task_manager = MockTaskManager::new();
    let coordinator = TpsCoordinator::new(config, task_manager);

    coordinator.start().await.unwrap();

    // Get initial metrics
    let metrics = coordinator.get_metrics().await.unwrap();

    assert_eq!(metrics.total_processed, 0);
    assert_eq!(metrics.total_failed, 0);
    assert_eq!(metrics.total_canceled, 0);
    assert_eq!(metrics.current_wip, 0);
    assert_eq!(metrics.defect_rate, 0.0);
    assert_eq!(metrics.andon_status, AndonStatus::Green);
    assert_eq!(metrics.andon_incidents, 0);
    assert_eq!(metrics.jidoka_halts, 0);

    // Submit and complete some tasks
    for i in 1..=10 {
        coordinator
            .submit_task(&format!("task-{}", i), "context-1", 0)
            .await
            .unwrap();
    }

    // Pull and complete 8 successful, 2 failed
    for _i in 1..=8 {
        let task = coordinator.pull_task("submitted").await.unwrap().unwrap();
        coordinator
            .complete_task_at_station(&task.id, "submitted", true)
            .await
            .unwrap();
    }

    for _i in 9..=10 {
        let task = coordinator.pull_task("submitted").await.unwrap().unwrap();
        coordinator
            .complete_task_at_station(&task.id, "submitted", false)
            .await
            .unwrap();
    }

    // Wait for metrics update
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Check updated metrics
    let metrics = coordinator.get_metrics().await.unwrap();

    assert_eq!(metrics.total_processed, 10);
    assert_eq!(metrics.total_failed, 2);
    assert_eq!(metrics.total_canceled, 0);
    assert!(metrics.current_wip < 10); // Some completed
    assert_eq!(metrics.defect_rate, 0.2); // 2/10 = 20%

    // Verify station metrics exist
    assert!(metrics.station_metrics.contains_key("submitted"));
    assert!(metrics.station_metrics.contains_key("working"));
    assert!(metrics.station_metrics.contains_key("review"));

    coordinator.stop().await.unwrap();
}

// =============================================================================
// Heijunka Level Loading Tests
// =============================================================================

#[tokio::test]
async fn test_work_distribution() {
    let config = CoordinatorConfig::builder()
        .stations(vec![
            Station::new("intake", 5),
            Station::new("process", 3),
            Station::new("review", 2),
        ])
        .heijunka_period_seconds(60.0)
        .heijunka_target_throughput(5)
        .build();

    let task_manager = MockTaskManager::new();
    let task_counter = task_manager.get_task_count_counter();
    let coordinator = TpsCoordinator::new(config, task_manager);

    coordinator.start().await.unwrap();

    // Submit tasks within heijunka limit
    for i in 1..=5 {
        coordinator
            .submit_task(&format!("task-{}", i), "context-1", 0)
            .await
            .unwrap();
    }

    // All 5 should succeed
    assert_eq!(task_counter.load(std::sync::atomic::Ordering::SeqCst), 5);

    // 6th task should still work (heijunka is advisory, not hard limit)
    coordinator
        .submit_task("task-6", "context-1", 0)
        .await
        .unwrap();

    assert_eq!(task_counter.load(std::sync::atomic::Ordering::SeqCst), 6);

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_heijunka_period_reset() {
    let config = CoordinatorConfig::builder()
        .stations(vec![Station::new("station1", 10)])
        .heijunka_period_seconds(1.0) // 1 second period for fast testing
        .heijunka_target_throughput(3)
        .build();

    let task_manager = MockTaskManager::new();
    let task_counter = task_manager.get_task_count_counter();
    let coordinator = TpsCoordinator::new(config, task_manager);

    coordinator.start().await.unwrap();

    // Submit tasks up to the limit
    for i in 1..=3 {
        coordinator
            .submit_task(&format!("task-{}", i), "context-1", 0)
            .await
            .unwrap();
    }

    // Wait for period to reset
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Now we should be able to submit more tasks
    for i in 4..=6 {
        coordinator
            .submit_task(&format!("task-{}", i), "context-1", 0)
            .await
            .unwrap();
    }

    assert_eq!(task_counter.load(std::sync::atomic::Ordering::SeqCst), 6);

    coordinator.stop().await.unwrap();
}

// =============================================================================
// Task Flow Through Pipeline Tests
// =============================================================================

#[tokio::test]
async fn test_task_flow_through_stations() {
    let config = CoordinatorConfig::builder()
        .stations(vec![
            Station::new("submitted", 5),
            Station::new("working", 3),
            Station::new("review", 2),
        ])
        .heijunka_period_seconds(60.0)
        .heijunka_target_throughput(10)
        .build();

    let task_manager = MockTaskManager::new();
    let coordinator = TpsCoordinator::new(config.clone(), task_manager);

    coordinator.start().await.unwrap();

    // Submit a task
    let task = coordinator
        .submit_task("task-1", "context-1", 0)
        .await
        .unwrap();
    assert_eq!(task.status.state, TaskState::Submitted);

    // Pull from first station
    let task = coordinator.pull_task("submitted").await.unwrap().unwrap();
    assert_eq!(task.id, "task-1");

    // Complete at first station - should move to "working"
    let _task = coordinator
        .complete_task_at_station("task-1", "submitted", true)
        .await
        .unwrap();

    // Pull from second station
    let task = coordinator.pull_task("working").await.unwrap().unwrap();
    assert_eq!(task.id, "task-1");

    // Complete at second station - should move to "review"
    let _task = coordinator
        .complete_task_at_station("task-1", "working", true)
        .await
        .unwrap();

    // Pull from final station
    let task = coordinator.pull_task("review").await.unwrap().unwrap();
    assert_eq!(task.id, "task-1");

    // Complete at final station - task should be marked Completed
    let task = coordinator
        .complete_task_at_station("task-1", "review", true)
        .await
        .unwrap();

    assert_eq!(task.status.state, TaskState::Completed);

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_task_failure_at_final_station() {
    let config = CoordinatorConfig::builder()
        .stations(vec![Station::new("submitted", 5), Station::new("final", 3)])
        .heijunka_period_seconds(60.0)
        .heijunka_target_throughput(10)
        .enable_jidoka(false) // Disable jidoka for this test
        .build();

    let task_manager = MockTaskManager::new();
    let coordinator = TpsCoordinator::new(config, task_manager);

    coordinator.start().await.unwrap();

    // Submit and move task through pipeline
    coordinator
        .submit_task("task-1", "context-1", 0)
        .await
        .unwrap();
    coordinator.pull_task("submitted").await.unwrap();
    coordinator
        .complete_task_at_station("task-1", "submitted", true)
        .await
        .unwrap();

    coordinator.pull_task("final").await.unwrap();

    // Fail at final station
    let task = coordinator
        .complete_task_at_station("task-1", "final", false)
        .await
        .unwrap();

    assert_eq!(task.status.state, TaskState::Failed);

    // Wait for metrics update
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Verify metrics reflect failure
    let metrics = coordinator.get_metrics().await.unwrap();
    assert_eq!(metrics.total_processed, 2); // Both stations count as processed
    assert_eq!(metrics.total_failed, 1);

    coordinator.stop().await.unwrap();
}

// =============================================================================
// Unit Tests for Domain Types
// =============================================================================

#[test]
fn test_station_utilization_calculation() {
    let station = Station::new("test", 10);

    assert_eq!(station.utilization(), 0.0);
    assert!(station.has_capacity());

    let mut station_with_wip = station.clone();
    station_with_wip.current_wip = Some(5);

    assert_eq!(station_with_wip.utilization(), 0.5);
    assert!(station_with_wip.has_capacity());

    station_with_wip.current_wip = Some(10);
    assert_eq!(station_with_wip.utilization(), 1.0);
    assert!(!station_with_wip.has_capacity());
}

#[test]
fn test_takt_time_calculation() {
    let takt = TaktTime::new(3600.0, 60); // 1 hour for 60 units
    assert_eq!(takt.takt_time_seconds, 60.0); // 1 minute per unit

    let mut takt = TaktTime::new(3600.0, 120);
    assert_eq!(takt.takt_time_seconds, 30.0); // 30 seconds per unit

    takt.update_demand(180);
    assert_eq!(takt.takt_time_seconds, 20.0); // 20 seconds per unit

    // Test zero demand edge case
    let takt = TaktTime::new(3600.0, 0);
    assert_eq!(takt.takt_time_seconds, 3600.0); // Falls back to available time
}

#[test]
fn test_heijunka_scheduler_period_management() {
    let scheduler = HeijunkaScheduler::new(10, 60.0);

    // Initially should accept work
    assert!(scheduler.should_accept_work());

    // Process some work
    for _ in 0..10 {
        // Would increment current_period_count in real usage
    }

    let mut scheduler = scheduler;
    scheduler.current_period_count = 10;

    // Should not accept more work
    assert!(!scheduler.should_accept_work());

    // Reset period
    scheduler.reset_period();
    assert_eq!(scheduler.current_period_count, 0);
    assert!(scheduler.should_accept_work());
}

#[test]
fn test_andon_status_severity() {
    assert_eq!(AndonStatus::Green.severity(), 0);
    assert_eq!(AndonStatus::Yellow.severity(), 1);
    assert_eq!(AndonStatus::Red.severity(), 2);

    // Test comparison
    assert!(AndonStatus::Red.severity() > AndonStatus::Yellow.severity());
    assert!(AndonStatus::Yellow.severity() > AndonStatus::Green.severity());
}

#[test]
fn test_jidoka_gate_default_state() {
    let gate = JidokaGate::default();

    assert!(!gate.is_halted);
    assert!(gate.halt_reason.is_none());
    assert!(gate.halted_at.is_none());
    assert_eq!(gate.blocked_count, 0);
}

#[test]
fn test_coordinator_config_defaults() {
    let config = CoordinatorConfig::default();

    assert_eq!(config.stations.len(), 3);
    assert_eq!(config.andon_yellow_threshold, 0.7);
    assert_eq!(config.andon_red_threshold, 0.9);
    assert_eq!(config.takt_time_seconds, 60.0);
    assert!(config.enable_jidoka);
    assert_eq!(config.jidoka_defect_threshold, 0.1);
    assert_eq!(config.heijunka_period_seconds, 300.0);
    assert_eq!(config.heijunka_target_throughput, 10);
    assert_eq!(config.max_queue_size, 1000);
    assert_eq!(config.metrics_interval_seconds, 30.0);
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[tokio::test]
async fn test_empty_pull_from_station() {
    let config = create_minimal_config();
    let task_manager = MockTaskManager::new();
    let coordinator = TpsCoordinator::new(config, task_manager);

    coordinator.start().await.unwrap();

    // Pull from empty station should return None
    let result = coordinator.pull_task("submitted").await.unwrap();
    assert!(result.is_none());

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_pull_from_nonexistent_station() {
    let config = create_minimal_config();
    let task_manager = MockTaskManager::new();
    let coordinator = TpsCoordinator::new(config, task_manager);

    coordinator.start().await.unwrap();

    // Pull from nonexistent station should error
    let result = coordinator.pull_task("nonexistent").await;
    assert!(result.is_err());

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_complete_nonexistent_task() {
    let config = create_minimal_config();
    let task_manager = MockTaskManager::new();
    let coordinator = TpsCoordinator::new(config, task_manager);

    coordinator.start().await.unwrap();

    // Complete nonexistent task - should handle gracefully
    // (The task manager will return NotFound, but coordinator should not crash)
    let result = coordinator
        .complete_task_at_station("nonexistent-task", "submitted", true)
        .await;

    // Should error since task doesn't exist
    assert!(result.is_err());

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_task_submissions() {
    let config = create_minimal_config();
    let task_manager = MockTaskManager::new();
    let task_counter = task_manager.get_task_count_counter();
    let coordinator = Arc::new(TpsCoordinator::new(config, task_manager));

    coordinator.start().await.unwrap();

    // Spawn multiple concurrent submissions
    let mut handles = vec![];

    for i in 0..20 {
        let coord = Arc::clone(&coordinator);
        let handle = tokio::spawn(async move {
            coord
                .submit_task(&format!("task-{}", i), "context-1", 0)
                .await
        });
        handles.push(handle);
    }

    // Wait for all submissions to complete
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Verify all tasks were created
    assert_eq!(task_counter.load(std::sync::atomic::Ordering::SeqCst), 20);

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_queue_capacity_limit() {
    let config = CoordinatorConfig::builder()
        .stations(vec![Station::new("submitted", 100)])
        .max_queue_size(5) // Very small queue for testing
        .heijunka_period_seconds(60.0)
        .heijunka_target_throughput(10)
        .build();

    let task_manager = MockTaskManager::new();
    let coordinator = TpsCoordinator::new(config, task_manager);

    coordinator.start().await.unwrap();

    // Submit tasks up to queue limit
    for i in 1..=5 {
        coordinator
            .submit_task(&format!("task-{}", i), "context-1", 0)
            .await
            .unwrap();
    }

    // 6th task should fail due to queue capacity
    let result = coordinator.submit_task("task-6", "context-1", 0).await;
    assert!(result.is_err());

    coordinator.stop().await.unwrap();
}
