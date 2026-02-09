/// Domain error types for macOS actuation
use thiserror::Error;

/// Errors that can occur during actuation
#[derive(Debug, Error)]
pub enum ActuationError {
    #[error("Actuation not permitted: {0}")]
    NotPermitted(String),

    #[error("Actuation timed out after {0} seconds")]
    Timeout(u64),

    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    #[error("Application not found: {0}")]
    ApplicationNotFound(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("System error: {0}")]
    SystemError(String),

    #[error("User cancelled actuation")]
    UserCancelled,

    #[error("Actuation not supported on this platform")]
    UnsupportedPlatform,

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for actuation operations
pub type ActuationResult<T> = Result<T, ActuationError>;
