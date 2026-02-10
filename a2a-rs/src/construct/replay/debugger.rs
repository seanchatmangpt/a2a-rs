//! Interactive replay debugger for stepping through receipt chains.
//!
//! This module provides an interactive debugger that allows stepping through
//! recorded execution steps, inspecting state at each point, and comparing
//! actual vs expected behavior.
//!
//! # Features
//!
//! - Step forward/backward through receipt chain
//! - Jump to specific receipts by index or hash
//! - Inspect state at any step
//! - Diff states between steps or against expected
//! - Display receipt chain visualization
//! - Breakpoint support on state predicates
//!
//! # Example
//!
//! ```rust
//! # use a2a_rs::construct::replay::debugger::ReplayDebugger;
//! # use a2a_rs::construct::replay::{ExecutionRecorder, StateSnapshot};
//! // Load a recording
//! let recorder = ExecutionRecorder::new();
//! let debugger = ReplayDebugger::from_recorder(recorder);
//!
//! // Step through execution
//! debugger.step_forward();
//! debugger.inspect_current_state();
//! debugger.diff_states(0, 1);
//! ```

use serde::{Deserialize, Serialize};

use crate::construct::runtime::{ExecutionReceipt, Operation};

// Import core replay types from sibling module
use super::core::{DifferenceKind, ExecutionRecorder, RecordedStep, SnapshotDiff, StateSnapshot};

#[cfg(feature = "tracing")]
use tracing::{debug, info, warn};

/// Interactive debugger for stepping through recorded executions.
///
/// Provides a REPL-style interface for inspecting and comparing execution states.
#[derive(Debug, Clone)]
pub struct ReplayDebugger {
    /// All recorded steps
    steps: Vec<RecordedStep>,

    /// Current step index (None = before first step)
    current_position: Option<usize>,

    /// Breakpoints defined by step index
    breakpoints: Vec<usize>,

    /// Display configuration
    config: DebuggerConfig,
}

/// Configuration for the debugger display and behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebuggerConfig {
    /// Show full receipt details vs abbreviated
    pub verbose_receipts: bool,

    /// Show full state snapshots vs summary
    pub verbose_state: bool,

    /// Maximum diff lines to display
    pub max_diff_lines: usize,

    /// Auto-break on state differences
    pub break_on_diff: bool,
}

impl Default for DebuggerConfig {
    fn default() -> Self {
        Self {
            verbose_receipts: false,
            verbose_state: false,
            max_diff_lines: 50,
            break_on_diff: false,
        }
    }
}

/// Result of a debugger step operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepResult {
    /// Successfully moved to new position
    Moved {
        from: Option<usize>,
        to: Option<usize>,
    },

    /// Already at beginning/end
    AtBoundary,

    /// Invalid target position
    InvalidPosition,

    /// Hit a breakpoint
    BreakpointHit { at: usize },
}

/// Information about the current debugger state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebuggerStatus {
    /// Total number of steps
    pub total_steps: usize,

    /// Current position (None = before first step)
    pub current_position: Option<usize>,

    /// Number of breakpoints set
    pub breakpoint_count: usize,

    /// Current step summary
    pub current_step_summary: Option<String>,
}

impl ReplayDebugger {
    /// Creates a new debugger from a sequence of recorded steps.
    pub fn new(steps: Vec<RecordedStep>) -> Self {
        Self {
            steps,
            current_position: None,
            breakpoints: Vec::new(),
            config: DebuggerConfig::default(),
        }
    }

    /// Creates a debugger from an ExecutionRecorder.
    pub fn from_recorder(recorder: &ExecutionRecorder) -> Self {
        Self::new(recorder.steps().to_vec())
    }

    /// Returns the total number of steps in the recording.
    pub fn total_steps(&self) -> usize {
        self.steps.len()
    }

    /// Returns the current position in the recording.
    pub fn current_position(&self) -> Option<usize> {
        self.current_position
    }

    /// Returns the current step, if positioned at one.
    pub fn current_step(&self) -> Option<&RecordedStep> {
        self.current_position.and_then(|pos| self.steps.get(pos))
    }

    /// Gets the state snapshot at the current position.
    pub fn current_state(&self) -> Option<&StateSnapshot> {
        self.current_step().map(|step| &step.state_after)
    }

