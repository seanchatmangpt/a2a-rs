//! Life Firewall service for admission control
//!
//! Provides a high-level service wrapper around the admission controller
//! with async queue support for handling admission requests.

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::domain::{
    A2AError, AdmissionDecision, IngressChannel, JidokaMode, SupplierQuality, SystemHealth,
    WorkPacket,
};
use crate::port::AsyncAdmissionController;

/// Admission request with response channel
#[derive(Debug)]
pub struct AdmissionRequest {
    /// Work packet requesting admission
    pub work_packet: WorkPacket,
    /// Channel to send admission decision back
    pub response_tx: tokio::sync::oneshot::Sender<Result<AdmissionDecision, A2AError>>,
}

/// Firewall service configuration
#[derive(Debug, Clone)]
pub struct FirewallConfig {
    /// Size of the admission request queue
    pub queue_size: usize,
    /// Number of concurrent admission processors
    pub num_processors: usize,
}

impl Default for FirewallConfig {
    fn default() -> Self {
        Self {
            queue_size: 1000,
            num_processors: 4,
        }
    }
}

/// Life Firewall service
///
/// Wraps an admission controller with an async queue for processing
/// admission requests concurrently.
pub struct FirewallService<C: AsyncAdmissionController> {
    controller: Arc<C>,
    admission_tx: mpsc::Sender<AdmissionRequest>,
}

impl<C: AsyncAdmissionController + 'static> FirewallService<C> {
    /// Create a new firewall service
    ///
    /// Note: Currently uses direct controller calls.
    /// A full queue-based implementation would require additional
    /// architecture for distributing work to multiple processors.
    pub fn new(controller: C, _config: FirewallConfig) -> Self {
        let controller = Arc::new(controller);

        // Placeholder channel for future queue-based implementation
        let (admission_tx, _admission_rx) = mpsc::channel(1);

        Self {
            controller,
            admission_tx,
        }
    }

    /// Process a single admission request
    async fn process_admission_request(controller: &C, request: AdmissionRequest) {
        let result = controller.request_admission(request.work_packet).await;

        // Send response back (ignore error if receiver dropped)
        let _ = request.response_tx.send(result);
    }

    /// Request admission for a work packet (async)
    ///
    /// Returns a future that resolves to an admission decision
    pub async fn request_admission(
        &self,
        work_packet: WorkPacket,
    ) -> Result<AdmissionDecision, A2AError> {
        // Direct call to controller for now
        // In a real implementation with queue, we'd use the channel
        self.controller.request_admission(work_packet).await
    }

    /// Get current system health
    pub async fn get_system_health(&self) -> Result<SystemHealth, A2AError> {
        self.controller.get_system_health().await
    }

    /// Get supplier quality metrics
    pub async fn get_supplier_quality(
        &self,
        supplier_id: &str,
    ) -> Result<SupplierQuality, A2AError> {
        self.controller.get_supplier_quality(supplier_id).await
    }

    /// Set Jidoka mode
    pub async fn set_jidoka_mode(&self, mode: JidokaMode) -> Result<(), A2AError> {
        self.controller.set_jidoka_mode(mode).await
    }

    /// Complete a work packet
    pub async fn complete_work(&self, work_packet_id: &str, success: bool) -> Result<(), A2AError> {
        self.controller.complete_work(work_packet_id, success).await
    }

    /// Get current WIP count
    pub async fn get_wip_count(&self) -> Result<usize, A2AError> {
        self.controller.get_wip_count().await
    }

    /// Get maximum WIP limit
    pub async fn get_wip_limit(&self) -> Result<usize, A2AError> {
        self.controller.get_wip_limit().await
    }

    /// Set WIP limit
    pub async fn set_wip_limit(&self, limit: usize) -> Result<(), A2AError> {
        self.controller.set_wip_limit(limit).await
    }

    /// Check if a channel is currently allowed
    pub async fn is_channel_allowed(&self, channel: IngressChannel) -> Result<bool, A2AError> {
        self.controller.is_channel_allowed(channel).await
    }

    /// Get metrics snapshot
    pub async fn get_metrics(&self) -> Result<FirewallMetrics, A2AError> {
        let health = self.get_system_health().await?;
        let queue_depth = self.admission_tx.max_capacity() - self.admission_tx.capacity();

        Ok(FirewallMetrics {
            current_wip: health.current_wip,
            max_wip: health.max_wip,
            jidoka_mode: health.jidoka_mode,
            quality_score: health.quality_score,
            queue_depth,
        })
    }
}

