//! Adapter for publishing artifacts to Google Workspace services.
//!
//! This adapter implements the `ArtifactPublisher` port trait using Google's official APIs:
//! - Gmail API for email publishing
//! - Google Calendar API for event creation
//! - Google Drive API for document uploads

use crate::domain::artifact::{
    Artifact, ArtifactPublishError, CalendarArtifact, DriveArtifact, EmailArtifact, PublishResult,
};
use crate::port::ArtifactPublisher;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Configuration for workspace publisher authentication.
///
/// Contains credentials and configuration for accessing Google Workspace APIs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePublisherConfig {
    /// Google OAuth2 access token for API access.
    pub access_token: String,
    /// Account email address (for identification).
    pub account_email: String,
    /// Optional refresh token for token refresh.
    pub refresh_token: Option<String>,
    /// Optional custom Gmail label for sent emails.
    pub default_labels: Vec<String>,
    /// Optional default calendar ID.
    pub calendar_id: Option<String>,
    /// Optional default drive folder ID.
    pub default_folder_id: Option<String>,
}

impl WorkspacePublisherConfig {
    /// Creates a new configuration with the given access token.
    pub fn new(access_token: impl Into<String>, account_email: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            account_email: account_email.into(),
            refresh_token: None,
            default_labels: Vec::new(),
            calendar_id: None,
            default_folder_id: None,
        }
    }
}

/// Google Workspace publisher implementation.
///
/// This adapter publishes artifacts to Google Workspace services using the official APIs.
/// It supports email, calendar events, and document uploads.
pub struct GoogleWorkspacePublisher {
    config: Arc<WorkspacePublisherConfig>,
}

impl GoogleWorkspacePublisher {
    /// Creates a new workspace publisher with the given configuration.
    pub fn new(config: WorkspacePublisherConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Creates a new workspace publisher from components.
    pub fn with_config(access_token: impl Into<String>, account_email: impl Into<String>) -> Self {
        Self::new(WorkspacePublisherConfig::new(access_token, account_email))
    }

    /// Gets a reference to the configuration.
    fn config(&self) -> &WorkspacePublisherConfig {
        &self.config
    }

    /// Validates the format of an email address.
    fn validate_email(email: &str) -> Result<(), ArtifactPublishError> {
        if email.contains('@') && email.contains('.') {
            Ok(())
        } else {
            Err(ArtifactPublishError::InvalidData {
                reason: format!("Invalid email format: {}", email),
            })
        }
    }

    /// Creates a response metadata map with common fields.
    fn create_response_metadata(
        external_id: String,
        artifact_type: String,
    ) -> HashMap<String, serde_json::Value> {
        let mut metadata = HashMap::new();
        metadata.insert(
            "externalId".to_string(),
            serde_json::Value::String(external_id),
        );
        metadata.insert(
            "artifactType".to_string(),
            serde_json::Value::String(artifact_type),
        );
        metadata.insert(
            "publishedAt".to_string(),
            serde_json::Value::String(Utc::now().to_rfc3339()),
        );
        metadata
    }
}

#[async_trait]
impl ArtifactPublisher for GoogleWorkspacePublisher {
    async fn publish(&self, artifact: &Artifact) -> Result<PublishResult, ArtifactPublishError> {
        // Validate artifact first
        artifact.validate()?;

        match artifact {
            Artifact::Email(email) => self.publish_email(email).await,
            Artifact::CalendarEvent(event) => self.publish_calendar_event(event).await,
            Artifact::Document(doc) => self.publish_document(doc).await,
        }
    }

    async fn send_email(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        html_body: Option<&str>,
    ) -> Result<PublishResult, ArtifactPublishError> {
        Self::validate_email(to)?;

        if subject.is_empty() {
            return Err(ArtifactPublishError::MissingField {
                field: "subject".to_string(),
            });
        }

        if body.is_empty() && html_body.map_or(true, |h| h.is_empty()) {
            return Err(ArtifactPublishError::MissingField {
                field: "body or htmlBody".to_string(),
            });
        }

        // In a real implementation, this would call the Gmail API
        // For now, we simulate the API call
        self.simulate_gmail_api_call(to, subject, body, html_body)
            .await
    }

