//! Typed packet domain types for the closed type system Σ
//!
//! Converts external interactions (Google Workspace API callbacks) into compiler-ready packets.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Typed packet conforming to closed type system Σ
///
/// Represents normalized external interactions ready for compiler processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypedPacket {
    /// Unique packet identifier
    pub id: Uuid,

    /// Timestamp when packet was created
    pub timestamp: DateTime<Utc>,

    /// Source of the packet
    pub source: PacketSource,

    /// Packet payload conforming to type system Σ
    pub payload: PacketPayload,

    /// Extracted context from the source event
    pub context: PacketContext,
}

/// Source of the packet
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum PacketSource {
    /// Google Workspace Gmail API callback
    Gmail {
        /// Gmail message ID
        message_id: String,
        /// Gmail thread ID
        thread_id: Option<String>,
        /// History ID for change tracking
        history_id: Option<u64>,
    },

    /// Google Workspace Calendar API callback
    Calendar {
        /// Calendar event ID
        event_id: String,
        /// Calendar ID
        calendar_id: String,
        /// Event status (confirmed, tentative, cancelled)
        status: String,
    },

    /// Google Workspace Drive API callback
    Drive {
        /// Drive file/folder ID
        file_id: String,
        /// Drive item type (file, folder)
        item_type: DriveItemType,
        /// MIME type
        mime_type: String,
    },
}

/// Drive item type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DriveItemType {
    File,
    Folder,
}

/// Packet payload conforming to type system Σ
///
/// Closed union type representing all valid packet types in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum PacketPayload {
    /// Email message packet
    Email {
        /// Email subject
        subject: String,
        /// Sender email address
        from: String,
        /// Recipient email addresses
        to: Vec<String>,
        /// CC email addresses
        cc: Vec<String>,
        /// Email body (text or HTML)
        body: String,
        /// Whether body is HTML
        is_html: bool,
        /// Attachments
        attachments: Vec<Attachment>,
    },

    /// Calendar event packet
    CalendarEvent {
        /// Event title/summary
        title: String,
        /// Event description
        description: Option<String>,
        /// Event start time
        start_time: DateTime<Utc>,
        /// Event end time
        end_time: DateTime<Utc>,
        /// Event location
        location: Option<String>,
        /// Attendees
        attendees: Vec<Attendee>,
        /// Meeting link (e.g., Google Meet)
        meeting_link: Option<String>,
    },

    /// Drive file packet
    DriveFile {
        /// File name
        name: String,
        /// File size in bytes
        size: u64,
        /// File owner
        owner: String,
        /// Shared with (list of email addresses)
        shared_with: Vec<String>,
        /// Last modified time
        last_modified: DateTime<Utc>,
        /// Whether file is trashed
        is_trashed: bool,
        /// Parent folder IDs
        parent_folders: Vec<String>,
    },

    /// Drive folder packet
    DriveFolder {
        /// Folder name
        name: String,
        /// Folder owner
        owner: String,
        /// Shared with (list of email addresses)
        shared_with: Vec<String>,
        /// Last modified time
        last_modified: DateTime<Utc>,
        /// Whether folder is trashed
        is_trashed: bool,
        /// Parent folder IDs
        parent_folders: Vec<String>,
    },
}

/// Email attachment metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    /// Attachment filename
    pub filename: String,
    /// MIME type
    pub mime_type: String,
    /// Size in bytes
    pub size: u64,
    /// Attachment ID (for retrieval)
    pub attachment_id: String,
}

/// Calendar event attendee
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attendee {
    /// Attendee email address
    pub email: String,
    /// Attendee display name
    pub display_name: Option<String>,
    /// Response status (accepted, declined, tentative, needsAction)
    pub response_status: String,
    /// Whether attendee is organizer
    pub is_organizer: bool,
    /// Whether attendee is optional
    pub is_optional: bool,
}

/// Extracted context from the source event
///
/// Provides additional metadata for compiler processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacketContext {
    /// User ID associated with the event
    pub user_id: String,

    /// Workspace domain (e.g., company.com)
    pub workspace_domain: String,

    /// Event type (created, updated, deleted)
    pub event_type: EventType,

    /// Raw webhook data (for debugging/auditing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_webhook_data: Option<serde_json::Value>,

    /// Additional custom metadata
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

/// Event type for change tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EventType {
    Created,
    Updated,
    Deleted,
    Shared,
    Unshared,
}

impl TypedPacket {
    /// Create a new typed packet
    #[must_use]
    pub fn new(source: PacketSource, payload: PacketPayload, context: PacketContext) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source,
            payload,
            context,
        }
    }

    /// Get packet type as a string for routing/logging
    #[must_use]
    pub fn packet_type(&self) -> &'static str {
        match &self.payload {
            PacketPayload::Email { .. } => "email",
            PacketPayload::CalendarEvent { .. } => "calendar_event",
            PacketPayload::DriveFile { .. } => "drive_file",
            PacketPayload::DriveFolder { .. } => "drive_folder",
        }
    }

    /// Validate packet conforms to type system Σ constraints
    ///
    /// # Errors
    /// Returns error if packet violates type system constraints
    pub fn validate(&self) -> Result<(), String> {
        // Validate source-payload alignment
        match (&self.source, &self.payload) {
            (PacketSource::Gmail { .. }, PacketPayload::Email { .. }) => Ok(()),
            (PacketSource::Calendar { .. }, PacketPayload::CalendarEvent { .. }) => Ok(()),
            (
                PacketSource::Drive {
                    item_type: DriveItemType::File,
                    ..
                },
                PacketPayload::DriveFile { .. },
            ) => Ok(()),
            (
                PacketSource::Drive {
                    item_type: DriveItemType::Folder,
                    ..
                },
                PacketPayload::DriveFolder { .. },
            ) => Ok(()),
            _ => Err(format!(
                "Source-payload mismatch: source type does not match payload type"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typed_packet_creation() {
        let source = PacketSource::Gmail {
            message_id: "msg123".to_string(),
            thread_id: Some("thread456".to_string()),
            history_id: Some(789),
        };

        let payload = PacketPayload::Email {
            subject: "Test Email".to_string(),
            from: "sender@example.com".to_string(),
            to: vec!["recipient@example.com".to_string()],
            cc: vec![],
            body: "Test body".to_string(),
            is_html: false,
            attachments: vec![],
        };

        let context = PacketContext {
            user_id: "user123".to_string(),
            workspace_domain: "example.com".to_string(),
            event_type: EventType::Created,
            raw_webhook_data: None,
            metadata: Default::default(),
        };

        let packet = TypedPacket::new(source, payload, context);

        assert_eq!(packet.packet_type(), "email");
        assert!(packet.validate().is_ok());
    }

    #[test]
    fn test_packet_validation_mismatch() {
        let source = PacketSource::Gmail {
            message_id: "msg123".to_string(),
            thread_id: None,
            history_id: None,
        };

        // Wrong payload for Gmail source
        let payload = PacketPayload::CalendarEvent {
            title: "Meeting".to_string(),
            description: None,
            start_time: Utc::now(),
            end_time: Utc::now(),
            location: None,
            attendees: vec![],
            meeting_link: None,
        };

        let context = PacketContext {
            user_id: "user123".to_string(),
            workspace_domain: "example.com".to_string(),
            event_type: EventType::Created,
            raw_webhook_data: None,
            metadata: Default::default(),
        };

        let packet = TypedPacket::new(source, payload, context);

        assert!(packet.validate().is_err());
    }
}
