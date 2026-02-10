//! Google Cloud Commerce Partner Procurement API adapter for approving accounts.

#[cfg(feature = "procurement-api")]
use crate::domain::{Account, AccountState, ApproveAccountRequest, Entitlement};
#[cfg(feature = "procurement-api")]
use crate::port::{AccountApprover, AccountApproverError, AccountApproverResult};
#[cfg(feature = "procurement-api")]
use async_trait::async_trait;
#[cfg(feature = "procurement-api")]
use reqwest::{Client, StatusCode, header};
#[cfg(feature = "procurement-api")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "procurement-api")]
use tracing::{debug, info, warn};

/// Base URL for the Cloud Commerce Partner Procurement API
#[cfg(feature = "procurement-api")]
const API_BASE_URL: &str = "https://cloudcommerceprocurement.googleapis.com/v1";

/// OAuth2 token response
#[cfg(feature = "procurement-api")]
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    token_type: String,
}

/// Google Cloud Commerce Partner Procurement API client
#[cfg(feature = "procurement-api")]
pub struct ProcurementApiClient {
    client: Client,
    access_token: String,
    _project_id: String,
}

#[cfg(feature = "procurement-api")]
impl ProcurementApiClient {
    /// Create a new Procurement API client
    ///
    /// # Arguments
    ///
    /// * `project_id` - Google Cloud project ID
    /// * `access_token` - OAuth2 access token with cloudcommerceprocurement scope
    ///
    /// # Returns
    ///
    /// A new ProcurementApiClient instance
    pub fn new(project_id: String, access_token: String) -> AccountApproverResult<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AccountApproverError::Other(e.to_string()))?;

        Ok(Self {
            client,
            access_token,
            _project_id: project_id,
        })
    }

    /// Create a new client using Application Default Credentials
    ///
    /// This reads credentials from GOOGLE_APPLICATION_CREDENTIALS environment variable
    /// or the default service account when running on Google Cloud.
    ///
    /// # Arguments
    ///
    /// * `project_id` - Google Cloud project ID
    ///
    /// # Returns
    ///
    /// A new ProcurementApiClient instance with automatically obtained credentials
    pub async fn with_default_credentials(_project_id: String) -> AccountApproverResult<Self> {
        // In production, this would use google-auth crate or similar
        // For now, we'll require explicit token
        Err(AccountApproverError::AuthenticationError(
            "Default credentials not yet implemented. Use new() with explicit token.".to_string(),
        ))
    }

    /// Build authorization headers
    fn auth_headers(&self) -> header::HeaderMap {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Bearer {}", self.access_token))
                .expect("Invalid token"),
        );
        headers
    }

    /// Make a GET request to the API
    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> AccountApproverResult<T> {
        let url = format!("{}{}", API_BASE_URL, path);
        debug!("GET {}", url);

        let response = self
            .client
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await
            .map_err(|e| AccountApproverError::RequestError(e.to_string()))?;

        match response.status() {
            StatusCode::OK => {
                let body = response
                    .json::<T>()
                    .await
                    .map_err(|e| AccountApproverError::ParseError(e.to_string()))?;
                Ok(body)
            }
            StatusCode::NOT_FOUND => Err(AccountApproverError::NotFound(path.to_string())),
            StatusCode::TOO_MANY_REQUESTS => Err(AccountApproverError::RateLimitError(
                "API rate limit exceeded".to_string(),
            )),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(AccountApproverError::RequestError(format!(
                    "HTTP {}: {}",
                    status, error_text
                )))
            }
        }
    }

    /// Make a POST request to the API
    async fn post<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> AccountApproverResult<T> {
        let url = format!("{}{}", API_BASE_URL, path);
        debug!("POST {}", url);

        let response = self
            .client
            .post(&url)
            .headers(self.auth_headers())
            .json(body)
            .send()
            .await
            .map_err(|e| AccountApproverError::RequestError(e.to_string()))?;

        match response.status() {
            StatusCode::OK => {
                let body = response
                    .json::<T>()
                    .await
                    .map_err(|e| AccountApproverError::ParseError(e.to_string()))?;
                Ok(body)
            }
            StatusCode::NOT_FOUND => Err(AccountApproverError::NotFound(path.to_string())),
            StatusCode::BAD_REQUEST => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Bad request".to_string());
                Err(AccountApproverError::InvalidState(error_text))
            }
            StatusCode::TOO_MANY_REQUESTS => Err(AccountApproverError::RateLimitError(
                "API rate limit exceeded".to_string(),
            )),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(AccountApproverError::RequestError(format!(
                    "HTTP {}: {}",
                    status, error_text
                )))
            }
        }
    }
}

#[cfg(feature = "procurement-api")]
#[async_trait]
impl AccountApprover for ProcurementApiClient {
    async fn get_entitlement(&self, name: &str) -> AccountApproverResult<Entitlement> {
        info!("Fetching entitlement: {}", name);
        self.get(&format!("/{}", name)).await
    }

    async fn get_account(&self, name: &str) -> AccountApproverResult<Account> {
        info!("Fetching account: {}", name);
        self.get(&format!("/{}", name)).await
    }

    async fn approve_account(
        &self,
        account_name: &str,
        request: &ApproveAccountRequest,
    ) -> AccountApproverResult<Account> {
        info!("Approving account: {}", account_name);

        // First check if the account is in pending state
        let account = self.get_account(account_name).await?;
        if account.state != AccountState::AccountStatePending {
            warn!(
                "Account {} is not in pending state: {:?}",
                account_name, account.state
            );
            return Err(AccountApproverError::InvalidState(format!(
                "Account is in state {:?}, expected ACCOUNT_STATE_PENDING",
                account.state
            )));
        }

        // Approve the account
        let path = format!("/{}/{}:approve", account_name, request.approval_name);
        let response: Account = self.post(&path, request).await?;

        info!(
            "Successfully approved account: {} -> {:?}",
            account_name, response.state
        );
        Ok(response)
    }

    async fn reject_account(
        &self,
        account_name: &str,
        reason: &str,
    ) -> AccountApproverResult<Account> {
        info!("Rejecting account: {} - Reason: {}", account_name, reason);

        #[derive(Serialize)]
        struct RejectRequest {
            reason: String,
        }

        let request = RejectRequest {
            reason: reason.to_string(),
        };

        let path = format!("/{}/rejected:reject", account_name);
        let response: Account = self.post(&path, &request).await?;

        info!(
            "Successfully rejected account: {} -> {:?}",
            account_name, response.state
        );
        Ok(response)
    }
}

#[cfg(all(test, feature = "procurement-api"))]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client =
            ProcurementApiClient::new("test-project".to_string(), "test-token".to_string());
        assert!(client.is_ok());
    }

    #[test]
    fn test_auth_headers() {
        let client =
            ProcurementApiClient::new("test-project".to_string(), "test-token".to_string())
                .unwrap();

        let headers = client.auth_headers();
        assert!(headers.contains_key(header::AUTHORIZATION));
        assert_eq!(
            headers.get(header::AUTHORIZATION).unwrap(),
            "Bearer test-token"
        );
    }
}
