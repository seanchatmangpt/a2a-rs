//! Port trait for asynchronous job queue operations.
//!
//! Defines the contract for enqueuing, dequeuing, and managing jobs
//! with support for OIDC token authentication and retry logic.

use crate::domain::{Job, JobExecutionResult, QueueError, RetryConfig};
use async_trait::async_trait;
use uuid::Uuid;

/// Port trait for managing async job queues.
///
/// Implementations must support:
/// - Enqueuing jobs with priority and scheduling
/// - Dequeuing jobs for execution
/// - Tracking and retrying failed jobs
/// - OIDC token authentication for secure delivery
///
/// # Guarantees
///
/// 1. **At-least-once delivery**: Jobs are delivered at least once
/// 2. **Retry logic**: Failed jobs are automatically retried with exponential backoff
/// 3. **OIDC authentication**: Jobs can use Google Cloud OIDC tokens for secure endpoints
/// 4. **Deterministic retry**: Retry timing is deterministic based on configuration
/// 5. **Status tracking**: Full job lifecycle visibility
///
/// # Example
///
/// ```ignore
/// use osiris_compiler::port::QueueAdapter;
/// use osiris_compiler::domain::{Job, HttpMethod};
///
/// let queue = MyQueueAdapter::new();
/// let job = Job::new(
///     HttpMethod::Post,
///     "https://example.com/webhook".to_string()
/// );
///
/// let job_id = queue.enqueue(job).await?;
/// println!("Enqueued: {}", job_id);
/// ```
#[async_trait]
pub trait QueueAdapter: Send + Sync {
    /// Enqueue a job for asynchronous execution.
    ///
    /// # Arguments
    ///
    /// * `job` - The job to enqueue
    ///
    /// # Returns
    ///
    /// The UUID of the enqueued job
    ///
    /// # Errors
    ///
    /// Returns `QueueError::EnqueueFailed` if:
    /// - The job configuration is invalid
    /// - The queue backend is unreachable
    /// - The job payload is too large
    /// - Authentication with the queue service fails
    async fn enqueue(&self, job: Job) -> Result<Uuid, QueueError>;

    /// Enqueue multiple jobs in a batch.
    ///
    /// Implementations should attempt all-or-nothing semantics where possible.
    ///
    /// # Arguments
    ///
    /// * `jobs` - The jobs to enqueue
    ///
    /// # Returns
    ///
    /// A tuple of (successfully_enqueued_count, failed_jobs_with_reasons)
    ///
    /// # Errors
    ///
    /// Returns `QueueError::EnqueueFailed` if the batch operation fails entirely.
    async fn enqueue_batch(
        &self,
        jobs: Vec<Job>,
    ) -> Result<(usize, Vec<(Uuid, QueueError)>), QueueError> {
        let mut successful = 0;
        let mut failed = Vec::new();

        for job in jobs {
            let job_id = job.id;
            match self.enqueue(job).await {
                Ok(_) => successful += 1,
                Err(e) => failed.push((job_id, e)),
            }
        }

        Ok((successful, failed))
    }

    /// Dequeue a job for execution.
    ///
    /// Typically called by a worker process. The dequeued job should be
    /// executed and then either acknowledged (completed) or nacked (for retry).
    ///
    /// # Returns
    ///
    /// The next job to execute, or `None` if the queue is empty.
    /// Returns the job with current status, retry count, and metadata.
    ///
    /// # Errors
    ///
    /// Returns `QueueError::DequeueFailed` if:
    /// - The queue backend is unreachable
    /// - Authentication fails
    /// - The job data is corrupted
    async fn dequeue(&self) -> Result<Option<Job>, QueueError>;

    /// Dequeue multiple jobs in a batch.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of jobs to dequeue
    ///
    /// # Returns
    ///
    /// Up to `limit` jobs ready for execution
    async fn dequeue_batch(&self, limit: usize) -> Result<Vec<Job>, QueueError> {
        let mut jobs = Vec::with_capacity(limit);

        for _ in 0..limit {
            match self.dequeue().await? {
                Some(job) => jobs.push(job),
                None => break,
            }
        }

        Ok(jobs)
    }

    /// Get a job by ID without dequeuing it.
    ///
    /// # Arguments
    ///
    /// * `job_id` - The ID of the job to retrieve
    ///
    /// # Returns
    ///
    /// The job with its current state
    ///
    /// # Errors
    ///
    /// Returns `QueueError::JobNotFound` if the job doesn't exist.
    async fn get_job(&self, job_id: Uuid) -> Result<Job, QueueError>;

    /// Record the successful execution of a job.
    ///
    /// Called after a job has been successfully executed.
    /// Removes the job from the queue and may trigger cleanup.
    ///
    /// # Arguments
    ///
    /// * `job_id` - The ID of the completed job
    /// * `result` - The execution result with response data
    ///
    /// # Errors
    ///
    /// Returns `QueueError::JobNotFound` if the job doesn't exist.
    async fn acknowledge(&self, job_id: Uuid, result: JobExecutionResult)
        -> Result<(), QueueError>;

