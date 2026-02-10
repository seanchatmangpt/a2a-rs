//! Port trait for artifact publishing to workspace APIs.
//!
//! This trait defines the contract for publishing artifacts to Google Workspace services
//! (Gmail, Calendar, Drive).

use crate::domain::artifact::{Artifact, ArtifactPublishError, PublishResult};
use async_trait::async_trait;

/// A publisher for artifacts to workspace services.
///
/// This port trait defines the contract for publishing artifacts to Google Workspace APIs.
/// Implementations must handle authentication, validation, and API communication.
///
/// # Publishing Guarantees
///
/// Implementations should provide:
/// - **Idempotence**: Same artifact published multiple times produces safe results
/// - **Validation**: Artifacts are validated before publishing
/// - **Error Reporting**: Clear error messages for debugging
/// - **Metadata Tracking**: Response metadata is captured and returned
///
/// # Supported Artifact Types
///
/// - **Email**: Sent via Gmail API
/// - **CalendarEvent**: Created via Google Calendar API
/// - **Document**: Uploaded via Google Drive API
///
/// # Example
///
/// ```ignore
/// use osiris_compiler::port::ArtifactPublisher;
/// use osiris_compiler::domain::Artifact;
///
/// let publisher = MyPublisher::new(credentials);
/// let artifact = Artifact::email("recipient@example.com", "Subject", "Body");
/// let result = publisher.publish(&artifact).await?;
/// println!("Published as: {}", result.external_id);
/// ```
#[async_trait]
pub trait ArtifactPublisher: Send + Sync {
    /// Publishes an artifact to the appropriate workspace service.
    ///
    /// This method handles routing to the correct service based on artifact type,
    /// validates the artifact, and publishes it to the target service.
    ///
    /// # Arguments
    ///
    /// * `artifact` - The artifact to publish
    ///
    /// # Returns
    ///
    /// Returns a `PublishResult` containing the external ID assigned by the service
    /// and response metadata.
    ///
    /// # Errors
    ///
    /// Returns `ArtifactPublishError` if:
    /// - Artifact validation fails
    /// - Authentication is invalid or expired
    /// - API request fails
    /// - Serialization/deserialization fails
    async fn publish(&self, artifact: &Artifact) -> Result<PublishResult, ArtifactPublishError>;

    /// Publishes multiple artifacts, returning results or first error.
    ///
    /// Implementations may optimize batch publishing if supported by the underlying
    /// service. The default implementation publishes serially.
    ///
    /// # Arguments
    ///
    /// * `artifacts` - Slice of artifacts to publish
    ///
    /// # Returns
    ///
    /// Returns a vector of `PublishResult`s in the same order as input artifacts.
    /// If any artifact fails, returns the error for that artifact.
    ///
    /// # Errors
    ///
    /// Returns on the first error encountered.
    async fn publish_batch(
        &self,
        artifacts: &[Artifact],
    ) -> Result<Vec<PublishResult>, ArtifactPublishError> {
        let mut results = Vec::with_capacity(artifacts.len());
        for artifact in artifacts {
            results.push(self.publish(artifact).await?);
        }
        Ok(results)
    }

    /// Sends an email via Gmail API.
    ///
    /// # Errors
    ///
    /// Returns `ArtifactPublishError` if:
    /// - Email validation fails
    /// - Authentication with Gmail fails
    /// - API request fails
    async fn send_email(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        html_body: Option<&str>,
    ) -> Result<PublishResult, ArtifactPublishError>;

    /// Creates a calendar event via Google Calendar API.
    ///
    /// # Errors
    ///
    /// Returns `ArtifactPublishError` if:
    /// - Event validation fails
    /// - Authentication with Calendar fails
    /// - API request fails
    /// - Time validation fails (start >= end)
    async fn create_calendar_event(
        &self,
        title: &str,
        start_time: &chrono::DateTime<chrono::Utc>,
        end_time: &chrono::DateTime<chrono::Utc>,
        description: Option<&str>,
        location: Option<&str>,
        attendees: &[String],
    ) -> Result<PublishResult, ArtifactPublishError>;

    /// Uploads a document to Google Drive.
    ///
    /// # Arguments
    ///
    /// * `filename` - Name of the file to upload
    /// * `mime_type` - MIME type of the content (e.g., "application/pdf")
    /// * `content` - Raw file content
    /// * `parent_folder_id` - Optional folder ID for organization
    ///
    /// # Errors
    ///
    /// Returns `ArtifactPublishError` if:
    /// - Document validation fails
    /// - Authentication with Drive fails
    /// - API request fails
    /// - Content is empty
    async fn upload_document(
        &self,
        filename: &str,
        mime_type: &str,
        content: &[u8],
        parent_folder_id: Option<&str>,
    ) -> Result<PublishResult, ArtifactPublishError>;

    /// Validates an artifact without publishing.
    ///
    /// This is a dry-run method to check if an artifact would be accepted
    /// without actually publishing it.
    ///
    /// # Arguments
    ///
    /// * `artifact` - The artifact to validate
    ///
    /// # Errors
    ///
    /// Returns `ArtifactPublishError` if validation fails.
    fn validate_artifact(&self, artifact: &Artifact) -> Result<(), ArtifactPublishError> {
        artifact.validate()
    }

    /// Checks if the publisher is properly authenticated.
    ///
    /// # Errors
    ///
    /// Returns `ArtifactPublishError::AuthenticationError` if authentication is invalid.
    async fn check_authentication(&self) -> Result<(), ArtifactPublishError>;

    /// Returns the name of the workspace account this publisher is configured for.
    fn account_name(&self) -> &str;
}
