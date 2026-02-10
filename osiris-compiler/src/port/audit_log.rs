//! Port trait for audit logging.
//!
//! Defines the interface for logging audit events with structured trace context.
//! Implementations can write to Cloud Logging, local files, databases, etc.

use crate::domain::{AuditError, AuditLogEntry};
use async_trait::async_trait;
use uuid::Uuid;

/// Port trait for audit logging.
///
/// Implementations log audit entries (user actions, state changes, receipt events)
/// with structured trace context and metadata. Supports querying historical logs.
#[async_trait]
pub trait AuditLog: Send + Sync {
    /// Logs an audit entry.
    ///
    /// Implementations should:
    /// 1. Validate the entry
    /// 2. Serialize with trace context
    /// 3. Write to the configured backend (Cloud Logging, file, database, etc.)
    /// 4. Handle failures gracefully (log locally if remote service unavailable)
    ///
    /// # Arguments
    /// * `entry` - The audit log entry to record
    ///
    /// # Returns
    /// The ID of the logged entry, or an error if logging failed
    async fn log(&self, entry: AuditLogEntry) -> Result<Uuid, AuditError>;

    /// Logs multiple audit entries as a batch.
    ///
    /// Implementations should attempt to batch-write for efficiency.
    /// If batching is not supported, fall back to individual writes.
    ///
    /// # Arguments
    /// * `entries` - The entries to log
    ///
    /// # Returns
    /// The number of entries successfully logged
    async fn log_batch(&self, entries: Vec<AuditLogEntry>) -> Result<usize, AuditError> {
        let mut count = 0;
        for entry in entries {
            if self.log(entry).await.is_ok() {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Retrieves audit logs for a specific resource.
    ///
    /// # Arguments
    /// * `resource_id` - The resource to retrieve logs for
    ///
    /// # Returns
    /// All audit log entries for the given resource
    async fn get_logs_for_resource(
        &self,
        resource_id: Uuid,
    ) -> Result<Vec<AuditLogEntry>, AuditError>;

    /// Retrieves audit logs for a specific actor (user/service).
    ///
    /// # Arguments
    /// * `actor` - The actor identifier (username, service name, etc.)
    ///
    /// # Returns
    /// All audit log entries by the given actor
    async fn get_logs_by_actor(&self, actor: &str) -> Result<Vec<AuditLogEntry>, AuditError>;

    /// Retrieves audit logs within a time range.
    ///
    /// # Arguments
    /// * `start` - Start time (inclusive)
    /// * `end` - End time (inclusive)
    ///
    /// # Returns
    /// All audit log entries in the time range
    async fn get_logs_in_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<AuditLogEntry>, AuditError>;

    /// Retrieves a specific audit log entry by ID.
    ///
    /// # Arguments
    /// * `entry_id` - The ID of the entry to retrieve
    ///
    /// # Returns
    /// The audit log entry, if found
    async fn get_log_entry(&self, entry_id: Uuid) -> Result<AuditLogEntry, AuditError>;

    /// Searches audit logs by event type.
    ///
    /// # Arguments
    /// * `event_type` - The event type to search for
    ///
    /// # Returns
    /// All audit log entries of the given type
    async fn get_logs_by_event_type(
        &self,
        event_type: crate::domain::AuditEventType,
    ) -> Result<Vec<AuditLogEntry>, AuditError>;

    /// Searches audit logs by trace ID for distributed tracing.
    ///
    /// # Arguments
    /// * `trace_id` - The W3C trace ID to search for
    ///
    /// # Returns
    /// All audit log entries with the given trace ID
    async fn get_logs_by_trace_id(&self, trace_id: &str) -> Result<Vec<AuditLogEntry>, AuditError>;

    /// Checks if the audit log backend is healthy.
    ///
    /// Used for readiness checks and health monitoring.
    ///
    /// # Returns
    /// `Ok(())` if healthy, `Err` with diagnostic message otherwise
    async fn health_check(&self) -> Result<(), AuditError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AuditEventType, AuditStatus};

    struct MockAuditLog;

    #[async_trait]
    impl AuditLog for MockAuditLog {
        async fn log(&self, _entry: AuditLogEntry) -> Result<Uuid, AuditError> {
            Ok(Uuid::new_v4())
        }

        async fn get_logs_for_resource(
            &self,
            _resource_id: Uuid,
        ) -> Result<Vec<AuditLogEntry>, AuditError> {
            Ok(vec![])
        }

        async fn get_logs_by_actor(&self, _actor: &str) -> Result<Vec<AuditLogEntry>, AuditError> {
            Ok(vec![])
        }

        async fn get_logs_in_range(
            &self,
            _start: chrono::DateTime<chrono::Utc>,
            _end: chrono::DateTime<chrono::Utc>,
        ) -> Result<Vec<AuditLogEntry>, AuditError> {
            Ok(vec![])
        }

        async fn get_log_entry(&self, _entry_id: Uuid) -> Result<AuditLogEntry, AuditError> {
            Err(AuditError::WriteError("Not found".to_string()))
        }

        async fn get_logs_by_event_type(
            &self,
            _event_type: crate::domain::AuditEventType,
        ) -> Result<Vec<AuditLogEntry>, AuditError> {
            Ok(vec![])
        }

        async fn get_logs_by_trace_id(
            &self,
            _trace_id: &str,
        ) -> Result<Vec<AuditLogEntry>, AuditError> {
            Ok(vec![])
        }

        async fn health_check(&self) -> Result<(), AuditError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_mock_audit_log() {
        let log = MockAuditLog;
        let entry = AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success);

        let entry_id = log.log(entry).await.unwrap();
        assert!(!entry_id.to_string().is_empty());

        let health = log.health_check().await;
        assert!(health.is_ok());
    }
}
