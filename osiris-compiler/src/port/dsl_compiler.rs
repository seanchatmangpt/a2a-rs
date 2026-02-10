//! DSL compiler port trait.
//!
//! Defines the interface for compiling external DSL representations
//! into internal 43-pattern workflow primitives.

use crate::domain::dsl::DslWorkflow;
use crate::domain::workflow::WorkflowPattern;
use async_trait::async_trait;
use thiserror::Error;

/// Errors that can occur during DSL compilation.
#[derive(Debug, Error)]
pub enum DslCompilerError {
    /// Invalid DSL syntax or structure
    #[error("Invalid DSL: {reason}")]
    InvalidDsl { reason: String },

    /// Element reference not found
    #[error("Element not found: {element_id}")]
    ElementNotFound { element_id: String },

    /// Flow reference not found
    #[error("Flow not found: {flow_id}")]
    FlowNotFound { flow_id: String },

    /// Unsupported DSL feature
    #[error("Unsupported feature: {feature}")]
    UnsupportedFeature { feature: String },

    /// Structural validation failed
    #[error("Structural validation failed: {reason}")]
    StructuralError { reason: String },

    /// Gateway without proper configuration
    #[error("Invalid gateway {gateway_id}: {reason}")]
    InvalidGateway { gateway_id: String, reason: String },

    /// Circular flow detected
    #[error("Circular flow detected involving elements: {elements:?}")]
    CircularFlow { elements: Vec<String> },

    /// Multiple start events without merge
    #[error("Multiple start events require proper merging")]
    MultipleStartEvents,

    /// No end events defined
    #[error("No end events defined in workflow")]
    NoEndEvents,

    /// Compilation failed
    #[error("Compilation failed: {message}")]
    CompilationFailed { message: String },
}

/// Result type for DSL compilation operations.
pub type DslCompilerResult<T> = Result<T, DslCompilerError>;

/// Port trait for DSL compilation.
///
/// Implementations transform external DSL formats (BPMN-like JSON/XML)
/// into internal WorkflowPattern representations using the 43 workflow patterns.
#[async_trait]
pub trait DslCompiler: Send + Sync {
    /// Compiles a DSL workflow into a WorkflowPattern.
    ///
    /// This performs structural validation and maps DSL elements to
    /// the appropriate workflow pattern primitives.
    async fn compile(&self, dsl: DslWorkflow) -> DslCompilerResult<WorkflowPattern>;

    /// Validates a DSL workflow without full compilation.
    ///
    /// Checks for structural correctness, reference validity, and
    /// adherence to workflow modeling constraints.
    async fn validate(&self, dsl: &DslWorkflow) -> DslCompilerResult<()>;

    /// Decompiles a WorkflowPattern back to DSL representation.
    ///
    /// Useful for round-trip conversion and tooling integration.
    async fn decompile(&self, pattern: &WorkflowPattern) -> DslCompilerResult<DslWorkflow>;

    /// Optimizes a DSL workflow before compilation.
    ///
    /// Applies optimizations like:
    /// - Removing redundant gateways
    /// - Simplifying exclusive choice chains
    /// - Merging parallel flows
    async fn optimize(&self, dsl: DslWorkflow) -> DslCompilerResult<DslWorkflow> {
        // Default: no optimization
        Ok(dsl)
    }
}

/// Compilation statistics and metrics.
#[derive(Debug, Clone)]
pub struct CompilationStats {
    /// Number of elements in DSL
    pub dsl_element_count: usize,
    /// Number of nodes in compiled pattern
    pub pattern_node_count: usize,
    /// Number of edges in compiled pattern
    pub pattern_edge_count: usize,
    /// Number of validation warnings
    pub warning_count: usize,
    /// Compilation time in milliseconds
    pub compilation_time_ms: u64,
}

/// Extended DSL compiler trait with statistics.
#[async_trait]
pub trait DslCompilerWithStats: DslCompiler {
    /// Compiles with statistics collection.
    async fn compile_with_stats(
        &self,
        dsl: DslWorkflow,
    ) -> DslCompilerResult<(WorkflowPattern, CompilationStats)>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compiler_error_display() {
        let err = DslCompilerError::ElementNotFound {
            element_id: "task1".to_string(),
        };
        assert!(err.to_string().contains("task1"));
    }

    #[test]
    fn test_circular_flow_error() {
        let err = DslCompilerError::CircularFlow {
            elements: vec![
                "task1".to_string(),
                "task2".to_string(),
                "task1".to_string(),
            ],
        };
        assert!(err.to_string().contains("task1"));
        assert!(err.to_string().contains("task2"));
    }
}