/// Firewall metrics snapshot
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirewallMetrics {
    /// Current work-in-progress count
    pub current_wip: usize,
    /// Maximum WIP limit
    pub max_wip: usize,
    /// Current Jidoka mode
    pub jidoka_mode: JidokaMode,
    /// Overall quality score
    pub quality_score: f64,
    /// Current admission queue depth
    pub queue_depth: usize,
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    use crate::adapter::business::DefaultAdmissionController;
    use crate::domain::WorkConstraints;

    fn create_test_work_packet(id: &str) -> WorkPacket {
        WorkPacket {
            id: id.to_string(),
            objective: "Test objective".to_string(),
            constraints: WorkConstraints {
                max_execution_time_secs: 60,
                max_memory_bytes: None,
                deadline: None,
            },
            acceptance_test: "Test acceptance criteria".to_string(),
            reversibility: true,
            channel: IngressChannel::Batch,
            supplier_id: Some("test-supplier".to_string()),
            priority: None,
        }
    }

    #[tokio::test]
    async fn test_firewall_service_admission() {
        let controller = DefaultAdmissionController::new();
        let service = FirewallService::new(controller, FirewallConfig::default());

        let packet = create_test_work_packet("work-1");
        let decision = service.request_admission(packet).await.unwrap();

        assert!(matches!(decision, AdmissionDecision::Admitted { .. }));
    }

    #[tokio::test]
    async fn test_firewall_service_health() {
        let controller = DefaultAdmissionController::new();
        let service = FirewallService::new(controller, FirewallConfig::default());

        let health = service.get_system_health().await.unwrap();
        assert_eq!(health.jidoka_mode, JidokaMode::Green);
        assert_eq!(health.current_wip, 0);
    }

    #[tokio::test]
    async fn test_firewall_service_metrics() {
        let controller = DefaultAdmissionController::new();
        let service = FirewallService::new(controller, FirewallConfig::default());

        let metrics = service.get_metrics().await.unwrap();
        assert_eq!(metrics.current_wip, 0);
        assert_eq!(metrics.jidoka_mode, JidokaMode::Green);
    }

    #[tokio::test]
    async fn test_firewall_service_jidoka_mode() {
        let controller = DefaultAdmissionController::new();
        let service = FirewallService::new(controller, FirewallConfig::default());

        service.set_jidoka_mode(JidokaMode::Yellow).await.unwrap();

        let health = service.get_system_health().await.unwrap();
        assert_eq!(health.jidoka_mode, JidokaMode::Yellow);

        // Check channel allowed
        let batch_allowed = service
            .is_channel_allowed(IngressChannel::Batch)
            .await
            .unwrap();
        assert!(!batch_allowed);

        let emergency_allowed = service
            .is_channel_allowed(IngressChannel::Emergency)
            .await
            .unwrap();
        assert!(emergency_allowed);
    }

    #[tokio::test]
    async fn test_firewall_service_complete_work() {
        let controller = DefaultAdmissionController::new();
        let service = FirewallService::new(controller, FirewallConfig::default());

        // Admit work
        let packet = create_test_work_packet("work-1");
        service.request_admission(packet).await.unwrap();

        let wip_before = service.get_wip_count().await.unwrap();
        assert_eq!(wip_before, 1);

        // Complete work
        service.complete_work("work-1", true).await.unwrap();

        let wip_after = service.get_wip_count().await.unwrap();
        assert_eq!(wip_after, 0);

        // Check supplier quality updated
        let quality = service.get_supplier_quality("test-supplier").await.unwrap();
        assert_eq!(quality.successful, 1);
    }
}
