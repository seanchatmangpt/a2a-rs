//! Telemetry sink port definitions
//!
//! Interface for writing operational telemetry data to external storage systems.
//! Captures WIP state, cycle times, and refusal metrics for analytics and monitoring.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{RefusalReceipt, WipSnapshot, WorkMetrics};

/// Telemetry sink errors
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[serde(tag = "error_type", rename_all = "camelCase")]
pub enum TelemetrySinkError {
    /// Failed to connect to sink
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Failed to write telemetry record
    #[error("Write failed: {0}")]
    WriteFailed(String),

    /// Schema validation error
    #[error("Schema validation failed: {0}")]
    SchemaValidationFailed(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Cycle time telemetry record
///
/// Captures work item lifecycle metrics for performance analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CycleTimeRecord {
    /// Work item identifier
    pub work_id: Uuid,

    /// Work type classification
    pub work_type: String,

    /// Time work arrived in system
    pub arrived_at: DateTime<Utc>,

    /// Time work started processing (if applicable)
    pub started_at: Option<DateTime<Utc>>,

    /// Time work completed
    pub completed_at: Option<DateTime<Utc>>,

    /// Queue time in milliseconds (arrival → start)
    pub queue_time_ms: Option<i64>,

    /// Cycle time in milliseconds (start → completion)
    pub cycle_time_ms: Option<i64>,

    /// Lead time in milliseconds (arrival → completion)
    pub lead_time_ms: Option<i64>,

    /// Gateway instance identifier
    pub gateway_id: String,

    /// Ingestion timestamp
    pub ingested_at: DateTime<Utc>,
}

impl CycleTimeRecord {
    /// Create cycle time record from work metrics
    pub fn from_work_metrics(work_metrics: &WorkMetrics, gateway_id: impl Into<String>) -> Self {
        Self {
            work_id: work_metrics.id,
            work_type: work_metrics.work_type.clone(),
            arrived_at: work_metrics.arrived_at,
            started_at: work_metrics.started_at,
            completed_at: work_metrics.completed_at,
            queue_time_ms: work_metrics.queue_time_ms,
            cycle_time_ms: work_metrics.cycle_time_ms,
            lead_time_ms: work_metrics.lead_time_ms,
            gateway_id: gateway_id.into(),
            ingested_at: Utc::now(),
        }
    }
}

/// WIP state telemetry record
///
/// Captures snapshot of Work-in-Progress state at point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WipStateRecord {
    /// Snapshot timestamp
    pub timestamp: DateTime<Utc>,

    /// Current WIP count
    pub current_wip: usize,

    /// WIP capacity limit
    pub wip_limit: usize,

    /// Available capacity slots
    pub available_slots: usize,

    /// Utilization percentage (0-100)
    pub utilization_pct: f64,

    /// Number of work items in progress
    pub in_progress_count: usize,

    /// Gateway instance identifier
    pub gateway_id: String,

    /// Ingestion timestamp
    pub ingested_at: DateTime<Utc>,
}

impl WipStateRecord {
    /// Create WIP state record from snapshot
    pub fn from_wip_snapshot(wip_snapshot: &WipSnapshot, gateway_id: impl Into<String>) -> Self {
        Self {
            timestamp: wip_snapshot.timestamp,
            current_wip: wip_snapshot.current_wip,
            wip_limit: wip_snapshot.wip_limit,
            available_slots: wip_snapshot.available,
            utilization_pct: wip_snapshot.utilization_pct,
            in_progress_count: wip_snapshot.in_progress.len(),
            gateway_id: gateway_id.into(),
            ingested_at: Utc::now(),
        }
    }
}

/// Refusal telemetry record
///
/// Captures work rejection events with reason codes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefusalRecord {
    /// Refusal receipt identifier
    pub receipt_id: Uuid,

    /// Rejected packet identifier
    pub packet_id: String,

    /// Refusal timestamp
    pub refused_at: DateTime<Utc>,

    /// Refusal reason category
    pub reason_category: String,

    /// Detailed reason message
    pub reason_message: String,

    /// Reason-specific code (e.g., auth code, type check code)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,

    /// Current WIP at time of refusal (if WIP-related)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_wip: Option<usize>,

    /// WIP limit at time of refusal (if WIP-related)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wip_limit: Option<usize>,

    /// Gateway instance identifier
    pub gateway_id: String,

    /// Cryptographic proof hash from receipt
    pub proof_hash: String,

    /// Ingestion timestamp
    pub ingested_at: DateTime<Utc>,
}

