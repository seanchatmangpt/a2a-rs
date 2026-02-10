//! Error types for SHACL validation.

use std::fmt;
use thiserror::Error;

/// Errors that can occur during SHACL validation.
#[derive(Debug, Clone, Error)]
pub enum ValidationError {
    /// RDF parsing error.
    #[error("RDF parsing error: {0}")]
    ParseError(String),

    /// Shape parsing error.
    #[error("Shape parsing error: {0}")]
    ShapeError(String),

    /// Constraint violation.
    #[error("Constraint violation: {message}")]
    ConstraintViolation {
        /// Human-readable message.
        message: String,
        /// Path to the violating node.
        focus_node: String,
        /// Property that failed validation.
        result_path: Option<String>,
        /// Constraint that was violated.
        constraint: String,
        /// Severity level.
        severity: Severity,
    },

    /// Multiple validation violations.
    #[error("Multiple validation failures: {0} violations")]
    MultipleViolations(Vec<ValidationError>),

    /// Invalid shape definition.
    #[error("Invalid shape definition: {0}")]
    InvalidShape(String),

    /// Unsupported constraint.
    #[error("Unsupported constraint: {0}")]
    UnsupportedConstraint(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    IoError(String),
}

/// Severity level for validation violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// Informational message.
    Info,
    /// Warning (does not fail validation).
    Warning,
    /// Violation (fails validation).
    Violation,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Info => write!(f, "Info"),
            Severity::Warning => write!(f, "Warning"),
            Severity::Violation => write!(f, "Violation"),
        }
    }
}

/// Result type for SHACL validation operations.
pub type ValidationResult<T> = Result<T, ValidationError>;

/// A validation report containing all constraint violations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    /// Whether the validation passed (no violations with Severity::Violation).
    pub conforms: bool,
    /// List of validation results.
    pub results: Vec<ValidationResult<()>>,
    /// Total number of violations.
    pub violation_count: usize,
    /// Total number of warnings.
    pub warning_count: usize,
}

impl ValidationReport {
    /// Create a new validation report.
    pub fn new() -> Self {
        Self {
            conforms: true,
            results: Vec::new(),
            violation_count: 0,
            warning_count: 0,
        }
    }

    /// Add a validation result to the report.
    pub fn add_result(&mut self, result: ValidationResult<()>) {
        if let Err(ref err) = result {
            match err {
                ValidationError::ConstraintViolation { severity, .. } => {
                    match severity {
                        Severity::Violation => {
                            self.violation_count += 1;
                            self.conforms = false;
                        }
                        Severity::Warning => {
                            self.warning_count += 1;
                        }
                        Severity::Info => {}
                    }
                }
                _ => {
                    self.violation_count += 1;
                    self.conforms = false;
                }
            }
        }
        self.results.push(result);
    }

    /// Check if the validation passed.
    pub fn is_valid(&self) -> bool {
        self.conforms
    }
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}
