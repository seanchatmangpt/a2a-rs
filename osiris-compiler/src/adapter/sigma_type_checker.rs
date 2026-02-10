//! SigmaTypeChecker adapter.
//!
//! Implements the TypeChecker port trait with strict validation against Σ.

use crate::domain::{Packet, PacketType, RefusalReason, RefusalReceipt, Sigma, TypeCheckResult};
use crate::port::TypeChecker;
use async_trait::async_trait;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Error types for type checking.
#[derive(Debug, thiserror::Error)]
pub enum TypeCheckError {
    #[error("Packet type not in Σ: {packet_type:?}")]
    TypeNotInSigma { packet_type: PacketType },

    #[error("Schema violation for {packet_type:?}: {errors:?}")]
    SchemaViolation {
        packet_type: PacketType,
        errors: Vec<String>,
    },

    #[error("Lock error: {0}")]
    LockError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),
}

/// Adapter implementing the TypeChecker port.
///
/// This implementation:
/// - Maintains an in-memory Σ (type system registry)
/// - Rejects any packet not in Σ with zero discretion
/// - Validates packet payloads against registered schemas
/// - Produces refusal receipts for all violations
#[derive(Debug, Clone)]
pub struct SigmaTypeChecker {
    /// The closed type system Σ
    sigma: Arc<RwLock<Sigma>>,
    /// Whether to enforce strict schema validation
    strict_schema_validation: bool,
}

impl SigmaTypeChecker {
    /// Creates a new type checker with an empty Σ.
    pub fn new() -> Self {
        Self {
            sigma: Arc::new(RwLock::new(Sigma::new())),
            strict_schema_validation: true,
        }
    }

    /// Creates a new type checker with a pre-populated Σ.
    pub fn with_sigma(sigma: Sigma) -> Self {
        Self {
            sigma: Arc::new(RwLock::new(sigma)),
            strict_schema_validation: true,
        }
    }

    /// Sets whether to enforce strict schema validation.
    pub fn with_strict_validation(mut self, strict: bool) -> Self {
        self.strict_schema_validation = strict;
        self
    }

    /// Validates packet payload against schema if one is registered.
    async fn validate_schema(&self, packet: &Packet) -> Result<Vec<String>, TypeCheckError> {
        let sigma = self.sigma.read().await;

        // If there's a schema registered, validate against it
        if let Some(schema) = sigma.schemas.get(&packet.packet_type) {
            // Simple field presence validation
            let mut errors = Vec::new();

            // Check required fields
            if let Some(obj) = packet.payload.as_object() {
                for required_field in &schema.required_fields {
                    if !obj.contains_key(required_field) {
                        errors.push(format!("Missing required field: {}", required_field));
                    }
                }
            } else if !schema.required_fields.is_empty() {
                errors.push("Payload must be an object".to_string());
            }

            Ok(errors)
        } else {
            // No schema registered, consider valid
            Ok(Vec::new())
        }
    }

    /// Creates a refusal receipt for a rejected packet.
    #[cfg(feature = "timestamps")]
    pub fn create_refusal_receipt(
        packet_id: String,
        reason: RefusalReason,
    ) -> Result<RefusalReceipt, Box<dyn Error + Send + Sync>> {
        use uuid::Uuid;

        Ok(RefusalReceipt {
            receipt_id: format!("refusal-{}", Uuid::new_v4()),
            packet_id,
            reason,
            timestamp: chrono::Utc::now(),
            signature: None,
            context: std::collections::HashMap::new(),
        })
    }

    #[cfg(not(feature = "timestamps"))]
    pub fn create_refusal_receipt(
        packet_id: String,
        reason: RefusalReason,
    ) -> Result<RefusalReceipt, Box<dyn Error + Send + Sync>> {
        use uuid::Uuid;

        Ok(RefusalReceipt {
            receipt_id: format!("refusal-{}", Uuid::new_v4()),
            packet_id,
            reason,
            timestamp: "".to_string(),
            signature: None,
            context: std::collections::HashMap::new(),
        })
    }
}

