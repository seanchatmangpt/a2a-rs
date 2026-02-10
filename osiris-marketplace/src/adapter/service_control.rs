//! Google Cloud Service Control adapter for usage reporting.

#[cfg(feature = "service-control")]
use crate::domain::{OperationUsage, UsageReport};
#[cfg(feature = "service-control")]
use crate::port::{UsageReporter, UsageReporterError, UsageReporterResult};
#[cfg(feature = "service-control")]
use async_trait::async_trait;
#[cfg(feature = "service-control")]
use chrono::Utc;
#[cfg(feature = "service-control")]
use google_servicecontrol1::client::Client as ServiceControlClient;
#[cfg(feature = "service-control")]
use google_servicecontrol1::oauth2::ServiceAccountAuthenticator;
#[cfg(feature = "service-control")]
use std::path::Path;
#[cfg(feature = "service-control")]
use tracing::{debug, info, warn};

/// Google Cloud Service Control API client for reporting usage
#[cfg(feature = "service-control")]
pub struct ServiceControlReporter {
    client: ServiceControlClient,
    service_name: String,
    project_id: String,
}

#[cfg(feature = "service-control")]
impl ServiceControlReporter {
    /// Create a new Service Control reporter
    ///
    /// # Arguments
    ///
    /// * `service_name` - The service name for usage reporting (e.g., "my-marketplace-service.prod.googleapis.com")
    /// * `project_id` - Google Cloud project ID
    /// * `credentials_path` - Path to service account JSON credentials file
    ///
    /// # Returns
    ///
    /// A new ServiceControlReporter instance
    pub async fn new<P: AsRef<Path>>(
        service_name: String,
        project_id: String,
        credentials_path: P,
    ) -> UsageReporterResult<Self> {
        // Load service account credentials
        let secret = yup_oauth2::read_service_account_key(credentials_path)
            .await
            .map_err(|e| UsageReporterError::AuthenticationError(e.to_string()))?;

        // Create authenticator
        let auth = ServiceAccountAuthenticator::builder(secret)
            .build()
            .await
            .map_err(|e| UsageReporterError::AuthenticationError(e.to_string()))?;

        // Create Service Control client
        let client = ServiceControlClient::new(auth);

        Ok(Self {
            client,
            service_name,
            project_id,
        })
    }

    /// Create a new Service Control reporter with default credentials
    ///
    /// This uses Application Default Credentials (ADC) from the environment.
    /// Set GOOGLE_APPLICATION_CREDENTIALS environment variable to the service account key path.
    ///
    /// # Arguments
    ///
    /// * `service_name` - The service name for usage reporting
    /// * `project_id` - Google Cloud project ID
    ///
    /// # Returns
    ///
    /// A new ServiceControlReporter instance
    pub async fn with_default_credentials(
        service_name: String,
        project_id: String,
    ) -> UsageReporterResult<Self> {
        // Try to load from GOOGLE_APPLICATION_CREDENTIALS
        let creds_path = std::env::var("GOOGLE_APPLICATION_CREDENTIALS").map_err(|_| {
            UsageReporterError::AuthenticationError(
                "GOOGLE_APPLICATION_CREDENTIALS not set".to_string(),
            )
        })?;

        Self::new(service_name, project_id, creds_path).await
    }

    /// Convert OperationUsage to Service Control API format
    #[cfg(feature = "service-control")]
    fn build_operation(&self, usage: &OperationUsage) -> google_servicecontrol1::api::Operation {
        let mut operation = google_servicecontrol1::api::Operation::default();
        operation.name = Some(usage.operation_id.clone());
        operation.operation_metadata = Some(serde_json::json!({
            "entitlement": usage.entitlement,
            "account": usage.account,
            "operationType": usage.operation_type.to_string(),
            "userId": usage.user_id,
        }));

        // Add labels as metadata
        if !usage.labels.is_empty() {
            operation.labels = Some(usage.labels.clone());
        }

        // Add metrics as custom attributes (stored in metadata)
        let mut metrics_data = Vec::new();
        for metric in &usage.metrics {
            metrics_data.push(serde_json::json!({
                "type": metric.metric_type.to_string(),
                "value": metric.value,
            }));
        }
        if !metrics_data.is_empty() {
            operation.operation_metadata = Some(serde_json::json!({
                "entitlement": usage.entitlement,
                "account": usage.account,
                "operationType": usage.operation_type.to_string(),
                "userId": usage.user_id,
                "metrics": metrics_data,
            }));
        }

        operation
    }
}

