//! Hook system domain types
//!
//! Defines types for Claude Code hook execution system supporting PreToolUse,
//! PostToolUse, and TaskCompleted hooks.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Hook event type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    /// Executed before a tool is invoked
    PreToolUse,
    /// Executed after a tool completes
    PostToolUse,
    /// Executed when a task is completed
    TaskCompleted,
}

impl std::fmt::Display for HookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookEvent::PreToolUse => write!(f, "PreToolUse"),
            HookEvent::PostToolUse => write!(f, "PostToolUse"),
            HookEvent::TaskCompleted => write!(f, "TaskCompleted"),
        }
    }
}

/// Permission decision for PreToolUse hooks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionDecision {
    /// Allow the operation to proceed
    Allow,
    /// Deny the operation
    Deny,
}

/// Input to a hook execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookInput {
    /// The hook event being triggered
    pub hook_event: HookEvent,
    /// Tool-specific input data
    pub tool_input: HashMap<String, serde_json::Value>,
    /// Optional tool name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// Optional additional context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<HashMap<String, serde_json::Value>>,
}

/// Hook-specific output for PreToolUse
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreToolUseOutput {
    /// The hook event name
    pub hook_event_name: String,
    /// Permission decision (allow or deny)
    pub permission_decision: PermissionDecision,
    /// Reason for the permission decision
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_decision_reason: Option<String>,
}

/// Hook-specific output for PostToolUse
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostToolUseOutput {
    /// The hook event name
    pub hook_event_name: String,
    /// Status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
}

/// Hook-specific output for TaskCompleted
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCompletedOutput {
    /// The hook event name
    pub hook_event_name: String,
    /// Status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
}

/// Output from a hook execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutput {
    /// Hook-specific output based on event type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_specific_output: Option<serde_json::Value>,
}

/// Hook configuration for a single hook
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookConfig {
    /// The hook event to trigger on
    pub event: HookEvent,
    /// Shell command to execute
    pub command: String,
    /// Working directory for command execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    /// Timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// Optional regex pattern for decidable checks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Environment variables to set
    #[serde(default)]
    pub env: HashMap<String, String>,
}

fn default_timeout() -> u64 {
    30_000 // 30 seconds default
}

/// Result of hook execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookResult {
    /// Whether the hook executed successfully
    pub success: bool,
    /// Exit code from the command
    pub exit_code: Option<i32>,
    /// Standard output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    /// Standard error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    /// Parsed hook output (if stdout contained valid JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<HookOutput>,
    /// Error message if execution failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_event_display() {
        assert_eq!(HookEvent::PreToolUse.to_string(), "PreToolUse");
        assert_eq!(HookEvent::PostToolUse.to_string(), "PostToolUse");
        assert_eq!(HookEvent::TaskCompleted.to_string(), "TaskCompleted");
    }

    #[test]
    fn test_hook_input_serialization() {
        let mut tool_input = HashMap::new();
        tool_input.insert("file_path".to_string(), serde_json::json!("/path/to/file"));
        tool_input.insert("content".to_string(), serde_json::json!("file contents"));

        let input = HookInput {
            hook_event: HookEvent::PreToolUse,
            tool_input,
            tool_name: Some("Write".to_string()),
            context: None,
        };

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("PreToolUse"));
        assert!(json.contains("file_path"));

        let deserialized: HookInput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.hook_event, HookEvent::PreToolUse);
    }

    #[test]
    fn test_permission_decision_serialization() {
        let json = serde_json::to_string(&PermissionDecision::Allow).unwrap();
        assert_eq!(json, "\"allow\"");

        let json = serde_json::to_string(&PermissionDecision::Deny).unwrap();
        assert_eq!(json, "\"deny\"");
    }

    #[test]
    fn test_hook_output_parsing() {
        let json = r#"{
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": "Architecture violation"
            }
        }"#;

        let output: HookOutput = serde_json::from_str(json).unwrap();
        assert!(output.hook_specific_output.is_some());

        let specific: PreToolUseOutput =
            serde_json::from_value(output.hook_specific_output.unwrap()).unwrap();
        assert_eq!(specific.permission_decision, PermissionDecision::Deny);
        assert_eq!(
            specific.permission_decision_reason,
            Some("Architecture violation".to_string())
        );
    }

    #[test]
    fn test_hook_config_defaults() {
        let config = HookConfig {
            event: HookEvent::PreToolUse,
            command: "echo test".to_string(),
            working_dir: None,
            timeout_ms: default_timeout(),
            pattern: None,
            env: HashMap::new(),
        };

        assert_eq!(config.timeout_ms, 30_000);
    }
}
