//! Autonomous Agent Coordinator with Toyota Production System (TPS) Principles
//!
//! This module implements a pull-based task coordination system inspired by TPS/Lean manufacturing:
//! - **Kanban Board**: WIP limits per station to prevent overload
//! - **Pull Scheduling**: Tasks are pulled when capacity is available (not pushed)
//! - **Andon System**: Real-time status monitoring (GREEN/YELLOW/RED)
//! - **Jidoka**: Automatic stopping on abnormalities
//! - **Heijunka**: Level loading for smooth workflow
//! - **Takt Time**: Rhythm-based scheduling aligned with demand
//! - **Kaizen**: Continuous improvement through metrics
//!
//! # Example
//! ```rust,ignore
//! use a2a_rs::services::coordinator::{TpsCoordinator, CoordinatorConfig, Station};
//!
//! let config = CoordinatorConfig::builder()
//!     .stations(vec![
//!         Station::new("intake", 5),
//!         Station::new("processing", 3),
//!         Station::new("review", 2),
//!     ])
//!     .andon_yellow_threshold(0.7)
//!     .andon_red_threshold(0.9)
//!     .takt_time_seconds(30.0)
//!     .build();
//!
//! let coordinator = TpsCoordinator::new(config, task_manager).await?;
//! coordinator.start().await?;
//! ```

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bon::Builder;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

#[cfg(feature = "tracing")]
use tracing::{debug, error, info, instrument, warn};

use crate::{Task, TaskState, domain::A2AError, port::AsyncTaskManager};

// =============================================================================
// Domain Types for TPS Coordination
// =============================================================================

/// Kanban station representing a stage in the task processing pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Station {
    /// Station identifier (e.g., "intake", "processing", "review")
    pub name: String,

    /// Maximum work-in-progress (WIP) limit for this station
    pub wip_limit: usize,

    /// Current number of tasks in this station
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_wip: Option<usize>,

    /// Station-specific metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

impl Station {
    pub fn new(name: impl Into<String>, wip_limit: usize) -> Self {
        Self {
            name: name.into(),
            wip_limit,
            current_wip: Some(0),
            metadata: None,
        }
    }

    /// Check if station has capacity to accept more work
    pub fn has_capacity(&self) -> bool {
        self.current_wip.unwrap_or(0) < self.wip_limit
    }

    /// Calculate utilization ratio (0.0 to 1.0)
    pub fn utilization(&self) -> f64 {
        if self.wip_limit == 0 {
            return 0.0;
        }
        self.current_wip.unwrap_or(0) as f64 / self.wip_limit as f64
    }
}

/// Andon system status levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AndonStatus {
    /// Normal operation - all stations within threshold
    Green,

    /// Warning - approaching capacity limits
    Yellow,

    /// Critical - capacity exceeded or system halted
    Red,
}

impl AndonStatus {
    /// Convert to numeric severity (higher = more severe)
    pub fn severity(&self) -> u8 {
        match self {
            AndonStatus::Green => 0,
            AndonStatus::Yellow => 1,
            AndonStatus::Red => 2,
        }
    }
}

/// Andon signal emitted when status changes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndonSignal {
    /// Current status level
    pub status: AndonStatus,

    /// Reason for the status
    pub reason: String,

    /// Affected station (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub station: Option<String>,

    /// Timestamp when signal was generated
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Additional context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Jidoka gate - stops pipeline on abnormality detection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JidokaGate {
    /// Whether the gate is currently halting the pipeline
    pub is_halted: bool,

    /// Reason for halt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub halt_reason: Option<String>,

    /// When the halt started
    #[serde(skip_serializing_if = "Option::is_none")]
    pub halted_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Number of tasks blocked by the halt
    pub blocked_count: usize,
}

impl Default for JidokaGate {
    fn default() -> Self {
        Self {
            is_halted: false,
            halt_reason: None,
            halted_at: None,
            blocked_count: 0,
        }
    }
}

/// Task entry in the queue with timing metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueuedTask {
    /// Task identifier
    task_id: String,

    /// Context identifier
    context_id: String,

    /// When task entered the queue (not serialized)
    #[serde(skip, default = "Instant::now")]
    queued_at: Instant,

    /// Priority level (higher = more urgent)
    priority: u32,

    /// Current station in the pipeline
    station: String,

    /// Estimated processing time (seconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    estimated_duration: Option<f64>,
}

