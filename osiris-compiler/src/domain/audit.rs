//! Audit logging domain types.
//!
//! Defines types for logging user actions, state changes, and receipt events
//! with structured tracing context for debugging and compliance.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// An audit log entry representing a significant system event.
///
/// Audit logs capture:
/// - User actions (compilation requests, configuration changes)
/// - State changes (transitions between operational states)
/// - Receipt events (creation, verification, storage)
/// - All with structured trace context for correlation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogEntry {
    /// Unique identifier for this audit log entry
    pub id: Uuid,

    /// Timestamp when the event occurred
    pub timestamp: DateTime<Utc>,

    /// Type of event being logged
    pub event_type: AuditEventType,

    /// Optional user or service that triggered the event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,

    /// Resource identifier (operation, receipt, artifact, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<Uuid>,

    /// Resource type (Operation, Receipt, Artifact, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,

    /// Action that was performed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,

    /// Status of the action
    pub status: AuditStatus,

    /// Details about the event (state change, user action, etc.)
    pub details: AuditDetails,

    /// Trace context for request correlation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_context: Option<TraceContext>,

    /// Additional structured metadata
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Severity level of the event
    pub severity: AuditSeverity,
}

/// Trace context for correlating logs across systems.
///
/// Contains W3C Trace Context (tracestate, traceparent) and custom identifiers
/// for distributed tracing and request correlation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TraceContext {
    /// W3C trace ID (globally unique)
    pub trace_id: String,

    /// W3C span ID (identifies this operation)
    pub span_id: String,

    /// W3C trace flags (sampling decision)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_flags: Option<String>,

    /// Parent span ID for causal relationships
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,

    /// Custom trace state key-value pairs
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub trace_state: HashMap<String, String>,

    /// Request ID for end-to-end tracing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl Default for TraceContext {
    fn default() -> Self {
        Self {
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string(),
            trace_flags: None,
            parent_span_id: None,
            trace_state: HashMap::new(),
            request_id: None,
        }
    }
}

/// Types of events that can be audited.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditEventType {
    /// User initiated a compilation operation
    CompilationStarted,

    /// Compilation completed successfully
    CompilationCompleted,

    /// Compilation failed
    CompilationFailed,

    /// User created a new operation
    OperationCreated,

    /// Operation was accepted for processing
    OperationAccepted,

    /// Operation was rejected or refused
    OperationRefused,

    /// Operation state changed
    OperationStateChanged,

    /// Receipt was created
    ReceiptCreated,

    /// Receipt was verified
    ReceiptVerified,

    /// Receipt verification failed
    ReceiptVerificationFailed,

    /// Receipt was stored
    ReceiptStored,

    /// State snapshot was created
    StateSnapshotCreated,

    /// Guard condition evaluated
    GuardEvaluated,

    /// Invariant check performed
    InvariantCheckPerformed,

    /// Invariant check failed
    InvariantCheckFailed,

    /// User authenticated
    UserAuthenticated,

    /// User authorization failed
    AuthorizationFailed,

    /// Configuration change
    ConfigurationChanged,

    /// System error occurred
    SystemError,

    /// Security-relevant event
    SecurityEvent,

    /// Other/custom audit event
    Other,
}

/// Status of an audited action.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditStatus {
    /// Action succeeded
    Success,

    /// Action failed
    Failure,

    /// Action was rejected
    Rejected,

    /// Action is pending
    Pending,

    /// Action was cancelled
    Cancelled,
}

/// Severity level for audit events.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditSeverity {
    /// Informational event
    Info,

    /// Warning - potential issue
    Warning,

    /// Error - operation failed
    Error,

    /// Critical - immediate attention required
    Critical,
}

