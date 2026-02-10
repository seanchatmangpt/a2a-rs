//! Default Life Firewall admission controller implementation
//!
//! Provides an in-memory implementation of the admission control port
//! with WIP limiting, supplier quality tracking, and Jidoka modes.

use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "server")]
use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::domain::{
    A2AError, AdmissionDecision, IngressChannel, JidokaMode, RefusalReason, RefusalReceipt,
    SupplierQuality, SystemHealth, WorkPacket,
};
use crate::port::AsyncAdmissionController;

/// Configuration for the admission controller
#[derive(Debug, Clone)]
pub struct AdmissionConfig {
    /// Maximum WIP (Work-In-Progress) tokens
    pub max_wip: usize,
    /// Minimum supplier quality score threshold (0.0-1.0)
    pub min_supplier_quality: f64,
    /// Initial Jidoka mode
    pub initial_jidoka_mode: JidokaMode,
}

impl Default for AdmissionConfig {
    fn default() -> Self {
        Self {
            max_wip: 10,
            min_supplier_quality: 0.5,
            initial_jidoka_mode: JidokaMode::Green,
        }
    }
}

/// Internal state for tracking work packets and suppliers
#[derive(Debug)]
struct AdmissionState {
    /// Current WIP count
    current_wip: usize,
    /// Maximum WIP limit
    max_wip: usize,
    /// Minimum supplier quality threshold
    min_supplier_quality: f64,
    /// Current Jidoka mode
    jidoka_mode: JidokaMode,
    /// Work packets currently in progress (keyed by packet ID)
    in_progress: HashMap<String, WorkPacket>,
    /// Supplier quality metrics (keyed by supplier ID)
    suppliers: HashMap<String, SupplierQuality>,
}

impl AdmissionState {
    fn new(config: AdmissionConfig) -> Self {
        Self {
            current_wip: 0,
            max_wip: config.max_wip,
            min_supplier_quality: config.min_supplier_quality,
            jidoka_mode: config.initial_jidoka_mode,
            in_progress: HashMap::new(),
            suppliers: HashMap::new(),
        }
    }

    fn calculate_quality_score(&self) -> f64 {
        if self.suppliers.is_empty() {
            return 1.0;
        }

        let total_score: f64 = self.suppliers.values().map(|sq| sq.quality_score).sum();
        total_score / self.suppliers.len() as f64
    }

    fn get_system_health(&self) -> SystemHealth {
        SystemHealth {
            jidoka_mode: self.jidoka_mode,
            current_wip: self.current_wip,
            max_wip: self.max_wip,
            quality_score: self.calculate_quality_score(),
        }
    }

    fn is_channel_allowed(&self, channel: IngressChannel) -> bool {
        match self.jidoka_mode {
            JidokaMode::Green => true,
            JidokaMode::Yellow => channel == IngressChannel::Emergency,
            JidokaMode::Red => false,
        }
    }

    fn check_wip_limit(&self) -> Result<(), RefusalReason> {
        if self.current_wip >= self.max_wip {
            Err(RefusalReason::WipLimitExceeded {
                current_wip: self.current_wip,
                max_wip: self.max_wip,
            })
        } else {
            Ok(())
        }
    }

    fn check_supplier_quality(&self, supplier_id: &str) -> Result<(), RefusalReason> {
        if let Some(supplier) = self.suppliers.get(supplier_id) {
            if supplier.quality_score < self.min_supplier_quality {
                return Err(RefusalReason::LowSupplierQuality {
                    supplier_id: supplier_id.to_string(),
                    quality_score: supplier.quality_score,
                    min_threshold: self.min_supplier_quality,
                });
            }
        }
        Ok(())
    }

    fn check_jidoka_mode(&self, channel: IngressChannel) -> Result<(), RefusalReason> {
        if !self.is_channel_allowed(channel) {
            Err(RefusalReason::JidokaModeRestriction {
                current_mode: self.jidoka_mode,
                channel,
            })
        } else {
            Ok(())
        }
    }
}

/// Default in-memory implementation of the admission controller
pub struct DefaultAdmissionController {
    state: Arc<RwLock<AdmissionState>>,
}

impl DefaultAdmissionController {
    /// Create a new admission controller with default configuration
    pub fn new() -> Self {
        Self::with_config(AdmissionConfig::default())
    }

    /// Create a new admission controller with custom configuration
    pub fn with_config(config: AdmissionConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(AdmissionState::new(config))),
        }
    }

    /// Create a refusal receipt
    fn create_refusal_receipt(
        work_packet_id: String,
        reason: RefusalReason,
        health: SystemHealth,
    ) -> RefusalReceipt {
        RefusalReceipt {
            work_packet_id,
            refused_at: chrono::Utc::now().to_rfc3339(),
            reason,
            system_health: health,
            message: None,
        }
    }
}

