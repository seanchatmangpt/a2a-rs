//! Cloud Logging audit logger adapter.
//!
//! Implements the AuditLog port using Google Cloud Logging with:
//! - Structured JSON logging
//! - W3C trace context support
//! - Batch log entry writing
//! - Local fallback if Cloud Logging is unavailable

use crate::domain::{AuditError, AuditEventType, AuditLogEntry};
use crate::port::AuditLog;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use uuid::Uuid;

/// Configuration for Cloud Logging audit logger.
#[derive(Debug, Clone)]
pub struct CloudLoggingConfig {
    /// Google Cloud Project ID
    pub project_id: String,

    /// Log name/identifier in Cloud Logging
    pub log_name: String,

    /// Whether to include full trace context in logs
    pub include_trace_context: bool,

    /// Maximum batch size before flushing
    pub batch_size: usize,

    /// Enable local fallback logging if Cloud Logging unavailable
    pub enable_local_fallback: bool,

    /// Custom labels to add to all log entries
    pub labels: HashMap<String, String>,
}

impl Default for CloudLoggingConfig {
    fn default() -> Self {
        Self {
            project_id: "default-project".to_string(),
            log_name: "osiris-compiler-audit".to_string(),
            include_trace_context: true,
            batch_size: 100,
            enable_local_fallback: true,
            labels: HashMap::new(),
        }
    }
}

/// Cloud Logging audit logger adapter.
///
/// Implements the AuditLog port by writing structured logs to Google Cloud Logging
/// with full trace context support. If Cloud Logging is unavailable, logs are stored
/// locally for later replay.
pub struct CloudLoggingAuditLogger {
    config: CloudLoggingConfig,
    // Local storage for entries when Cloud Logging is unavailable
    local_buffer: Arc<Mutex<Vec<AuditLogEntry>>>,
    #[cfg(feature = "cloud-logging")]
    client: Option<Arc<google_cloud_logging::client::Client>>,
}

