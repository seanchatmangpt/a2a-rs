//! Runtime components for the construct framework
//!
//! This module provides deterministic runtime execution primitives:
//! - Scheduler: Deterministic task ordering and execution
//! - Epoch-based logical clocking
//! - Work-in-progress (WIP) limits
//! - Fair scheduling across stations
//! - Bounded Actuation: Finite, deterministic state updates
//! - Executor: Runtime μ function implementing the complete execution pipeline

pub mod actuation;
pub mod executor;
pub mod scheduler;

pub use actuation::{
    ActuationError, ActuationReceipt, BoundedActuator, StateUpdate, UpdateBatch, UpdateLimit,
};
pub use executor::{
    ExecutionContext, ExecutionReceipt, Operation, Runtime, RuntimeError, RuntimeEvent,
    RuntimeOutput,
};
pub use scheduler::{PriorityClass, ScheduledTask, Scheduler, SchedulerError};