    /// Record a failed job execution and schedule retry if allowed.
    ///
    /// Called when a job execution fails. If retries remain, schedules
    /// the job for retry with appropriate backoff. Otherwise marks as failed.
    ///
    /// # Arguments
    ///
    /// * `job_id` - The ID of the failed job
    /// * `error_message` - Description of the failure
    /// * `retry_config` - Configuration for retry backoff
    ///
    /// # Returns
    ///
    /// `true` if the job was scheduled for retry, `false` if retries exhausted
    ///
    /// # Errors
    ///
    /// Returns `QueueError::RetryFailed` if:
    /// - The job doesn't exist
    /// - Updating the job state fails
    /// - Scheduling the retry fails
    async fn nack(
        &self,
        job_id: Uuid,
        error_message: String,
        retry_config: &RetryConfig,
    ) -> Result<bool, QueueError>;

    /// Cancel a job before it's executed.
    ///
    /// # Arguments
    ///
    /// * `job_id` - The ID of the job to cancel
    ///
    /// # Errors
    ///
    /// Returns `QueueError::JobNotFound` if the job doesn't exist.
    /// May return error if the job has already started execution.
    async fn cancel(&self, job_id: Uuid) -> Result<(), QueueError>;

    /// Get the count of jobs in a specific status.
    ///
    /// # Arguments
    ///
    /// * `status` - Filter by job status
    ///
    /// # Returns
    ///
    /// The number of jobs with the specified status
    async fn count_by_status(&self, status: crate::domain::JobStatus) -> Result<usize, QueueError> {
        let _ = status;
        Ok(0) // Default implementation returns 0
    }

    /// Purge dead-letter jobs (jobs that have exhausted retries).
    ///
    /// # Returns
    ///
    /// The number of jobs that were purged
    ///
    /// # Errors
    ///
    /// Returns `QueueError::ApiError` if the purge operation fails.
    async fn purge_dead_letters(&self) -> Result<usize, QueueError> {
        Ok(0) // Default implementation no-op
    }
}

/// Configuration for a Cloud Tasks queue adapter.
///
/// Contains all necessary configuration to connect to a Cloud Tasks queue
/// and authenticate requests using OIDC tokens.
#[derive(Debug, Clone)]
pub struct QueueConfig {
    /// Google Cloud project ID
    pub project_id: String,

    /// Cloud Tasks queue name
    pub queue_name: String,

    /// The location/region of the queue (e.g., "us-central1")
    pub location: String,

    /// Service account email for OIDC token generation
    pub service_account_email: String,

    /// Optional path to service account key file
    /// If not specified, uses Application Default Credentials
    pub service_account_key: Option<String>,

    /// Default retry configuration for jobs
    pub retry_config: RetryConfig,

    /// Maximum payload size in bytes (default 100KB)
    pub max_payload_size: usize,

    /// Optional timeout for job execution in seconds
    pub execution_timeout_secs: Option<u32>,
}

impl QueueConfig {
    /// Create a new queue configuration.
    pub fn new(project_id: String, queue_name: String, location: String) -> Self {
        Self {
            project_id,
            queue_name,
            location,
            service_account_email: String::new(),
            service_account_key: None,
            retry_config: RetryConfig::default(),
            max_payload_size: 100 * 1024,      // 100 KB
            execution_timeout_secs: Some(600), // 10 minutes
        }
    }

    /// Set the service account email.
    pub fn with_service_account_email(mut self, email: String) -> Self {
        self.service_account_email = email;
        self
    }

    /// Set the service account key path.
    pub fn with_service_account_key(mut self, path: String) -> Self {
        self.service_account_key = Some(path);
        self
    }

    /// Set the retry configuration.
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Set the maximum payload size.
    pub fn with_max_payload_size(mut self, size: usize) -> Self {
        self.max_payload_size = size;
        self
    }

    /// Set the execution timeout.
    pub fn with_execution_timeout(mut self, timeout_secs: u32) -> Self {
        self.execution_timeout_secs = Some(timeout_secs);
        self
    }

    /// Create configuration from environment variables.
    ///
    /// Expected environment variables:
    /// - `GCP_PROJECT_ID` (required)
    /// - `CLOUD_TASKS_QUEUE_NAME` (required)
    /// - `CLOUD_TASKS_LOCATION` (required, e.g., "us-central1")
    /// - `CLOUD_TASKS_SERVICE_ACCOUNT_EMAIL` (required)
    /// - `GOOGLE_APPLICATION_CREDENTIALS` (optional)
    pub fn from_env() -> Result<Self, QueueError> {
        let project_id = std::env::var("GCP_PROJECT_ID").map_err(|_| {
            QueueError::InvalidJobConfig("Missing GCP_PROJECT_ID environment variable".to_string())
        })?;

        let queue_name = std::env::var("CLOUD_TASKS_QUEUE_NAME").map_err(|_| {
            QueueError::InvalidJobConfig(
                "Missing CLOUD_TASKS_QUEUE_NAME environment variable".to_string(),
            )
        })?;

        let location = std::env::var("CLOUD_TASKS_LOCATION").map_err(|_| {
            QueueError::InvalidJobConfig(
                "Missing CLOUD_TASKS_LOCATION environment variable".to_string(),
            )
        })?;

        let service_account_email =
            std::env::var("CLOUD_TASKS_SERVICE_ACCOUNT_EMAIL").map_err(|_| {
                QueueError::InvalidJobConfig(
                    "Missing CLOUD_TASKS_SERVICE_ACCOUNT_EMAIL environment variable".to_string(),
                )
            })?;

        Ok(Self::new(project_id, queue_name, location)
            .with_service_account_email(service_account_email)
            .with_service_account_key(
                std::env::var("GOOGLE_APPLICATION_CREDENTIALS")
                    .ok()
                    .unwrap_or_default(),
            ))
    }
}
