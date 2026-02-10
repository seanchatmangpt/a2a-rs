//! Port trait for approving account resources via Partner Procurement API.

use crate::domain::{Account, ApproveAccountRequest, Entitlement};
use async_trait::async_trait;
use thiserror::Error;

/// Errors that can occur when approving accounts
#[derive(Debug, Error)]
pub enum AccountApproverError {
    /// Failed to authenticate with the API
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    /// Account not found
    #[error("Account not found: {0}")]
    NotFound(String),

    /// Account is in invalid state for approval
    #[error("Invalid account state: {0}")]
    InvalidState(String),

    /// API request failed
    #[error("API request failed: {0}")]
    RequestError(String),

    /// Failed to parse API response
    #[error("Failed to parse response: {0}")]
    ParseError(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimitError(String),

    /// Other error
    #[error("Account approver error: {0}")]
    Other(String),
}

/// Result type for account approver operations
pub type AccountApproverResult<T> = Result<T, AccountApproverError>;

/// Port trait for approving account resources via the Partner Procurement API.
///
/// Implementations should:
/// - Authenticate with Google Cloud using service account credentials
/// - Make HTTPS requests to the Cloud Commerce Partner Procurement API
/// - Handle rate limiting and retries
/// - Parse API responses into domain types
#[async_trait]
pub trait AccountApprover: Send + Sync {
    /// Get an entitlement by its resource name.
    ///
    /// # Arguments
    ///
    /// * `name` - The entitlement resource name (e.g., "providers/{provider}/entitlements/{id}")
    ///
    /// # Returns
    ///
    /// The entitlement resource
    async fn get_entitlement(&self, name: &str) -> AccountApproverResult<Entitlement>;

    /// Get an account by its resource name.
    ///
    /// # Arguments
    ///
    /// * `name` - The account resource name (e.g., "providers/{provider}/accounts/{id}")
    ///
    /// # Returns
    ///
    /// The account resource
    async fn get_account(&self, name: &str) -> AccountApproverResult<Account>;

    /// Approve an account resource.
    ///
    /// This moves the account from ACCOUNT_STATE_PENDING to ACCOUNT_STATE_APPROVED,
    /// which signals to Google Cloud Marketplace that the account is ready for use.
    ///
    /// # Arguments
    ///
    /// * `account_name` - The account resource name to approve
    /// * `request` - The approval request with optional properties
    ///
    /// # Returns
    ///
    /// The updated account resource
    async fn approve_account(
        &self,
        account_name: &str,
        request: &ApproveAccountRequest,
    ) -> AccountApproverResult<Account>;

    /// Reject an account resource.
    ///
    /// This moves the account to ACCOUNT_STATE_REJECTED and cancels the entitlement.
    ///
    /// # Arguments
    ///
    /// * `account_name` - The account resource name to reject
    /// * `reason` - The reason for rejection
    ///
    /// # Returns
    ///
    /// The updated account resource
    async fn reject_account(
        &self,
        account_name: &str,
        reason: &str,
    ) -> AccountApproverResult<Account>;
}
