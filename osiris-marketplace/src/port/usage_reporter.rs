//! Port trait for reporting usage to Google Cloud Service Control API.

use crate::domain::{OperationUsage, UsageReport};
use async_trait::async_trait;
use thiserror::Error;

/// Errors that can occur when reporting usage
#[derive(Debug, Error)]
pub enum UsageReporterError {
    /// Failed to authenticate with the API
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    /// Operation not found
    #[error("Operation not found: {0}")]
    NotFound(String),

    /// Invalid operation usage format
    #[error("Invalid operation usage: {0}")]
    InvalidUsage(String),

    /// API request failed
    #[error("API request failed: {0}")]
    RequestError(String),

    /// Failed to parse API response
    #[error("Failed to parse response: {0}")]
    ParseError(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimitError(String),

    /// Service unavailable
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Other error
    #[error("Usage reporter error: {0}")]
    Other(String),
}

/// Result type for usage reporter operations
pub type UsageReporterResult<T> = Result<T, UsageReporterError>;

/// Port trait for reporting operation usage to Google Cloud Service Control API.
///
/// Implementations should:
/// - Authenticate with Google Cloud using service account credentials
/// - Make HTTPS requests to the Service Control API (services.report endpoint)
/// - Handle rate limiting and retries
/// - Convert domain OperationUsage to API format
/// - Parse API responses
#[async_trait]
pub trait UsageReporter: Send + Sync {
    /// Report a single operation's usage to Service Control.
    ///
    /// # Arguments
    ///
    /// * `usage` - The operation usage to report
    ///
    /// # Returns
    ///
    /// A UsageReport confirming the submission
    async fn report_operation(&self, usage: &OperationUsage) -> UsageReporterResult<UsageReport>;

    /// Report multiple operations' usage in a batch.
    ///
    /// This is more efficient than calling report_operation multiple times,
    /// as it can batch multiple operations in a single API call.
    ///
    /// # Arguments
    ///
    /// * `usages` - A slice of operations to report
    ///
    /// # Returns
    ///
    /// A UsageReport confirming the batch submission
    async fn report_batch(&self, usages: &[OperationUsage]) -> UsageReporterResult<UsageReport>;

    /// Check if a service account is properly configured
    ///
    /// # Returns
    ///
    /// Ok if authentication is valid, Err otherwise
    async fn verify_credentials(&self) -> UsageReporterResult<()>;

    /// Get the service name that this reporter uses
    fn get_service_name(&self) -> &str;

    /// Get the project ID for this reporter
    fn get_project_id(&self) -> &str;
}
