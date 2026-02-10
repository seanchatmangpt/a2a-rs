//! Admission control port definitions
//!
//! Defines the interface for Life Firewall admission control,
//! which gates work entry based on WIP limits, quality scores,
//! and system health indicators.

#[cfg(feature = "server")]
use async_trait::async_trait;

use crate::domain::{
    A2AError, AdmissionDecision, IngressChannel, JidokaMode, SupplierQuality, SystemHealth,
    WorkPacket,
};

/// Port trait for admission control
///
/// Implementations gate work entry using:
/// - WIP token limiting (backpressure)
/// - Supplier quality scoring
/// - Jidoka modes (GREEN/YELLOW/RED)
pub trait AdmissionController {
    /// Request admission for a work packet
    ///
    /// Returns an admission decision (admitted or refused with receipt)
    fn request_admission(&self, work_packet: WorkPacket) -> Result<AdmissionDecision, A2AError>;

    /// Get current system health indicators
    fn get_system_health(&self) -> Result<SystemHealth, A2AError>;

    /// Get supplier quality metrics
    fn get_supplier_quality(&self, supplier_id: &str) -> Result<SupplierQuality, A2AError>;

    /// Update Jidoka mode (quality gate)
    fn set_jidoka_mode(&self, mode: JidokaMode) -> Result<(), A2AError>;

    /// Complete a work packet (release WIP token, update quality metrics)
    fn complete_work(&self, work_packet_id: &str, success: bool) -> Result<(), A2AError>;

    /// Get current WIP count
    fn get_wip_count(&self) -> Result<usize, A2AError>;

    /// Get maximum WIP limit
    fn get_wip_limit(&self) -> Result<usize, A2AError>;

    /// Update WIP limit (for dynamic backpressure adjustment)
    fn set_wip_limit(&self, limit: usize) -> Result<(), A2AError>;
}

#[cfg(feature = "server")]
#[async_trait]
/// Async port trait for admission control
pub trait AsyncAdmissionController: Send + Sync {
    /// Request admission for a work packet
    ///
    /// Returns an admission decision (admitted or refused with receipt)
    async fn request_admission(
        &self,
        work_packet: WorkPacket,
    ) -> Result<AdmissionDecision, A2AError>;

    /// Get current system health indicators
    async fn get_system_health(&self) -> Result<SystemHealth, A2AError>;

    /// Get supplier quality metrics
    async fn get_supplier_quality<'a>(
        &self,
        supplier_id: &'a str,
    ) -> Result<SupplierQuality, A2AError>;

    /// Update Jidoka mode (quality gate)
    async fn set_jidoka_mode(&self, mode: JidokaMode) -> Result<(), A2AError>;

    /// Complete a work packet (release WIP token, update quality metrics)
    async fn complete_work<'a>(
        &self,
        work_packet_id: &'a str,
        success: bool,
    ) -> Result<(), A2AError>;

    /// Get current WIP count
    async fn get_wip_count(&self) -> Result<usize, A2AError>;

    /// Get maximum WIP limit
    async fn get_wip_limit(&self) -> Result<usize, A2AError>;

    /// Update WIP limit (for dynamic backpressure adjustment)
    async fn set_wip_limit(&self, limit: usize) -> Result<(), A2AError>;

    /// Validate work packet before admission
    async fn validate_work_packet<'a>(&self, work_packet: &'a WorkPacket) -> Result<(), A2AError> {
        // Default validation
        if work_packet.id.trim().is_empty() {
            return Err(A2AError::ValidationError {
                field: "id".to_string(),
                message: "Work packet ID cannot be empty".to_string(),
            });
        }

        if work_packet.objective.trim().is_empty() {
            return Err(A2AError::ValidationError {
                field: "objective".to_string(),
                message: "Work objective cannot be empty".to_string(),
            });
        }

        if work_packet.constraints.max_execution_time_secs == 0 {
            return Err(A2AError::ValidationError {
                field: "constraints.max_execution_time_secs".to_string(),
                message: "Max execution time must be greater than 0".to_string(),
            });
        }

        Ok(())
    }

    /// Check if admission is allowed for given channel under current Jidoka mode
    async fn is_channel_allowed(&self, channel: IngressChannel) -> Result<bool, A2AError> {
        let health = self.get_system_health().await?;

        Ok(match health.jidoka_mode {
            JidokaMode::Green => true,
            JidokaMode::Yellow => channel == IngressChannel::Emergency,
            JidokaMode::Red => false,
        })
    }
}
