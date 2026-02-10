//! Replay module for determinism verification and debugging.
//!
//! This module provides tools for recording, replaying, and debugging
//! execution sequences to verify deterministic behavior.
//!
//! # Components
//!
//! - `core` - Core recording and state snapshot types
//! - `debugger` - Interactive debugger for stepping through recordings
//!
//! # Features
//!
//! The replay functionality integrates with:
//! - Receipt chains (`receipts` feature)
//! - State snapshots (BTreeMap-based for determinism)
//! - Execution recording and comparison
//!
//! # Example
//!
//! ```rust
//! # use a2a_rs::construct::replay::{ExecutionRecorder, ReplayDebugger};
//! // Record execution
//! let mut recorder = ExecutionRecorder::new();
//! // ... record steps ...
//!
//! // Debug the recording
//! let mut debugger = ReplayDebugger::from_recorder(&recorder);
//! debugger.step_forward();
//! let report = debugger.inspect_current();
//! ```

pub mod core;
pub mod debugger;

// Re-export core types
pub use core::{
    DifferenceKind, ExecutionRecorder, ExecutionReplayer, RecordedStep, ReplayResult, SnapshotDiff,
    StateSnapshot,
};

#[cfg(feature = "receipts")]
pub use core::ReceiptChainVerifier;

// Re-export debugger types
pub use debugger::{
    DebuggerConfig, DebuggerStatus, ReplayDebugger, StepReport, StepResult, StepSummary,
};