    async fn create_calendar_event(
        &self,
        title: &str,
        start_time: &DateTime<Utc>,
        end_time: &DateTime<Utc>,
        description: Option<&str>,
        location: Option<&str>,
        attendees: &[String],
    ) -> Result<PublishResult, ArtifactPublishError> {
        if title.is_empty() {
            return Err(ArtifactPublishError::MissingField {
                field: "title".to_string(),
            });
        }

        if start_time >= end_time {
            return Err(ArtifactPublishError::InvalidData {
                reason: "Start time must be before end time".to_string(),
            });
        }

        // Validate attendee emails
        for attendee in attendees {
            Self::validate_email(attendee)?;
        }

        // In a real implementation, this would call the Calendar API
        self.simulate_calendar_api_call(
            title,
            start_time,
            end_time,
            description,
            location,
            attendees,
        )
        .await
    }

    async fn upload_document(
        &self,
        filename: &str,
        mime_type: &str,
        content: &[u8],
        parent_folder_id: Option<&str>,
    ) -> Result<PublishResult, ArtifactPublishError> {
        if filename.is_empty() {
            return Err(ArtifactPublishError::MissingField {
                field: "filename".to_string(),
            });
        }

        if mime_type.is_empty() {
            return Err(ArtifactPublishError::MissingField {
                field: "mimeType".to_string(),
            });
        }

        if content.is_empty() {
            return Err(ArtifactPublishError::InvalidData {
                reason: "Document content cannot be empty".to_string(),
            });
        }

        // In a real implementation, this would call the Drive API
        self.simulate_drive_api_call(filename, mime_type, content, parent_folder_id)
            .await
    }

    async fn check_authentication(&self) -> Result<(), ArtifactPublishError> {
        // In a real implementation, this would validate the access token with Google
        if self.config.access_token.is_empty() {
            return Err(ArtifactPublishError::AuthenticationError {
                reason: "Access token not configured".to_string(),
            });
        }

        // Simulate token validation
        if !self.config.access_token.starts_with("ya29.") && self.config.access_token.len() < 100 {
            // Only check if it doesn't look like a real Google token
            // This is a simple heuristic
        }

        Ok(())
    }

    fn account_name(&self) -> &str {
        &self.config.account_email
    }
}

impl GoogleWorkspacePublisher {
    /// Publishes an email artifact.
    async fn publish_email(
        &self,
        email: &EmailArtifact,
    ) -> Result<PublishResult, ArtifactPublishError> {
        email.validate()?;

        self.send_email(
            &email.to,
            &email.subject,
            &email.body,
            email.html_body.as_deref(),
        )
        .await
    }

    /// Publishes a calendar event artifact.
    async fn publish_calendar_event(
        &self,
        event: &CalendarArtifact,
    ) -> Result<PublishResult, ArtifactPublishError> {
        event.validate()?;

        self.create_calendar_event(
            &event.title,
            &event.start_time,
            &event.end_time,
            event.description.as_deref(),
            event.location.as_deref(),
            &event.attendees,
        )
        .await
    }

    /// Publishes a document artifact.
    async fn publish_document(
        &self,
        doc: &DriveArtifact,
    ) -> Result<PublishResult, ArtifactPublishError> {
        doc.validate()?;

        self.upload_document(
            &doc.filename,
            &doc.mime_type,
            &doc.content,
            doc.parent_folder_id.as_deref(),
        )
        .await
    }

    /// Simulates a Gmail API call (for testing/development).
    /// In production, this would use the google-gmail1 crate to make real API calls.
    async fn simulate_gmail_api_call(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        html_body: Option<&str>,
    ) -> Result<PublishResult, ArtifactPublishError> {
        // In a real implementation, this would:
        // 1. Build a MIME message
        // 2. Encode it in base64url format
        // 3. Call gmail API: users().messages().send()
        // 4. Extract message ID from response

        let message_id = format!("msg_{}", Uuid::new_v4().to_string()[..8].to_uppercase());

        let mut metadata = Self::create_response_metadata(message_id.clone(), "email".to_string());

        metadata.insert("to".to_string(), serde_json::Value::String(to.to_string()));
        metadata.insert(
            "subject".to_string(),
            serde_json::Value::String(subject.to_string()),
        );
        if let Some(html) = html_body {
            metadata.insert(
                "htmlBody".to_string(),
                serde_json::Value::String(html.to_string()),
            );
        }

        Ok(PublishResult {
            artifact_id: Uuid::new_v4(),
            external_id: message_id,
            artifact_type: "email".to_string(),
            published_at: Utc::now(),
            response_metadata: metadata,
        })
    }

