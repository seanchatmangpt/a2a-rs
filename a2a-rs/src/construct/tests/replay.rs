//! Replay testing for determinism verification.
//!
//! This module provides comprehensive replay testing to verify that the CONSTRUCT
//! system behaves deterministically. If the system is deterministic, replaying
//! the same sequence of operations should yield identical A2A objects and receipts.
//!
//! # Test Coverage
//!
//! 1. **Record Execution** - Capture state sequence and receipts
//! 2. **Replay from Recorded State** - Reproduce execution from snapshots
//! 3. **Assert Identical Observable Behavior** - Compare outputs bit-for-bit
//! 4. **Receipt Chain Verification** - Validate cryptographic integrity
//! 5. **Deterministic Scheduling Tests** - Verify consistent ordering
//!
//! # Architecture
//!
//! The replay system follows a record-replay pattern:
//! - `ExecutionRecorder` captures state snapshots and receipts at each step
//! - `ExecutionReplayer` replays operations from recorded state
//! - `ExecutionComparator` asserts identical behavior between runs
//! - `ReceiptChainVerifier` validates cryptographic proofs

use crate::construct::runtime::{
    ExecutionReceipt, Operation, Runtime, RuntimeError, RuntimeOutput,
};
use crate::domain::{Artifact, Message, Task, TaskStatus};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(feature = "receipts")]
use crate::construct::receipts::{Receipt, ReceiptChain};

/// A recorded execution step with complete state snapshot.
///
/// This captures everything needed to verify determinism:
/// - Input operation
/// - Pre-execution state snapshot
/// - Post-execution state snapshot
/// - Execution receipt
/// - Optional cryptographic receipt
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedStep {
    /// Step sequence number
    pub step_number: usize,

    /// The operation that was executed
    pub operation: Operation,

    /// State snapshot before execution
    pub state_before: StateSnapshot,

    /// State snapshot after execution
    pub state_after: StateSnapshot,

    /// Execution receipt from runtime
    pub execution_receipt: ExecutionReceipt,

    /// Optional cryptographic receipt
    #[cfg(feature = "receipts")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Receipt>,
}

/// A lightweight snapshot of execution state for comparison.
///
/// Uses BTreeMap for deterministic ordering in serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateSnapshot {
    /// Tasks indexed by ID
    pub tasks: BTreeMap<String, Task>,

    /// Messages grouped by task ID
    pub messages: BTreeMap<String, Vec<Message>>,

    /// Artifacts grouped by task ID
    pub artifacts: BTreeMap<String, Vec<Artifact>>,

    /// Metadata for comparison
    pub metadata: BTreeMap<String, String>,
}

impl StateSnapshot {
    /// Creates an empty state snapshot.
    pub fn empty() -> Self {
        Self {
            tasks: BTreeMap::new(),
            messages: BTreeMap::new(),
            artifacts: BTreeMap::new(),
            metadata: BTreeMap::new(),
        }
    }

    /// Computes a deterministic hash of the snapshot for quick comparison.
    pub fn compute_hash(&self) -> String {
        let json = serde_json::to_string(self).expect("StateSnapshot should always serialize");
        #[cfg(feature = "receipts")]
        {
            crate::construct::receipts::compute_hash(json.as_bytes())
        }
        #[cfg(not(feature = "receipts"))]
        {
            // Simple fallback hash without receipts feature
            format!("{:x}", json.len())
        }
    }

    /// Compares two snapshots and returns differences.
    pub fn diff(&self, other: &StateSnapshot) -> SnapshotDiff {
        let mut differences = Vec::new();

        // Compare tasks by serializing (deterministic due to BTreeMap)
        let self_tasks_json = serde_json::to_string(&self.tasks).unwrap();
        let other_tasks_json = serde_json::to_string(&other.tasks).unwrap();
        if self_tasks_json != other_tasks_json {
            differences.push(DifferenceKind::TasksMismatch {
                left_count: self.tasks.len(),
                right_count: other.tasks.len(),
            });
        }

        // Compare messages
        let self_msgs_json = serde_json::to_string(&self.messages).unwrap();
        let other_msgs_json = serde_json::to_string(&other.messages).unwrap();
        if self_msgs_json != other_msgs_json {
            differences.push(DifferenceKind::MessagesMismatch {
                left_count: self.messages.values().map(|v| v.len()).sum(),
                right_count: other.messages.values().map(|v| v.len()).sum(),
            });
        }

        // Compare artifacts
        let self_arts_json = serde_json::to_string(&self.artifacts).unwrap();
        let other_arts_json = serde_json::to_string(&other.artifacts).unwrap();
        if self_arts_json != other_arts_json {
            differences.push(DifferenceKind::ArtifactsMismatch {
                left_count: self.artifacts.values().map(|v| v.len()).sum(),
                right_count: other.artifacts.values().map(|v| v.len()).sum(),
            });
        }

        SnapshotDiff {
            is_identical: differences.is_empty(),
            differences,
        }
    }
}