impl CloudLoggingAuditLogger {
    /// Creates a new Cloud Logging audit logger.
    ///
    /// If the "cloud-logging" feature is enabled and authentication is available,
    /// this will connect to Google Cloud Logging. Otherwise, logs are buffered locally.
    pub async fn new(config: CloudLoggingConfig) -> Result<Self, AuditError> {
        #[cfg(feature = "cloud-logging")]
        let client = Self::create_cloud_logging_client(&config).await;

        #[cfg(not(feature = "cloud-logging"))]
        let client: Option<String> = None;

        Ok(Self {
            config,
            local_buffer: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "cloud-logging")]
            client,
        })
    }

    /// Creates a new in-memory audit logger (no Cloud Logging).
    ///
    /// Useful for testing and when Cloud Logging is not available.
    pub fn in_memory(config: CloudLoggingConfig) -> Self {
        Self {
            config,
            local_buffer: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "cloud-logging")]
            client: None,
        }
    }

    /// Creates a default Cloud Logging audit logger.
    pub async fn default_config(project_id: String) -> Result<Self, AuditError> {
        let mut config = CloudLoggingConfig::default();
        config.project_id = project_id;
        Self::new(config).await
    }

    /// Converts an audit entry to a Cloud Logging JSON payload.
    ///
    /// The payload includes:
    /// - Standard log fields (timestamp, severity, message)
    /// - Trace context (if available)
    /// - Structured data (labels, metadata)
    /// - JSON payload with all entry details
    fn entry_to_cloud_logging_payload(&self, entry: &AuditLogEntry) -> serde_json::Value {
        let mut payload = json!({
            "auditEntry": {
                "id": entry.id.to_string(),
                "timestamp": entry.timestamp.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                "eventType": format!("{:?}", entry.event_type),
                "status": format!("{:?}", entry.status),
                "severity": format!("{:?}", entry.severity),
            }
        });

        // Add actor if present
        if let Some(actor) = &entry.actor {
            payload["auditEntry"]["actor"] = json!(actor);
        }

        // Add resource information
        if let Some(resource_id) = entry.resource_id {
            payload["auditEntry"]["resourceId"] = json!(resource_id.to_string());
        }
        if let Some(resource_type) = &entry.resource_type {
            payload["auditEntry"]["resourceType"] = json!(resource_type);
        }
        if let Some(action) = &entry.action {
            payload["auditEntry"]["action"] = json!(action);
        }

        // Add details (structured audit details)
        payload["auditEntry"]["details"] =
            serde_json::to_value(&entry.details).unwrap_or_else(|_| json!({}));

        // Add trace context if available and enabled
        if self.config.include_trace_context {
            if let Some(trace_context) = &entry.trace_context {
                payload["traceContext"] = json!({
                    "traceId": trace_context.trace_id,
                    "spanId": trace_context.span_id,
                    "traceFlags": trace_context.trace_flags,
                    "parentSpanId": trace_context.parent_span_id,
                    "requestId": trace_context.request_id,
                    "traceState": trace_context.trace_state,
                });

                // Add trace-related fields for Cloud Logging filtering
                payload["trace"] = json!(format!(
                    "projects/{}/traces/{}",
                    self.config.project_id, trace_context.trace_id
                ));
                payload["spanId"] = json!(trace_context.span_id);
            }
        }

        // Add metadata
        if !entry.metadata.is_empty() {
            payload["metadata"] =
                serde_json::to_value(&entry.metadata).unwrap_or_else(|_| json!({}));
        }

        // Add configured labels
        let mut labels = self.config.labels.clone();
        if let Some(resource_type) = &entry.resource_type {
            labels.insert("resource_type".to_string(), resource_type.clone());
        }
        labels.insert("event_type".to_string(), format!("{:?}", entry.event_type));

        payload["labels"] = serde_json::to_value(labels).unwrap_or_else(|_| json!({}));

        payload
    }

    /// Formats an entry as a human-readable log message.
    fn entry_to_log_message(&self, entry: &AuditLogEntry) -> String {
        format!(
            "[{}] {} ({}): {} - Resource: {} - Actor: {} - Message: {}",
            entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
            format!("{:?}", entry.event_type),
            format!("{:?}", entry.severity),
            format!("{:?}", entry.status),
            entry
                .resource_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            entry.actor.as_ref().map(|a| a.as_str()).unwrap_or("System"),
            entry.action.as_ref().map(|a| a.as_str()).unwrap_or(""),
        )
    }

    /// Writes an entry to the local buffer (fallback mechanism).
    fn write_to_local_buffer(&self, entry: AuditLogEntry) -> Result<Uuid, AuditError> {
        let entry_id = entry.id;
        let mut buffer = self
            .local_buffer
            .lock()
            .map_err(|e| AuditError::WriteError(format!("Failed to lock local buffer: {}", e)))?;

        buffer.push(entry);

        // Trim buffer if it exceeds reasonable size (10k entries)
        if buffer.len() > 10000 {
            buffer.drain(0..5000);
        }

        Ok(entry_id)
    }

    /// Retrieves entries from local buffer.
    fn get_from_local_buffer(
        &self,
        filter_fn: impl Fn(&AuditLogEntry) -> bool,
    ) -> Result<Vec<AuditLogEntry>, AuditError> {
        let buffer = self
            .local_buffer
            .lock()
            .map_err(|e| AuditError::WriteError(format!("Failed to lock local buffer: {}", e)))?;

        Ok(buffer
            .iter()
            .filter(|entry| filter_fn(entry))
            .cloned()
            .collect())
    }

    #[cfg(feature = "cloud-logging")]
    async fn create_cloud_logging_client(
        _config: &CloudLoggingConfig,
    ) -> Option<Arc<google_cloud_logging::client::Client>> {
        // In a real implementation, this would create a properly authenticated client
        // For now, we return None to indicate Cloud Logging is not available
        // Users would need to provide authentication via environment variables or config
        None
    }

    /// Drains the local buffer and returns all buffered entries.
    pub fn drain_local_buffer(&self) -> Result<Vec<AuditLogEntry>, AuditError> {
        let mut buffer = self
            .local_buffer
            .lock()
            .map_err(|e| AuditError::WriteError(format!("Failed to lock local buffer: {}", e)))?;

        Ok(buffer.drain(..).collect())
    }

    /// Returns the number of entries in the local buffer.
    pub fn local_buffer_size(&self) -> Result<usize, AuditError> {
        let buffer = self
            .local_buffer
            .lock()
            .map_err(|e| AuditError::WriteError(format!("Failed to lock local buffer: {}", e)))?;

        Ok(buffer.len())
    }
}