/// Heijunka (level loading) scheduler
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeijunkaScheduler {
    /// Target throughput per takt time period
    pub target_throughput: usize,

    /// Number of tasks processed in current period
    pub current_period_count: usize,

    /// When current period started (not serialized)
    #[serde(skip, default = "Instant::now")]
    pub period_start: Instant,

    /// Length of scheduling period (seconds)
    pub period_length: f64,
}

impl HeijunkaScheduler {
    pub fn new(target_throughput: usize, period_length: f64) -> Self {
        Self {
            target_throughput,
            current_period_count: 0,
            period_start: Instant::now(),
            period_length,
        }
    }

    /// Check if we should accept more work in this period
    pub fn should_accept_work(&self) -> bool {
        self.current_period_count < self.target_throughput
    }

    /// Reset period counters
    pub fn reset_period(&mut self) {
        self.current_period_count = 0;
        self.period_start = Instant::now();
    }

    /// Check if period has elapsed
    pub fn period_elapsed(&self) -> bool {
        self.period_start.elapsed().as_secs_f64() >= self.period_length
    }
}

/// Takt time calculator
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaktTime {
    /// Available working time per period (seconds)
    pub available_time: f64,

    /// Customer demand per period
    pub demand: usize,

    /// Calculated takt time (available_time / demand)
    pub takt_time_seconds: f64,
}

impl TaktTime {
    pub fn new(available_time: f64, demand: usize) -> Self {
        let takt_time_seconds = if demand > 0 {
            available_time / demand as f64
        } else {
            available_time
        };

        Self {
            available_time,
            demand,
            takt_time_seconds,
        }
    }

    /// Update demand and recalculate takt time
    pub fn update_demand(&mut self, new_demand: usize) {
        self.demand = new_demand;
        self.takt_time_seconds = if new_demand > 0 {
            self.available_time / new_demand as f64
        } else {
            self.available_time
        };
    }
}

/// Comprehensive metrics for the coordination system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordinatorMetrics {
    /// Total tasks processed
    pub total_processed: u64,

    /// Total tasks failed
    pub total_failed: u64,

    /// Total tasks canceled
    pub total_canceled: u64,

    /// Current work in progress across all stations
    pub current_wip: usize,

    /// Average cycle time (end-to-end processing time in seconds)
    pub avg_cycle_time_seconds: f64,

    /// Average lead time (queue + processing time in seconds)
    pub avg_lead_time_seconds: f64,

    /// Throughput (tasks per minute)
    pub throughput_per_minute: f64,

    /// Defect rate (failed / total processed)
    pub defect_rate: f64,

    /// Current andon status
    pub andon_status: AndonStatus,

    /// Number of andon incidents (Yellow or Red)
    pub andon_incidents: u64,

    /// Number of jidoka halts
    pub jidoka_halts: u64,

    /// Station-specific metrics
    pub station_metrics: HashMap<String, StationMetrics>,

    /// Last updated timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Default for CoordinatorMetrics {
    fn default() -> Self {
        Self {
            total_processed: 0,
            total_failed: 0,
            total_canceled: 0,
            current_wip: 0,
            avg_cycle_time_seconds: 0.0,
            avg_lead_time_seconds: 0.0,
            throughput_per_minute: 0.0,
            defect_rate: 0.0,
            andon_status: AndonStatus::Green,
            andon_incidents: 0,
            jidoka_halts: 0,
            station_metrics: HashMap::new(),
            updated_at: chrono::Utc::now(),
        }
    }
}

/// Station-specific metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationMetrics {
    /// Current WIP at this station
    pub current_wip: usize,

    /// WIP limit
    pub wip_limit: usize,

    /// Utilization ratio (0.0 to 1.0)
    pub utilization: f64,

    /// Average processing time at this station
    pub avg_processing_time_seconds: f64,

    /// Number of tasks processed
    pub tasks_processed: u64,

    /// Number of tasks currently blocked
    pub tasks_blocked: usize,
}

// =============================================================================
// Configuration
// =============================================================================