impl Default for SigmaTypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypeChecker for SigmaTypeChecker {
    async fn is_admissible(&self, packet: &Packet) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let sigma = self.sigma.read().await;
        Ok(sigma.is_admissible(&packet.packet_type))
    }

    async fn check(
        &self,
        packet: &Packet,
    ) -> Result<TypeCheckResult, Box<dyn Error + Send + Sync>> {
        // Step 1: Check if packet type is in Σ
        if !self.is_admissible(packet).await? {
            return Ok(TypeCheckResult::TypeNotInSigma {
                packet_id: packet.id.clone(),
                attempted_type: packet.packet_type.clone(),
                reason: format!(
                    "Packet type {} is not registered in Σ (closed type system)",
                    packet.packet_type.fqn()
                ),
            });
        }

        // Step 2: Validate schema if strict validation is enabled
        if self.strict_schema_validation {
            let errors = self.validate_schema(packet).await?;
            if !errors.is_empty() {
                return Ok(TypeCheckResult::SchemaViolation {
                    packet_id: packet.id.clone(),
                    packet_type: packet.packet_type.clone(),
                    errors,
                });
            }
        }

        // Step 3: Packet is valid
        Ok(TypeCheckResult::Valid {
            packet_id: packet.id.clone(),
            packet_type: packet.packet_type.clone(),
        })
    }

    async fn get_sigma(&self) -> Result<Sigma, Box<dyn Error + Send + Sync>> {
        let sigma = self.sigma.read().await;
        Ok(sigma.clone())
    }

    async fn update_sigma(&mut self, sigma: Sigma) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut current = self.sigma.write().await;
        *current = sigma;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_reject_packet_not_in_sigma() {
        let checker = SigmaTypeChecker::new();

        let packet = Packet {
            id: "pkt-1".to_string(),
            packet_type: PacketType::new("test", "UnknownType", "1.0"),
            payload: serde_json::json!({}),
            metadata: HashMap::new(),
        };

        let result = checker.check(&packet).await.unwrap();
        assert!(matches!(result, TypeCheckResult::TypeNotInSigma { .. }));
    }

    #[tokio::test]
    async fn test_accept_packet_in_sigma() {
        let mut sigma = Sigma::new();
        sigma.register(PacketType::new("test", "ValidType", "1.0"));

        let checker = SigmaTypeChecker::with_sigma(sigma);

        let packet = Packet {
            id: "pkt-1".to_string(),
            packet_type: PacketType::new("test", "ValidType", "1.0"),
            payload: serde_json::json!({}),
            metadata: HashMap::new(),
        };

        let result = checker.check(&packet).await.unwrap();
        assert!(matches!(result, TypeCheckResult::Valid { .. }));
    }

    #[tokio::test]
    async fn test_schema_validation() {
        use crate::domain::TypeSchema;

        let mut sigma = Sigma::new();
        let packet_type = PacketType::new("test", "RequiredFields", "1.0");
        sigma.register_with_schema(
            packet_type.clone(),
            TypeSchema {
                schema: serde_json::json!({}),
                required_fields: vec!["name".to_string(), "value".to_string()],
            },
        );

        let checker = SigmaTypeChecker::with_sigma(sigma);

        // Missing required fields
        let packet = Packet {
            id: "pkt-1".to_string(),
            packet_type: packet_type.clone(),
            payload: serde_json::json!({"name": "test"}),
            metadata: HashMap::new(),
        };

        let result = checker.check(&packet).await.unwrap();
        assert!(matches!(result, TypeCheckResult::SchemaViolation { .. }));

        // All required fields present
        let packet2 = Packet {
            id: "pkt-2".to_string(),
            packet_type,
            payload: serde_json::json!({"name": "test", "value": 42}),
            metadata: HashMap::new(),
        };

        let result2 = checker.check(&packet2).await.unwrap();
        assert!(matches!(result2, TypeCheckResult::Valid { .. }));
    }

    #[tokio::test]
    async fn test_update_sigma() {
        let mut checker = SigmaTypeChecker::new();

        let packet_type = PacketType::new("test", "NewType", "1.0");
        let packet = Packet {
            id: "pkt-1".to_string(),
            packet_type: packet_type.clone(),
            payload: serde_json::json!({}),
            metadata: HashMap::new(),
        };

        // Initially not admissible
        assert!(!checker.is_admissible(&packet).await.unwrap());

        // Update Σ
        let mut new_sigma = Sigma::new();
        new_sigma.register(packet_type);
        checker.update_sigma(new_sigma).await.unwrap();

        // Now admissible
        assert!(checker.is_admissible(&packet).await.unwrap());
    }
}