#[async_trait]
impl AuditLog for CloudLoggingAuditLogger {
    async fn log(&self, entry: AuditLogEntry) -> Result<Uuid, AuditError> {
        let entry_id = entry.id;
        let message = self.entry_to_log_message(&entry);
        let payload = self.entry_to_cloud_logging_payload(&entry);

        #[cfg(feature = "cloud-logging")]
        {
            if let Some(_client) = &self.client {
                // In a real implementation, write to Cloud Logging here
                // For now, we demonstrate the structure:
                // client.write_log_entries(log_name, vec![LogEntry {
                //     json_payload: Some(payload),
                //     text_payload: Some(message),
                //     severity: entry.severity,
                //     timestamp: entry.timestamp,
                //     trace: format!("projects/{}/traces/{}", ...),
                //     ...
                // }]).await?;

                // For now, fall back to local logging
                tracing::info!(
                    event_type = ?entry.event_type,
                    resource_id = ?entry.resource_id,
                    trace_id = ?entry.trace_context.as_ref().map(|t| &t.trace_id),
                    "{}", message
                );

                return self.write_to_local_buffer(entry);
            }
        }

        // Log to tracing/structured logging
        tracing::info!(
            event_type = ?entry.event_type,
            resource_id = ?entry.resource_id,
            actor = ?entry.actor,
            trace_id = ?entry.trace_context.as_ref().map(|t| &t.trace_id),
            payload = %payload,
            "{}", message
        );

        // Store in local buffer if fallback is enabled
        if self.config.enable_local_fallback {
            self.write_to_local_buffer(entry)
        } else {
            Ok(entry_id)
        }
    }

    async fn log_batch(&self, entries: Vec<AuditLogEntry>) -> Result<usize, AuditError> {
        let mut success_count = 0;

        #[cfg(feature = "cloud-logging")]
        {
            if let Some(_client) = &self.client {
                // In a real implementation, batch write to Cloud Logging
                // Collect all entries as log entries and write in batches
                // For now, write each individually
                for entry in entries {
                    if self.log(entry).await.is_ok() {
                        success_count += 1;
                    }
                }
                return Ok(success_count);
            }
        }

        // Write to local buffer
        for entry in entries {
            if self.log(entry).await.is_ok() {
                success_count += 1;
            }
        }

        Ok(success_count)
    }

    async fn get_logs_for_resource(
        &self,
        resource_id: Uuid,
    ) -> Result<Vec<AuditLogEntry>, AuditError> {
        self.get_from_local_buffer(|entry| entry.resource_id == Some(resource_id))
    }

    async fn get_logs_by_actor(&self, actor: &str) -> Result<Vec<AuditLogEntry>, AuditError> {
        self.get_from_local_buffer(|entry| entry.actor.as_ref().map(|a| a.as_str()) == Some(actor))
    }

    async fn get_logs_in_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<AuditLogEntry>, AuditError> {
        self.get_from_local_buffer(|entry| entry.timestamp >= start && entry.timestamp <= end)
    }