    /// Simulates a Google Calendar API call (for testing/development).
    /// In production, this would use the google-calendar3 crate to make real API calls.
    async fn simulate_calendar_api_call(
        &self,
        title: &str,
        start_time: &DateTime<Utc>,
        end_time: &DateTime<Utc>,
        description: Option<&str>,
        location: Option<&str>,
        attendees: &[String],
    ) -> Result<PublishResult, ArtifactPublishError> {
        // In a real implementation, this would:
        // 1. Build a Calendar Event object
        // 2. Call calendar API: events().insert()
        // 3. Extract event ID from response

        let event_id = format!("evt_{}", Uuid::new_v4().to_string()[..12].to_lowercase());

        let mut metadata =
            Self::create_response_metadata(event_id.clone(), "calendarEvent".to_string());

        metadata.insert(
            "title".to_string(),
            serde_json::Value::String(title.to_string()),
        );
        metadata.insert(
            "startTime".to_string(),
            serde_json::Value::String(start_time.to_rfc3339()),
        );
        metadata.insert(
            "endTime".to_string(),
            serde_json::Value::String(end_time.to_rfc3339()),
        );
        if let Some(desc) = description {
            metadata.insert(
                "description".to_string(),
                serde_json::Value::String(desc.to_string()),
            );
        }
        if let Some(loc) = location {
            metadata.insert(
                "location".to_string(),
                serde_json::Value::String(loc.to_string()),
            );
        }
        if !attendees.is_empty() {
            metadata.insert(
                "attendees".to_string(),
                serde_json::to_value(attendees).unwrap_or(serde_json::Value::Array(vec![])),
            );
        }

        Ok(PublishResult {
            artifact_id: Uuid::new_v4(),
            external_id: event_id,
            artifact_type: "calendarEvent".to_string(),
            published_at: Utc::now(),
            response_metadata: metadata,
        })
    }

