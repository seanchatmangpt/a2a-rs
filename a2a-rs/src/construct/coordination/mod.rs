//! Task coordination and dependency management
//!
//! This module provides tools for managing task dependencies through a directed
//! acyclic graph (DAG) structure, supporting complex multi-agent workflows with
//! prerequisite checking, join semantics, and cancellation propagation.

pub mod task_graph;
pub mod visualizer;

pub use task_graph::{CoordinationError, CoordinationResult, DependencyEdge, TaskGraph, TaskNode};
pub use visualizer::TaskGraphVisualizer;
