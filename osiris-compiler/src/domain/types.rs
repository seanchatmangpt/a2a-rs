//! Domain types for Osiris closed type system Σ and H-guards.
//!
//! This module defines the core types for packet verification:
//! - Σ (Sigma): The closed type system of admissible packet types
//! - H-guards: Explicit inadmissible-before temporal constraints
//! - Refusal receipts: Cryptographic proofs of rejection

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A packet type identifier in the closed type system Σ.
///
/// Only packets with registered `PacketType` values are admissible.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacketType {
    /// Namespace (e.g., "a2a", "osiris", "custom")
    pub namespace: String,
    /// Type name (e.g., "Message", "Task", "AuthRequest")
    pub name: String,
    /// Version (e.g., "0.3.0")
    pub version: String,
}

impl PacketType {
    /// Creates a new packet type identifier.
    pub fn new(
        namespace: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
            version: version.into(),
        }
    }

    /// Returns the fully qualified type identifier.
    pub fn fqn(&self) -> String {
        format!("{}.{}.{}", self.namespace, self.name, self.version)
    }
}

/// The closed type system Σ (Sigma).
///
/// Defines the complete set of admissible packet types.
/// Any packet not in Σ MUST be rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sigma {
    /// Set of admissible packet types
    pub admissible_types: HashSet<PacketType>,
    /// Optional schema definitions for each type
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub schemas: HashMap<PacketType, TypeSchema>,
}

impl Sigma {
    /// Creates an empty type system.
    pub fn new() -> Self {
        Self {
            admissible_types: HashSet::new(),
            schemas: HashMap::new(),
        }
    }

    /// Registers a packet type as admissible.
    pub fn register(&mut self, packet_type: PacketType) {
        self.admissible_types.insert(packet_type);
    }

    /// Registers a packet type with schema validation.
    pub fn register_with_schema(&mut self, packet_type: PacketType, schema: TypeSchema) {
        self.admissible_types.insert(packet_type.clone());
        self.schemas.insert(packet_type, schema);
    }

    /// Checks if a packet type is admissible.
    pub fn is_admissible(&self, packet_type: &PacketType) -> bool {
        self.admissible_types.contains(packet_type)
    }
}

impl Default for Sigma {
    fn default() -> Self {
        Self::new()
    }
}

/// Schema definition for a packet type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeSchema {
    /// JSON Schema or other schema representation
    pub schema: serde_json::Value,
    /// Required fields
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub required_fields: Vec<String>,
}

/// H-guard: Explicit inadmissible-before temporal constraint.
///
/// Represents a gate that blocks packets until a precondition is satisfied.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HGuard {
    /// Unique guard identifier
    pub id: String,
    /// Packet type this guard applies to
    pub packet_type: PacketType,
    /// Condition that must be satisfied
    pub condition: GuardCondition,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Condition for an H-guard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum GuardCondition {
    /// Requires a specific packet type to have been processed first
    #[serde(rename_all = "camelCase")]
    RequiresPrior {
        packet_type: PacketType,
        #[serde(skip_serializing_if = "Option::is_none")]
        packet_id: Option<String>,
    },
    /// Requires a temporal delay since some event
    #[serde(rename_all = "camelCase")]
    TemporalDelay {
        #[cfg(feature = "timestamps")]
        #[serde(skip_serializing_if = "Option::is_none")]
        not_before: Option<chrono::DateTime<chrono::Utc>>,
        #[cfg(not(feature = "timestamps"))]
        #[serde(skip_serializing_if = "Option::is_none")]
        not_before: Option<String>,
    },
    /// Requires specific state to be reached
    #[serde(rename_all = "camelCase")]
    StateRequirement {
        required_state: String,
        #[serde(skip_serializing_if = "HashMap::is_empty", default)]
        context: HashMap<String, serde_json::Value>,
    },
    /// Custom condition with arbitrary logic
    #[serde(rename_all = "camelCase")]
    Custom {
        predicate: String,
        #[serde(skip_serializing_if = "HashMap::is_empty", default)]
        parameters: HashMap<String, serde_json::Value>,
    },
}

/// Result of H-guard evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum GuardEvaluationResult {
    /// Guard is satisfied; packet may proceed
    #[serde(rename_all = "camelCase")]
    Satisfied { guard_id: String },
    /// Guard is not satisfied; packet is inadmissible
    #[serde(rename_all = "camelCase")]
    Violated {
        guard_id: String,
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        retry_after: Option<String>,
    },
}

/// A packet submitted for verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Packet {
    /// Unique packet identifier
    pub id: String,
    /// Packet type (must be in Σ)
    pub packet_type: PacketType,
    /// Packet payload
    pub payload: serde_json::Value,
    /// Optional metadata
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Result of type checking a packet against Σ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "camelCase")]
pub enum TypeCheckResult {
    /// Packet type is in Σ and valid
    #[serde(rename_all = "camelCase")]
    Valid {
        packet_id: String,
        packet_type: PacketType,
    },
    /// Packet type is not in Σ (inadmissible)
    #[serde(rename_all = "camelCase")]
    TypeNotInSigma {
        packet_id: String,
        attempted_type: PacketType,
        reason: String,
    },
    /// Packet type is in Σ but payload is malformed
    #[serde(rename_all = "camelCase")]
    SchemaViolation {
        packet_id: String,
        packet_type: PacketType,
        errors: Vec<String>,
    },
}