impl Default for DefaultAdmissionController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "server")]
#[async_trait]
impl AsyncAdmissionController for DefaultAdmissionController {
    async fn request_admission(
        &self,
        work_packet: WorkPacket,
    ) -> Result<AdmissionDecision, A2AError> {
        // Validate work packet
        self.validate_work_packet(&work_packet).await?;

        let mut state = self.state.write().await;

        // Get current system health for potential refusal receipt
        let health = state.get_system_health();

        // Check Jidoka mode first (quality gate)
        if let Err(reason) = state.check_jidoka_mode(work_packet.channel) {
            return Ok(AdmissionDecision::Refused {
                receipt: Self::create_refusal_receipt(work_packet.id.clone(), reason, health),
            });
        }

        // Check WIP limit (backpressure)
        if let Err(reason) = state.check_wip_limit() {
            return Ok(AdmissionDecision::Refused {
                receipt: Self::create_refusal_receipt(work_packet.id.clone(), reason, health),
            });
        }

        // Check supplier quality if supplier is specified
        if let Some(ref supplier_id) = work_packet.supplier_id {
            if let Err(reason) = state.check_supplier_quality(supplier_id) {
                return Ok(AdmissionDecision::Refused {
                    receipt: Self::create_refusal_receipt(work_packet.id.clone(), reason, health),
                });
            }
        }

        // All checks passed - admit the work
        let token_id = uuid::Uuid::new_v4().to_string();
        let work_packet_id = work_packet.id.clone();

        state
            .in_progress
            .insert(work_packet_id.clone(), work_packet);
        state.current_wip += 1;

        Ok(AdmissionDecision::Admitted {
            work_packet_id,
            admitted_at: chrono::Utc::now().to_rfc3339(),
            assigned_token_id: token_id,
        })
    }

    async fn get_system_health(&self) -> Result<SystemHealth, A2AError> {
        let state = self.state.read().await;
        Ok(state.get_system_health())
    }

