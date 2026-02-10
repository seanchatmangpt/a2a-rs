//! Life Firewall domain types for admission control
//!
//! This module defines the core types for a Life Firewall admission control system
//! that gates work entry using WIP token limiting, supplier quality tracking,
//! and Jidoka-style quality modes.

use serde::{Deserialize, Serialize};

/// Ingress channel classifications for incoming work
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IngressChannel {
    /// Regular batched work with standard SLA
    Batch,
    /// Pre-scheduled work with committed delivery time
    Scheduled,
    /// High-priority emergency work requiring immediate attention
    Emergency,
}

/// Jidoka quality modes that gate admission decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JidokaMode {
    /// Normal operation - all channels accepting work
    Green,
    /// Degraded operation - emergency only
    Yellow,
    /// System halt - no new work accepted
    Red,
}

impl Default for JidokaMode {
    fn default() -> Self {
        JidokaMode::Green
    }
}

/// Work packet representing a unit of work requesting admission
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkPacket {
    /// Unique identifier for this work packet
    pub id: String,
    /// What this work aims to accomplish
    pub objective: String,
    /// Resource and temporal constraints
    pub constraints: WorkConstraints,
    /// How to verify successful completion
    pub acceptance_test: String,
    /// Whether this work can be cleanly rolled back
    pub reversibility: bool,
    /// Ingress channel classification
    pub channel: IngressChannel,
    /// Optional supplier/source identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplier_id: Option<String>,
    /// Optional priority score (higher = more urgent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
}

/// Resource and temporal constraints for work execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkConstraints {
    /// Maximum execution time in seconds
    pub max_execution_time_secs: u64,
    /// Maximum memory usage in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_memory_bytes: Option<u64>,
    /// Deadline for completion (ISO 8601 timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
}

/// Reasons for refusing admission
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum RefusalReason {
    /// WIP token limit exceeded
    #[serde(rename_all = "camelCase")]
    WipLimitExceeded { current_wip: usize, max_wip: usize },
    /// Supplier quality score too low
    #[serde(rename_all = "camelCase")]
    LowSupplierQuality {
        supplier_id: String,
        quality_score: f64,
        min_threshold: f64,
    },
    /// Jidoka mode prevents admission
    #[serde(rename_all = "camelCase")]
    JidokaModeRestriction {
        current_mode: JidokaMode,
        channel: IngressChannel,
    },
    /// Resource constraints cannot be satisfied
    #[serde(rename_all = "camelCase")]
    ResourceConstraintsUnsatisfiable { reason: String },
    /// Missing required fields or validation failure
    #[serde(rename_all = "camelCase")]
    ValidationFailure { field: String, message: String },
}

/// Receipt documenting refusal of a work packet
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefusalReceipt {
    /// ID of the refused work packet
    pub work_packet_id: String,
    /// When refusal occurred (ISO 8601 timestamp)
    pub refused_at: String,
    /// Structured reason for refusal
    pub reason: RefusalReason,
    /// Current system health indicators
    pub system_health: SystemHealth,
    /// Optional human-readable message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// System health indicators reported with refusal receipts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemHealth {
    /// Current Jidoka mode
    pub jidoka_mode: JidokaMode,
    /// Current WIP count
    pub current_wip: usize,
    /// Maximum WIP tokens
    pub max_wip: usize,
    /// Overall system quality score (0.0-1.0)
    pub quality_score: f64,
}

/// Supplier quality metrics for tracking defect rates
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierQuality {
    /// Supplier identifier
    pub supplier_id: String,
    /// Total work packets submitted
    pub total_submitted: u64,
    /// Work packets that passed acceptance tests
    pub successful: u64,
    /// Work packets that failed acceptance tests
    pub defects: u64,
    /// Calculated quality score (0.0-1.0)
    pub quality_score: f64,
    /// When metrics were last updated
    pub last_updated: String,
}

impl SupplierQuality {
    /// Create a new supplier quality record
    pub fn new(supplier_id: String) -> Self {
        Self {
            supplier_id,
            total_submitted: 0,
            successful: 0,
            defects: 0,
            quality_score: 1.0, // Start optimistic
            last_updated: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Record a successful work completion
    pub fn record_success(&mut self) {
        self.successful += 1;
        self.total_submitted += 1;
        self.recalculate_score();
        self.last_updated = chrono::Utc::now().to_rfc3339();
    }

    /// Record a defect/failure
    pub fn record_defect(&mut self) {
        self.defects += 1;
        self.total_submitted += 1;
        self.recalculate_score();
        self.last_updated = chrono::Utc::now().to_rfc3339();
    }

    /// Recalculate quality score based on success rate
    fn recalculate_score(&mut self) {
        if self.total_submitted == 0 {
            self.quality_score = 1.0;
        } else {
            self.quality_score = self.successful as f64 / self.total_submitted as f64;
        }
    }
}

/// Admission decision result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum AdmissionDecision {
    /// Work packet admitted for processing
    #[serde(rename_all = "camelCase")]
    Admitted {
        work_packet_id: String,
        admitted_at: String,
        assigned_token_id: String,
    },
    /// Work packet refused with receipt
    #[serde(rename_all = "camelCase")]
    Refused { receipt: RefusalReceipt },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supplier_quality_new() {
        let sq = SupplierQuality::new("supplier-1".to_string());
        assert_eq!(sq.supplier_id, "supplier-1");
        assert_eq!(sq.total_submitted, 0);
        assert_eq!(sq.quality_score, 1.0);
    }

    #[test]
    fn test_supplier_quality_scoring() {
        let mut sq = SupplierQuality::new("supplier-1".to_string());

        sq.record_success();
        assert_eq!(sq.total_submitted, 1);
        assert_eq!(sq.successful, 1);
        assert_eq!(sq.quality_score, 1.0);

        sq.record_defect();
        assert_eq!(sq.total_submitted, 2);
        assert_eq!(sq.defects, 1);
        assert_eq!(sq.quality_score, 0.5);

        sq.record_success();
        assert_eq!(sq.total_submitted, 3);
        assert_eq!(sq.successful, 2);
        assert!((sq.quality_score - 0.6666).abs() < 0.01);
    }

    #[test]
    fn test_jidoka_mode_default() {
        assert_eq!(JidokaMode::default(), JidokaMode::Green);
    }

    #[test]
    fn test_ingress_channel_serialization() {
        let channel = IngressChannel::Emergency;
        let json = serde_json::to_string(&channel).unwrap();
        assert_eq!(json, "\"emergency\"");
    }

    #[test]
    fn test_refusal_reason_serialization() {
        let reason = RefusalReason::WipLimitExceeded {
            current_wip: 10,
            max_wip: 5,
        };
        let json = serde_json::to_value(&reason).unwrap();
        assert_eq!(json["type"], "wipLimitExceeded");
        assert_eq!(json["currentWip"], 10);
        assert_eq!(json["maxWip"], 5);
    }
}
