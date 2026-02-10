//! Domain types for publishable artifacts.
//!
//! This module defines the data structures for artifacts that can be published
//! to workspace APIs (Gmail, Calendar, Drive).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Result type for artifact operations.
pub type ArtifactResult<T> = Result<T, ArtifactPublishError>;

/// Error type for artifact publishing operations.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactPublishError {
    /// Authentication failed with the workspace service.
    #[error("Authentication failed: {reason}")]
    AuthenticationError { reason: String },

    /// Required field is missing.
    #[error("Missing required field: {field}")]
    MissingField { field: String },

    /// Invalid artifact data.
    #[error("Invalid artifact data: {reason}")]
    InvalidData { reason: String },

    /// API request failed.
    #[error("API request failed: {reason}")]
    RequestFailed { reason: String },

    /// Serialization/deserialization error.
    #[error("Serialization error: {reason}")]
    SerializationError { reason: String },

    /// Network error.
    #[error("Network error: {reason}")]
    NetworkError { reason: String },

    /// Artifact type not supported.
    #[error("Unsupported artifact type: {artifact_type}")]
    UnsupportedArtifactType { artifact_type: String },
}

/// A publishable artifact in the workspace system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Artifact {
    /// An email artifact to be sent via Gmail.
    Email(EmailArtifact),
    /// A calendar event artifact to be created via Google Calendar.
    CalendarEvent(CalendarArtifact),
    /// A document artifact to be uploaded to Google Drive.
    Document(DriveArtifact),
}