    /// Steps forward one step in the recording.
    ///
    /// Returns `StepResult` indicating what happened.
    pub fn step_forward(&mut self) -> StepResult {
        let from = self.current_position;

        let to = match self.current_position {
            None => {
                if self.steps.is_empty() {
                    return StepResult::AtBoundary;
                }
                Some(0)
            }
            Some(pos) if pos + 1 < self.steps.len() => Some(pos + 1),
            _ => return StepResult::AtBoundary,
        };

        self.current_position = to;

        // Check for breakpoint
        if let Some(pos) = to {
            if self.breakpoints.contains(&pos) {
                return StepResult::BreakpointHit { at: pos };
            }
        }

        StepResult::Moved { from, to }
    }

    /// Steps backward one step in the recording.
    pub fn step_back(&mut self) -> StepResult {
        let from = self.current_position;

        let to = match self.current_position {
            None => return StepResult::AtBoundary,
            Some(0) => None,
            Some(pos) => Some(pos - 1),
        };

        self.current_position = to;

        StepResult::Moved { from, to }
    }

    /// Jumps to a specific step index.
    ///
    /// Returns `StepResult::InvalidPosition` if index is out of bounds.
    pub fn goto_step(&mut self, index: usize) -> StepResult {
        if index >= self.steps.len() {
            return StepResult::InvalidPosition;
        }

        let from = self.current_position;
        self.current_position = Some(index);

        if self.breakpoints.contains(&index) {
            StepResult::BreakpointHit { at: index }
        } else {
            StepResult::Moved {
                from,
                to: Some(index),
            }
        }
    }

    /// Jumps to the step with the given receipt hash.
    ///
    /// Returns the step index if found, None otherwise.
    #[cfg(feature = "receipts")]
    pub fn goto_receipt(&mut self, receipt_hash: &str) -> Option<usize> {
        for (idx, step) in self.steps.iter().enumerate() {
            if let Some(ref receipt) = step.receipt {
                if receipt.receipt_hash == receipt_hash {
                    self.current_position = Some(idx);
                    return Some(idx);
                }
            }
        }
        None
    }

    /// Resets to the beginning (before first step).
    pub fn reset(&mut self) {
        self.current_position = None;
    }

    /// Jumps to the end (last step).
    pub fn goto_end(&mut self) -> StepResult {
        if self.steps.is_empty() {
            return StepResult::AtBoundary;
        }

        let from = self.current_position;
        let to = Some(self.steps.len() - 1);

        self.current_position = to;

        StepResult::Moved { from, to }
    }

    /// Computes the diff between two state snapshots.
    ///
    /// Returns None if either step index is invalid.
    pub fn diff_states(&self, step1: usize, step2: usize) -> Option<SnapshotDiff> {
        let state1 = self.steps.get(step1).map(|s| &s.state_after)?;
        let state2 = self.steps.get(step2).map(|s| &s.state_after)?;

        Some(state1.diff(state2))
    }

    /// Computes the diff between current state and a specific step.
    pub fn diff_with_current(&self, step: usize) -> Option<SnapshotDiff> {
        let current_pos = self.current_position?;
        self.diff_states(current_pos, step)
    }

    /// Computes the diff between current state and previous step.
    pub fn diff_with_previous(&self) -> Option<SnapshotDiff> {
        let current_pos = self.current_position?;
        if current_pos == 0 {
            return None;
        }
        self.diff_states(current_pos - 1, current_pos)
    }

    /// Adds a breakpoint at the given step index.
    ///
    /// Returns true if breakpoint was added, false if invalid index.
    pub fn add_breakpoint(&mut self, step: usize) -> bool {
        if step >= self.steps.len() {
            return false;
        }

        if !self.breakpoints.contains(&step) {
            self.breakpoints.push(step);
            self.breakpoints.sort_unstable();
        }

        true
    }

    /// Removes a breakpoint at the given step index.
    pub fn remove_breakpoint(&mut self, step: usize) -> bool {
        if let Some(pos) = self.breakpoints.iter().position(|&b| b == step) {
            self.breakpoints.remove(pos);
            true
        } else {
            false
        }
    }

    /// Lists all active breakpoints.
    pub fn list_breakpoints(&self) -> &[usize] {
        &self.breakpoints
    }