/// Configuration for the TPS Coordinator
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordinatorConfig {
    /// Kanban stations in the pipeline (in order)
    pub stations: Vec<Station>,

    /// Andon yellow threshold (utilization ratio)
    #[builder(default = 0.7)]
    pub andon_yellow_threshold: f64,

    /// Andon red threshold (utilization ratio)
    #[builder(default = 0.9)]
    pub andon_red_threshold: f64,

    /// Target takt time in seconds
    #[builder(default = 60.0)]
    pub takt_time_seconds: f64,

    /// Enable automatic jidoka halts on high defect rate
    #[builder(default = true)]
    pub enable_jidoka: bool,

    /// Defect rate threshold for jidoka halt
    #[builder(default = 0.1)]
    pub jidoka_defect_threshold: f64,

    /// Heijunka period length in seconds
    #[builder(default = 300.0)]
    pub heijunka_period_seconds: f64,

    /// Target throughput per heijunka period
    #[builder(default = 10)]
    pub heijunka_target_throughput: usize,

    /// Maximum queue size across all stations
    #[builder(default = 1000)]
    pub max_queue_size: usize,

    /// Metrics collection interval in seconds
    #[builder(default = 30.0)]
    pub metrics_interval_seconds: f64,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            stations: vec![
                Station::new("submitted", 10),
                Station::new("working", 5),
                Station::new("review", 2),
            ],
            andon_yellow_threshold: 0.7,
            andon_red_threshold: 0.9,
            takt_time_seconds: 60.0,
            enable_jidoka: true,
            jidoka_defect_threshold: 0.1,
            heijunka_period_seconds: 300.0,
            heijunka_target_throughput: 10,
            max_queue_size: 1000,
            metrics_interval_seconds: 30.0,
        }
    }
}

// =============================================================================
// Coordinator State
// =============================================================================

/// Internal state of the coordinator
struct CoordinatorState {
    /// Task queues per station (pull-based)
    station_queues: HashMap<String, VecDeque<QueuedTask>>,

    /// Current WIP count per station
    station_wip: HashMap<String, usize>,

    /// Task timing data (for metrics)
    task_timings: HashMap<String, TaskTiming>,

    /// Current andon status
    andon_status: AndonStatus,

    /// Jidoka gate state
    jidoka_gate: JidokaGate,

    /// Heijunka scheduler
    heijunka: HeijunkaScheduler,

    /// Takt time calculator
    takt_time: TaktTime,

    /// Accumulated metrics
    metrics: CoordinatorMetrics,

    /// Last metrics update time
    last_metrics_update: Instant,
}

#[derive(Debug, Clone)]
struct TaskTiming {
    queued_at: Instant,
    started_at: Option<Instant>,
    completed_at: Option<Instant>,
    station: String,
}

impl CoordinatorState {
    fn new(config: &CoordinatorConfig) -> Self {
        let mut station_queues = HashMap::new();
        let mut station_wip = HashMap::new();
        let mut station_metrics = HashMap::new();

        for station in &config.stations {
            station_queues.insert(station.name.clone(), VecDeque::new());
            station_wip.insert(station.name.clone(), 0);
            station_metrics.insert(
                station.name.clone(),
                StationMetrics {
                    current_wip: 0,
                    wip_limit: station.wip_limit,
                    utilization: 0.0,
                    avg_processing_time_seconds: 0.0,
                    tasks_processed: 0,
                    tasks_blocked: 0,
                },
            );
        }

        let heijunka = HeijunkaScheduler::new(
            config.heijunka_target_throughput,
            config.heijunka_period_seconds,
        );

        let takt_time = TaktTime::new(
            config.heijunka_period_seconds,
            config.heijunka_target_throughput,
        );

        let mut metrics = CoordinatorMetrics::default();
        metrics.station_metrics = station_metrics;

        Self {
            station_queues,
            station_wip,
            task_timings: HashMap::new(),
            andon_status: AndonStatus::Green,
            jidoka_gate: JidokaGate::default(),
            heijunka,
            takt_time,
            metrics,
            last_metrics_update: Instant::now(),
        }
    }
}

// =============================================================================
// TPS Coordinator
// =============================================================================

/// Main coordinator implementing TPS principles
pub struct TpsCoordinator<T: AsyncTaskManager> {
    config: CoordinatorConfig,
    state: Arc<RwLock<CoordinatorState>>,
    task_manager: Arc<T>,
    shutdown_signal: Arc<Mutex<bool>>,
}