impl Artifact {
    /// Creates a new email artifact.
    pub fn email(
        to: impl Into<String>,
        subject: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self::Email(EmailArtifact {
            id: Uuid::new_v4(),
            to: to.into(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: subject.into(),
            body: body.into(),
            html_body: None,
            attachments: Vec::new(),
            labels: Vec::new(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        })
    }

    /// Creates a new calendar event artifact.
    pub fn calendar_event(
        title: impl Into<String>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Self {
        Self::CalendarEvent(CalendarArtifact {
            id: Uuid::new_v4(),
            title: title.into(),
            description: None,
            start_time,
            end_time,
            attendees: Vec::new(),
            location: None,
            timezone: "UTC".to_string(),
            is_all_day: false,
            reminders: Vec::new(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        })
    }

    /// Creates a new drive artifact.
    pub fn document(
        filename: impl Into<String>,
        mime_type: impl Into<String>,
        content: Vec<u8>,
    ) -> Self {
        Self::Document(DriveArtifact {
            id: Uuid::new_v4(),
            filename: filename.into(),
            mime_type: mime_type.into(),
            content,
            parent_folder_id: None,
            sharing_permissions: Vec::new(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        })
    }

    /// Returns the artifact's unique identifier.
    pub fn id(&self) -> Uuid {
        match self {
            Artifact::Email(a) => a.id,
            Artifact::CalendarEvent(a) => a.id,
            Artifact::Document(a) => a.id,
        }
    }

    /// Returns the artifact's creation timestamp.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Artifact::Email(a) => a.timestamp,
            Artifact::CalendarEvent(a) => a.timestamp,
            Artifact::Document(a) => a.timestamp,
        }
    }

    /// Validates the artifact for publishing.
    pub fn validate(&self) -> ArtifactResult<()> {
        match self {
            Artifact::Email(a) => a.validate(),
            Artifact::CalendarEvent(a) => a.validate(),
            Artifact::Document(a) => a.validate(),
        }
    }
}

/// An email artifact to be sent via Gmail.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailArtifact {
    /// Unique identifier for this artifact.
    pub id: Uuid,
    /// Primary recipient email address.
    pub to: String,
    /// Carbon copy recipients.
    pub cc: Vec<String>,
    /// Blind carbon copy recipients.
    pub bcc: Vec<String>,
    /// Email subject line.
    pub subject: String,
    /// Plain text email body.
    pub body: String,
    /// Optional HTML email body.
    pub html_body: Option<String>,
    /// Email attachments (filenames).
    pub attachments: Vec<String>,
    /// Gmail labels to apply to sent email.
    pub labels: Vec<String>,
    /// Creation timestamp.
    pub timestamp: DateTime<Utc>,
    /// Additional metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl EmailArtifact {
    /// Validates the email artifact.
    pub fn validate(&self) -> ArtifactResult<()> {
        if self.to.is_empty() {
            return Err(ArtifactPublishError::MissingField {
                field: "to".to_string(),
            });
        }

        if self.subject.is_empty() {
            return Err(ArtifactPublishError::MissingField {
                field: "subject".to_string(),
            });
        }

        if self.body.is_empty() && self.html_body.as_ref().map_or(true, |h| h.is_empty()) {
            return Err(ArtifactPublishError::MissingField {
                field: "body or htmlBody".to_string(),
            });
        }

        // Validate email addresses (basic check)
        if !self.to.contains('@') {
            return Err(ArtifactPublishError::InvalidData {
                reason: "Invalid recipient email address".to_string(),
            });
        }

        Ok(())
    }

    /// Adds a carbon copy recipient.
    pub fn add_cc(&mut self, email: impl Into<String>) {
        self.cc.push(email.into());
    }

    /// Adds a blind carbon copy recipient.
    pub fn add_bcc(&mut self, email: impl Into<String>) {
        self.bcc.push(email.into());
    }

    /// Sets the HTML body for this email.
    pub fn set_html_body(&mut self, html: impl Into<String>) {
        self.html_body = Some(html.into());
    }

    /// Adds a Gmail label.
    pub fn add_label(&mut self, label: impl Into<String>) {
        self.labels.push(label.into());
    }
}

/// A calendar event artifact to be created in Google Calendar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarArtifact {
    /// Unique identifier for this artifact.
    pub id: Uuid,
    /// Event title.
    pub title: String,
    /// Event description.
    pub description: Option<String>,
    /// Event start time.
    pub start_time: DateTime<Utc>,
    /// Event end time.
    pub end_time: DateTime<Utc>,
    /// Event attendees (email addresses).
    pub attendees: Vec<String>,
    /// Event location.
    pub location: Option<String>,
    /// Timezone for the event.
    pub timezone: String,
    /// Whether this is an all-day event.
    pub is_all_day: bool,
    /// Reminder settings (in minutes before event).
    pub reminders: Vec<u32>,
    /// Creation timestamp.
    pub timestamp: DateTime<Utc>,
    /// Additional metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl CalendarArtifact {
    /// Validates the calendar artifact.
    pub fn validate(&self) -> ArtifactResult<()> {
        if self.title.is_empty() {
            return Err(ArtifactPublishError::MissingField {
                field: "title".to_string(),
            });
        }

        if self.start_time >= self.end_time {
            return Err(ArtifactPublishError::InvalidData {
                reason: "Start time must be before end time".to_string(),
            });
        }

        Ok(())
    }

    /// Adds an attendee to the calendar event.
    pub fn add_attendee(&mut self, email: impl Into<String>) {
        self.attendees.push(email.into());
    }

    /// Sets the event description.
    pub fn set_description(&mut self, desc: impl Into<String>) {
        self.description = Some(desc.into());
    }

    /// Sets the event location.
    pub fn set_location(&mut self, location: impl Into<String>) {
        self.location = Some(location.into());
    }

    /// Adds a reminder for this event.
    pub fn add_reminder(&mut self, minutes_before: u32) {
        self.reminders.push(minutes_before);
    }
}

/// A document artifact to be uploaded to Google Drive.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveArtifact {
    /// Unique identifier for this artifact.
    pub id: Uuid,
    /// Filename for the uploaded document.
    pub filename: String,
    /// MIME type of the document.
    pub mime_type: String,
    /// Document content (raw bytes).
    #[serde(with = "serde_bytes")]
    pub content: Vec<u8>,
    /// Optional parent folder ID for organization.
    pub parent_folder_id: Option<String>,
    /// Sharing permissions to apply.
    pub sharing_permissions: Vec<SharingPermission>,
    /// Creation timestamp.
    pub timestamp: DateTime<Utc>,
    /// Additional metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl DriveArtifact {
    /// Validates the drive artifact.
    pub fn validate(&self) -> ArtifactResult<()> {
        if self.filename.is_empty() {
            return Err(ArtifactPublishError::MissingField {
                field: "filename".to_string(),
            });
        }

        if self.mime_type.is_empty() {
            return Err(ArtifactPublishError::MissingField {
                field: "mimeType".to_string(),
            });
        }

        if self.content.is_empty() {
            return Err(ArtifactPublishError::InvalidData {
                reason: "Document content cannot be empty".to_string(),
            });
        }

        Ok(())
    }