impl RefusalRecord {
    /// Create refusal record from receipt
    pub fn from_refusal_receipt(receipt: &RefusalReceipt, gateway_id: impl Into<String>) -> Self {
        let (category, message, code, wip_context) = match &receipt.reason {
            crate::domain::RefusalReason::WipCapExceeded {
                current,
                limit,
                message,
            } => (
                "WIP_CAP_EXCEEDED".to_string(),
                message.clone(),
                None,
                Some((*current, *limit)),
            ),
            crate::domain::RefusalReason::AuthenticationFailed { code, message } => (
                "AUTHENTICATION_FAILED".to_string(),
                message.clone(),
                Some(format!("{:?}", code)),
                None,
            ),
            crate::domain::RefusalReason::GuardFailed {
                guard_id,
                condition,
                message,
            } => (
                "GUARD_FAILED".to_string(),
                message.clone(),
                Some(format!("guard={} condition={}", guard_id, condition)),
                None,
            ),
            crate::domain::RefusalReason::TypeCheckFailed {
                code,
                attempted_type,
                message,
                ..
            } => (
                "TYPE_CHECK_FAILED".to_string(),
                message.clone(),
                Some(format!("{:?}:{}", code, attempted_type)),
                None,
            ),
        };

        let (current_wip, wip_limit) = wip_context.unwrap_or((0, 0));

        Self {
            receipt_id: receipt.receipt_id,
            packet_id: receipt.packet_id.clone(),
            refused_at: receipt.timestamp,
            reason_category: category,
            reason_message: message,
            reason_code: code,
            current_wip: if current_wip > 0 {
                Some(current_wip)
            } else {
                None
            },
            wip_limit: if wip_limit > 0 { Some(wip_limit) } else { None },
            gateway_id: gateway_id.into(),
            proof_hash: receipt.proof_hash.clone(),
            ingested_at: Utc::now(),
        }
    }
}

/// Telemetry sink batch configuration
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum records per batch before flushing
    pub max_batch_size: usize,

    /// Maximum time to wait before flushing (seconds)
    pub flush_interval_sec: u64,

    /// Enable batch compression
    pub enable_compression: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            flush_interval_sec: 10,
            enable_compression: true,
        }
    }
}

/// Telemetry sink port trait
///
/// Defines interface for writing telemetry data to external systems.
/// Implementations handle batching, serialization, and delivery.
#[async_trait]
pub trait TelemetrySink: Send + Sync {
    /// Write cycle time telemetry
    async fn record_cycle_time(&self, record: CycleTimeRecord) -> Result<(), TelemetrySinkError>;

    /// Write batch of cycle time records
    async fn record_cycle_times_batch(
        &self,
        records: Vec<CycleTimeRecord>,
    ) -> Result<(), TelemetrySinkError>;

    /// Write WIP state telemetry
    async fn record_wip_state(&self, record: WipStateRecord) -> Result<(), TelemetrySinkError>;

    /// Write batch of WIP state records
    async fn record_wip_states_batch(
        &self,
        records: Vec<WipStateRecord>,
    ) -> Result<(), TelemetrySinkError>;

    /// Write refusal telemetry
    async fn record_refusal(&self, record: RefusalRecord) -> Result<(), TelemetrySinkError>;

    /// Write batch of refusal records
    async fn record_refusals_batch(
        &self,
        records: Vec<RefusalRecord>,
    ) -> Result<(), TelemetrySinkError>;

    /// Flush any pending batched telemetry
    async fn flush(&self) -> Result<(), TelemetrySinkError>;

    /// Check if sink is healthy and connected
    async fn health_check(&self) -> Result<(), TelemetrySinkError>;

    /// Get sink statistics
    async fn get_stats(&self) -> TelemetrySinkStats;
}

/// Telemetry sink statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySinkStats {
    /// Total records written
    pub total_records_written: u64,

    /// Total records failed
    pub total_records_failed: u64,

    /// Current batch queue depth
    pub queue_depth: usize,

    /// Last flush timestamp
    pub last_flush_at: Option<DateTime<Utc>>,

    /// Last error message (if any)
    pub last_error: Option<String>,

    /// Is sink connected
    pub connected: bool,
}