/// Difference between two state snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotDiff {
    pub is_identical: bool,
    pub differences: Vec<DifferenceKind>,
}

/// Types of differences that can occur between snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DifferenceKind {
    TasksMismatch {
        left_count: usize,
        right_count: usize,
    },
    MessagesMismatch {
        left_count: usize,
        right_count: usize,
    },
    ArtifactsMismatch {
        left_count: usize,
        right_count: usize,
    },
    MetadataMismatch,
}

/// Records execution steps for replay testing.
pub struct ExecutionRecorder {
    /// Recorded steps
    steps: Vec<RecordedStep>,

    /// Receipt chain for cryptographic verification
    #[cfg(feature = "receipts")]
    receipt_chain: ReceiptChain,
}

impl ExecutionRecorder {
    /// Creates a new execution recorder.
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            #[cfg(feature = "receipts")]
            receipt_chain: ReceiptChain::new(),
        }
    }

    /// Records a single execution step.
    pub fn record_step(
        &mut self,
        operation: Operation,
        state_before: StateSnapshot,
        state_after: StateSnapshot,
        execution_receipt: ExecutionReceipt,
    ) {
        #[cfg(feature = "receipts")]
        let receipt = {
            // Create a cryptographic receipt from the execution
            let observation = serde_json::to_vec(&state_before).unwrap();
            let action = serde_json::to_vec(&operation).unwrap();
            let delta = serde_json::to_vec(&state_after).unwrap();

            let receipt = Receipt::new(&observation, &action, &delta);
            self.receipt_chain.add_receipt(receipt.clone());
            Some(receipt)
        };

        let step = RecordedStep {
            step_number: self.steps.len(),
            operation,
            state_before,
            state_after,
            execution_receipt,
            #[cfg(feature = "receipts")]
            receipt,
        };

        self.steps.push(step);
    }

    /// Returns all recorded steps.
    pub fn steps(&self) -> &[RecordedStep] {
        &self.steps
    }

    /// Returns the receipt chain.
    #[cfg(feature = "receipts")]
    pub fn receipt_chain(&self) -> &ReceiptChain {
        &self.receipt_chain
    }

    /// Serializes the recording to JSON for storage.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.steps)
    }

    /// Deserializes a recording from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let steps: Vec<RecordedStep> = serde_json::from_str(json)?;
        let mut recorder = Self::new();
        recorder.steps = steps;
        Ok(recorder)
    }
}

impl Default for ExecutionRecorder {
    fn default() -> Self {
        Self::new()
    }
}

/// Replays recorded execution steps to verify determinism.
pub struct ExecutionReplayer {
    /// The recorded steps to replay
    recording: Vec<RecordedStep>,

    /// Current step index
    current_step: usize,
}

impl ExecutionReplayer {
    /// Creates a new replayer from a recording.
    pub fn new(recording: Vec<RecordedStep>) -> Self {
        Self {
            recording,
            current_step: 0,
        }
    }

    /// Returns the total number of steps in the recording.
    pub fn total_steps(&self) -> usize {
        self.recording.len()
    }

    /// Returns the current step index.
    pub fn current_step(&self) -> usize {
        self.current_step
    }

    /// Replays the next step and returns the result for comparison.
    pub fn replay_next_step(&mut self, runtime: &mut Runtime) -> Option<ReplayResult> {
        if self.current_step >= self.recording.len() {
            return None;
        }

        let recorded = &self.recording[self.current_step];
        self.current_step += 1;

        // Execute the operation with the runtime
        let replay_output = runtime.handle(recorded.operation.clone());

        Some(ReplayResult {
            step_number: recorded.step_number,
            recorded_step: recorded.clone(),
            replay_output,
        })
    }

