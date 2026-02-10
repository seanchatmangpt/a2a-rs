//! Observability layer for CONSTRUCT runtime
//!
//! This module provides comprehensive tracing and metrics for the CONSTRUCT
//! execution pipeline, enabling:
//! - Distributed tracing with span context propagation
//! - Metrics counters for operations, errors, and state transitions
//! - Timing information for performance analysis
//! - Audit trails for guard rejections and invariant violations
//!
//! # Architecture
//!
//! The observability layer instruments four key subsystems:
//! - **Runtime Executor**: μ(O) execution pipeline stages
//! - **Scheduler**: Λ task ordering and execution decisions
//! - **Guards**: Admission control and refusal tracking
//! - **Invariants**: State validation and violation detection
//!
//! # Usage
//!
//! ```rust
//! use a2a_rs::construct::observability::{ObservabilityContext, RuntimeMetrics};
//!
//! // Initialize metrics tracking
//! let metrics = RuntimeMetrics::new();
//!
//! // Track an operation
//! metrics.increment_operations_total("create_task");
//! metrics.increment_stage_completions("type_check");
//! ```

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[cfg(feature = "tracing")]
use tracing::{Span, debug, error, info, info_span, instrument, warn};

/// Metrics registry for CONSTRUCT runtime operations
///
/// Thread-safe counter tracking for all observable events in the system.
/// Uses atomic counters for lock-free concurrent access.
#[derive(Debug, Clone)]
pub struct RuntimeMetrics {
    /// Total operations by type
    operations_total: Arc<BTreeMap<String, AtomicU64>>,

    /// Operations by stage completion
    stage_completions: Arc<BTreeMap<String, AtomicU64>>,

    /// Guard evaluations (admitted vs rejected)
    guard_admissions: Arc<AtomicU64>,
    guard_rejections: Arc<AtomicU64>,

    /// Invariant checks (passed vs failed)
    invariant_checks_passed: Arc<AtomicU64>,
    invariant_checks_failed: Arc<AtomicU64>,

    /// Scheduler operations
    tasks_submitted: Arc<AtomicU64>,
    tasks_completed: Arc<AtomicU64>,
    tasks_cancelled: Arc<AtomicU64>,
    scheduler_selections: Arc<AtomicU64>,

    /// Error counters
    type_check_errors: Arc<AtomicU64>,
    admission_errors: Arc<AtomicU64>,
    transformation_errors: Arc<AtomicU64>,
    invariant_errors: Arc<AtomicU64>,
    execution_errors: Arc<AtomicU64>,
}

impl RuntimeMetrics {
    /// Create a new metrics registry
    pub fn new() -> Self {
        Self {
            operations_total: Arc::new(BTreeMap::new()),
            stage_completions: Arc::new(BTreeMap::new()),
            guard_admissions: Arc::new(AtomicU64::new(0)),
            guard_rejections: Arc::new(AtomicU64::new(0)),
            invariant_checks_passed: Arc::new(AtomicU64::new(0)),
            invariant_checks_failed: Arc::new(AtomicU64::new(0)),
            tasks_submitted: Arc::new(AtomicU64::new(0)),
            tasks_completed: Arc::new(AtomicU64::new(0)),
            tasks_cancelled: Arc::new(AtomicU64::new(0)),
            scheduler_selections: Arc::new(AtomicU64::new(0)),
            type_check_errors: Arc::new(AtomicU64::new(0)),
            admission_errors: Arc::new(AtomicU64::new(0)),
            transformation_errors: Arc::new(AtomicU64::new(0)),
            invariant_errors: Arc::new(AtomicU64::new(0)),
            execution_errors: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Increment total operations counter for a given operation type
    pub fn increment_operations_total(&self, operation_type: &str) {
        // Note: In production, this would use a concurrent HashMap or similar
        // For now, we track in tracing spans
        #[cfg(feature = "tracing")]
        debug!(
            operation_type = operation_type,
            "Incrementing operations total"
        );
    }

    /// Increment stage completions counter
    pub fn increment_stage_completions(&self, stage: &str) {
        #[cfg(feature = "tracing")]
        debug!(stage = stage, "Stage completed");
    }

    /// Record a guard admission
    pub fn record_guard_admission(&self, guard_name: &str) {
        self.guard_admissions.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "tracing")]
        debug!(guard = guard_name, "Guard admission");
    }

