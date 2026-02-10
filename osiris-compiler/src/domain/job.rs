//! Domain types for Cloud Tasks queue job management.
//!
//! Defines the core types for enqueuing, dequeuing, and retrying jobs
//! with OIDC token support.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A job to be executed by the Cloud Tasks queue.
///
/// Represents a unit of work that can be enqueued, executed, and retried
/// with full OIDC authentication support for secure HTTP delivery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    /// Unique identifier for this job
    pub id: Uuid,

    /// The HTTP method to use for job execution (GET, POST, PUT, DELETE, etc.)
    pub http_method: HttpMethod,

    /// The target URL where the job should be delivered
    pub target_url: String,

    /// Optional request headers for the HTTP call
    pub headers: HashMap<String, String>,

    /// Optional request body for the HTTP call
    pub body: Option<Vec<u8>>,

    /// OIDC token configuration for authentication
    pub oidc_token: Option<OidcTokenConfig>,

    /// Maximum number of retry attempts for this job
    pub max_retries: u32,

    /// Current retry attempt number (0 on first attempt)
    pub retry_count: u32,

    /// When the job was created
    pub created_at: DateTime<Utc>,

    /// When the job should be scheduled for execution
    pub scheduled_time: DateTime<Utc>,

    /// The current status of the job
    pub status: JobStatus,

    /// Optional failure reason if the job failed
    pub failure_reason: Option<String>,

    /// Optional custom metadata for the job
    pub metadata: HashMap<String, String>,
}

/// HTTP methods supported by Cloud Tasks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    /// HTTP GET
    Get,
    /// HTTP POST
    Post,
    /// HTTP PUT
    Put,
    /// HTTP DELETE
    Delete,
    /// HTTP PATCH
    Patch,
    /// HTTP HEAD
    Head,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::Get => write!(f, "GET"),
            HttpMethod::Post => write!(f, "POST"),
            HttpMethod::Put => write!(f, "PUT"),
            HttpMethod::Delete => write!(f, "DELETE"),
            HttpMethod::Patch => write!(f, "PATCH"),
            HttpMethod::Head => write!(f, "HEAD"),
        }
    }
}

/// OIDC token configuration for Cloud Tasks HTTP authentication.
///
/// Uses service account key to generate OIDC tokens that authenticate
/// HTTP requests to Google Cloud services and other OIDC-compatible endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OidcTokenConfig {
    /// Google Cloud service account email
    pub service_account_email: String,

    /// Optional audience claim for the OIDC token
    /// If not specified, defaults to the target URL origin
    pub audience: Option<String>,

    /// Token time-to-live in seconds (typically 3600)
    pub ttl_seconds: u32,
}

impl OidcTokenConfig {
    /// Create a new OIDC token configuration.
    pub fn new(service_account_email: String) -> Self {
        Self {
            service_account_email,
            audience: None,
            ttl_seconds: 3600,
        }
    }

    /// Set the audience claim for the token.
    pub fn with_audience(mut self, audience: String) -> Self {
        self.audience = Some(audience);
        self
    }

    /// Set the token TTL in seconds.
    pub fn with_ttl(mut self, ttl_seconds: u32) -> Self {
        self.ttl_seconds = ttl_seconds;
        self
    }
}

/// The current execution status of a job.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JobStatus {
    /// Job has been created but not yet enqueued
    Created,

    /// Job is pending execution
    Pending,

    /// Job is currently being executed
    Running,

    /// Job completed successfully
    Completed,

    /// Job failed and exceeded retry limit
    Failed,

    /// Job was cancelled before completion
    Cancelled,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Created => write!(f, "CREATED"),
            JobStatus::Pending => write!(f, "PENDING"),
            JobStatus::Running => write!(f, "RUNNING"),
            JobStatus::Completed => write!(f, "COMPLETED"),
            JobStatus::Failed => write!(f, "FAILED"),
            JobStatus::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

/// Result of a job execution attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobExecutionResult {
    /// The job that was executed
    pub job_id: Uuid,

    /// HTTP status code from the execution (if successful)
    pub status_code: Option<u16>,

    /// Response body from the HTTP call
    pub response_body: Option<Vec<u8>>,

    /// When the execution was attempted
    pub executed_at: DateTime<Utc>,

    /// Whether the execution succeeded
    pub success: bool,

    /// Optional error message if execution failed
    pub error_message: Option<String>,
}

