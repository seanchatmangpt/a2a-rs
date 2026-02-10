//! Port trait for packet normalization
//!
//! Defines the interface for converting external interactions into typed packets.

use crate::domain::packet::TypedPacket;
use async_trait::async_trait;
use serde_json::Value;

/// Error type for packet normalization operations
#[derive(Debug, thiserror::Error)]
pub enum NormalizationError {
    /// Invalid webhook payload format
    #[error("Invalid webhook payload: {0}")]
    InvalidPayload(String),

    /// Missing required field in webhook data
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Unsupported webhook type
    #[error("Unsupported webhook type: {0}")]
    UnsupportedType(String),

    /// JSON parsing error
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Type system validation error
    #[error("Type system validation error: {0}")]
    ValidationError(String),

    /// External API error
    #[error("External API error: {0}")]
    ApiError(String),
}

/// Port trait for normalizing external interactions into typed packets
///
/// Implementations convert vendor-specific webhook payloads into standardized
/// TypedPacket instances that conform to the closed type system Σ.
#[async_trait]
pub trait PacketNormalizer: Send + Sync {
    /// Normalize a Gmail webhook payload into a typed packet
    ///
    /// # Arguments
    /// * `webhook_data` - Raw Gmail API webhook/push notification payload
    ///
    /// # Errors
    /// Returns `NormalizationError` if payload is invalid or cannot be normalized
    async fn normalize_gmail(&self, webhook_data: Value)
    -> Result<TypedPacket, NormalizationError>;

    /// Normalize a Calendar webhook payload into a typed packet
    ///
    /// # Arguments
    /// * `webhook_data` - Raw Calendar API webhook/push notification payload
    ///
    /// # Errors
    /// Returns `NormalizationError` if payload is invalid or cannot be normalized
    async fn normalize_calendar(
        &self,
        webhook_data: Value,
    ) -> Result<TypedPacket, NormalizationError>;

    /// Normalize a Drive webhook payload into a typed packet
    ///
    /// # Arguments
    /// * `webhook_data` - Raw Drive API webhook/push notification payload
    ///
    /// # Errors
    /// Returns `NormalizationError` if payload is invalid or cannot be normalized
    async fn normalize_drive(&self, webhook_data: Value)
    -> Result<TypedPacket, NormalizationError>;

    /// Normalize any Google Workspace webhook by detecting its type
    ///
    /// # Arguments
    /// * `webhook_data` - Raw webhook payload from any Google Workspace service
    ///
    /// # Errors
    /// Returns `NormalizationError` if payload type cannot be detected or normalized
    async fn normalize_auto(&self, webhook_data: Value) -> Result<TypedPacket, NormalizationError> {
        // Default implementation attempts to detect type from payload structure
        if webhook_data.get("message_id").is_some() || webhook_data.get("messageId").is_some() {
            self.normalize_gmail(webhook_data).await
        } else if webhook_data.get("event_id").is_some() || webhook_data.get("eventId").is_some() {
            self.normalize_calendar(webhook_data).await
        } else if webhook_data.get("file_id").is_some() || webhook_data.get("fileId").is_some() {
            self.normalize_drive(webhook_data).await
        } else {
            Err(NormalizationError::UnsupportedType(
                "Could not detect webhook type from payload".to_string(),
            ))
        }
    }

    /// Validate a packet conforms to type system Σ constraints
    ///
    /// # Arguments
    /// * `packet` - Typed packet to validate
    ///
    /// # Errors
    /// Returns `NormalizationError::ValidationError` if packet violates constraints
    fn validate_packet(&self, packet: &TypedPacket) -> Result<(), NormalizationError> {
        packet
            .validate()
            .map_err(NormalizationError::ValidationError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::packet::{EventType, PacketContext, PacketPayload, PacketSource};
    use serde_json::json;

    struct MockNormalizer;

    #[async_trait]
    impl PacketNormalizer for MockNormalizer {
        async fn normalize_gmail(&self, _data: Value) -> Result<TypedPacket, NormalizationError> {
            let source = PacketSource::Gmail {
                message_id: "msg123".to_string(),
                thread_id: None,
                history_id: None,
            };

            let payload = PacketPayload::Email {
                subject: "Test".to_string(),
                from: "test@example.com".to_string(),
                to: vec![],
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

            Ok(TypedPacket::new(source, payload, context))
        }

        async fn normalize_calendar(
            &self,
            _data: Value,
        ) -> Result<TypedPacket, NormalizationError> {
            unimplemented!()
        }

        async fn normalize_drive(&self, _data: Value) -> Result<TypedPacket, NormalizationError> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_normalize_auto_gmail() {
        let normalizer = MockNormalizer;
        let data = json!({
            "message_id": "msg123",
            "subject": "Test"
        });

        let result = normalizer.normalize_auto(data).await;
        assert!(result.is_ok());
    }
}