    /// Record a guard rejection
    pub fn record_guard_rejection(&self, guard_name: &str, reason: &str) {
        self.guard_rejections.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "tracing")]
        warn!(guard = guard_name, reason = reason, "Guard rejection");
    }

    /// Record an invariant check that passed
    pub fn record_invariant_passed(&self, invariant_name: &str) {
        self.invariant_checks_passed.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "tracing")]
        debug!(invariant = invariant_name, "Invariant check passed");
    }

    /// Record an invariant check that failed
    pub fn record_invariant_failed(&self, invariant_name: &str, violation: &str) {
        self.invariant_checks_failed.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "tracing")]
        error!(
            invariant = invariant_name,
            violation = violation,
            "Invariant violation detected"
        );
    }

    /// Record a task submission to the scheduler
    pub fn record_task_submitted(&self, task_id: &str, priority: &str) {
        self.tasks_submitted.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "tracing")]
        debug!(task_id = task_id, priority = priority, "Task submitted");
    }

    /// Record a task completion
    pub fn record_task_completed(&self, task_id: &str) {
        self.tasks_completed.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "tracing")]
        info!(task_id = task_id, "Task completed");
    }

    /// Record a task cancellation
    pub fn record_task_cancelled(&self, task_id: &str) {
        self.tasks_cancelled.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "tracing")]
        warn!(task_id = task_id, "Task cancelled");
    }

    /// Record a scheduler selection decision
    pub fn record_scheduler_selection(&self, task_id: &str) {
        self.scheduler_selections.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "tracing")]
        debug!(task_id = task_id, "Scheduler selected task");
    }

    /// Record a type check error
    pub fn record_type_check_error(&self, message: &str) {
        self.type_check_errors.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "tracing")]
        error!(error = message, "Type check failed");
    }

    /// Record an admission error
    pub fn record_admission_error(&self, guard: &str, reason: &str) {
        self.admission_errors.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "tracing")]
        error!(guard = guard, reason = reason, "Admission denied");
    }

    /// Record a transformation error
    pub fn record_transformation_error(&self, message: &str) {
        self.transformation_errors.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "tracing")]
        error!(error = message, "Transformation failed");
    }

    /// Record an invariant error
    pub fn record_invariant_error(&self, invariant: &str, violation: &str) {
        self.invariant_errors.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "tracing")]
        error!(
            invariant = invariant,
            violation = violation,
            "Invariant error"
        );
    }

    /// Record an execution error
    pub fn record_execution_error(&self, message: &str) {
        self.execution_errors.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "tracing")]
        error!(error = message, "Execution failed");
    }

    /// Get snapshot of current metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            guard_admissions: self.guard_admissions.load(Ordering::Relaxed),
            guard_rejections: self.guard_rejections.load(Ordering::Relaxed),
            invariant_checks_passed: self.invariant_checks_passed.load(Ordering::Relaxed),
            invariant_checks_failed: self.invariant_checks_failed.load(Ordering::Relaxed),
            tasks_submitted: self.tasks_submitted.load(Ordering::Relaxed),
            tasks_completed: self.tasks_completed.load(Ordering::Relaxed),
            tasks_cancelled: self.tasks_cancelled.load(Ordering::Relaxed),
            scheduler_selections: self.scheduler_selections.load(Ordering::Relaxed),
            type_check_errors: self.type_check_errors.load(Ordering::Relaxed),
            admission_errors: self.admission_errors.load(Ordering::Relaxed),
            transformation_errors: self.transformation_errors.load(Ordering::Relaxed),
            invariant_errors: self.invariant_errors.load(Ordering::Relaxed),
            execution_errors: self.execution_errors.load(Ordering::Relaxed),
        }
    }

    /// Reset all metrics to zero
    pub fn reset(&self) {
        self.guard_admissions.store(0, Ordering::Relaxed);
        self.guard_rejections.store(0, Ordering::Relaxed);
        self.invariant_checks_passed.store(0, Ordering::Relaxed);
        self.invariant_checks_failed.store(0, Ordering::Relaxed);
        self.tasks_submitted.store(0, Ordering::Relaxed);
        self.tasks_completed.store(0, Ordering::Relaxed);
        self.tasks_cancelled.store(0, Ordering::Relaxed);
        self.scheduler_selections.store(0, Ordering::Relaxed);
        self.type_check_errors.store(0, Ordering::Relaxed);
        self.admission_errors.store(0, Ordering::Relaxed);
        self.transformation_errors.store(0, Ordering::Relaxed);
        self.invariant_errors.store(0, Ordering::Relaxed);
        self.execution_errors.store(0, Ordering::Relaxed);
    }
}