/// Configuration for job retry behavior.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,

    /// Initial backoff duration before first retry
    pub initial_backoff: Duration,

    /// Maximum backoff duration between retries
    pub max_backoff: Duration,

    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_backoff: Duration::seconds(5),
            max_backoff: Duration::seconds(600), // 10 minutes
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Create a new retry configuration with custom values.
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    /// Calculate the backoff duration for a given retry attempt.
    pub fn calculate_backoff(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return self.initial_backoff;
        }

        let multiplier = self.backoff_multiplier.powi(attempt as i32);
        let backoff_secs = (self.initial_backoff.num_seconds() as f64 * multiplier).ceil() as i64;
        let backoff = Duration::seconds(backoff_secs);

        if backoff > self.max_backoff {
            self.max_backoff
        } else {
            backoff
        }
    }

    /// Calculate when the next retry should be scheduled.
    pub fn next_retry_time(&self, attempt: u32, last_attempt: DateTime<Utc>) -> DateTime<Utc> {
        let backoff = self.calculate_backoff(attempt);
        last_attempt + backoff
    }
}

impl Job {
    /// Create a new job with the given HTTP method and target URL.
    pub fn new(http_method: HttpMethod, target_url: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            http_method,
            target_url,
            headers: HashMap::new(),
            body: None,
            oidc_token: None,
            max_retries: 5,
            retry_count: 0,
            created_at: Utc::now(),
            scheduled_time: Utc::now(),
            status: JobStatus::Created,
            failure_reason: None,
            metadata: HashMap::new(),
        }
    }

    /// Add a header to the job request.
    pub fn with_header(mut self, key: String, value: String) -> Self {
        self.headers.insert(key, value);
        self
    }

    /// Set the request body.
    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }

    /// Configure OIDC token authentication.
    pub fn with_oidc_token(mut self, oidc_token: OidcTokenConfig) -> Self {
        self.oidc_token = Some(oidc_token);
        self
    }

    /// Set the maximum number of retries.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Schedule the job for a specific time.
    pub fn scheduled_at(mut self, time: DateTime<Utc>) -> Self {
        self.scheduled_time = time;
        self
    }

    /// Add custom metadata.
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_creation() {
        let job = Job::new(HttpMethod::Post, "https://example.com/webhook".to_string());

        assert_eq!(job.http_method, HttpMethod::Post);
        assert_eq!(job.target_url, "https://example.com/webhook");
        assert_eq!(job.status, JobStatus::Created);
        assert_eq!(job.retry_count, 0);
        assert_eq!(job.max_retries, 5);
    }

    #[test]
    fn test_job_builder() {
        let job = Job::new(HttpMethod::Post, "https://example.com/webhook".to_string())
            .with_header("Content-Type".to_string(), "application/json".to_string())
            .with_body(b"test".to_vec())
            .with_max_retries(3);

        assert_eq!(job.headers.get("Content-Type").unwrap(), "application/json");
        assert_eq!(job.body, Some(b"test".to_vec()));
        assert_eq!(job.max_retries, 3);
    }

    #[test]
    fn test_oidc_token_config() {
        let config = OidcTokenConfig::new("sa@project.iam.gserviceaccount.com".to_string())
            .with_audience("https://example.com".to_string())
            .with_ttl(1800);

        assert_eq!(
            config.service_account_email,
            "sa@project.iam.gserviceaccount.com"
        );
        assert_eq!(config.audience, Some("https://example.com".to_string()));
        assert_eq!(config.ttl_seconds, 1800);
    }

    #[test]
    fn test_http_method_display() {
        assert_eq!(HttpMethod::Get.to_string(), "GET");
        assert_eq!(HttpMethod::Post.to_string(), "POST");
        assert_eq!(HttpMethod::Put.to_string(), "PUT");
        assert_eq!(HttpMethod::Delete.to_string(), "DELETE");
        assert_eq!(HttpMethod::Patch.to_string(), "PATCH");
        assert_eq!(HttpMethod::Head.to_string(), "HEAD");
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_retry_config_backoff_calculation() {
        let config = RetryConfig::default();

        let backoff_0 = config.calculate_backoff(0);
        assert_eq!(backoff_0, Duration::seconds(5));

        let backoff_1 = config.calculate_backoff(1);
        assert_eq!(backoff_1, Duration::seconds(10));

        let backoff_2 = config.calculate_backoff(2);
        assert_eq!(backoff_2, Duration::seconds(20));
    }

    #[test]
    fn test_retry_config_max_backoff() {
        let config = RetryConfig::default();

        // Very high attempt number should be capped at max_backoff
        let backoff = config.calculate_backoff(100);
        assert_eq!(backoff, config.max_backoff);
    }

    #[test]
    fn test_retry_timing() {
        let config = RetryConfig::default();
        let now = Utc::now();

        let next_retry = config.next_retry_time(0, now);
        assert!(next_retry > now);
        assert_eq!(next_retry - now, Duration::seconds(5));
    }

    #[test]
    fn test_job_status_ordering() {
        assert!(JobStatus::Created < JobStatus::Pending);
        assert!(JobStatus::Pending < JobStatus::Running);
        assert!(JobStatus::Running < JobStatus::Completed);
    }
}