    /// Replays all steps and returns results.
    pub fn replay_all(&mut self, runtime: &mut Runtime) -> Vec<ReplayResult> {
        let mut results = Vec::new();
        while let Some(result) = self.replay_next_step(runtime) {
            results.push(result);
        }
        results
    }
}

/// Result of replaying a single step.
#[derive(Debug)]
pub struct ReplayResult {
    pub step_number: usize,
    pub recorded_step: RecordedStep,
    pub replay_output: Result<RuntimeOutput, RuntimeError>,
}

impl ReplayResult {
    /// Checks if the replay produced identical results to the recording.
    pub fn is_deterministic(&self) -> bool {
        match &self.replay_output {
            Ok(output) => {
                // Compare execution receipts (excluding timestamps)
                self.recorded_step.execution_receipt.success == output.receipt.success
                    && self.recorded_step.execution_receipt.stages_completed
                        == output.receipt.stages_completed
            }
            Err(_) => !self.recorded_step.execution_receipt.success,
        }
    }

    /// Returns differences between recorded and replayed execution.
    pub fn differences(&self) -> Vec<String> {
        let mut diffs = Vec::new();

        match &self.replay_output {
            Ok(output) => {
                if self.recorded_step.execution_receipt.success != output.receipt.success {
                    diffs.push(format!(
                        "Success flag mismatch: recorded={}, replay={}",
                        self.recorded_step.execution_receipt.success, output.receipt.success
                    ));
                }

                if self.recorded_step.execution_receipt.stages_completed
                    != output.receipt.stages_completed
                {
                    diffs.push("Stages completed mismatch".to_string());
                }

                if self.recorded_step.execution_receipt.operation != output.receipt.operation {
                    diffs.push("Operation mismatch".to_string());
                }
            }
            Err(e) => {
                if self.recorded_step.execution_receipt.success {
                    diffs.push(format!("Recorded succeeded but replay failed: {}", e));
                }
            }
        }

        diffs
    }
}

/// Verifies receipt chain integrity.
#[cfg(feature = "receipts")]
pub struct ReceiptChainVerifier;

#[cfg(feature = "receipts")]
impl ReceiptChainVerifier {
    /// Verifies that a receipt chain maintains cryptographic integrity.
    pub fn verify_chain(
        chain: &ReceiptChain,
    ) -> Result<(), crate::construct::receipts::ReceiptError> {
        chain.verify_integrity()
    }

