//! Workflow modeling and pattern analysis
//!
//! This module provides workflow graph modeling, pattern detection,
//! and completeness analysis based on the Workflow Patterns Initiative.

pub mod patterns;

pub use patterns::{
    PatternCategory, StateType, WorkflowAnalysis, WorkflowError, WorkflowGraph, WorkflowPattern,
    WorkflowState, WorkflowTransition,
};