    async fn get_log_entry(&self, entry_id: Uuid) -> Result<AuditLogEntry, AuditError> {
        let entries = self.get_from_local_buffer(|entry| entry.id == entry_id)?;

        entries
            .into_iter()
            .next()
            .ok_or_else(|| AuditError::WriteError(format!("Entry not found: {}", entry_id)))
    }

    async fn get_logs_by_event_type(
        &self,
        event_type: AuditEventType,
    ) -> Result<Vec<AuditLogEntry>, AuditError> {
        self.get_from_local_buffer(|entry| entry.event_type == event_type)
    }

    async fn get_logs_by_trace_id(&self, trace_id: &str) -> Result<Vec<AuditLogEntry>, AuditError> {
        self.get_from_local_buffer(|entry| {
            entry.trace_context.as_ref().map(|t| t.trace_id.as_str()) == Some(trace_id)
        })
    }

    async fn health_check(&self) -> Result<(), AuditError> {
        #[cfg(feature = "cloud-logging")]
        {
            if let Some(_client) = &self.client {
                // In a real implementation, test Cloud Logging connectivity
                // For now, just check that we can access the client
                return Ok(());
            }
        }

        // Check that local buffer is accessible
        let _lock_guard = self
            .local_buffer
            .lock()
            .map_err(|e| AuditError::ServiceError(format!("Local buffer lock failed: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AuditDetails, AuditStatus};

    #[tokio::test]
    async fn test_cloud_logging_config_default() {
        let config = CloudLoggingConfig::default();
        assert_eq!(config.project_id, "default-project");
        assert_eq!(config.log_name, "osiris-compiler-audit");
        assert!(config.include_trace_context);
        assert!(config.enable_local_fallback);
    }

    #[tokio::test]
    async fn test_audit_logger_creation() {
        let config = CloudLoggingConfig::default();
        let logger = CloudLoggingAuditLogger::in_memory(config);
        assert!(logger.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_log_entry() {
        let config = CloudLoggingConfig::default();
        let logger = CloudLoggingAuditLogger::in_memory(config);

        let entry = AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success);
        let entry_id = entry.id;

        let result = logger.log(entry).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), entry_id);
    }

    #[tokio::test]
    async fn test_log_batch() {
        let config = CloudLoggingConfig::default();
        let logger = CloudLoggingAuditLogger::in_memory(config);

        let entries = vec![
            AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success),
            AuditLogEntry::new(AuditEventType::CompilationCompleted, AuditStatus::Success),
            AuditLogEntry::new(AuditEventType::OperationCreated, AuditStatus::Success),
        ];

        let count = logger.log_batch(entries).await.unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_get_logs_for_resource() {
        let config = CloudLoggingConfig::default();
        let logger = CloudLoggingAuditLogger::in_memory(config);

        let resource_id = Uuid::new_v4();
        let mut entry = AuditLogEntry::new(AuditEventType::OperationCreated, AuditStatus::Success);
        entry.resource_id = Some(resource_id);

        logger.log(entry).await.unwrap();

        let logs = logger.get_logs_for_resource(resource_id).await.unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].resource_id, Some(resource_id));
    }

    #[tokio::test]
    async fn test_get_logs_by_actor() {
        let config = CloudLoggingConfig::default();
        let logger = CloudLoggingAuditLogger::in_memory(config);

        let entry = AuditLogEntry::user_action(
            "user@example.com".to_string(),
            "CompileModule".to_string(),
            Uuid::new_v4(),
        );

        logger.log(entry).await.unwrap();

        let logs = logger.get_logs_by_actor("user@example.com").await.unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].actor, Some("user@example.com".to_string()));
    }

    #[tokio::test]
    async fn test_get_logs_by_event_type() {
        let config = CloudLoggingConfig::default();
        let logger = CloudLoggingAuditLogger::in_memory(config);

        let entry1 = AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success);
        let entry2 = AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success);
        let entry3 = AuditLogEntry::new(AuditEventType::CompilationCompleted, AuditStatus::Success);

        logger.log(entry1).await.unwrap();
        logger.log(entry2).await.unwrap();
        logger.log(entry3).await.unwrap();

        let logs = logger
            .get_logs_by_event_type(AuditEventType::CompilationStarted)
            .await
            .unwrap();
        assert_eq!(logs.len(), 2);
    }

    #[tokio::test]
    async fn test_get_log_entry() {
        let config = CloudLoggingConfig::default();
        let logger = CloudLoggingAuditLogger::in_memory(config);

        let entry = AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success);
        let entry_id = entry.id;

        logger.log(entry).await.unwrap();

        let retrieved = logger.get_log_entry(entry_id).await.unwrap();
        assert_eq!(retrieved.id, entry_id);
    }

    #[tokio::test]
    async fn test_get_logs_by_trace_id() {
        use crate::domain::TraceContext;

        let config = CloudLoggingConfig::default();
        let logger = CloudLoggingAuditLogger::in_memory(config);

        let trace = TraceContext {
            trace_id: "abc123".to_string(),
            span_id: "def456".to_string(),
            trace_flags: Some("01".to_string()),
            parent_span_id: None,
            trace_state: HashMap::new(),
            request_id: None,
        };

        let entry = AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success)
            .with_trace_context(trace.clone());

        logger.log(entry).await.unwrap();

        let logs = logger.get_logs_by_trace_id("abc123").await.unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].trace_context.as_ref().unwrap().trace_id, "abc123");
    }

    #[tokio::test]
    async fn test_get_logs_in_range() {
        let config = CloudLoggingConfig::default();
        let logger = CloudLoggingAuditLogger::in_memory(config);

        let now = Utc::now();
        let future = now + chrono::Duration::hours(1);

        let entry = AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success);
        logger.log(entry).await.unwrap();

        let logs = logger.get_logs_in_range(now, future).await.unwrap();
        assert_eq!(logs.len(), 1);
    }

    #[test]
    fn test_entry_to_cloud_logging_payload() {
        let config = CloudLoggingConfig::default();
        let logger = CloudLoggingAuditLogger::in_memory(config);

        let entry = AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success);
        let payload = logger.entry_to_cloud_logging_payload(&entry);

        assert!(payload.get("auditEntry").is_some());
        assert!(payload["auditEntry"].get("id").is_some());
        assert!(payload["auditEntry"].get("eventType").is_some());
        assert!(payload["auditEntry"].get("status").is_some());
    }

    #[test]
    fn test_entry_to_log_message() {
        let config = CloudLoggingConfig::default();
        let logger = CloudLoggingAuditLogger::in_memory(config);

        let entry = AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success);
        let message = logger.entry_to_log_message(&entry);

        assert!(message.contains("CompilationStarted"));
        assert!(message.contains("Success"));
    }

    #[tokio::test]
    async fn test_local_buffer_size() {
        let config = CloudLoggingConfig::default();
        let logger = CloudLoggingAuditLogger::in_memory(config);

        let entry = AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success);
        logger.log(entry).await.unwrap();

        let size = logger.local_buffer_size().unwrap();
        assert_eq!(size, 1);
    }

    #[tokio::test]
    async fn test_drain_local_buffer() {
        let config = CloudLoggingConfig::default();
        let logger = CloudLoggingAuditLogger::in_memory(config);

        let entry1 = AuditLogEntry::new(AuditEventType::CompilationStarted, AuditStatus::Success);
        let entry2 = AuditLogEntry::new(AuditEventType::CompilationCompleted, AuditStatus::Success);

        logger.log(entry1).await.unwrap();
        logger.log(entry2).await.unwrap();

        let drained = logger.drain_local_buffer().unwrap();
        assert_eq!(drained.len(), 2);

        let size = logger.local_buffer_size().unwrap();
        assert_eq!(size, 0);
    }
}