    /// Clears all breakpoints.
    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    /// Continues execution until next breakpoint or end.
    pub fn continue_execution(&mut self) -> StepResult {
        loop {
            match self.step_forward() {
                StepResult::BreakpointHit { at } => {
                    return StepResult::BreakpointHit { at };
                }
                StepResult::AtBoundary => {
                    return StepResult::AtBoundary;
                }
                StepResult::Moved { .. } => {
                    // Keep going
                    continue;
                }
                other => return other,
            }
        }
    }

    /// Returns the debugger status.
    pub fn status(&self) -> DebuggerStatus {
        let current_step_summary = self.current_step().map(|step| {
            format!(
                "Step {}: {}",
                step.step_number,
                operation_summary(&step.operation)
            )
        });

        DebuggerStatus {
            total_steps: self.steps.len(),
            current_position: self.current_position,
            breakpoint_count: self.breakpoints.len(),
            current_step_summary,
        }
    }

    /// Updates the debugger configuration.
    pub fn configure(&mut self, config: DebuggerConfig) {
        self.config = config;
    }

    /// Returns the current configuration.
    pub fn config(&self) -> &DebuggerConfig {
        &self.config
    }

    /// Searches for steps matching a predicate.
    ///
    /// Returns indices of matching steps.
    pub fn search<F>(&self, predicate: F) -> Vec<usize>
    where
        F: Fn(&RecordedStep) -> bool,
    {
        self.steps
            .iter()
            .enumerate()
            .filter_map(|(idx, step)| if predicate(step) { Some(idx) } else { None })
            .collect()
    }

    /// Searches for steps with specific operation type.
    pub fn search_by_operation(&self, operation_type: &str) -> Vec<usize> {
        self.search(|step| operation_summary(&step.operation).contains(operation_type))
    }

    /// Gets a detailed report for the current step.
    pub fn inspect_current(&self) -> Option<StepReport> {
        let step = self.current_step()?;

        Some(StepReport {
            step_number: step.step_number,
            operation: operation_summary(&step.operation),
            operation_details: format!("{:#?}", step.operation),
            execution_success: step.execution_receipt.success,
            stages_completed: step.execution_receipt.stages_completed.clone(),
            state_before_summary: state_summary(&step.state_before),
            state_after_summary: state_summary(&step.state_after),
            state_diff: step.state_before.diff(&step.state_after),
            #[cfg(feature = "receipts")]
            receipt_hash: step.receipt.as_ref().map(|r| r.receipt_hash.clone()),
        })
    }

    /// Gets all steps as a list of summaries.
    pub fn list_all_steps(&self) -> Vec<StepSummary> {
        self.steps
            .iter()
            .map(|step| StepSummary {
                step_number: step.step_number,
                operation: operation_summary(&step.operation),
                success: step.execution_receipt.success,
                has_breakpoint: self.breakpoints.contains(&step.step_number),
            })
            .collect()
    }

    /// Exports the current debugger state as JSON.
    pub fn export_state(&self) -> Result<String, serde_json::Error> {
        #[derive(Serialize)]
        struct DebuggerExport<'a> {
            total_steps: usize,
            current_position: Option<usize>,
            breakpoints: &'a [usize],
            steps: &'a [RecordedStep],
            config: &'a DebuggerConfig,
        }

        let export = DebuggerExport {
            total_steps: self.steps.len(),
            current_position: self.current_position,
            breakpoints: &self.breakpoints,
            steps: &self.steps,
            config: &self.config,
        };

        serde_json::to_string_pretty(&export)
    }
}

/// Detailed report for a single step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepReport {
    pub step_number: usize,
    pub operation: String,
    pub operation_details: String,
    pub execution_success: bool,
    pub stages_completed: Vec<String>,
    pub state_before_summary: String,
    pub state_after_summary: String,
    pub state_diff: SnapshotDiff,
    #[cfg(feature = "receipts")]
    pub receipt_hash: Option<String>,
}

/// Brief summary of a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepSummary {
    pub step_number: usize,
    pub operation: String,
    pub success: bool,
    pub has_breakpoint: bool,
}

// Helper functions

