//! Google Workspace API normalizer adapter
//!
//! Converts Google Workspace API callbacks (Gmail, Calendar, Drive) into typed packets.

use crate::domain::packet::{
    Attachment, Attendee, DriveItemType, EventType, PacketContext, PacketPayload, PacketSource,
    TypedPacket,
};
use crate::port::packet_normalizer::{NormalizationError, PacketNormalizer};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

/// Google Workspace packet normalizer
///
/// Converts Gmail, Calendar, and Drive API webhooks into standardized TypedPacket instances.
#[derive(Debug, Clone)]
pub struct WorkspaceNormalizer {
    /// Default workspace domain for context
    default_domain: String,
}

impl WorkspaceNormalizer {
    /// Create a new workspace normalizer
    ///
    /// # Arguments
    /// * `default_domain` - Default workspace domain (e.g., "example.com")
    #[must_use]
    pub fn new(default_domain: impl Into<String>) -> Self {
        Self {
            default_domain: default_domain.into(),
        }
    }

    /// Extract string field from JSON value
    fn extract_string(data: &Value, field: &str) -> Result<String, NormalizationError> {
        data.get(field)
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| NormalizationError::MissingField(field.to_string()))
    }

    /// Extract optional string field from JSON value
    fn extract_optional_string(data: &Value, field: &str) -> Option<String> {
        data.get(field)
            .and_then(Value::as_str)
            .map(ToString::to_string)
    }

    /// Extract string array from JSON value
    fn extract_string_array(data: &Value, field: &str) -> Vec<String> {
        data.get(field)
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Extract timestamp from JSON value
    fn extract_timestamp(data: &Value, field: &str) -> Result<DateTime<Utc>, NormalizationError> {
        let timestamp_str = Self::extract_string(data, field)?;
        timestamp_str
            .parse::<DateTime<Utc>>()
            .map_err(|e| NormalizationError::InvalidPayload(format!("Invalid timestamp: {}", e)))
    }

    /// Extract u64 from JSON value
    fn extract_u64(data: &Value, field: &str) -> Result<u64, NormalizationError> {
        data.get(field)
            .and_then(Value::as_u64)
            .ok_or_else(|| NormalizationError::MissingField(field.to_string()))
    }

    /// Determine event type from webhook data
    fn determine_event_type(data: &Value) -> EventType {
        // Check for explicit event type field
        if let Some(event_type) = data.get("eventType").or_else(|| data.get("event_type")) {
            if let Some(event_str) = event_type.as_str() {
                return match event_str.to_lowercase().as_str() {
                    "created" | "create" => EventType::Created,
                    "updated" | "update" | "modified" | "modify" => EventType::Updated,
                    "deleted" | "delete" | "trash" | "trashed" => EventType::Deleted,
                    "shared" | "share" => EventType::Shared,
                    "unshared" | "unshare" => EventType::Unshared,
                    _ => EventType::Updated, // Default to updated
                };
            }
        }

        // Infer from resource state if available
        if let Some(resource_state) = data
            .get("resourceState")
            .or_else(|| data.get("resource_state"))
        {
            if let Some(state) = resource_state.as_str() {
                return match state {
                    "sync" | "exists" => EventType::Updated,
                    "not_exists" => EventType::Deleted,
                    _ => EventType::Updated,
                };
            }
        }

        // Default to updated if cannot determine
        EventType::Updated
    }

    /// Extract user ID from webhook data
    fn extract_user_id(data: &Value) -> Result<String, NormalizationError> {
        // Try various common fields for user ID
        Self::extract_optional_string(data, "userId")
            .or_else(|| Self::extract_optional_string(data, "user_id"))
            .or_else(|| Self::extract_optional_string(data, "email"))
            .or_else(|| Self::extract_optional_string(data, "emailAddress"))
            .ok_or_else(|| NormalizationError::MissingField("userId/email".to_string()))
    }

    /// Extract workspace domain from email or use default
    fn extract_workspace_domain(&self, data: &Value) -> String {
        // Try to extract from email field
        if let Some(email) = Self::extract_optional_string(data, "email")
            .or_else(|| Self::extract_optional_string(data, "emailAddress"))
        {
            if let Some(domain) = email.split('@').nth(1) {
                return domain.to_string();
            }
        }

        self.default_domain.clone()
    }
}