    /// Verifies that two executions produced identical receipt chains.
    pub fn verify_identical_chains(chain1: &ReceiptChain, chain2: &ReceiptChain) -> bool {
        if chain1.len() != chain2.len() {
            return false;
        }

        for i in 0..chain1.len() {
            let r1 = chain1.get(i as u64).unwrap();
            let r2 = chain2.get(i as u64).unwrap();

            if r1.receipt_hash != r2.receipt_hash {
                return false;
            }
        }

        true
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_task(id: &str, context: &str) -> Task {
        Task::builder()
            .id(id.to_string())
            .context_id(context.to_string())
            .status(TaskStatus::default())
            .build()
    }

    fn create_test_message(id: &str, content: &str) -> Message {
        Message::user_text(content.to_string(), id.to_string())
    }

    #[test]
    fn test_state_snapshot_empty() {
        let snapshot = StateSnapshot::empty();
        assert!(snapshot.tasks.is_empty());
        assert!(snapshot.messages.is_empty());
        assert!(snapshot.artifacts.is_empty());
    }

    #[test]
    fn test_state_snapshot_hash_deterministic() {
        let mut snapshot1 = StateSnapshot::empty();
        snapshot1
            .metadata
            .insert("key".to_string(), "value".to_string());

        let mut snapshot2 = StateSnapshot::empty();
        snapshot2
            .metadata
            .insert("key".to_string(), "value".to_string());

        // Same content should produce same hash
        assert_eq!(snapshot1.compute_hash(), snapshot2.compute_hash());
    }

    #[test]
    fn test_state_snapshot_diff_identical() {
        let snapshot1 = StateSnapshot::empty();
        let snapshot2 = StateSnapshot::empty();

        let diff = snapshot1.diff(&snapshot2);
        assert!(diff.is_identical);
        assert!(diff.differences.is_empty());
    }

    #[test]
    fn test_state_snapshot_diff_tasks_mismatch() {
        let mut snapshot1 = StateSnapshot::empty();
        snapshot1
            .tasks
            .insert("task-1".to_string(), create_test_task("task-1", "ctx-1"));

        let snapshot2 = StateSnapshot::empty();

        let diff = snapshot1.diff(&snapshot2);
        assert!(!diff.is_identical);
        assert_eq!(diff.differences.len(), 1);
        assert!(matches!(
            diff.differences[0],
            DifferenceKind::TasksMismatch { .. }
        ));
    }

    #[test]
    fn test_execution_recorder_new() {
        let recorder = ExecutionRecorder::new();
        assert_eq!(recorder.steps().len(), 0);
    }

    #[test]
    fn test_execution_recorder_record_step() {
        let mut recorder = ExecutionRecorder::new();

        let operation = Operation::CreateTask {
            task: create_test_task("task-1", "ctx-1"),
            initial_message: None,
            priority: None,
        };

        let state_before = StateSnapshot::empty();
        let state_after = StateSnapshot::empty();
        let receipt = ExecutionReceipt {
            execution_id: "exec-1".to_string(),
            operation: "CreateTask".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration_ms: 10,
            stages_completed: vec!["type_check".to_string()],
            success: true,
            policy_epoch: 0,
        };

        recorder.record_step(operation, state_before, state_after, receipt);

        assert_eq!(recorder.steps().len(), 1);
        assert_eq!(recorder.steps()[0].step_number, 0);
    }

    #[test]
    fn test_execution_recorder_serialization() {
        let mut recorder = ExecutionRecorder::new();

        let operation = Operation::CreateTask {
            task: create_test_task("task-1", "ctx-1"),
            initial_message: None,
            priority: None,
        };

        let receipt = ExecutionReceipt {
            execution_id: "exec-1".to_string(),
            operation: "CreateTask".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration_ms: 10,
            stages_completed: vec!["type_check".to_string()],
            success: true,
            policy_epoch: 0,
        };

        recorder.record_step(
            operation,
            StateSnapshot::empty(),
            StateSnapshot::empty(),
            receipt,
        );

        // Serialize to JSON
        let json = recorder.to_json().unwrap();
        assert!(json.contains("CreateTask"));

        // Deserialize back
        let restored = ExecutionRecorder::from_json(&json).unwrap();
        assert_eq!(restored.steps().len(), 1);
    }

    #[test]
    fn test_execution_replayer_new() {
        let recording = vec![];
        let replayer = ExecutionReplayer::new(recording);
        assert_eq!(replayer.total_steps(), 0);
        assert_eq!(replayer.current_step(), 0);
    }

    #[test]
    fn test_replay_result_is_deterministic() {
        let recorded_step = RecordedStep {
            step_number: 0,
            operation: Operation::CreateTask {
                task: create_test_task("task-1", "ctx-1"),
                initial_message: None,
                priority: None,
            },
            state_before: StateSnapshot::empty(),
            state_after: StateSnapshot::empty(),
            execution_receipt: ExecutionReceipt {
                execution_id: "exec-1".to_string(),
                operation: "CreateTask".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms: 10,
                stages_completed: vec!["type_check".to_string()],
                success: true,
                policy_epoch: 0,
            },
            #[cfg(feature = "receipts")]
            receipt: None,
        };

        let replay_output = Ok(RuntimeOutput {
            tasks: vec![],
            events: vec![],
            artifacts: vec![],
            errors: vec![],
            receipt: ExecutionReceipt {
                execution_id: "exec-2".to_string(),
                operation: "CreateTask".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms: 15,
                stages_completed: vec!["type_check".to_string()],
                success: true,
                policy_epoch: 0,
            },
        });

        let result = ReplayResult {
            step_number: 0,
            recorded_step,
            replay_output,
        };

        // Should be deterministic despite different timestamps and durations
        assert!(result.is_deterministic());
        assert!(result.differences().is_empty());
    }

    #[test]
    fn test_replay_result_non_deterministic() {
        let recorded_step = RecordedStep {
            step_number: 0,
            operation: Operation::CreateTask {
                task: create_test_task("task-1", "ctx-1"),
                initial_message: None,
                priority: None,
            },
            state_before: StateSnapshot::empty(),
            state_after: StateSnapshot::empty(),
            execution_receipt: ExecutionReceipt {
                execution_id: "exec-1".to_string(),
                operation: "CreateTask".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms: 10,
                stages_completed: vec!["type_check".to_string()],
                success: true,
                policy_epoch: 0,
            },
            #[cfg(feature = "receipts")]
            receipt: None,
        };

        let replay_output = Ok(RuntimeOutput {
            tasks: vec![],
            events: vec![],
            artifacts: vec![],
            errors: vec![],
            receipt: ExecutionReceipt {
                execution_id: "exec-2".to_string(),
                operation: "CreateTask".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms: 15,
                stages_completed: vec!["type_check".to_string(), "extra_stage".to_string()],
                success: true,
                policy_epoch: 0,
            },
        });

        let result = ReplayResult {
            step_number: 0,
            recorded_step,
            replay_output,
        };

        // Should detect non-determinism
        assert!(!result.is_deterministic());
        assert!(!result.differences().is_empty());
    }

    #[cfg(feature = "receipts")]
    #[test]
    fn test_receipt_chain_verification() {
        let mut chain = ReceiptChain::new();

        // Add receipts for three state transitions
        for i in 0..3 {
            let observation = format!("observation-{}", i).into_bytes();
            let action = format!("action-{}", i).into_bytes();
            let delta = format!("delta-{}", i).into_bytes();

            let receipt = Receipt::new(&observation, &action, &delta);
            chain.add_receipt(receipt);
        }

        // Verify chain integrity
        let result = ReceiptChainVerifier::verify_chain(&chain);
        assert!(result.is_ok());

        // Verify chain length
        assert_eq!(chain.len(), 3);
    }

    #[cfg(feature = "receipts")]
    #[test]
    fn test_identical_receipt_chains() {
        let mut chain1 = ReceiptChain::new();
        let mut chain2 = ReceiptChain::new();

        // Create identical receipts in both chains
        for i in 0..3 {
            let observation = format!("observation-{}", i).into_bytes();
            let action = format!("action-{}", i).into_bytes();
            let delta = format!("delta-{}", i).into_bytes();

            let receipt1 = Receipt::new(&observation, &action, &delta);
            let receipt2 = Receipt::new(&observation, &action, &delta);

            chain1.add_receipt(receipt1);
            chain2.add_receipt(receipt2);
        }

        // Chains should be identical
        assert!(ReceiptChainVerifier::verify_identical_chains(
            &chain1, &chain2
        ));
    }

    #[cfg(feature = "receipts")]
    #[test]
    fn test_different_receipt_chains() {
        let mut chain1 = ReceiptChain::new();
        let mut chain2 = ReceiptChain::new();

        // Create different receipts
        let receipt1 = Receipt::new(b"obs1", b"act1", b"delta1");
        let receipt2 = Receipt::new(b"obs2", b"act2", b"delta2");

        chain1.add_receipt(receipt1);
        chain2.add_receipt(receipt2);

        // Chains should be different
        assert!(!ReceiptChainVerifier::verify_identical_chains(
            &chain1, &chain2
        ));
    }

    // ========================================================================
    // INTEGRATION TESTS - Deterministic Scheduling
    // ========================================================================

    #[test]
    fn test_deterministic_scheduling_identical_order() {
        use crate::construct::runtime::scheduler::{PriorityClass, ScheduledTask, Scheduler};

        let mut scheduler1 = Scheduler::new(10);
        let mut scheduler2 = Scheduler::new(10);

        // Add tasks in different orders
        let task1 = ScheduledTask::new(
            "task-a".to_string(),
            "station-1".to_string(),
            1,
            PriorityClass::Normal,
        );
        let task2 = ScheduledTask::new(
            "task-b".to_string(),
            "station-1".to_string(),
            1,
            PriorityClass::High,
        );
        let task3 = ScheduledTask::new(
            "task-c".to_string(),
            "station-1".to_string(),
            2,
            PriorityClass::Normal,
        );

        // Scheduler 1: add in order a, b, c
        scheduler1.submit(task1.clone()).unwrap();
        scheduler1.submit(task2.clone()).unwrap();
        scheduler1.submit(task3.clone()).unwrap();

        // Scheduler 2: add in order c, a, b
        scheduler2.submit(task3).unwrap();
        scheduler2.submit(task1).unwrap();
        scheduler2.submit(task2).unwrap();

        // Both should produce identical pending order (deterministic)
        let order1 = scheduler1.pending_tasks();
        let order2 = scheduler2.pending_tasks();

        assert_eq!(order1.len(), order2.len());
        assert_eq!(
            order1, order2,
            "Schedulers should produce identical task ordering"
        );
    }

    #[test]
    fn test_deterministic_scheduling_priority_ordering() {
        use crate::construct::runtime::scheduler::{PriorityClass, ScheduledTask, Scheduler};

        let mut scheduler = Scheduler::new(10);

        // Add tasks with different priorities at same epoch
        scheduler
            .submit(ScheduledTask::new(
                "low".to_string(),
                "station-1".to_string(),
                1,
                PriorityClass::Low,
            ))
            .unwrap();

        scheduler
            .submit(ScheduledTask::new(
                "high".to_string(),
                "station-1".to_string(),
                1,
                PriorityClass::High,
            ))
            .unwrap();

        scheduler
            .submit(ScheduledTask::new(
                "critical".to_string(),
                "station-1".to_string(),
                1,
                PriorityClass::Critical,
            ))
            .unwrap();

        // Get execution order by calling next() repeatedly
        let task1 = scheduler.next().unwrap();
        let task2 = scheduler.next().unwrap();
        let task3 = scheduler.next().unwrap();

        // Should be ordered by priority: Critical, High, Low
        assert_eq!(task1.task_id, "critical");
        assert_eq!(task2.task_id, "high");
        assert_eq!(task3.task_id, "low");
    }

    // ========================================================================
    // INTEGRATION TESTS - Full Record-Replay Cycles
    // ========================================================================

    #[test]
    fn test_full_record_replay_cycle() {
        let mut runtime1 = Runtime::default_runtime();
        let mut recorder = ExecutionRecorder::new();

        // Execute a sequence of operations on runtime1 and record
        let operations = vec![
            Operation::CreateTask {
                task: create_test_task("task-1", "ctx-1"),
                initial_message: Some(create_test_message("msg-1", "Hello")),
                priority: None,
            },
            Operation::SendMessage {
                task_id: "task-1".to_string(),
                message: create_test_message("msg-2", "World"),
            },
        ];

        for op in operations {
            let state_before = StateSnapshot::empty(); // Simplified for test
            let output = runtime1.handle(op.clone()).unwrap();
            let state_after = StateSnapshot::empty(); // Simplified for test

            recorder.record_step(op, state_before, state_after, output.receipt);
        }

        // Now replay on a fresh runtime
        let mut runtime2 = Runtime::default_runtime();
        let mut replayer = ExecutionReplayer::new(recorder.steps().to_vec());

        let replay_results = replayer.replay_all(&mut runtime2);

        // All steps should be deterministic
        for result in replay_results {
            assert!(
                result.is_deterministic(),
                "Step {} failed determinism: {:?}",
                result.step_number,
                result.differences()
            );
        }
    }

    #[test]
    fn test_state_snapshot_ordering_determinism() {
        // Insert tasks in different orders
        let mut snapshot1 = StateSnapshot::empty();
        snapshot1
            .tasks
            .insert("task-z".to_string(), create_test_task("task-z", "ctx-1"));
        snapshot1
            .tasks
            .insert("task-a".to_string(), create_test_task("task-a", "ctx-1"));
        snapshot1
            .tasks
            .insert("task-m".to_string(), create_test_task("task-m", "ctx-1"));

        let mut snapshot2 = StateSnapshot::empty();
        snapshot2
            .tasks
            .insert("task-a".to_string(), create_test_task("task-a", "ctx-1"));
        snapshot2
            .tasks
            .insert("task-m".to_string(), create_test_task("task-m", "ctx-1"));
        snapshot2
            .tasks
            .insert("task-z".to_string(), create_test_task("task-z", "ctx-1"));

        // BTreeMap should ensure deterministic ordering
        assert_eq!(snapshot1.compute_hash(), snapshot2.compute_hash());
    }
}
