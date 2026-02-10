//! Core types for replay recording and state snapshots.
//!
//! This module provides the foundational types used for recording execution
//! sequences and comparing states for determinism verification.

use crate::construct::runtime::{
    ExecutionReceipt, Operation, Runtime, RuntimeError, RuntimeOutput,
};
use crate::domain::{Artifact, Message, Task};
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
