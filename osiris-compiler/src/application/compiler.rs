//! High-level compiler orchestration.
//!
//! This module provides the main Compiler struct that implements
//! the deterministic compilation function μ: O → A.

use crate::adapter::LambdaOrderer;
use crate::domain::{Operation, OrderingError};
use crate::port::DeterministicOrderer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Configuration for the compiler.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerConfig {
    /// Enable verbose logging
    #[serde(default)]
    pub verbose: bool,

    /// Enable strict validation
    #[serde(default)]
    pub strict_mode: bool,

    /// Maximum number of operations to compile in one batch
    #[serde(default = "default_max_batch_size")]
    pub max_batch_size: usize,
}

fn default_max_batch_size() -> usize {
    1000
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            verbose: false,
            strict_mode: false,
            max_batch_size: default_max_batch_size(),
        }
    }
}

/// The Osiris Compiler: implements deterministic compilation μ: O → A.
///
/// The compiler takes a set of operations O and produces an ordered
/// sequence of actions A through deterministic ordering.
///
/// # Architecture
///
/// The compiler uses:
/// - **LambdaOrderer** for deterministic operation ordering
/// - **Domain types** for operations and errors
/// - **Port traits** for extensibility
///
/// # Example
///
/// ```rust,ignore
/// use osiris_compiler::application::{Compiler, CompilerConfig};
/// use osiris_compiler::domain::{Operation, OperationKind};
///
/// let config = CompilerConfig::default();
/// let compiler = Compiler::new(config);
///
/// let operations = vec![
///     Operation::new(OperationKind::Parse { input: "main.rs".into() }, 1),
///     Operation::new(OperationKind::TypeCheck { module_id: "main".into() }, 2),
/// ];
///
/// let actions = compiler.compile(operations)?;
/// ```
#[derive(Clone)]
pub struct Compiler {
    config: CompilerConfig,
    orderer: Arc<dyn DeterministicOrderer>,
}

impl std::fmt::Debug for Compiler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Compiler")
            .field("config", &self.config)
            .field("orderer", &"<dyn DeterministicOrderer>")
            .finish()
    }
}

impl Compiler {
    /// Create a new compiler with the given configuration.
    pub fn new(config: CompilerConfig) -> Self {
        let orderer = Arc::new(LambdaOrderer::default());
        Self { config, orderer }
    }

    /// Create a compiler with a custom orderer.
    pub fn with_orderer(config: CompilerConfig, orderer: Arc<dyn DeterministicOrderer>) -> Self {
        Self { config, orderer }
    }

    /// Compile operations into deterministically ordered actions.
    ///
    /// This is the main compilation function μ: O → A.
    ///
    /// # Arguments
    ///
    /// * `operations` - The set of operations to compile
    ///
    /// # Returns
    ///
    /// A deterministically ordered sequence of actions (operations).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Batch size exceeds maximum
    /// - Operations are invalid
    /// - Circular dependencies are detected
    /// - Conflicts between operations exist
    pub fn compile(&self, operations: Vec<Operation>) -> Result<Vec<Operation>, OrderingError> {
        // Validate batch size
        if operations.len() > self.config.max_batch_size {
            return Err(OrderingError::InvalidOperation(format!(
                "Batch size {} exceeds maximum {}",
                operations.len(),
                self.config.max_batch_size
            )));
        }

        // Use the orderer to establish deterministic order
        let ordered_operations = self.orderer.order(operations)?;

        Ok(ordered_operations)
    }

    /// Get the compiler configuration.
    pub fn config(&self) -> &CompilerConfig {
        &self.config
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new(CompilerConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::OperationKind;

    #[test]
    fn test_compiler_creation() {
        let compiler = Compiler::default();
        assert_eq!(compiler.config().max_batch_size, 1000);
    }

    #[test]
    fn test_compile_operations() {
        let compiler = Compiler::default();

        let operations = vec![
            Operation::new(
                OperationKind::Parse {
                    input: "main.rs".into(),
                },
                1,
            ),
            Operation::new(
                OperationKind::TypeCheck {
                    module_id: "main".into(),
                },
                2,
            ),
        ];

        let result = compiler.compile(operations);
        assert!(result.is_ok());

        let ordered = result.unwrap();
        // TypeCheck (priority 2) should come before Parse (priority 1)
        assert_eq!(ordered[0].priority, 2);
        assert_eq!(ordered[1].priority, 1);
    }

    #[test]
    fn test_batch_size_limit() {
        let config = CompilerConfig {
            max_batch_size: 2,
            ..Default::default()
        };
        let compiler = Compiler::new(config);

        let operations = vec![
            Operation::new(OperationKind::Parse { input: "1".into() }, 1),
            Operation::new(OperationKind::Parse { input: "2".into() }, 1),
            Operation::new(OperationKind::Parse { input: "3".into() }, 1),
        ];

        let result = compiler.compile(operations);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            OrderingError::InvalidOperation(_)
        ));
    }

    #[test]
    fn test_deterministic_compilation() {
        let compiler = Compiler::default();

        let operations = vec![
            Operation::new(OperationKind::Parse { input: "a".into() }, 1),
            Operation::new(
                OperationKind::TypeCheck {
                    module_id: "b".into(),
                },
                2,
            ),
            Operation::new(OperationKind::CodeGen { target: "c".into() }, 1),
        ];

        let result1 = compiler.compile(operations.clone()).unwrap();
        let result2 = compiler.compile(operations).unwrap();

        // Same input should produce same output
        assert_eq!(result1, result2);
    }
}