/// Details about an audit event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AuditDetails {
    /// User action on a resource
    #[serde(rename_all = "camelCase")]
    UserAction {
        /// What the user did
        action_description: String,

        /// Expected vs actual if applicable
        #[serde(skip_serializing_if = "Option::is_none")]
        expected_state: Option<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        actual_state: Option<String>,
    },

    /// State transition in the system
    #[serde(rename_all = "camelCase")]
    StateChange {
        /// Previous state
        previous_state: String,

        /// New state
        new_state: String,

        /// Reason for change
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },

    /// Receipt-related event
    #[serde(rename_all = "camelCase")]
    ReceiptEvent {
        /// Receipt ID
        receipt_id: Uuid,

        /// Operation ID the receipt attests to
        operation_id: Uuid,

        /// Event description
        event_description: String,

        /// Hash values if relevant
        #[serde(skip_serializing_if = "Option::is_none")]
        operation_hash: Option<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        attestation_hash: Option<String>,
    },

    /// Guard or constraint evaluation
    #[serde(rename_all = "camelCase")]
    GuardEvaluation {
        /// Guard name or ID
        guard_id: String,

        /// Whether guard condition passed
        condition_passed: bool,

        /// Condition description
        condition: String,

        /// Reason for pass/fail
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },

    /// Invariant check result
    #[serde(rename_all = "camelCase")]
    InvariantCheck {
        /// Invariant being checked
        invariant_id: String,

        /// Whether invariant held
        invariant_held: bool,

        /// Check description
        check_description: String,

        /// Violation details if applicable
        #[serde(skip_serializing_if = "Option::is_none")]
        violation: Option<String>,
    },

    /// Authorization/authentication event
    #[serde(rename_all = "camelCase")]
    AuthEvent {
        /// Auth method used (JWT, OAuth2, etc.)
        auth_method: String,

        /// Subject being authenticated
        subject: String,

        /// Scopes/permissions granted
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        scopes: Vec<String>,

        /// Reason if denied
        #[serde(skip_serializing_if = "Option::is_none")]
        denial_reason: Option<String>,
    },

    /// Error details
    #[serde(rename_all = "camelCase")]
    ErrorDetails {
        /// Error message
        message: String,

        /// Error code
        #[serde(skip_serializing_if = "Option::is_none")]
        error_code: Option<String>,

        /// Stack trace or detailed context
        #[serde(skip_serializing_if = "Option::is_none")]
        context: Option<String>,
    },

    /// Generic unstructured details
    Unstructured(serde_json::Value),
}

/// Error type for audit logging operations.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum AuditError {
    /// Failed to serialize audit entry
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Failed to write audit log
    #[error("Write error: {0}")]
    WriteError(String),

    /// Failed to format log entry
    #[error("Format error: {0}")]
    FormatError(String),

    /// Trace context is invalid
    #[error("Invalid trace context: {0}")]
    InvalidTraceContext(String),

    /// Failed to send to logging service
    #[error("Service error: {0}")]
    ServiceError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