impl<T: AsyncTaskManager> TpsCoordinator<T> {
    /// Create a new TPS coordinator
    pub fn new(config: CoordinatorConfig, task_manager: T) -> Self {
        let state = Arc::new(RwLock::new(CoordinatorState::new(&config)));

        Self {
            config,
            state,
            task_manager: Arc::new(task_manager),
            shutdown_signal: Arc::new(Mutex::new(false)),
        }
    }

    /// Start the coordinator event loop
    #[cfg_attr(feature = "tracing", instrument(skip(self), fields(
        coordinator.stations = self.config.stations.len()
    )))]
    pub async fn start(&self) -> Result<(), A2AError> {
        #[cfg(feature = "tracing")]
        info!("Starting TPS Coordinator");

        let mut shutdown = self.shutdown_signal.lock().await;
        *shutdown = false;
        drop(shutdown);

        // Spawn background tasks
        self.spawn_metrics_collector();
        self.spawn_heijunka_scheduler();
        self.spawn_andon_monitor();

        #[cfg(feature = "tracing")]
        info!("TPS Coordinator started successfully");

        Ok(())
    }

    /// Stop the coordinator
    #[cfg_attr(feature = "tracing", instrument(skip(self)))]
    pub async fn stop(&self) -> Result<(), A2AError> {
        #[cfg(feature = "tracing")]
        info!("Stopping TPS Coordinator");

        let mut shutdown = self.shutdown_signal.lock().await;
        *shutdown = true;

        #[cfg(feature = "tracing")]
        info!("TPS Coordinator stopped");

        Ok(())
    }

    /// Submit a new task to the coordinator (enters first station)
    #[cfg_attr(feature = "tracing", instrument(skip(self), fields(
        task_id = %task_id,
        context_id = %context_id
    )))]
    pub async fn submit_task(
        &self,
        task_id: &str,
        context_id: &str,
        priority: u32,
    ) -> Result<Task, A2AError> {
        let task_id = task_id.to_string();
        let context_id = context_id.to_string();

        #[cfg(feature = "tracing")]
        debug!("Submitting task to coordinator");

        // Check jidoka gate
        let state = self.state.read().await;
        if state.jidoka_gate.is_halted {
            #[cfg(feature = "tracing")]
            warn!(
                "Jidoka gate is halted: {}",
                state
                    .jidoka_gate
                    .halt_reason
                    .as_deref()
                    .unwrap_or("unknown")
            );
            return Err(A2AError::Internal(format!(
                "System halted by Jidoka: {}",
                state
                    .jidoka_gate
                    .halt_reason
                    .as_deref()
                    .unwrap_or("unknown")
            )));
        }

        // Check heijunka
        if !state.heijunka.should_accept_work() {
            #[cfg(feature = "tracing")]
            warn!("Heijunka limit reached for current period");
            return Err(A2AError::Internal(
                "Capacity limit reached for current period".to_string(),
            ));
        }

        // Check queue size
        let total_queued: usize = state.station_queues.values().map(|q| q.len()).sum();
        if total_queued >= self.config.max_queue_size {
            #[cfg(feature = "tracing")]
            error!("Maximum queue size exceeded");
            return Err(A2AError::Internal("Queue capacity exceeded".to_string()));
        }
        drop(state);

        // Create task in task manager
        let task = self.task_manager.create_task(&task_id, &context_id).await?;

        // Add to first station queue
        let first_station = &self.config.stations[0].name;
        let queued_task = QueuedTask {
            task_id: task_id.clone(),
            context_id: context_id.clone(),
            queued_at: Instant::now(),
            priority,
            station: first_station.clone(),
            estimated_duration: None,
        };

        let mut state = self.state.write().await;
        state
            .station_queues
            .get_mut(first_station)
            .ok_or_else(|| A2AError::Internal("First station not found".to_string()))?
            .push_back(queued_task);

        state.task_timings.insert(
            task_id.clone(),
            TaskTiming {
                queued_at: Instant::now(),
                started_at: None,
                completed_at: None,
                station: first_station.clone(),
            },
        );

        #[cfg(feature = "tracing")]
        info!(
            task_id = %task_id,
            station = %first_station,
            "Task submitted to coordinator"
        );

        Ok(task)
    }

    /// Pull next task from a station (pull-based scheduling)
    #[cfg_attr(feature = "tracing", instrument(skip(self), fields(
        station = %station_name
    )))]
    pub async fn pull_task(&self, station_name: &str) -> Result<Option<Task>, A2AError> {
        let mut state = self.state.write().await;

        // Check if station exists
        let station_config = self
            .config
            .stations
            .iter()
            .find(|s| s.name == station_name)
            .ok_or_else(|| A2AError::Internal(format!("Station not found: {}", station_name)))?;

        // Check WIP limit
        let current_wip = state.station_wip.get(station_name).copied().unwrap_or(0);
        if current_wip >= station_config.wip_limit {
            #[cfg(feature = "tracing")]
            debug!(
                station = %station_name,
                current_wip,
                wip_limit = station_config.wip_limit,
                "Station at WIP limit"
            );
            return Ok(None);
        }

        // Pull from queue (highest priority first)
        let queue = state
            .station_queues
            .get_mut(station_name)
            .ok_or_else(|| A2AError::Internal("Station queue not found".to_string()))?;

        if queue.is_empty() {
            return Ok(None);
        }

        // Simple priority-based pull (in production, consider more sophisticated algorithms)
        let queued_task = queue
            .pop_front()
            .ok_or_else(|| A2AError::Internal("Failed to pull task from queue".to_string()))?;

        let queue_size = queue.len();
        // Queue reference is dropped here, allowing access to other parts of state

        // Update WIP
        *state.station_wip.get_mut(station_name).unwrap() += 1;

        // Update timing
        if let Some(timing) = state.task_timings.get_mut(&queued_task.task_id) {
            timing.started_at = Some(Instant::now());
        }

        #[cfg(feature = "tracing")]
        info!(
            task_id = %queued_task.task_id,
            station = %station_name,
            queue_size = queue_size,
            "Task pulled from station"
        );

        // Get task from task manager
        let task = self
            .task_manager
            .get_task(&queued_task.task_id, None)
            .await?;

        Ok(Some(task))
    }

    /// Complete task at a station and move to next station or finish
    #[cfg_attr(feature = "tracing", instrument(skip(self), fields(
        task_id = %task_id,
        station = %current_station
    )))]
    pub async fn complete_task_at_station(
        &self,
        task_id: &str,
        current_station: &str,
        success: bool,
    ) -> Result<Task, A2AError> {
        let mut state = self.state.write().await;

        // Update WIP
        if let Some(wip) = state.station_wip.get_mut(current_station) {
            if *wip > 0 {
                *wip -= 1;
            }
        }

        // Find next station
        let current_index = self
            .config
            .stations
            .iter()
            .position(|s| s.name == current_station)
            .ok_or_else(|| A2AError::Internal("Current station not found".to_string()))?;

        let next_station_index = current_index + 1;
        let is_final_station = next_station_index >= self.config.stations.len();

        // Update metrics
        state.metrics.total_processed += 1;
        if !success {
            state.metrics.total_failed += 1;
        }

        // Update timing
        if let Some(timing) = state.task_timings.get_mut(task_id) {
            timing.completed_at = Some(Instant::now());
        }

        drop(state);

        // Update task state
        let task = if is_final_station {
            // Final station - mark as completed or failed
            let final_state = if success {
                TaskState::Completed
            } else {
                TaskState::Failed
            };

            #[cfg(feature = "tracing")]
            info!(
                task_id = %task_id,
                station = %current_station,
                success = success,
                "Task completed at final station"
            );

            self.task_manager
                .update_task_status(task_id, final_state, None)
                .await?
        } else {
            // Move to next station
            let next_station = &self.config.stations[next_station_index].name;

            #[cfg(feature = "tracing")]
            debug!(
                task_id = %task_id,
                from_station = %current_station,
                to_station = %next_station,
                "Moving task to next station"
            );

            let mut state = self.state.write().await;

            // Add to next station queue
            let context_id = self.task_manager.get_task(task_id, None).await?.context_id;

            let queued_task = QueuedTask {
                task_id: task_id.to_string(),
                context_id,
                queued_at: Instant::now(),
                priority: 0,
                station: next_station.clone(),
                estimated_duration: None,
            };

            state
                .station_queues
                .get_mut(next_station)
                .ok_or_else(|| A2AError::Internal("Next station queue not found".to_string()))?
                .push_back(queued_task);

            // Update timing
            if let Some(timing) = state.task_timings.get_mut(task_id) {
                timing.station = next_station.clone();
            }

            drop(state);

            self.task_manager.get_task(task_id, None).await?
        };

        // Check if we need to trigger jidoka
        if self.config.enable_jidoka && !success {
            self.check_jidoka().await?;
        }

        Ok(task)
    }

    /// Get current coordinator metrics
    pub async fn get_metrics(&self) -> Result<CoordinatorMetrics, A2AError> {
        let state = self.state.read().await;
        Ok(state.metrics.clone())
    }

    /// Get current andon status
    pub async fn get_andon_status(&self) -> Result<AndonStatus, A2AError> {
        let state = self.state.read().await;
        Ok(state.andon_status)
    }

    /// Get jidoka gate status
    pub async fn get_jidoka_gate(&self) -> Result<JidokaGate, A2AError> {
        let state = self.state.read().await;
        Ok(state.jidoka_gate.clone())
    }

    /// Manually trigger jidoka halt
    #[cfg_attr(feature = "tracing", instrument(skip(self)))]
    pub async fn trigger_jidoka_halt(&self, reason: String) -> Result<(), A2AError> {
        #[cfg(feature = "tracing")]
        warn!(reason = %reason, "Triggering manual jidoka halt");

        let mut state = self.state.write().await;
        state.jidoka_gate.is_halted = true;
        state.jidoka_gate.halt_reason = Some(reason);
        state.jidoka_gate.halted_at = Some(chrono::Utc::now());
        state.metrics.jidoka_halts += 1;

        Ok(())
    }

    /// Resume from jidoka halt
    #[cfg_attr(feature = "tracing", instrument(skip(self)))]
    pub async fn resume_from_jidoka(&self) -> Result<(), A2AError> {
        #[cfg(feature = "tracing")]
        info!("Resuming from jidoka halt");

        let mut state = self.state.write().await;
        state.jidoka_gate.is_halted = false;
        state.jidoka_gate.halt_reason = None;
        state.jidoka_gate.halted_at = None;
        state.jidoka_gate.blocked_count = 0;

        Ok(())
    }

    // Internal helper methods

    async fn check_jidoka(&self) -> Result<(), A2AError> {
        let mut state = self.state.write().await;

        // Calculate defect rate
        let defect_rate = state.metrics.defect_rate;

        if defect_rate > self.config.jidoka_defect_threshold {
            #[cfg(feature = "tracing")]
            error!(
                defect_rate = defect_rate,
                threshold = self.config.jidoka_defect_threshold,
                "Defect rate exceeded threshold - triggering jidoka halt"
            );

            state.jidoka_gate.is_halted = true;
            state.jidoka_gate.halt_reason = Some(format!(
                "Defect rate {:.2}% exceeds threshold",
                defect_rate * 100.0
            ));
            state.jidoka_gate.halted_at = Some(chrono::Utc::now());
            state.metrics.jidoka_halts += 1;
        }

        Ok(())
    }

    fn spawn_metrics_collector(&self) {
        let state = Arc::clone(&self.state);
        let shutdown = Arc::clone(&self.shutdown_signal);
        let interval_secs = self.config.metrics_interval_seconds;

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs_f64(interval_secs)).await;

                let shutdown_flag = shutdown.lock().await;
                if *shutdown_flag {
                    break;
                }
                drop(shutdown_flag);

                let mut state = state.write().await;

                // Update metrics
                let current_wip: usize = state.station_wip.values().sum();
                state.metrics.current_wip = current_wip;

                // Calculate defect rate
                if state.metrics.total_processed > 0 {
                    state.metrics.defect_rate =
                        state.metrics.total_failed as f64 / state.metrics.total_processed as f64;
                }

                // Update station metrics
                // Collect station data to avoid borrow checker issues
                let station_data: Vec<(String, usize)> = state
                    .station_wip
                    .iter()
                    .map(|(name, wip)| (name.clone(), *wip))
                    .collect();

                for (station_name, wip) in station_data {
                    if let Some(station_metric) =
                        state.metrics.station_metrics.get_mut(&station_name)
                    {
                        station_metric.current_wip = wip;
                        station_metric.utilization = wip as f64 / station_metric.wip_limit as f64;
                    }
                }

                state.metrics.updated_at = chrono::Utc::now();
                state.last_metrics_update = Instant::now();

                #[cfg(feature = "tracing")]
                debug!(
                    total_processed = state.metrics.total_processed,
                    current_wip = state.metrics.current_wip,
                    defect_rate = state.metrics.defect_rate,
                    "Metrics updated"
                );
            }
        });
    }

    fn spawn_heijunka_scheduler(&self) {
        let state = Arc::clone(&self.state);
        let shutdown = Arc::clone(&self.shutdown_signal);
        let period_secs = self.config.heijunka_period_seconds;

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs_f64(period_secs)).await;

                let shutdown_flag = shutdown.lock().await;
                if *shutdown_flag {
                    break;
                }
                drop(shutdown_flag);

                let mut state = state.write().await;

                if state.heijunka.period_elapsed() {
                    #[cfg(feature = "tracing")]
                    debug!(
                        processed = state.heijunka.current_period_count,
                        target = state.heijunka.target_throughput,
                        "Heijunka period ended"
                    );

                    state.heijunka.reset_period();
                }
            }
        });
    }

    fn spawn_andon_monitor(&self) {
        let state = Arc::clone(&self.state);
        let shutdown = Arc::clone(&self.shutdown_signal);
        let yellow_threshold = self.config.andon_yellow_threshold;
        let red_threshold = self.config.andon_red_threshold;
        let stations = self.config.stations.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;

                let shutdown_flag = shutdown.lock().await;
                if *shutdown_flag {
                    break;
                }
                drop(shutdown_flag);

                let mut state = state.write().await;

                // Calculate max utilization across all stations
                let max_utilization = stations
                    .iter()
                    .map(|s| {
                        let wip = state.station_wip.get(&s.name).copied().unwrap_or(0);
                        wip as f64 / s.wip_limit as f64
                    })
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(0.0);

                let old_status = state.andon_status;
                let new_status = if max_utilization >= red_threshold {
                    AndonStatus::Red
                } else if max_utilization >= yellow_threshold {
                    AndonStatus::Yellow
                } else {
                    AndonStatus::Green
                };

                if new_status != old_status {
                    #[cfg(feature = "tracing")]
                    warn!(
                        old_status = ?old_status,
                        new_status = ?new_status,
                        max_utilization = max_utilization,
                        "Andon status changed"
                    );

                    state.andon_status = new_status;

                    if new_status.severity() > AndonStatus::Green.severity() {
                        state.metrics.andon_incidents += 1;
                    }
                }
            }
        });
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_station_capacity() {
        let mut station = Station::new("test", 5);
        assert!(station.has_capacity());
        assert_eq!(station.utilization(), 0.0);

        station.current_wip = Some(3);
        assert!(station.has_capacity());
        assert_eq!(station.utilization(), 0.6);

        station.current_wip = Some(5);
        assert!(!station.has_capacity());
        assert_eq!(station.utilization(), 1.0);
    }

    #[test]
    fn test_andon_severity() {
        assert_eq!(AndonStatus::Green.severity(), 0);
        assert_eq!(AndonStatus::Yellow.severity(), 1);
        assert_eq!(AndonStatus::Red.severity(), 2);
    }

    #[test]
    fn test_takt_time_calculation() {
        let takt = TaktTime::new(3600.0, 60);
        assert_eq!(takt.takt_time_seconds, 60.0);

        let mut takt = TaktTime::new(3600.0, 120);
        assert_eq!(takt.takt_time_seconds, 30.0);

        takt.update_demand(180);
        assert_eq!(takt.takt_time_seconds, 20.0);
    }

    #[test]
    fn test_heijunka_scheduler() {
        let mut scheduler = HeijunkaScheduler::new(10, 60.0);
        assert!(scheduler.should_accept_work());

        scheduler.current_period_count = 10;
        assert!(!scheduler.should_accept_work());

        scheduler.reset_period();
        assert_eq!(scheduler.current_period_count, 0);
        assert!(scheduler.should_accept_work());
    }

    #[test]
    fn test_jidoka_gate_default() {
        let gate = JidokaGate::default();
        assert!(!gate.is_halted);
        assert!(gate.halt_reason.is_none());
        assert_eq!(gate.blocked_count, 0);
    }
}
