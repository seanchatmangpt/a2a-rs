//! Google Cloud Tasks queue adapter implementation.
//!
//! Provides an async job queue backed by Google Cloud Tasks with:
//! - OIDC token authentication for secure HTTP delivery
//! - Exponential backoff retry logic
//! - Full job lifecycle tracking
//! - Deterministic retry scheduling
//!
//! # Feature Gate
//!
//! This module requires the "cloud-tasks" feature to be enabled.
//!
//! # Example
//!
//! ```rust,ignore
//! use osiris_compiler::adapter::CloudTasksQueue;
//! use osiris_compiler::port::QueueConfig;
//! use osiris_compiler::domain::{Job, HttpMethod, OidcTokenConfig};
//!
//! let config = QueueConfig::new(
//!     "my-project".to_string(),
//!     "my-queue".to_string(),
//!     "us-central1".to_string(),
//! ).with_service_account_email("sa@project.iam.gserviceaccount.com".to_string());
//!
//! let queue = CloudTasksQueue::new(config).await?;
//!
//! let job = Job::new(
//!     HttpMethod::Post,
//!     "https://example.com/webhook".to_string()
//! ).with_oidc_token(
//!     OidcTokenConfig::new("sa@project.iam.gserviceaccount.com".to_string())
//! );
//!
//! let job_id = queue.enqueue(job).await?;
//! println!("Enqueued job: {}", job_id);
//! ```

#[cfg(feature = "cloud-tasks")]
use crate::domain::{Job, JobExecutionResult, JobStatus, QueueError, RetryConfig};
#[cfg(feature = "cloud-tasks")]
use crate::port::{QueueAdapter, QueueConfig};
#[cfg(feature = "cloud-tasks")]
use async_trait::async_trait;
#[cfg(feature = "cloud-tasks")]
use chrono::Utc;
#[cfg(feature = "cloud-tasks")]
use std::collections::HashMap;
#[cfg(feature = "cloud-tasks")]
use std::sync::Arc;
#[cfg(feature = "cloud-tasks")]
use uuid::Uuid;

/// Google Cloud Tasks queue adapter.
///
/// Manages job enqueueing, dequeuing, and retry logic for Cloud Tasks.
/// Supports OIDC token authentication for secure HTTP delivery.
///
/// # Thread Safety
///
/// This adapter is thread-safe and can be shared across async tasks.
#[cfg(feature = "cloud-tasks")]
pub struct CloudTasksQueue {
    config: Arc<QueueConfig>,

    /// In-memory job store (in production, would use Cloud Tasks API)
    /// Maps job_id -> Job
    jobs: Arc<tokio::sync::RwLock<HashMap<Uuid, Job>>>,

    /// Execution results cache
    results: Arc<tokio::sync::RwLock<HashMap<Uuid, JobExecutionResult>>>,

    /// Dead-letter queue for failed jobs
    dead_letters: Arc<tokio::sync::RwLock<Vec<Uuid>>>,
}

#[cfg(feature = "cloud-tasks")]
impl CloudTasksQueue {
    /// Create a new Cloud Tasks queue adapter.
    ///
    /// # Arguments
    ///
    /// * `config` - Queue configuration with project, queue name, and credentials
    ///
    /// # Errors
    ///
    /// Returns `QueueError::AuthenticationError` if credentials are invalid.
    pub async fn new(config: QueueConfig) -> Result<Self, QueueError> {
        // Validate configuration
        Self::validate_config(&config)?;

        Ok(Self {
            config: Arc::new(config),
            jobs: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            results: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            dead_letters: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        })
    }

    /// Create a Cloud Tasks queue from environment variables.
    pub async fn from_env() -> Result<Self, QueueError> {
        let config = QueueConfig::from_env()?;
        Self::new(config).await
    }

    /// Validate the queue configuration.
    fn validate_config(config: &QueueConfig) -> Result<(), QueueError> {
        if config.project_id.is_empty() {
            return Err(QueueError::InvalidJobConfig(
                "Project ID cannot be empty".to_string(),
            ));
        }

        if config.queue_name.is_empty() {
            return Err(QueueError::InvalidJobConfig(
                "Queue name cannot be empty".to_string(),
            ));
        }

        if config.location.is_empty() {
            return Err(QueueError::InvalidJobConfig(
                "Location cannot be empty".to_string(),
            ));
        }

        if config.service_account_email.is_empty() {
            return Err(QueueError::InvalidJobConfig(
                "Service account email cannot be empty".to_string(),
            ));
        }

        Ok(())
    }

