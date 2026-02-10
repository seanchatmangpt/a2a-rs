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

use crate::construct::runtime::{Operation, Runtime};
use crate::domain::{Task, TaskStatus};

// Re-export replay types from the main replay module
pub use crate::construct::replay::{
    DifferenceKind, ExecutionRecorder, ExecutionReplayer, RecordedStep, ReplayResult, SnapshotDiff,
    StateSnapshot,
};

#[cfg(feature = "receipts")]
pub use crate::construct::replay::ReceiptChainVerifier;

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
