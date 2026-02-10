//! Application handler for tracking and reporting operation usage

use crate::domain::{
    EntitlementEvent, MetricType, OperationType, OperationUsage, UsageMetric, UsageReport,
};
use crate::port::UsageReporter;
use std::sync::Arc;
use tracing::{error, info};

/// Handles tracking and reporting of operation usage to Service Control
pub struct UsageTrackingHandler<U: UsageReporter> {
    usage_reporter: Arc<U>,
    service_name: String,
}

impl<U: UsageReporter> UsageTrackingHandler<U> {
    /// Create a new usage tracking handler
    ///
    /// # Arguments
    ///
    /// * `usage_reporter` - UsageReporter implementation (e.g., Service Control)
    /// * `service_name` - Service name for usage reporting
    pub fn new(usage_reporter: Arc<U>, service_name: String) -> Self {
        Self {
            usage_reporter,
            service_name,
        }
    }

    /// Track and report usage for an entitlement event
    ///
    /// # Arguments
    ///
    /// * `event` - The entitlement event that triggered usage
    /// * `account_name` - The account resource name
    /// * `operation_type` - Type of operation being tracked
    pub async fn track_entitlement_usage(
        &self,
        event: &EntitlementEvent,
        account_name: &str,
        operation_type: OperationType,
    ) -> Result<UsageReport, Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Tracking usage for entitlement event: {:?}, operation: {:?}",
            event.event_type, operation_type
        );

        // Create operation usage tracking
        let operation_id = format!("{}_{}", event.entitlement, uuid::Uuid::new_v4());
        let mut usage = OperationUsage::new(
            operation_id,
            operation_type,
            event.entitlement.clone(),
            account_name.to_string(),
            self.service_name.clone(),
        );

        // Add metrics based on event type
        usage = match event.event_type {
            crate::domain::EntitlementEventType::EntitlementOfferAccepted => {
                // New entitlement provisioning
                usage.add_metric(UsageMetric::new(MetricType::ActiveUsers, 1))
            }
            crate::domain::EntitlementEventType::EntitlementActive => {
                // Entitlement is now active
                usage.add_metric(UsageMetric::new(MetricType::ActiveUsers, 1))
            }
            crate::domain::EntitlementEventType::EntitlementCancelled => {
                // Entitlement cancellation
                usage.add_metric(UsageMetric::new(MetricType::ActiveUsers, 0))
            }
            crate::domain::EntitlementEventType::EntitlementPlanChanged => {
                // Plan modification - track as API call
                usage.add_metric(UsageMetric::new(MetricType::ApiCalls, 1))
            }
            crate::domain::EntitlementEventType::EntitlementDeleted => {
                // Entitlement deletion
                usage.add_metric(UsageMetric::new(MetricType::ActiveUsers, 0))
            }
            crate::domain::EntitlementEventType::Unknown => {
                // Unknown event - just track as API call
                usage.add_metric(UsageMetric::new(MetricType::ApiCalls, 1))
            }
        };

        // Report the usage
        self.usage_reporter
            .report_operation(&usage)
            .await
            .map_err(|e| {
                error!("Failed to report usage: {:?}", e);
                Box::new(e) as Box<dyn std::error::Error + Send + Sync>
            })
    }

    /// Track and report usage for multiple operations
    ///
    /// # Arguments
    ///
    /// * `usages` - A slice of operations to report
    pub async fn track_batch_usage(
        &self,
        usages: &[OperationUsage],
    ) -> Result<UsageReport, Box<dyn std::error::Error + Send + Sync>> {
        info!("Tracking batch usage for {} operations", usages.len());

        self.usage_reporter.report_batch(usages).await.map_err(|e| {
            error!("Failed to report batch usage: {:?}", e);
            Box::new(e) as Box<dyn std::error::Error + Send + Sync>
        })
    }

    /// Verify usage reporter credentials
    pub async fn verify_reporter_credentials(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.usage_reporter.verify_credentials().await.map_err(|e| {
            error!("Failed to verify usage reporter credentials: {:?}", e);
            Box::new(e) as Box<dyn std::error::Error + Send + Sync>
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::EntitlementEventType;
    use crate::port::UsageReporterResult;
    use async_trait::async_trait;
    use chrono::Utc;

    struct MockUsageReporter {
        service_name: String,
    }

    #[async_trait]
    impl UsageReporter for MockUsageReporter {
        async fn report_operation(
            &self,
            usage: &OperationUsage,
        ) -> UsageReporterResult<UsageReport> {
            Ok(UsageReport {
                service_name: self.service_name.clone(),
                operation_ids: vec![usage.operation_id.clone()],
                report_timestamp: Utc::now(),
                success: true,
                error_message: None,
            })
        }

        async fn report_batch(
            &self,
            usages: &[OperationUsage],
        ) -> UsageReporterResult<UsageReport> {
            Ok(UsageReport {
                service_name: self.service_name.clone(),
                operation_ids: usages.iter().map(|u| u.operation_id.clone()).collect(),
                report_timestamp: Utc::now(),
                success: true,
                error_message: None,
            })
        }

        async fn verify_credentials(&self) -> UsageReporterResult<()> {
            Ok(())
        }

        fn get_service_name(&self) -> &str {
            &self.service_name
        }

        fn get_project_id(&self) -> &str {
            "test-project"
        }
    }

    #[tokio::test]
    async fn test_track_entitlement_usage() {
        let reporter = Arc::new(MockUsageReporter {
            service_name: "test-service.googleapis.com".to_string(),
        });
        let handler =
            UsageTrackingHandler::new(reporter, "test-service.googleapis.com".to_string());

        let event = EntitlementEvent {
            event_type: EntitlementEventType::EntitlementOfferAccepted,
            entitlement: "providers/test/entitlements/123".to_string(),
            event_timestamp: Utc::now(),
        };

        let result = handler
            .track_entitlement_usage(
                &event,
                "providers/test/accounts/456",
                OperationType::ProvisionEntitlement,
            )
            .await;

        assert!(result.is_ok());
        let report = result.unwrap();
        assert_eq!(report.service_name, "test-service.googleapis.com");
        assert_eq!(report.operation_ids.len(), 1);
        assert!(report.success);
    }

    #[tokio::test]
    async fn test_track_batch_usage() {
        let reporter = Arc::new(MockUsageReporter {
            service_name: "test-service.googleapis.com".to_string(),
        });
        let handler =
            UsageTrackingHandler::new(reporter, "test-service.googleapis.com".to_string());

        let usages = vec![
            OperationUsage::new(
                "op-1".to_string(),
                OperationType::ProvisionEntitlement,
                "providers/test/entitlements/1".to_string(),
                "providers/test/accounts/1".to_string(),
                "test-service.googleapis.com".to_string(),
            ),
            OperationUsage::new(
                "op-2".to_string(),
                OperationType::ModifyEntitlement,
                "providers/test/entitlements/2".to_string(),
                "providers/test/accounts/2".to_string(),
                "test-service.googleapis.com".to_string(),
            ),
        ];

        let result = handler.track_batch_usage(&usages).await;

        assert!(result.is_ok());
        let report = result.unwrap();
        assert_eq!(report.operation_ids.len(), 2);
        assert!(report.success);
    }
}