impl Default for RuntimeMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Point-in-time snapshot of metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSnapshot {
    pub guard_admissions: u64,
    pub guard_rejections: u64,
    pub invariant_checks_passed: u64,
    pub invariant_checks_failed: u64,
    pub tasks_submitted: u64,
    pub tasks_completed: u64,
    pub tasks_cancelled: u64,
    pub scheduler_selections: u64,
    pub type_check_errors: u64,
    pub admission_errors: u64,
    pub transformation_errors: u64,
    pub invariant_errors: u64,
    pub execution_errors: u64,
}

impl MetricsSnapshot {
    /// Calculate total errors across all categories
    pub fn total_errors(&self) -> u64 {
        self.type_check_errors
            + self.admission_errors
            + self.transformation_errors
            + self.invariant_errors
            + self.execution_errors
    }

    /// Calculate guard rejection rate (0.0 to 1.0)
    pub fn guard_rejection_rate(&self) -> f64 {
        let total = self.guard_admissions + self.guard_rejections;
        if total == 0 {
            0.0
        } else {
            self.guard_rejections as f64 / total as f64
        }
    }

    /// Calculate invariant failure rate (0.0 to 1.0)
    pub fn invariant_failure_rate(&self) -> f64 {
        let total = self.invariant_checks_passed + self.invariant_checks_failed;
        if total == 0 {
            0.0
        } else {
            self.invariant_checks_failed as f64 / total as f64
        }
    }

    /// Calculate task completion rate (0.0 to 1.0)
    pub fn task_completion_rate(&self) -> f64 {
        let total = self.tasks_submitted;
        if total == 0 {
            0.0
        } else {
            self.tasks_completed as f64 / total as f64
        }
    }
}

/// Timing information for operation execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationTiming {
    /// Operation name or identifier
    pub operation: String,

    /// Total duration in milliseconds
    pub duration_ms: u64,

    /// Per-stage timing breakdown
    pub stages: BTreeMap<String, u64>,

    /// Timestamp when operation started
    pub started_at: String,

    /// Timestamp when operation completed
    pub completed_at: String,
}

impl OperationTiming {
    /// Create a new operation timing record
    pub fn new(operation: String, duration: Duration) -> Self {
        let now = chrono::Utc::now();
        let started_at =
            (now - chrono::Duration::from_std(duration).unwrap_or_default()).to_rfc3339();
        let completed_at = now.to_rfc3339();

        Self {
            operation,
            duration_ms: duration.as_millis() as u64,
            stages: BTreeMap::new(),
            started_at,
            completed_at,
        }
    }

    /// Add a stage timing
    pub fn add_stage(&mut self, stage: String, duration_ms: u64) {
        self.stages.insert(stage, duration_ms);
    }

    /// Get the longest stage
    pub fn slowest_stage(&self) -> Option<(&str, u64)> {
        self.stages
            .iter()
            .max_by_key(|(_, duration)| *duration)
            .map(|(name, duration)| (name.as_str(), *duration))
    }
}

/// Observability context for tracking execution
///
/// Maintains metrics and provides span management for distributed tracing.
#[derive(Debug, Clone)]
pub struct ObservabilityContext {
    /// Metrics registry
    pub metrics: RuntimeMetrics,

    /// Execution ID for correlation
    pub execution_id: String,

    /// Policy epoch for audit trails
    pub policy_epoch: u64,

    /// Start time for timing measurements
    start_time: Instant,
}