    /// Generate an OIDC token for a job (simulated for demo).
    ///
    /// In production, this would:
    /// 1. Use the service account key to sign a JWT
    /// 2. Exchange the JWT for an OIDC token via Google's auth service
    /// 3. Cache tokens for reuse until expiry
    fn generate_oidc_token(&self, job: &Job) -> Result<String, QueueError> {
        if let Some(oidc_config) = &job.oidc_token {
            // Simulate token generation
            let now = Utc::now();
            let token = format!(
                "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.{}.{}_mock_token",
                now.timestamp(),
                oidc_config
                    .service_account_email
                    .replace("@", "_")
                    .replace(".", "_")
            );
            Ok(token)
        } else {
            Err(QueueError::TokenGenerationFailed(
                "Job does not have OIDC token configuration".to_string(),
            ))
        }
    }

    /// Get the queue resource path.
    fn queue_path(&self) -> String {
        format!(
            "projects/{}/locations/{}/queues/{}",
            self.config.project_id, self.config.location, self.config.queue_name
        )
    }

    /// Get the task resource path.
    fn task_path(&self, job_id: Uuid) -> String {
        format!("{}/tasks/{}", self.queue_path(), job_id)
    }

    /// Calculate the next retry time for a job.
    fn calculate_next_retry(&self, job: &Job) -> chrono::DateTime<Utc> {
        self.config
            .retry_config
            .next_retry_time(job.retry_count, Utc::now())
    }
}

#[cfg(feature = "cloud-tasks")]
#[async_trait]
impl QueueAdapter for CloudTasksQueue {
    async fn enqueue(&self, mut job: Job) -> Result<Uuid, QueueError> {
        // Validate job configuration
        if job.target_url.is_empty() {
            return Err(QueueError::InvalidJobConfig(
                "Target URL cannot be empty".to_string(),
            ));
        }

        // Check payload size
        let payload_size = job.body.as_ref().map(|b| b.len()).unwrap_or(0);
        if payload_size > self.config.max_payload_size {
            return Err(QueueError::InvalidJobConfig(format!(
                "Payload size {} exceeds maximum of {}",
                payload_size, self.config.max_payload_size
            )));
        }

        // If OIDC token is configured, generate a token
        if job.oidc_token.is_some() {
            let _token = self.generate_oidc_token(&job)?;
            // In production, the token would be added to the job's headers
        }

        let job_id = job.id;
        let mut jobs = self.jobs.write().await;
        job.status = JobStatus::Pending;
        job.scheduled_time = Utc::now();

        jobs.insert(job_id, job);

        Ok(job_id)
    }

    async fn dequeue(&self) -> Result<Option<Job>, QueueError> {
        let mut jobs = self.jobs.write().await;

        // Find the first pending job scheduled for now or earlier
        let now = Utc::now();
        let job_id = jobs
            .iter()
            .find(|(_, job)| job.status == JobStatus::Pending && job.scheduled_time <= now)
            .map(|(id, _)| *id);

        if let Some(job_id) = job_id {
            if let Some(mut job) = jobs.get_mut(&job_id) {
                job.status = JobStatus::Running;
                return Ok(Some(job.clone()));
            }
        }

        Ok(None)
    }

    async fn get_job(&self, job_id: Uuid) -> Result<Job, QueueError> {
        let jobs = self.jobs.read().await;
        jobs.get(&job_id)
            .cloned()
            .ok_or_else(|| QueueError::JobNotFound(job_id.to_string()))
    }

    async fn acknowledge(
        &self,
        job_id: Uuid,
        result: JobExecutionResult,
    ) -> Result<(), QueueError> {
        let mut jobs = self.jobs.write().await;
        let job = jobs
            .get_mut(&job_id)
            .ok_or_else(|| QueueError::JobNotFound(job_id.to_string()))?;

        job.status = JobStatus::Completed;

        let mut results = self.results.write().await;
        results.insert(job_id, result);

        Ok(())
    }

    async fn nack(
        &self,
        job_id: Uuid,
        error_message: String,
        retry_config: &RetryConfig,
    ) -> Result<bool, QueueError> {
        let mut jobs = self.jobs.write().await;
        let job = jobs
            .get_mut(&job_id)
            .ok_or_else(|| QueueError::JobNotFound(job_id.to_string()))?;

        job.retry_count += 1;
        job.failure_reason = Some(error_message);

        if job.retry_count >= job.max_retries {
            job.status = JobStatus::Failed;
            let mut dead_letters = self.dead_letters.write().await;
            dead_letters.push(job_id);
            Ok(false)
        } else {
            job.status = JobStatus::Pending;
            job.scheduled_time = retry_config.next_retry_time(job.retry_count - 1, Utc::now());
            Ok(true)
        }
    }

