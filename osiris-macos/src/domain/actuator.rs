/// Core actuation domain types
use serde::{Deserialize, Serialize};

/// Represents a bounded actuation command
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActuationCommand {
    /// Unique identifier for this command
    pub id: String,
    /// Type of actuation to perform
    pub command_type: ActuationType,
    /// Parameters for the actuation
    pub parameters: serde_json::Value,
    /// Safety bounds and constraints
    pub bounds: ActuationBounds,
}

/// Types of actuations supported
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActuationType {
    /// Launch an application
    LaunchApplication,
    /// Execute an AppleScript
    ExecuteAppleScript,
    /// Simulate keyboard input
    KeyboardInput,
    /// Simulate mouse action
    MouseAction,
    /// File system operation
    FileSystemOperation,
    /// Process management
    ProcessManagement,
    /// System preference change
    SystemPreference,
}

/// Safety bounds for actuation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActuationBounds {
    /// Maximum execution time in seconds
    pub timeout_seconds: u64,
    /// Allowed applications (None = all allowed)
    pub allowed_applications: Option<Vec<String>>,
    /// Allowed file paths (None = all allowed)
    pub allowed_paths: Option<Vec<String>>,
    /// Whether to require user confirmation
    pub require_confirmation: bool,
    /// Whether to allow destructive operations
    pub allow_destructive: bool,
}

impl Default for ActuationBounds {
    fn default() -> Self {
        Self {
            timeout_seconds: 30,
            allowed_applications: None,
            allowed_paths: None,
            require_confirmation: true,
            allow_destructive: false,
        }
    }
}

/// Outcome of an actuation command
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActuationOutcome {
    /// Command ID this result corresponds to
    pub command_id: String,
    /// Whether the actuation succeeded
    pub success: bool,
    /// Output or result data
    pub output: Option<serde_json::Value>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Status of a running actuation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActuationStatus {
    /// Waiting for user confirmation
    PendingConfirmation,
    /// Currently executing
    InProgress,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed,
    /// Cancelled by user or timeout
    Cancelled,
}