#[async_trait]
impl PacketNormalizer for WorkspaceNormalizer {
    async fn normalize_gmail(
        &self,
        webhook_data: Value,
    ) -> Result<TypedPacket, NormalizationError> {
        // Extract Gmail message metadata
        let message_id = Self::extract_string(&webhook_data, "messageId")
            .or_else(|_| Self::extract_string(&webhook_data, "message_id"))?;

        let thread_id = Self::extract_optional_string(&webhook_data, "threadId")
            .or_else(|| Self::extract_optional_string(&webhook_data, "thread_id"));

        let history_id = webhook_data
            .get("historyId")
            .or_else(|| webhook_data.get("history_id"))
            .and_then(Value::as_u64);

        // Create packet source
        let source = PacketSource::Gmail {
            message_id: message_id.clone(),
            thread_id,
            history_id,
        };

        // Extract email payload fields
        let subject = Self::extract_string(&webhook_data, "subject")
            .unwrap_or_else(|_| "(No Subject)".to_string());

        let from = Self::extract_string(&webhook_data, "from")?;
        let to = Self::extract_string_array(&webhook_data, "to");
        let cc = Self::extract_string_array(&webhook_data, "cc");

        let body = Self::extract_optional_string(&webhook_data, "body")
            .or_else(|| Self::extract_optional_string(&webhook_data, "snippet"))
            .unwrap_or_default();

        let is_html = webhook_data
            .get("mimeType")
            .or_else(|| webhook_data.get("mime_type"))
            .and_then(Value::as_str)
            .map(|mime| mime.contains("html"))
            .unwrap_or(false);

        // Extract attachments if present
        let attachments = if let Some(attachments_data) = webhook_data.get("attachments") {
            if let Some(attachments_array) = attachments_data.as_array() {
                attachments_array
                    .iter()
                    .filter_map(|att| {
                        Some(Attachment {
                            filename: Self::extract_string(att, "filename").ok()?,
                            mime_type: Self::extract_string(att, "mimeType")
                                .or_else(|_| Self::extract_string(att, "mime_type"))
                                .ok()?,
                            size: Self::extract_u64(att, "size").unwrap_or(0),
                            attachment_id: Self::extract_string(att, "attachmentId")
                                .or_else(|_| Self::extract_string(att, "attachment_id"))
                                .ok()?,
                        })
                    })
                    .collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        let payload = PacketPayload::Email {
            subject,
            from,
            to,
            cc,
            body,
            is_html,
            attachments,
        };

        // Extract context
        let user_id = Self::extract_user_id(&webhook_data)?;
        let workspace_domain = self.extract_workspace_domain(&webhook_data);
        let event_type = Self::determine_event_type(&webhook_data);

        let context = PacketContext {
            user_id,
            workspace_domain,
            event_type,
            raw_webhook_data: Some(webhook_data),
            metadata: std::collections::HashMap::new(),
        };

        let packet = TypedPacket::new(source, payload, context);
        self.validate_packet(&packet)?;

        Ok(packet)
    }

    async fn normalize_calendar(
        &self,
        webhook_data: Value,
    ) -> Result<TypedPacket, NormalizationError> {
        // Extract Calendar event metadata
        let event_id = Self::extract_string(&webhook_data, "eventId")
            .or_else(|_| Self::extract_string(&webhook_data, "event_id"))
            .or_else(|_| Self::extract_string(&webhook_data, "id"))?;

        let calendar_id = Self::extract_string(&webhook_data, "calendarId")
            .or_else(|_| Self::extract_string(&webhook_data, "calendar_id"))?;

        let status = Self::extract_optional_string(&webhook_data, "status")
            .unwrap_or_else(|| "confirmed".to_string());

        // Create packet source
        let source = PacketSource::Calendar {
            event_id,
            calendar_id,
            status,
        };

        // Extract calendar event payload
        let title = Self::extract_string(&webhook_data, "summary")
            .or_else(|_| Self::extract_string(&webhook_data, "title"))?;

        let description = Self::extract_optional_string(&webhook_data, "description");

        let start_time = Self::extract_timestamp(&webhook_data, "startTime")
            .or_else(|_| Self::extract_timestamp(&webhook_data, "start"))?;

        let end_time = Self::extract_timestamp(&webhook_data, "endTime")
            .or_else(|_| Self::extract_timestamp(&webhook_data, "end"))?;

        let location = Self::extract_optional_string(&webhook_data, "location");
        let meeting_link = Self::extract_optional_string(&webhook_data, "hangoutLink")
            .or_else(|| Self::extract_optional_string(&webhook_data, "conferenceData"));

        // Extract attendees
        let attendees = if let Some(attendees_data) = webhook_data.get("attendees") {
            if let Some(attendees_array) = attendees_data.as_array() {
                attendees_array
                    .iter()
                    .filter_map(|att| {
                        Some(Attendee {
                            email: Self::extract_string(att, "email").ok()?,
                            display_name: Self::extract_optional_string(att, "displayName"),
                            response_status: Self::extract_optional_string(att, "responseStatus")
                                .unwrap_or_else(|| "needsAction".to_string()),
                            is_organizer: att
                                .get("organizer")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            is_optional: att
                                .get("optional")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        })
                    })
                    .collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        let payload = PacketPayload::CalendarEvent {
            title,
            description,
            start_time,
            end_time,
            location,
            attendees,
            meeting_link,
        };

        // Extract context
        let user_id = Self::extract_user_id(&webhook_data)?;
        let workspace_domain = self.extract_workspace_domain(&webhook_data);
        let event_type = Self::determine_event_type(&webhook_data);

        let context = PacketContext {
            user_id,
            workspace_domain,
            event_type,
            raw_webhook_data: Some(webhook_data),
            metadata: std::collections::HashMap::new(),
        };

        let packet = TypedPacket::new(source, payload, context);
        self.validate_packet(&packet)?;

        Ok(packet)
    }

    async fn normalize_drive(
        &self,
        webhook_data: Value,
    ) -> Result<TypedPacket, NormalizationError> {
        // Extract Drive file/folder metadata
        let file_id = Self::extract_string(&webhook_data, "fileId")
            .or_else(|_| Self::extract_string(&webhook_data, "file_id"))
            .or_else(|_| Self::extract_string(&webhook_data, "id"))?;

        let mime_type = Self::extract_string(&webhook_data, "mimeType")
            .or_else(|_| Self::extract_string(&webhook_data, "mime_type"))?;

        // Determine if it's a folder
        let is_folder = mime_type == "application/vnd.google-apps.folder";

        let item_type = if is_folder {
            DriveItemType::Folder
        } else {
            DriveItemType::File
        };

        // Create packet source
        let source = PacketSource::Drive {
            file_id,
            item_type: item_type.clone(),
            mime_type: mime_type.clone(),
        };

        // Extract common fields
        let name = Self::extract_string(&webhook_data, "name")?;
        let owner = Self::extract_string(&webhook_data, "owner").or_else(|_| {
            webhook_data
                .get("owners")
                .and_then(|o| o.as_array())
                .and_then(|arr| arr.first())
                .and_then(|o| o.get("emailAddress"))
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .ok_or_else(|| NormalizationError::MissingField("owner".to_string()))
        })?;

        let shared_with = Self::extract_string_array(&webhook_data, "sharedWith")
            .iter()
            .chain(
                webhook_data
                    .get("permissions")
                    .and_then(Value::as_array)
                    .map(|perms| {
                        perms
                            .iter()
                            .filter_map(|p| {
                                p.get("emailAddress")
                                    .and_then(Value::as_str)
                                    .map(ToString::to_string)
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
                    .iter(),
            )
            .map(Clone::clone)
            .collect();

        let last_modified = Self::extract_timestamp(&webhook_data, "modifiedTime")
            .or_else(|_| Self::extract_timestamp(&webhook_data, "modified_time"))
            .unwrap_or_else(|_| Utc::now());

        let is_trashed = webhook_data
            .get("trashed")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let parent_folders = Self::extract_string_array(&webhook_data, "parents");

        // Create appropriate payload based on item type
        let payload = if is_folder {
            PacketPayload::DriveFolder {
                name,
                owner,
                shared_with,
                last_modified,
                is_trashed,
                parent_folders,
            }
        } else {
            let size = Self::extract_u64(&webhook_data, "size").unwrap_or(0);

            PacketPayload::DriveFile {
                name,
                size,
                owner,
                shared_with,
                last_modified,
                is_trashed,
                parent_folders,
            }
        };

        // Extract context
        let user_id = Self::extract_user_id(&webhook_data)?;
        let workspace_domain = self.extract_workspace_domain(&webhook_data);
        let event_type = Self::determine_event_type(&webhook_data);

        let context = PacketContext {
            user_id,
            workspace_domain,
            event_type,
            raw_webhook_data: Some(webhook_data),
            metadata: std::collections::HashMap::new(),
        };

        let packet = TypedPacket::new(source, payload, context);
        self.validate_packet(&packet)?;

        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_normalize_gmail() {
        let normalizer = WorkspaceNormalizer::new("example.com");

        let webhook_data = json!({
            "messageId": "msg123",
            "threadId": "thread456",
            "historyId": 789,
            "subject": "Test Email",
            "from": "sender@example.com",
            "to": ["recipient@example.com"],
            "cc": [],
            "body": "Test body content",
            "mimeType": "text/plain",
            "userId": "user123",
            "email": "user@example.com"
        });

        let result = normalizer.normalize_gmail(webhook_data).await;
        assert!(result.is_ok());

        let packet = result.unwrap();
        assert_eq!(packet.packet_type(), "email");
        assert!(packet.validate().is_ok());
    }

    #[tokio::test]
    async fn test_normalize_calendar() {
        let normalizer = WorkspaceNormalizer::new("example.com");

        let webhook_data = json!({
            "eventId": "evt123",
            "calendarId": "cal456",
            "status": "confirmed",
            "summary": "Team Meeting",
            "startTime": "2026-02-09T10:00:00Z",
            "endTime": "2026-02-09T11:00:00Z",
            "userId": "user123",
            "email": "user@example.com"
        });

        let result = normalizer.normalize_calendar(webhook_data).await;
        assert!(result.is_ok());

        let packet = result.unwrap();
        assert_eq!(packet.packet_type(), "calendar_event");
        assert!(packet.validate().is_ok());
    }

    #[tokio::test]
    async fn test_normalize_drive_file() {
        let normalizer = WorkspaceNormalizer::new("example.com");

        let webhook_data = json!({
            "fileId": "file123",
            "name": "document.pdf",
            "mimeType": "application/pdf",
            "size": 1024,
            "owner": "owner@example.com",
            "modifiedTime": "2026-02-09T10:00:00Z",
            "trashed": false,
            "parents": ["folder1"],
            "userId": "user123",
            "email": "user@example.com"
        });

        let result = normalizer.normalize_drive(webhook_data).await;
        assert!(result.is_ok());

        let packet = result.unwrap();
        assert_eq!(packet.packet_type(), "drive_file");
        assert!(packet.validate().is_ok());
    }

    #[tokio::test]
    async fn test_normalize_drive_folder() {
        let normalizer = WorkspaceNormalizer::new("example.com");

        let webhook_data = json!({
            "fileId": "folder123",
            "name": "My Folder",
            "mimeType": "application/vnd.google-apps.folder",
            "owner": "owner@example.com",
            "modifiedTime": "2026-02-09T10:00:00Z",
            "trashed": false,
            "parents": [],
            "userId": "user123",
            "email": "user@example.com"
        });

        let result = normalizer.normalize_drive(webhook_data).await;
        assert!(result.is_ok());

        let packet = result.unwrap();
        assert_eq!(packet.packet_type(), "drive_folder");
        assert!(packet.validate().is_ok());
    }

    #[tokio::test]
    async fn test_normalize_auto_detection() {
        let normalizer = WorkspaceNormalizer::new("example.com");

        // Test Gmail auto-detection
        let gmail_data = json!({
            "messageId": "msg123",
            "subject": "Test",
            "from": "sender@example.com",
            "to": ["recipient@example.com"],
            "userId": "user123"
        });

        let result = normalizer.normalize_auto(gmail_data).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().packet_type(), "email");

        // Test Calendar auto-detection
        let calendar_data = json!({
            "eventId": "evt123",
            "calendarId": "cal456",
            "summary": "Meeting",
            "startTime": "2026-02-09T10:00:00Z",
            "endTime": "2026-02-09T11:00:00Z",
            "userId": "user123"
        });

        let result = normalizer.normalize_auto(calendar_data).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().packet_type(), "calendar_event");

        // Test Drive auto-detection
        let drive_data = json!({
            "fileId": "file123",
            "name": "doc.pdf",
            "mimeType": "application/pdf",
            "size": 1024,
            "owner": "owner@example.com",
            "modifiedTime": "2026-02-09T10:00:00Z",
            "userId": "user123"
        });

        let result = normalizer.normalize_auto(drive_data).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().packet_type(), "drive_file");
    }
}