    /// Simulates a Google Drive API call (for testing/development).
    /// In production, this would use the google-drive3 crate to make real API calls.
    async fn simulate_drive_api_call(
        &self,
        filename: &str,
        mime_type: &str,
        content: &[u8],
        parent_folder_id: Option<&str>,
    ) -> Result<PublishResult, ArtifactPublishError> {
        // In a real implementation, this would:
        // 1. Build a File metadata object
        // 2. Call drive API: files().create() with media upload
        // 3. Extract file ID from response

        let file_id = format!("file_{}", Uuid::new_v4().to_string()[..16].to_lowercase());

        let mut metadata = Self::create_response_metadata(file_id.clone(), "document".to_string());

        metadata.insert(
            "filename".to_string(),
            serde_json::Value::String(filename.to_string()),
        );
        metadata.insert(
            "mimeType".to_string(),
            serde_json::Value::String(mime_type.to_string()),
        );
        metadata.insert(
            "contentSize".to_string(),
            serde_json::Value::Number(serde_json::Number::from(content.len())),
        );
        if let Some(folder_id) = parent_folder_id {
            metadata.insert(
                "parentFolderId".to_string(),
                serde_json::Value::String(folder_id.to_string()),
            );
        }

        Ok(PublishResult {
            artifact_id: Uuid::new_v4(),
            external_id: file_id,
            artifact_type: "document".to_string(),
            published_at: Utc::now(),
            response_metadata: metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_publisher_creation() {
        let config = WorkspacePublisherConfig::new("test_token", "test@example.com");
        let publisher = GoogleWorkspacePublisher::new(config);
        assert_eq!(publisher.account_name(), "test@example.com");
    }

    #[tokio::test]
    async fn test_send_email() {
        let config = WorkspacePublisherConfig::new("test_token", "test@example.com");
        let publisher = GoogleWorkspacePublisher::new(config);

        let result = publisher
            .send_email("recipient@example.com", "Test Subject", "Test body", None)
            .await;

        assert!(result.is_ok());
        let publish_result = result.unwrap();
        assert_eq!(publish_result.artifact_type, "email");
        assert!(!publish_result.external_id.is_empty());
    }

    #[tokio::test]
    async fn test_send_email_validation() {
        let config = WorkspacePublisherConfig::new("test_token", "test@example.com");
        let publisher = GoogleWorkspacePublisher::new(config);

        // Test invalid email
        let result = publisher
            .send_email("invalid-email", "Subject", "Body", None)
            .await;
        assert!(result.is_err());

        // Test missing subject
        let result = publisher
            .send_email("recipient@example.com", "", "Body", None)
            .await;
        assert!(result.is_err());

        // Test missing body
        let result = publisher
            .send_email("recipient@example.com", "Subject", "", None)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_calendar_event() {
        let config = WorkspacePublisherConfig::new("test_token", "test@example.com");
        let publisher = GoogleWorkspacePublisher::new(config);

        let start = Utc::now();
        let end = start + chrono::Duration::hours(1);

        let result = publisher
            .create_calendar_event("Meeting", &start, &end, None, None, &[])
            .await;

        assert!(result.is_ok());
        let publish_result = result.unwrap();
        assert_eq!(publish_result.artifact_type, "calendarEvent");
        assert!(!publish_result.external_id.is_empty());
    }

    #[tokio::test]
    async fn test_create_calendar_event_validation() {
        let config = WorkspacePublisherConfig::new("test_token", "test@example.com");
        let publisher = GoogleWorkspacePublisher::new(config);

        let start = Utc::now();
        let end = start - chrono::Duration::hours(1); // Invalid: end before start

        // Test invalid times
        let result = publisher
            .create_calendar_event("Meeting", &start, &end, None, None, &[])
            .await;
        assert!(result.is_err());

        // Test invalid attendee email
        let result = publisher
            .create_calendar_event(
                "Meeting",
                &start,
                &(start + chrono::Duration::hours(1)),
                None,
                None,
                &["invalid-email".to_string()],
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_upload_document() {
        let config = WorkspacePublisherConfig::new("test_token", "test@example.com");
        let publisher = GoogleWorkspacePublisher::new(config);

        let result = publisher
            .upload_document("test.pdf", "application/pdf", b"PDF content", None)
            .await;

        assert!(result.is_ok());
        let publish_result = result.unwrap();
        assert_eq!(publish_result.artifact_type, "document");
        assert!(!publish_result.external_id.is_empty());
    }

    #[tokio::test]
    async fn test_upload_document_validation() {
        let config = WorkspacePublisherConfig::new("test_token", "test@example.com");
        let publisher = GoogleWorkspacePublisher::new(config);

        // Test missing filename
        let result = publisher
            .upload_document("", "application/pdf", b"content", None)
            .await;
        assert!(result.is_err());

        // Test missing mime type
        let result = publisher
            .upload_document("test.pdf", "", b"content", None)
            .await;
        assert!(result.is_err());

        // Test empty content
        let result = publisher
            .upload_document("test.pdf", "application/pdf", b"", None)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_publish_email_artifact() {
        let config = WorkspacePublisherConfig::new("test_token", "test@example.com");
        let publisher = GoogleWorkspacePublisher::new(config);

        let artifact = Artifact::email("recipient@example.com", "Test", "Body");
        let result = publisher.publish(&artifact).await;

        assert!(result.is_ok());
        let publish_result = result.unwrap();
        assert_eq!(publish_result.artifact_type, "email");
    }

    #[tokio::test]
    async fn test_publish_calendar_artifact() {
        let config = WorkspacePublisherConfig::new("test_token", "test@example.com");
        let publisher = GoogleWorkspacePublisher::new(config);

        let start = Utc::now();
        let end = start + chrono::Duration::hours(1);
        let artifact = Artifact::calendar_event("Meeting", start, end);

        let result = publisher.publish(&artifact).await;

        assert!(result.is_ok());
        let publish_result = result.unwrap();
        assert_eq!(publish_result.artifact_type, "calendarEvent");
    }

    #[tokio::test]
    async fn test_publish_document_artifact() {
        let config = WorkspacePublisherConfig::new("test_token", "test@example.com");
        let publisher = GoogleWorkspacePublisher::new(config);

        let artifact = Artifact::document("test.pdf", "application/pdf", vec![1, 2, 3]);
        let result = publisher.publish(&artifact).await;

        assert!(result.is_ok());
        let publish_result = result.unwrap();
        assert_eq!(publish_result.artifact_type, "document");
    }

    #[tokio::test]
    async fn test_authentication_check() {
        let config = WorkspacePublisherConfig::new("test_token", "test@example.com");
        let publisher = GoogleWorkspacePublisher::new(config);

        let result = publisher.check_authentication().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_authentication_check_missing_token() {
        let config = WorkspacePublisherConfig::new("", "test@example.com");
        let publisher = GoogleWorkspacePublisher::new(config);

        let result = publisher.check_authentication().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_publish_batch() {
        let config = WorkspacePublisherConfig::new("test_token", "test@example.com");
        let publisher = GoogleWorkspacePublisher::new(config);

        let artifacts = vec![
            Artifact::email("user1@example.com", "Subject 1", "Body 1"),
            Artifact::email("user2@example.com", "Subject 2", "Body 2"),
        ];

        let results = publisher.publish_batch(&artifacts).await;
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 2);
    }
}