impl ObservabilityContext {
    /// Create a new observability context
    pub fn new(execution_id: String, policy_epoch: u64) -> Self {
        Self {
            metrics: RuntimeMetrics::new(),
            execution_id,
            policy_epoch,
            start_time: Instant::now(),
        }
    }

    /// Create a new context with existing metrics
    pub fn with_metrics(execution_id: String, policy_epoch: u64, metrics: RuntimeMetrics) -> Self {
        Self {
            metrics,
            execution_id,
            policy_epoch,
            start_time: Instant::now(),
        }
    }

    /// Get elapsed time since context creation
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Create operation timing from elapsed time
    pub fn create_timing(&self, operation: String) -> OperationTiming {
        OperationTiming::new(operation, self.elapsed())
    }

    /// Create a tracing span for runtime execution
    #[cfg(feature = "tracing")]
    pub fn runtime_span(&self, operation: &str) -> Span {
        info_span!(
            "runtime_execution",
            execution_id = %self.execution_id,
            operation = operation,
            policy_epoch = self.policy_epoch
        )
    }

    /// Create a tracing span for a specific stage
    #[cfg(feature = "tracing")]
    pub fn stage_span(&self, stage: &str) -> Span {
        info_span!(
            "runtime_stage",
            execution_id = %self.execution_id,
            stage = stage
        )
    }

    /// Create a tracing span for guard evaluation
    #[cfg(feature = "tracing")]
    pub fn guard_span(&self, guard_name: &str) -> Span {
        info_span!(
            "guard_evaluation",
            execution_id = %self.execution_id,
            guard = guard_name,
            policy_epoch = self.policy_epoch
        )
    }

    /// Create a tracing span for invariant check
    #[cfg(feature = "tracing")]
    pub fn invariant_span(&self, invariant_name: &str) -> Span {
        info_span!(
            "invariant_check",
            execution_id = %self.execution_id,
            invariant = invariant_name
        )
    }

    /// Create a tracing span for scheduler operation
    #[cfg(feature = "tracing")]
    pub fn scheduler_span(&self, operation: &str) -> Span {
        info_span!(
            "scheduler_operation",
            execution_id = %self.execution_id,
            operation = operation
        )
    }
}

/// Helper to create an instrumented guard wrapper
///
/// Wraps any guard with observability instrumentation, recording
/// admissions, rejections, and timing information.
#[cfg(feature = "tracing")]
#[derive(Debug, Clone)]
pub struct InstrumentedGuard<G> {
    guard: G,
    metrics: RuntimeMetrics,
}

#[cfg(feature = "tracing")]
impl<G> InstrumentedGuard<G> {
    /// Create a new instrumented guard
    pub fn new(guard: G, metrics: RuntimeMetrics) -> Self {
        Self { guard, metrics }
    }
}

#[cfg(feature = "tracing")]
impl<G: crate::construct::guards::Guard> crate::construct::guards::Guard for InstrumentedGuard<G> {
    #[instrument(skip(self, input), fields(guard = self.guard.name(), policy_epoch = policy_epoch))]
    fn check(
        &self,
        input: &serde_json::Value,
        context: &str,
        policy_epoch: u64,
    ) -> Result<(), crate::construct::guards::RefusalReceipt> {
        let start = Instant::now();
        let result = self.guard.check(input, context, policy_epoch);
        let duration = start.elapsed();

        match &result {
            Ok(_) => {
                self.metrics.record_guard_admission(self.guard.name());
                debug!(
                    guard = self.guard.name(),
                    duration_us = duration.as_micros(),
                    "Guard admitted"
                );
            }
            Err(receipt) => {
                self.metrics
                    .record_guard_rejection(self.guard.name(), &receipt.reason);
                warn!(
                    guard = self.guard.name(),
                    duration_us = duration.as_micros(),
                    refusal_code = ?receipt.code,
                    "Guard rejected"
                );
            }
        }

        result
    }

    fn name(&self) -> &str {
        self.guard.name()
    }

    fn description(&self) -> String {
        self.guard.description()
    }
}

/// Helper to create an instrumented invariant wrapper
///
/// Wraps any invariant with observability instrumentation, recording
/// passes, failures, and timing information.
#[cfg(feature = "tracing")]
#[derive(Debug, Clone)]
pub struct InstrumentedInvariant<I> {
    invariant: I,
    metrics: RuntimeMetrics,
}