    async fn get_supplier_quality<'a>(
        &self,
        supplier_id: &'a str,
    ) -> Result<SupplierQuality, A2AError> {
        let state = self.state.read().await;
        state
            .suppliers
            .get(supplier_id)
            .cloned()
            .ok_or_else(|| A2AError::InvalidRequest(format!("Supplier not found: {}", supplier_id)))
    }

    async fn set_jidoka_mode(&self, mode: JidokaMode) -> Result<(), A2AError> {
        let mut state = self.state.write().await;
        state.jidoka_mode = mode;
        Ok(())
    }

    async fn complete_work<'a>(
        &self,
        work_packet_id: &'a str,
        success: bool,
    ) -> Result<(), A2AError> {
        let mut state = self.state.write().await;

        // Remove from in-progress
        let work_packet = state.in_progress.remove(work_packet_id).ok_or_else(|| {
            A2AError::InvalidRequest(format!("Work packet not found: {}", work_packet_id))
        })?;

        // Release WIP token
        if state.current_wip > 0 {
            state.current_wip -= 1;
        }

        // Update supplier quality if supplier is specified
        if let Some(supplier_id) = work_packet.supplier_id {
            let supplier = state
                .suppliers
                .entry(supplier_id.clone())
                .or_insert_with(|| SupplierQuality::new(supplier_id));

            if success {
                supplier.record_success();
            } else {
                supplier.record_defect();
            }
        }

        Ok(())
    }

    async fn get_wip_count(&self) -> Result<usize, A2AError> {
        let state = self.state.read().await;
        Ok(state.current_wip)
    }

    async fn get_wip_limit(&self) -> Result<usize, A2AError> {
        let state = self.state.read().await;
        Ok(state.max_wip)
    }

    async fn set_wip_limit(&self, limit: usize) -> Result<(), A2AError> {
        let mut state = self.state.write().await;
        state.max_wip = limit;
        Ok(())
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    fn create_test_work_packet(id: &str, channel: IngressChannel) -> WorkPacket {
        WorkPacket {
            id: id.to_string(),
            objective: "Test objective".to_string(),
            constraints: crate::domain::WorkConstraints {
                max_execution_time_secs: 60,
                max_memory_bytes: None,
                deadline: None,
            },
            acceptance_test: "Test acceptance criteria".to_string(),
            reversibility: true,
            channel,
            supplier_id: Some("test-supplier".to_string()),
            priority: None,
        }
    }

    #[tokio::test]
    async fn test_admission_basic() {
        let controller = DefaultAdmissionController::new();
        let packet = create_test_work_packet("work-1", IngressChannel::Batch);

        let decision = controller.request_admission(packet).await.unwrap();
        assert!(matches!(decision, AdmissionDecision::Admitted { .. }));

        let wip = controller.get_wip_count().await.unwrap();
        assert_eq!(wip, 1);
    }

    #[tokio::test]
    async fn test_wip_limit() {
        let config = AdmissionConfig {
            max_wip: 2,
            ..Default::default()
        };
        let controller = DefaultAdmissionController::with_config(config);

        // Admit first two packets
        let packet1 = create_test_work_packet("work-1", IngressChannel::Batch);
        let packet2 = create_test_work_packet("work-2", IngressChannel::Batch);

        controller.request_admission(packet1).await.unwrap();
        controller.request_admission(packet2).await.unwrap();

        // Third should be refused
        let packet3 = create_test_work_packet("work-3", IngressChannel::Batch);
        let decision = controller.request_admission(packet3).await.unwrap();

        match decision {
            AdmissionDecision::Refused { receipt } => {
                assert!(matches!(
                    receipt.reason,
                    RefusalReason::WipLimitExceeded { .. }
                ));
            }
            _ => panic!("Expected refusal"),
        }
    }

    #[tokio::test]
    async fn test_jidoka_mode_yellow() {
        let controller = DefaultAdmissionController::new();

        // Set to yellow mode
        controller
            .set_jidoka_mode(JidokaMode::Yellow)
            .await
            .unwrap();

        // Batch work should be refused
        let batch = create_test_work_packet("work-1", IngressChannel::Batch);
        let decision = controller.request_admission(batch).await.unwrap();
        assert!(matches!(decision, AdmissionDecision::Refused { .. }));

        // Emergency should be admitted
        let emergency = create_test_work_packet("work-2", IngressChannel::Emergency);
        let decision = controller.request_admission(emergency).await.unwrap();
        assert!(matches!(decision, AdmissionDecision::Admitted { .. }));
    }

    #[tokio::test]
    async fn test_jidoka_mode_red() {
        let controller = DefaultAdmissionController::new();

        // Set to red mode
        controller.set_jidoka_mode(JidokaMode::Red).await.unwrap();

        // All work should be refused
        let emergency = create_test_work_packet("work-1", IngressChannel::Emergency);
        let decision = controller.request_admission(emergency).await.unwrap();
        assert!(matches!(decision, AdmissionDecision::Refused { .. }));
    }

    #[tokio::test]
    async fn test_supplier_quality_tracking() {
        let controller = DefaultAdmissionController::new();

        // Admit and complete work successfully
        let packet = create_test_work_packet("work-1", IngressChannel::Batch);
        controller.request_admission(packet).await.unwrap();
        controller.complete_work("work-1", true).await.unwrap();

        // Check supplier quality
        let quality = controller
            .get_supplier_quality("test-supplier")
            .await
            .unwrap();
        assert_eq!(quality.successful, 1);
        assert_eq!(quality.quality_score, 1.0);

        // Complete with defect
        let packet2 = create_test_work_packet("work-2", IngressChannel::Batch);
        controller.request_admission(packet2).await.unwrap();
        controller.complete_work("work-2", false).await.unwrap();

        let quality = controller
            .get_supplier_quality("test-supplier")
            .await
            .unwrap();
        assert_eq!(quality.defects, 1);
        assert_eq!(quality.quality_score, 0.5);
    }

    #[tokio::test]
    async fn test_complete_work_releases_token() {
        let config = AdmissionConfig {
            max_wip: 1,
            ..Default::default()
        };
        let controller = DefaultAdmissionController::with_config(config);

        // Admit first packet
        let packet1 = create_test_work_packet("work-1", IngressChannel::Batch);
        controller.request_admission(packet1).await.unwrap();

        // Second should be refused due to WIP limit
        let packet2 = create_test_work_packet("work-2", IngressChannel::Batch);
        let decision = controller.request_admission(packet2).await.unwrap();
        assert!(matches!(decision, AdmissionDecision::Refused { .. }));

        // Complete first work
        controller.complete_work("work-1", true).await.unwrap();

        // Now second packet should be admitted
        let packet3 = create_test_work_packet("work-3", IngressChannel::Batch);
        let decision = controller.request_admission(packet3).await.unwrap();
        assert!(matches!(decision, AdmissionDecision::Admitted { .. }));
    }
}