/// Refusal receipt: cryptographic proof of packet rejection.
///
/// Issued when a packet violates Σ or H-guard constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefusalReceipt {
    /// Unique receipt identifier
    pub receipt_id: String,
    /// Rejected packet identifier
    pub packet_id: String,
    /// Reason for refusal
    pub reason: RefusalReason,
    /// Timestamp of refusal
    #[cfg(feature = "timestamps")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    #[cfg(not(feature = "timestamps"))]
    pub timestamp: String,
    /// Optional signature/proof
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Optional additional context
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub context: HashMap<String, serde_json::Value>,
}

/// Reason for packet refusal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "category", rename_all = "camelCase")]
pub enum RefusalReason {
    /// Packet type not in Σ
    #[serde(rename_all = "camelCase")]
    TypeNotInSigma {
        attempted_type: PacketType,
        message: String,
    },
    /// H-guard violated
    #[serde(rename_all = "camelCase")]
    GuardViolation {
        guard_id: String,
        guard_condition: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        retry_after: Option<String>,
    },
    /// Schema validation failed
    #[serde(rename_all = "camelCase")]
    SchemaViolation {
        packet_type: PacketType,
        errors: Vec<String>,
    },
    /// Q invariant violated
    #[serde(rename_all = "camelCase")]
    InvariantViolation {
        invariant_ids: Vec<String>,
        message: String,
    },
}

#[cfg(feature = "builders")]
impl RefusalReceipt {
    /// Creates a builder for RefusalReceipt.
    pub fn builder() -> RefusalReceiptBuilder {
        RefusalReceiptBuilder::default()
    }
}

#[cfg(feature = "builders")]
#[derive(Default)]
pub struct RefusalReceiptBuilder {
    receipt_id: Option<String>,
    packet_id: Option<String>,
    reason: Option<RefusalReason>,
    #[cfg(feature = "timestamps")]
    timestamp: Option<chrono::DateTime<chrono::Utc>>,
    #[cfg(not(feature = "timestamps"))]
    timestamp: Option<String>,
    signature: Option<String>,
    context: HashMap<String, serde_json::Value>,
}

#[cfg(feature = "builders")]
impl RefusalReceiptBuilder {
    pub fn receipt_id(mut self, receipt_id: impl Into<String>) -> Self {
        self.receipt_id = Some(receipt_id.into());
        self
    }

    pub fn packet_id(mut self, packet_id: impl Into<String>) -> Self {
        self.packet_id = Some(packet_id.into());
        self
    }

    pub fn reason(mut self, reason: RefusalReason) -> Self {
        self.reason = Some(reason);
        self
    }

    #[cfg(feature = "timestamps")]
    pub fn timestamp(mut self, timestamp: chrono::DateTime<chrono::Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    #[cfg(not(feature = "timestamps"))]
    pub fn timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }

    pub fn signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }

    pub fn context(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.context.insert(key.into(), value);
        self
    }

    pub fn build(self) -> Result<RefusalReceipt, &'static str> {
        Ok(RefusalReceipt {
            receipt_id: self.receipt_id.ok_or("receipt_id is required")?,
            packet_id: self.packet_id.ok_or("packet_id is required")?,
            reason: self.reason.ok_or("reason is required")?,
            timestamp: self.timestamp.ok_or("timestamp is required")?,
            signature: self.signature,
            context: self.context,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_type_fqn() {
        let pt = PacketType::new("a2a", "Message", "0.3.0");
        assert_eq!(pt.fqn(), "a2a.Message.0.3.0");
    }

    #[test]
    fn test_sigma_registration() {
        let mut sigma = Sigma::new();
        let pt = PacketType::new("test", "Type1", "1.0");

        assert!(!sigma.is_admissible(&pt));
        sigma.register(pt.clone());
        assert!(sigma.is_admissible(&pt));
    }

    #[test]
    fn test_type_check_result_serialization() {
        let result = TypeCheckResult::Valid {
            packet_id: "pkt-123".to_string(),
            packet_type: PacketType::new("a2a", "Message", "0.3.0"),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: TypeCheckResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }

    #[cfg(feature = "builders")]
    #[test]
    fn test_refusal_receipt_builder() {
        let receipt = RefusalReceipt::builder()
            .receipt_id("rcpt-123")
            .packet_id("pkt-456")
            .reason(RefusalReason::TypeNotInSigma {
                attempted_type: PacketType::new("bad", "Type", "1.0"),
                message: "Type not registered".to_string(),
            })
            .timestamp("2026-02-09T12:00:00Z")
            .build()
            .unwrap();

        assert_eq!(receipt.receipt_id, "rcpt-123");
        assert_eq!(receipt.packet_id, "pkt-456");
    }
}