#[cfg(feature = "tracing")]
impl<I> InstrumentedInvariant<I> {
    /// Create a new instrumented invariant
    pub fn new(invariant: I, metrics: RuntimeMetrics) -> Self {
        Self { invariant, metrics }
    }
}

#[cfg(feature = "tracing")]
impl<T, I: crate::construct::invariants::Invariant<T>> crate::construct::invariants::Invariant<T>
    for InstrumentedInvariant<I>
{
    #[instrument(skip(self, value), fields(invariant = self.invariant.name()))]
    fn check(&self, value: &T) -> crate::construct::invariants::InvariantResult {
        let start = Instant::now();
        let result = self.invariant.check(value);
        let duration = start.elapsed();

        match &result {
            Ok(_) => {
                self.metrics.record_invariant_passed(self.invariant.name());
                debug!(
                    invariant = self.invariant.name(),
                    duration_us = duration.as_micros(),
                    "Invariant passed"
                );
            }
            Err(violation) => {
                self.metrics
                    .record_invariant_failed(self.invariant.name(), &violation.to_string());
                error!(
                    invariant = self.invariant.name(),
                    duration_us = duration.as_micros(),
                    violation = %violation,
                    "Invariant failed"
                );
            }
        }

        result
    }

    fn name(&self) -> &str {
        self.invariant.name()
    }

    fn description(&self) -> &str {
        self.invariant.description()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = RuntimeMetrics::new();
        let snapshot = metrics.snapshot();

        assert_eq!(snapshot.guard_admissions, 0);
        assert_eq!(snapshot.guard_rejections, 0);
        assert_eq!(snapshot.total_errors(), 0);
    }

    #[test]
    fn test_metrics_recording() {
        let metrics = RuntimeMetrics::new();

        metrics.record_guard_admission("test_guard");
        metrics.record_guard_rejection("test_guard", "test reason");
        metrics.record_invariant_passed("test_invariant");
        metrics.record_task_submitted("task-1", "normal");

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.guard_admissions, 1);
        assert_eq!(snapshot.guard_rejections, 1);
        assert_eq!(snapshot.invariant_checks_passed, 1);
        assert_eq!(snapshot.tasks_submitted, 1);
    }

    #[test]
    fn test_metrics_rates() {
        let metrics = RuntimeMetrics::new();

        metrics.record_guard_admission("g1");
        metrics.record_guard_admission("g2");
        metrics.record_guard_rejection("g3", "reason");

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.guard_rejection_rate(), 1.0 / 3.0);
    }

    #[test]
    fn test_metrics_reset() {
        let metrics = RuntimeMetrics::new();

        metrics.record_guard_admission("test");
        metrics.record_task_submitted("task-1", "normal");

        metrics.reset();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.guard_admissions, 0);
        assert_eq!(snapshot.tasks_submitted, 0);
    }

    #[test]
    fn test_operation_timing() {
        let mut timing = OperationTiming::new("test_op".to_string(), Duration::from_millis(100));

        timing.add_stage("stage1".to_string(), 30);
        timing.add_stage("stage2".to_string(), 70);

        assert_eq!(timing.duration_ms, 100);
        assert_eq!(timing.stages.len(), 2);

        let (slowest, duration) = timing.slowest_stage().unwrap();
        assert_eq!(slowest, "stage2");
        assert_eq!(duration, 70);
    }

    #[test]
    fn test_observability_context() {
        let ctx = ObservabilityContext::new("exec-123".to_string(), 42);

        assert_eq!(ctx.execution_id, "exec-123");
        assert_eq!(ctx.policy_epoch, 42);
        assert!(ctx.elapsed().as_nanos() > 0);
    }

    #[test]
    fn test_metrics_snapshot_serialization() {
        let metrics = RuntimeMetrics::new();
        metrics.record_guard_admission("test");

        let snapshot = metrics.snapshot();
        let json = serde_json::to_string(&snapshot).unwrap();

        assert!(json.contains("guardAdmissions"));
        assert!(json.contains("\"1\""));
    }
}