    /// Sets the parent folder for organization.
    pub fn set_parent_folder(&mut self, folder_id: impl Into<String>) {
        self.parent_folder_id = Some(folder_id.into());
    }

    /// Adds a sharing permission for this document.
    pub fn add_sharing_permission(&mut self, permission: SharingPermission) {
        self.sharing_permissions.push(permission);
    }
}

/// Sharing permission for a Google Drive document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharingPermission {
    /// The type of principal (user, group, domain, etc.).
    pub r#type: String,
    /// Email address or domain of the principal.
    pub value: String,
    /// Permission level (reader, commenter, writer).
    pub role: String,
}

impl SharingPermission {
    /// Creates a reader permission for a user.
    pub fn reader(email: impl Into<String>) -> Self {
        Self {
            r#type: "user".to_string(),
            value: email.into(),
            role: "reader".to_string(),
        }
    }

    /// Creates a writer permission for a user.
    pub fn writer(email: impl Into<String>) -> Self {
        Self {
            r#type: "user".to_string(),
            value: email.into(),
            role: "writer".to_string(),
        }
    }

    /// Creates a viewer permission for a domain.
    pub fn domain_reader(domain: impl Into<String>) -> Self {
        Self {
            r#type: "domain".to_string(),
            value: domain.into(),
            role: "reader".to_string(),
        }
    }
}

/// Result of publishing an artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishResult {
    /// The artifact that was published.
    pub artifact_id: Uuid,
    /// The external ID assigned by the service (e.g., message ID, event ID).
    pub external_id: String,
    /// The artifact type that was published.
    pub artifact_type: String,
    /// Timestamp of successful publication.
    pub published_at: DateTime<Utc>,
    /// Service-specific response metadata.
    pub response_metadata: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_artifact_creation() {
        let artifact = Artifact::email("test@example.com", "Test Subject", "Test body");
        assert!(matches!(artifact, Artifact::Email(_)));
        assert!(artifact.validate().is_ok());
    }

    #[test]
    fn test_email_artifact_validation_missing_to() {
        let email = EmailArtifact {
            id: Uuid::new_v4(),
            to: String::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Test".to_string(),
            body: "Body".to_string(),
            html_body: None,
            attachments: Vec::new(),
            labels: Vec::new(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        };

        assert!(email.validate().is_err());
    }

    #[test]
    fn test_calendar_artifact_creation() {
        let start = Utc::now();
        let end = start + chrono::Duration::hours(1);
        let artifact = Artifact::calendar_event("Meeting", start, end);
        assert!(matches!(artifact, Artifact::CalendarEvent(_)));
        assert!(artifact.validate().is_ok());
    }

    #[test]
    fn test_calendar_artifact_validation_invalid_times() {
        let start = Utc::now();
        let end = start - chrono::Duration::hours(1);

        let artifact = CalendarArtifact {
            id: Uuid::new_v4(),
            title: "Meeting".to_string(),
            description: None,
            start_time: start,
            end_time: end,
            attendees: Vec::new(),
            location: None,
            timezone: "UTC".to_string(),
            is_all_day: false,
            reminders: Vec::new(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        };

        assert!(artifact.validate().is_err());
    }

    #[test]
    fn test_document_artifact_creation() {
        let artifact = Artifact::document("test.pdf", "application/pdf", vec![1, 2, 3]);
        assert!(matches!(artifact, Artifact::Document(_)));
        assert!(artifact.validate().is_ok());
    }

    #[test]
    fn test_sharing_permission_builder() {
        let reader = SharingPermission::reader("user@example.com");
        assert_eq!(reader.role, "reader");

        let writer = SharingPermission::writer("user@example.com");
        assert_eq!(writer.role, "writer");

        let domain = SharingPermission::domain_reader("example.com");
        assert_eq!(domain.r#type, "domain");
    }
}