#[cfg(feature = "service-control")]
#[async_trait]
impl UsageReporter for ServiceControlReporter {
    async fn report_operation(&self, usage: &OperationUsage) -> UsageReporterResult<UsageReport> {
        info!(
            "Reporting operation usage: {} for service: {}",
            usage.operation_id, self.service_name
        );

        // Build the operation object
        let operation = self.build_operation(usage);

        // Prepare the report request
        let mut report_request = google_servicecontrol1::api::ReportRequest::default();
        report_request.operations = Some(vec![operation]);

        // Execute the report
        debug!(
            "Submitting usage report to Service Control for service: {}",
            self.service_name
        );

        let result = self
            .client
            .services()
            .report(&self.service_name, report_request)
            .doit()
            .await
            .map_err(|e| {
                warn!("Service Control report failed: {:?}", e);
                UsageReporterError::RequestError(format!("Service Control API error: {:?}", e))
            })?;

        info!(
            "Successfully reported operation {} to Service Control",
            usage.operation_id
        );

        // Build response
        let report = UsageReport {
            service_name: self.service_name.clone(),
            operation_ids: vec![usage.operation_id.clone()],
            report_timestamp: Utc::now(),
            success: true,
            error_message: None,
        };

        debug!("Usage report response: {:?}", result);

        Ok(report)
    }

    async fn report_batch(&self, usages: &[OperationUsage]) -> UsageReporterResult<UsageReport> {
        if usages.is_empty() {
            return Err(UsageReporterError::InvalidUsage(
                "Cannot report empty batch".to_string(),
            ));
        }

        info!(
            "Reporting batch of {} operations for service: {}",
            usages.len(),
            self.service_name
        );

        // Build operations for all usages
        let operations: Vec<google_servicecontrol1::api::Operation> =
            usages.iter().map(|u| self.build_operation(u)).collect();

        // Prepare the batch report request
        let mut report_request = google_servicecontrol1::api::ReportRequest::default();
        report_request.operations = Some(operations);

        // Execute the batch report
        debug!(
            "Submitting batch usage report ({} operations) to Service Control",
            usages.len()
        );

        let result = self
            .client
            .services()
            .report(&self.service_name, report_request)
            .doit()
            .await
            .map_err(|e| {
                warn!("Service Control batch report failed: {:?}", e);
                UsageReporterError::RequestError(format!(
                    "Service Control batch report error: {:?}",
                    e
                ))
            })?;

        let operation_ids: Vec<String> = usages.iter().map(|u| u.operation_id.clone()).collect();

        info!(
            "Successfully reported batch of {} operations to Service Control",
            usages.len()
        );

        // Build response
        let report = UsageReport {
            service_name: self.service_name.clone(),
            operation_ids,
            report_timestamp: Utc::now(),
            success: true,
            error_message: None,
        };

        debug!("Batch usage report response: {:?}", result);

        Ok(report)
    }

    async fn verify_credentials(&self) -> UsageReporterResult<()> {
        debug!(
            "Verifying Service Control credentials for service: {}",
            self.service_name
        );

        // Attempt a minimal report to verify authentication
        let test_operation = google_servicecontrol1::api::Operation {
            name: Some("verify-credentials-test".to_string()),
            ..Default::default()
        };

        let mut report_request = google_servicecontrol1::api::ReportRequest::default();
        report_request.operations = Some(vec![test_operation]);

        self.client
            .services()
            .report(&self.service_name, report_request)
            .doit()
            .await
            .map_err(|e| {
                warn!("Service Control credential verification failed: {:?}", e);
                UsageReporterError::AuthenticationError(format!(
                    "Failed to verify Service Control credentials: {:?}",
                    e
                ))
            })?;

        info!("Service Control credentials verified successfully");
        Ok(())
    }

    fn get_service_name(&self) -> &str {
        &self.service_name
    }

    fn get_project_id(&self) -> &str {
        &self.project_id
    }
}

#[cfg(all(test, feature = "service-control"))]
mod tests {
    use super::*;
    use crate::domain::{MetricType, OperationType, UsageMetric};

    #[test]
    fn test_operation_usage_to_service_control_format() {
        // This test would require a mock Service Control client
        // For now, we just verify the structure can be created

        let usage = OperationUsage::new(
            "op-12345".to_string(),
            OperationType::ProvisionEntitlement,
            "providers/test/entitlements/123".to_string(),
            "providers/test/accounts/456".to_string(),
            "test-service.googleapis.com".to_string(),
        )
        .add_metric(UsageMetric::new(MetricType::ActiveUsers, 50))
        .with_label("region".to_string(), "us-central1".to_string())
        .with_user_id("user-789".to_string());

        assert_eq!(usage.operation_id, "op-12345");
        assert_eq!(usage.metrics.len(), 1);
        assert_eq!(usage.labels.len(), 1);
        assert_eq!(usage.user_id, Some("user-789".to_string()));
    }
}
