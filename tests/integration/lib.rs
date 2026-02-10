//! Integration tests for a2a-rs workspace
//!
//! This test suite provides end-to-end testing of the OSIRIS protocol with:
//! - Server spawning (edge + compiler)
//! - Operation submission and verification
//! - Receipt validation
//! - Refusal path testing
//! - Workflow execution

pub mod common;
pub mod basic_compilation;
pub mod pipeline_stages;
pub mod refusal_handling;
pub mod workflow_execution;
pub mod end_to_end;