impl AuditLogEntry {
    /// Creates a new audit log entry with minimal required fields.
    pub fn new(event_type: AuditEventType, status: AuditStatus) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type,
            actor: None,
            resource_id: None,
            resource_type: None,
            action: None,
            status,
            details: AuditDetails::Unstructured(serde_json::json!({})),
            trace_context: None,
            metadata: HashMap::new(),
            severity: AuditSeverity::Info,
        }
    }

    /// Creates an audit entry for a successful user action.
    pub fn user_action(actor: String, action: String, resource_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::OperationCreated,
            actor: Some(actor),
            resource_id: Some(resource_id),
            resource_type: Some("Operation".to_string()),
            action: Some(action.clone()),
            status: AuditStatus::Success,
            details: AuditDetails::UserAction {
                action_description: action,
                expected_state: None,
                actual_state: None,
            },
            trace_context: None,
            metadata: HashMap::new(),
            severity: AuditSeverity::Info,
        }
    }

    /// Creates an audit entry for a state change.
    pub fn state_change(resource_id: Uuid, previous_state: String, new_state: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::OperationStateChanged,
            actor: None,
            resource_id: Some(resource_id),
            resource_type: Some("State".to_string()),
            action: Some("StateTransition".to_string()),
            status: AuditStatus::Success,
            details: AuditDetails::StateChange {
                previous_state,
                new_state,
                reason: None,
            },
            trace_context: None,
            metadata: HashMap::new(),
            severity: AuditSeverity::Info,
        }
    }

    /// Creates an audit entry for a receipt event.
    pub fn receipt_event(receipt_id: Uuid, operation_id: Uuid, event_description: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::ReceiptCreated,
            actor: None,
            resource_id: Some(receipt_id),
            resource_type: Some("Receipt".to_string()),
            action: Some("ReceiptEvent".to_string()),
            status: AuditStatus::Success,
            details: AuditDetails::ReceiptEvent {
                receipt_id,
                operation_id,
                event_description,
                operation_hash: None,
                attestation_hash: None,
            },
            trace_context: None,
            metadata: HashMap::new(),
            severity: AuditSeverity::Info,
        }
    }

    /// Adds trace context to this audit entry.
    pub fn with_trace_context(mut self, trace_context: TraceContext) -> Self {
        self.trace_context = Some(trace_context);
        self
    }

    /// Sets the severity level.
    pub fn with_severity(mut self, severity: AuditSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Adds metadata.
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_audit_entry() {
        let entry = AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success);
        assert_eq!(entry.event_type, AuditEventType::CompilationStarted);
        assert_eq!(entry.status, AuditStatus::Success);
        assert_eq!(entry.severity, AuditSeverity::Info);
    }

    #[test]
    fn test_user_action_entry() {
        let resource_id = Uuid::new_v4();
        let entry = AuditLogEntry::user_action(
            "user@example.com".to_string(),
            "CompileModule".to_string(),
            resource_id,
        );
        assert_eq!(entry.actor, Some("user@example.com".to_string()));
        assert_eq!(entry.resource_id, Some(resource_id));
        assert_eq!(entry.status, AuditStatus::Success);
    }

    #[test]
    fn test_state_change_entry() {
        let resource_id = Uuid::new_v4();
        let entry = AuditLogEntry::state_change(
            resource_id,
            "Pending".to_string(),
            "Completed".to_string(),
        );
        assert_eq!(entry.event_type, AuditEventType::OperationStateChanged);
        assert_eq!(entry.resource_id, Some(resource_id));
    }

    #[test]
    fn test_receipt_event_entry() {
        let receipt_id = Uuid::new_v4();
        let operation_id = Uuid::new_v4();
        let entry = AuditLogEntry::receipt_event(
            receipt_id,
            operation_id,
            "Receipt created and signed".to_string(),
        );
        assert_eq!(entry.event_type, AuditEventType::ReceiptCreated);
        assert_eq!(entry.resource_id, Some(receipt_id));
    }

    #[test]
    fn test_trace_context() {
        let trace = TraceContext::default();
        assert!(!trace.trace_id.is_empty());
        assert!(!trace.span_id.is_empty());
    }

    #[test]
    fn test_audit_entry_with_trace() {
        let trace = TraceContext {
            trace_id: "abc123".to_string(),
            span_id: "def456".to_string(),
            trace_flags: Some("01".to_string()),
            parent_span_id: None,
            trace_state: HashMap::new(),
            request_id: Some("req-789".to_string()),
        };

        let entry = AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success)
            .with_trace_context(trace.clone());

        assert_eq!(entry.trace_context, Some(trace));
    }

    #[test]
    fn test_audit_entry_serialization() {
        let entry = AuditLogEntry::new(AuditEventType::OperationCreated, AuditStatus::Success)
            .with_severity(AuditSeverity::Warning);

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: AuditLogEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry.event_type, deserialized.event_type);
        assert_eq!(entry.status, deserialized.status);
        assert_eq!(entry.severity, deserialized.severity);
    }
}
