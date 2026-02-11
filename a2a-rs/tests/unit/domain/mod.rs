//! Comprehensive unit tests for domain layer types
//!
//! This module contains Chicago School TDD-style tests for all domain types:
//! - Message, Part, Artifact, Role (message_test.rs)
//! - Task, TaskState, TaskStatus (task_test.rs)
//! - AgentCard, AgentSkill, AgentCapabilities, SecurityScheme (agent_test.rs)
//! - Validation errors and validators (validation_test.rs)
//! - Event types (events_test.rs)

mod message_test;
mod task_test;
mod agent_test;
mod validation_test;
mod events_test;
