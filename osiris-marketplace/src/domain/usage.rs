//! Domain types for Cloud Marketplace usage reporting.
//!
//! These types represent operation usage metrics for billing via Google Cloud
//! Service Control API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Operation type for usage tracking
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationType {
    /// Entitlement provisioning operation
    ProvisionEntitlement,
    /// Entitlement modification operation
    ModifyEntitlement,
    /// Entitlement cancellation operation
    CancelEntitlement,
    /// Custom operation type
    Custom(String),
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationType::ProvisionEntitlement => write!(f, "ProvisionEntitlement"),
            OperationType::ModifyEntitlement => write!(f, "ModifyEntitlement"),
            OperationType::CancelEntitlement => write!(f, "CancelEntitlement"),
            OperationType::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// Metric type for usage tracking
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetricType {
    /// Monthly active users
    ActiveUsers,
    /// API calls
    ApiCalls,
    /// Data processed (in GB)
    DataProcessedGb,
    /// Support incidents
    SupportIncidents,
    /// Custom metric
    Custom(String),
}

impl std::fmt::Display for MetricType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetricType::ActiveUsers => write!(f, "activeUsers"),
            MetricType::ApiCalls => write!(f, "apiCalls"),
            MetricType::DataProcessedGb => write!(f, "dataProcessedGb"),
            MetricType::SupportIncidents => write!(f, "supportIncidents"),
            MetricType::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// A single usage metric for an operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetric {
    /// Type of metric being reported
    pub metric_type: MetricType,
    /// Metric value
    pub value: i64,
}

impl UsageMetric {
    /// Create a new usage metric
    pub fn new(metric_type: MetricType, value: i64) -> Self {
        Self { metric_type, value }
    }
}

/// Operation usage information for Cloud Marketplace billing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationUsage {
    /// Unique operation identifier
    pub operation_id: String,
    /// Type of operation
    pub operation_type: OperationType,
    /// Associated entitlement resource name
    pub entitlement: String,
    /// Associated account resource name
    pub account: String,
    /// Timestamp of the operation
    pub operation_timestamp: DateTime<Utc>,
    /// Service name for usage tracking (e.g., "servicemanagement.googleapis.com")
    pub service_name: String,
    /// Usage metrics for this operation
    pub metrics: Vec<UsageMetric>,
    /// Custom labels for filtering/grouping
    #[serde(default)]
    pub labels: std::collections::HashMap<String, String>,
    /// Optional user ID from input properties
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

impl OperationUsage {
    /// Create a new operation usage entry
    pub fn new(
        operation_id: String,
        operation_type: OperationType,
        entitlement: String,
        account: String,
        service_name: String,
    ) -> Self {
        Self {
            operation_id,
            operation_type,
            entitlement,
            account,
            operation_timestamp: Utc::now(),
            service_name,
            metrics: Vec::new(),
            labels: std::collections::HashMap::new(),
            user_id: None,
        }
    }

    /// Add a metric to the operation
    pub fn add_metric(mut self, metric: UsageMetric) -> Self {
        self.metrics.push(metric);
        self
    }

    /// Add a label
    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }

    /// Set the user ID
    pub fn with_user_id(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }
}

/// Report from usage reporting operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageReport {
    /// Service name that performed the operation
    pub service_name: String,
    /// Operation IDs that were reported
    pub operation_ids: Vec<String>,
    /// Timestamp when the report was submitted
    pub report_timestamp: DateTime<Utc>,
    /// Whether the report was successfully processed
    pub success: bool,
    /// Optional error message if report failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_usage_creation() {
        let usage = OperationUsage::new(
            "op-123".to_string(),
            OperationType::ProvisionEntitlement,
            "providers/test/entitlements/123".to_string(),
            "providers/test/accounts/456".to_string(),
            "servicemanagement.googleapis.com".to_string(),
        );

        assert_eq!(usage.operation_id, "op-123");
        assert_eq!(usage.operation_type, OperationType::ProvisionEntitlement);
        assert_eq!(usage.metrics.len(), 0);
        assert_eq!(usage.labels.len(), 0);
        assert!(usage.user_id.is_none());
    }

    #[test]
    fn test_operation_usage_with_metrics() {
        let usage = OperationUsage::new(
            "op-456".to_string(),
            OperationType::ModifyEntitlement,
            "providers/test/entitlements/123".to_string(),
            "providers/test/accounts/456".to_string(),
            "servicemanagement.googleapis.com".to_string(),
        )
        .add_metric(UsageMetric::new(MetricType::ActiveUsers, 100))
        .add_metric(UsageMetric::new(MetricType::ApiCalls, 5000));

        assert_eq!(usage.metrics.len(), 2);
        assert_eq!(usage.metrics[0].value, 100);
        assert_eq!(usage.metrics[1].value, 5000);
    }

    #[test]
    fn test_operation_usage_with_labels() {
        let usage = OperationUsage::new(
            "op-789".to_string(),
            OperationType::CancelEntitlement,
            "providers/test/entitlements/123".to_string(),
            "providers/test/accounts/456".to_string(),
            "servicemanagement.googleapis.com".to_string(),
        )
        .with_label("region".to_string(), "us-east1".to_string())
        .with_label("tier".to_string(), "premium".to_string());

        assert_eq!(usage.labels.len(), 2);
        assert_eq!(usage.labels.get("region"), Some(&"us-east1".to_string()));
        assert_eq!(usage.labels.get("tier"), Some(&"premium".to_string()));
    }

    #[test]
    fn test_metric_type_display() {
        assert_eq!(MetricType::ActiveUsers.to_string(), "activeUsers");
        assert_eq!(MetricType::ApiCalls.to_string(), "apiCalls");
        assert_eq!(MetricType::DataProcessedGb.to_string(), "dataProcessedGb");
        assert_eq!(MetricType::SupportIncidents.to_string(), "supportIncidents");
    }

    #[test]
    fn test_operation_type_display() {
        assert_eq!(
            OperationType::ProvisionEntitlement.to_string(),
            "ProvisionEntitlement"
        );
        assert_eq!(
            OperationType::ModifyEntitlement.to_string(),
            "ModifyEntitlement"
        );
        assert_eq!(
            OperationType::CancelEntitlement.to_string(),
            "CancelEntitlement"
        );
    }
}