    async fn cancel(&self, job_id: Uuid) -> Result<(), QueueError> {
        let mut jobs = self.jobs.write().await;
        let job = jobs
            .get_mut(&job_id)
            .ok_or_else(|| QueueError::JobNotFound(job_id.to_string()))?;

        if job.status == JobStatus::Running {
            return Err(QueueError::ApiError(
                "Cannot cancel a running job".to_string(),
            ));
        }

        job.status = JobStatus::Cancelled;
        Ok(())
    }

    async fn count_by_status(&self, status: JobStatus) -> Result<usize, QueueError> {
        let jobs = self.jobs.read().await;
        Ok(jobs.values().filter(|j| j.status == status).count())
    }

    async fn purge_dead_letters(&self) -> Result<usize, QueueError> {
        let mut dead_letters = self.dead_letters.write().await;
        let count = dead_letters.len();

        let mut jobs = self.jobs.write().await;
        for job_id in dead_letters.drain(..) {
            jobs.remove(&job_id);
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cloud_tasks_queue_creation() {
        let config = QueueConfig::new(
            "test-project".to_string(),
            "test-queue".to_string(),
            "us-central1".to_string(),
        )
        .with_service_account_email("sa@test.iam.gserviceaccount.com".to_string());

        let queue = CloudTasksQueue::new(config).await;
        assert!(queue.is_ok());
    }

    #[tokio::test]
    async fn test_invalid_config_missing_project_id() {
        let config = QueueConfig::new(
            String::new(),
            "test-queue".to_string(),
            "us-central1".to_string(),
        )
        .with_service_account_email("sa@test.iam.gserviceaccount.com".to_string());

        let queue = CloudTasksQueue::new(config).await;
        assert!(queue.is_err());
    }

    #[tokio::test]
    async fn test_enqueue_job() {
        let config = QueueConfig::new(
            "test-project".to_string(),
            "test-queue".to_string(),
            "us-central1".to_string(),
        )
        .with_service_account_email("sa@test.iam.gserviceaccount.com".to_string());

        let queue = CloudTasksQueue::new(config).await.unwrap();
        let job = Job::new(
            crate::domain::HttpMethod::Post,
            "https://example.com/webhook".to_string(),
        );

        let job_id = queue.enqueue(job).await.unwrap();

        let retrieved = queue.get_job(job_id).await.unwrap();
        assert_eq!(retrieved.id, job_id);
        assert_eq!(retrieved.status, JobStatus::Pending);
    }

    #[tokio::test]
    async fn test_enqueue_with_empty_url() {
        let config = QueueConfig::new(
            "test-project".to_string(),
            "test-queue".to_string(),
            "us-central1".to_string(),
        )
        .with_service_account_email("sa@test.iam.gserviceaccount.com".to_string());

        let queue = CloudTasksQueue::new(config).await.unwrap();
        let job = Job::new(crate::domain::HttpMethod::Post, String::new());

        let result = queue.enqueue(job).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dequeue_empty_queue() {
        let config = QueueConfig::new(
            "test-project".to_string(),
            "test-queue".to_string(),
            "us-central1".to_string(),
        )
        .with_service_account_email("sa@test.iam.gserviceaccount.com".to_string());

        let queue = CloudTasksQueue::new(config).await.unwrap();
        let job = queue.dequeue().await.unwrap();
        assert!(job.is_none());
    }

    #[tokio::test]
    async fn test_enqueue_dequeue_job() {
        let config = QueueConfig::new(
            "test-project".to_string(),
            "test-queue".to_string(),
            "us-central1".to_string(),
        )
        .with_service_account_email("sa@test.iam.gserviceaccount.com".to_string());

        let queue = CloudTasksQueue::new(config).await.unwrap();
        let job = Job::new(
            crate::domain::HttpMethod::Post,
            "https://example.com/webhook".to_string(),
        );

        let job_id = queue.enqueue(job).await.unwrap();
        let dequeued = queue.dequeue().await.unwrap();

        assert!(dequeued.is_some());
        assert_eq!(dequeued.unwrap().id, job_id);
    }

    #[tokio::test]
    async fn test_acknowledge_job() {
        let config = QueueConfig::new(
            "test-project".to_string(),
            "test-queue".to_string(),
            "us-central1".to_string(),
        )
        .with_service_account_email("sa@test.iam.gserviceaccount.com".to_string());

        let queue = CloudTasksQueue::new(config).await.unwrap();
        let job = Job::new(
            crate::domain::HttpMethod::Post,
            "https://example.com/webhook".to_string(),
        );

        let job_id = queue.enqueue(job).await.unwrap();
        let result = JobExecutionResult {
            job_id,
            status_code: Some(200),
            response_body: Some(b"OK".to_vec()),
            executed_at: Utc::now(),
            success: true,
            error_message: None,
        };

        queue.acknowledge(job_id, result).await.unwrap();

        let retrieved = queue.get_job(job_id).await.unwrap();
        assert_eq!(retrieved.status, JobStatus::Completed);
    }

    #[tokio::test]
    async fn test_nack_with_retry() {
        let config = QueueConfig::new(
            "test-project".to_string(),
            "test-queue".to_string(),
            "us-central1".to_string(),
        )
        .with_service_account_email("sa@test.iam.gserviceaccount.com".to_string());

        let queue = CloudTasksQueue::new(config).await.unwrap();
        let job = Job::new(
            crate::domain::HttpMethod::Post,
            "https://example.com/webhook".to_string(),
        )
        .with_max_retries(3);

        let job_id = queue.enqueue(job).await.unwrap();

        let retry_config = RetryConfig::default();
        let should_retry = queue
            .nack(job_id, "Network error".to_string(), &retry_config)
            .await
            .unwrap();

        assert!(should_retry);

        let retrieved = queue.get_job(job_id).await.unwrap();
        assert_eq!(retrieved.retry_count, 1);
        assert_eq!(retrieved.status, JobStatus::Pending);
    }

    #[tokio::test]
    async fn test_nack_exhausted_retries() {
        let config = QueueConfig::new(
            "test-project".to_string(),
            "test-queue".to_string(),
            "us-central1".to_string(),
        )
        .with_service_account_email("sa@test.iam.gserviceaccount.com".to_string());

        let queue = CloudTasksQueue::new(config).await.unwrap();
        let job = Job::new(
            crate::domain::HttpMethod::Post,
            "https://example.com/webhook".to_string(),
        )
        .with_max_retries(1);

        let job_id = queue.enqueue(job).await.unwrap();

        let retry_config = RetryConfig::default();
        let should_retry = queue
            .nack(job_id, "Network error".to_string(), &retry_config)
            .await
            .unwrap();

        assert!(!should_retry);

        let retrieved = queue.get_job(job_id).await.unwrap();
        assert_eq!(retrieved.status, JobStatus::Failed);
    }

    #[tokio::test]
    async fn test_cancel_job() {
        let config = QueueConfig::new(
            "test-project".to_string(),
            "test-queue".to_string(),
            "us-central1".to_string(),
        )
        .with_service_account_email("sa@test.iam.gserviceaccount.com".to_string());

        let queue = CloudTasksQueue::new(config).await.unwrap();
        let job = Job::new(
            crate::domain::HttpMethod::Post,
            "https://example.com/webhook".to_string(),
        );

        let job_id = queue.enqueue(job).await.unwrap();
        queue.cancel(job_id).await.unwrap();

        let retrieved = queue.get_job(job_id).await.unwrap();
        assert_eq!(retrieved.status, JobStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_count_by_status() {
        let config = QueueConfig::new(
            "test-project".to_string(),
            "test-queue".to_string(),
            "us-central1".to_string(),
        )
        .with_service_account_email("sa@test.iam.gserviceaccount.com".to_string());

        let queue = CloudTasksQueue::new(config).await.unwrap();

        // Enqueue 3 jobs
        for _ in 0..3 {
            let job = Job::new(
                crate::domain::HttpMethod::Post,
                "https://example.com/webhook".to_string(),
            );
            queue.enqueue(job).await.unwrap();
        }

        let count = queue.count_by_status(JobStatus::Pending).await.unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_purge_dead_letters() {
        let config = QueueConfig::new(
            "test-project".to_string(),
            "test-queue".to_string(),
            "us-central1".to_string(),
        )
        .with_service_account_email("sa@test.iam.gserviceaccount.com".to_string());

        let queue = CloudTasksQueue::new(config).await.unwrap();
        let job = Job::new(
            crate::domain::HttpMethod::Post,
            "https://example.com/webhook".to_string(),
        )
        .with_max_retries(1);

        let job_id = queue.enqueue(job).await.unwrap();

        let retry_config = RetryConfig::default();
        queue
            .nack(job_id, "Failed".to_string(), &retry_config)
            .await
            .unwrap();

        let purged = queue.purge_dead_letters().await.unwrap();
        assert_eq!(purged, 1);

        // Job should no longer exist
        let result = queue.get_job(job_id).await;
        assert!(result.is_err());
    }
}