fn operation_summary(op: &Operation) -> String {
    match op {
        Operation::CreateTask { task, .. } => format!("CreateTask({})", task.id),
        Operation::SendMessage { task_id, .. } => format!("SendMessage({})", task_id),
        Operation::UpdateTaskState { task_id, state } => {
            format!("UpdateTaskState({}, {:?})", task_id, state)
        }
        Operation::AddArtifact { task_id, .. } => format!("AddArtifact({})", task_id),
        Operation::CompleteTask { task_id, .. } => format!("CompleteTask({})", task_id),
        Operation::CancelTask { task_id } => format!("CancelTask({})", task_id),
    }
}

fn state_summary(state: &StateSnapshot) -> String {
    format!(
        "tasks={}, messages={}, artifacts={}",
        state.tasks.len(),
        state
            .messages
            .values()
            .map(|v: &Vec<_>| v.len())
            .sum::<usize>(),
        state
            .artifacts
            .values()
            .map(|v: &Vec<_>| v.len())
            .sum::<usize>()
    )
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::construct::runtime::ExecutionReceipt;
    use crate::domain::{Task, TaskStatus};

    fn create_test_step(num: usize) -> RecordedStep {
        let task = Task::builder()
            .id(format!("task-{}", num))
            .context_id("ctx-1".to_string())
            .status(TaskStatus::default())
            .build();

        RecordedStep {
            step_number: num,
            operation: Operation::CreateTask {
                task,
                initial_message: None,
                priority: None,
            },
            state_before: StateSnapshot::empty(),
            state_after: StateSnapshot::empty(),
            execution_receipt: ExecutionReceipt {
                execution_id: format!("exec-{}", num),
                operation: "CreateTask".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms: 10,
                stages_completed: vec!["type_check".to_string()],
                success: true,
                policy_epoch: 0,
            },
            #[cfg(feature = "receipts")]
            receipt: None,
        }
    }

    #[test]
    fn test_debugger_creation() {
        let steps = vec![create_test_step(0), create_test_step(1)];
        let debugger = ReplayDebugger::new(steps);

        assert_eq!(debugger.total_steps(), 2);
        assert_eq!(debugger.current_position(), None);
    }

    #[test]
    fn test_step_forward() {
        let steps = vec![create_test_step(0), create_test_step(1)];
        let mut debugger = ReplayDebugger::new(steps);

        // Start before first step
        assert_eq!(debugger.current_position(), None);

        // Step forward to first step
        let result = debugger.step_forward();
        assert_eq!(
            result,
            StepResult::Moved {
                from: None,
                to: Some(0)
            }
        );
        assert_eq!(debugger.current_position(), Some(0));

        // Step forward to second step
        let result = debugger.step_forward();
        assert_eq!(
            result,
            StepResult::Moved {
                from: Some(0),
                to: Some(1)
            }
        );
        assert_eq!(debugger.current_position(), Some(1));

        // Try to step past end
        let result = debugger.step_forward();
        assert_eq!(result, StepResult::AtBoundary);
        assert_eq!(debugger.current_position(), Some(1));
    }

    #[test]
    fn test_step_back() {
        let steps = vec![create_test_step(0), create_test_step(1)];
        let mut debugger = ReplayDebugger::new(steps);

        // Go to end
        debugger.goto_end();
        assert_eq!(debugger.current_position(), Some(1));

        // Step back
        let result = debugger.step_back();
        assert_eq!(
            result,
            StepResult::Moved {
                from: Some(1),
                to: Some(0)
            }
        );

        // Step back to beginning
        let result = debugger.step_back();
        assert_eq!(
            result,
            StepResult::Moved {
                from: Some(0),
                to: None
            }
        );

        // Try to step before beginning
        let result = debugger.step_back();
        assert_eq!(result, StepResult::AtBoundary);
    }

    #[test]
    fn test_goto_step() {
        let steps = vec![
            create_test_step(0),
            create_test_step(1),
            create_test_step(2),
        ];
        let mut debugger = ReplayDebugger::new(steps);

        // Jump to middle
        let result = debugger.goto_step(1);
        assert_eq!(
            result,
            StepResult::Moved {
                from: None,
                to: Some(1)
            }
        );
        assert_eq!(debugger.current_position(), Some(1));

        // Jump to invalid
        let result = debugger.goto_step(10);
        assert_eq!(result, StepResult::InvalidPosition);
        assert_eq!(debugger.current_position(), Some(1)); // Unchanged
    }

    #[test]
    fn test_reset_and_goto_end() {
        let steps = vec![create_test_step(0), create_test_step(1)];
        let mut debugger = ReplayDebugger::new(steps);

        debugger.step_forward();
        assert_eq!(debugger.current_position(), Some(0));

        debugger.reset();
        assert_eq!(debugger.current_position(), None);

        debugger.goto_end();
        assert_eq!(debugger.current_position(), Some(1));
    }

    #[test]
    fn test_breakpoints() {
        let steps = vec![
            create_test_step(0),
            create_test_step(1),
            create_test_step(2),
        ];
        let mut debugger = ReplayDebugger::new(steps);

        // Add breakpoint at step 1
        assert!(debugger.add_breakpoint(1));
        assert_eq!(debugger.list_breakpoints(), &[1]);

        // Step to breakpoint
        debugger.step_forward(); // Step 0
        let result = debugger.step_forward(); // Step 1 (breakpoint)

        assert_eq!(result, StepResult::BreakpointHit { at: 1 });

        // Remove breakpoint
        assert!(debugger.remove_breakpoint(1));
        assert!(debugger.list_breakpoints().is_empty());
    }

    #[test]
    fn test_continue_execution() {
        let steps = vec![
            create_test_step(0),
            create_test_step(1),
            create_test_step(2),
            create_test_step(3),
        ];
        let mut debugger = ReplayDebugger::new(steps);

        // Set breakpoint at step 2
        debugger.add_breakpoint(2);

        // Continue from start
        let result = debugger.continue_execution();
        assert_eq!(result, StepResult::BreakpointHit { at: 2 });
        assert_eq!(debugger.current_position(), Some(2));

        // Continue to end
        debugger.clear_breakpoints();
        let result = debugger.continue_execution();
        assert_eq!(result, StepResult::AtBoundary);
        assert_eq!(debugger.current_position(), Some(3));
    }

    #[test]
    fn test_diff_states() {
        let mut step0 = create_test_step(0);
        let mut step1 = create_test_step(1);

        // Modify state_after for step 0 to have a task
        step1.state_after.tasks.insert(
            "task-1".to_string(),
            Task::builder()
                .id("task-1".to_string())
                .context_id("ctx-1".to_string())
                .status(TaskStatus::default())
                .build(),
        );

        let steps = vec![step0, step1];
        let debugger = ReplayDebugger::new(steps);

        let diff = debugger.diff_states(0, 1).unwrap();
        assert!(!diff.is_identical);
    }

    #[test]
    fn test_status() {
        let steps = vec![create_test_step(0), create_test_step(1)];
        let mut debugger = ReplayDebugger::new(steps);

        debugger.step_forward();

        let status = debugger.status();
        assert_eq!(status.total_steps, 2);
        assert_eq!(status.current_position, Some(0));
        assert!(status.current_step_summary.is_some());
    }

    #[test]
    fn test_search() {
        let steps = vec![
            create_test_step(0),
            create_test_step(1),
            create_test_step(2),
        ];
        let debugger = ReplayDebugger::new(steps);

        // Search for all CreateTask operations
        let results = debugger.search_by_operation("CreateTask");
        assert_eq!(results.len(), 3);

        // Search for specific step number
        let results = debugger.search(|step| step.step_number == 1);
        assert_eq!(results, vec![1]);
    }

    #[test]
    fn test_inspect_current() {
        let steps = vec![create_test_step(0)];
        let mut debugger = ReplayDebugger::new(steps);

        // No current step initially
        assert!(debugger.inspect_current().is_none());

        // Step forward and inspect
        debugger.step_forward();
        let report = debugger.inspect_current().unwrap();

        assert_eq!(report.step_number, 0);
        assert!(report.operation.contains("CreateTask"));
        assert!(report.execution_success);
    }

    #[test]
    fn test_list_all_steps() {
        let steps = vec![create_test_step(0), create_test_step(1)];
        let mut debugger = ReplayDebugger::new(steps);

        debugger.add_breakpoint(1);

        let summaries = debugger.list_all_steps();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].step_number, 0);
        assert!(!summaries[0].has_breakpoint);
        assert!(summaries[1].has_breakpoint);
    }

    #[test]
    fn test_export_state() {
        let steps = vec![create_test_step(0)];
        let debugger = ReplayDebugger::new(steps);

        let json = debugger.export_state().unwrap();
        assert!(json.contains("total_steps"));
        assert!(json.contains("current_position"));
    }
}
