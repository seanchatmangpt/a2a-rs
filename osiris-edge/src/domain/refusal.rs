//! Refusal domain types for inadmissible-before receipts
//!
//! Domain types for cryptographic refusal receipts issued when packets
//! violate admission control policies (WIP limits, auth, guards, type checks).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Cryptographic refusal receipt proving packet rejection
///
/// Issued when a packet is inadmissible-before (no negotiation, no discretionary bypass).
/// Contains cryptographic proof (hash) and structured reason code.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefusalReceipt {
    /// Unique receipt identifier
    pub receipt_id: Uuid,

    /// Rejected packet identifier
    pub packet_id: String,

    /// Timestamp of refusal
    pub timestamp: DateTime<Utc>,

    /// Reason for refusal
    pub reason: RefusalReason,

    /// Cryptographic hash proving authenticity
    ///
    /// SHA-256 hash of (packet_id, timestamp, reason, issuer)
    pub proof_hash: String,

    /// Issuer of the receipt (gateway identity)
    pub issuer: String,

    /// Optional additional context
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub context: HashMap<String, String>,

    /// Optional retry-after hint (ISO 8601 duration or timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<String>,
}

/// Reason for packet refusal
///
/// Structured reason codes for different refusal categories.
/// All refusals are inadmissible-before (no negotiation, no discretionary bypass).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "category", rename_all = "camelCase")]
pub enum RefusalReason {
    /// WIP (Work-in-Progress) capacity limit exceeded
    #[serde(rename_all = "camelCase")]
    WipCapExceeded {
        /// Current WIP count
        current: usize,
        /// WIP limit
        limit: usize,
        /// Human-readable message
        message: String,
    },

    /// Authentication failed
    #[serde(rename_all = "camelCase")]
    AuthenticationFailed {
        /// Auth error code
        code: AuthErrorCode,
        /// Human-readable message
        message: String,
    },

    /// H-guard violated (inadmissible-before constraint)
    #[serde(rename_all = "camelCase")]
    GuardFailed {
        /// Guard identifier
        guard_id: String,
        /// Guard condition that was violated
        condition: String,
        /// Human-readable message
        message: String,
    },

    /// Type check failed (packet not in Σ or schema violation)
    #[serde(rename_all = "camelCase")]
    TypeCheckFailed {
        /// Type check error code
        code: TypeCheckErrorCode,
        /// Attempted packet type
        attempted_type: String,
        /// Human-readable message
        message: String,
        /// Validation errors (for schema violations)
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        errors: Vec<String>,
    },
}

/// Authentication error codes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthErrorCode {
    /// Token is missing
    MissingToken,
    /// Token format is invalid
    InvalidTokenFormat,
    /// Token signature verification failed
    InvalidSignature,
    /// Token has expired
    TokenExpired,
    /// Token issuer is not trusted
    InvalidIssuer,
    /// Token audience is incorrect
    InvalidAudience,
    /// Required claim is missing
    MissingClaim,
    /// Authorization failed (valid token, insufficient permissions)
    InsufficientPermissions,
}

/// Type check error codes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TypeCheckErrorCode {
    /// Packet type not in closed type system Σ
    TypeNotInSigma,
    /// Packet payload violates schema
    SchemaViolation,
    /// Packet payload is malformed/unparseable
    MalformedPayload,
    /// Source-payload type mismatch
    SourcePayloadMismatch,
}

impl RefusalReceipt {
    /// Create a new refusal receipt
    #[must_use]
    pub fn new(
        packet_id: impl Into<String>,
        reason: RefusalReason,
        issuer: impl Into<String>,
    ) -> Self {
        let packet_id = packet_id.into();
        let issuer = issuer.into();
        let receipt_id = Uuid::new_v4();
        let timestamp = Utc::now();

        // Generate cryptographic proof hash
        let proof_hash = Self::compute_proof_hash(&packet_id, &timestamp, &reason, &issuer);

        Self {
            receipt_id,
            packet_id,
            timestamp,
            reason,
            proof_hash,
            issuer,
            context: HashMap::new(),
            retry_after: None,
        }
    }

    /// Create a new refusal receipt with retry-after hint
    #[must_use]
    pub fn with_retry_after(
        packet_id: impl Into<String>,
        reason: RefusalReason,
        issuer: impl Into<String>,
        retry_after: impl Into<String>,
    ) -> Self {
        let mut receipt = Self::new(packet_id, reason, issuer);
        receipt.retry_after = Some(retry_after.into());
        receipt
    }

    /// Add context to the receipt
    pub fn add_context(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.context.insert(key.into(), value.into());
    }

    /// Compute cryptographic proof hash
    ///
    /// SHA-256 hash of (packet_id || timestamp || reason_bytes || issuer)
    fn compute_proof_hash(
        packet_id: &str,
        timestamp: &DateTime<Utc>,
        reason: &RefusalReason,
        issuer: &str,
    ) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(packet_id.as_bytes());
        hasher.update(timestamp.to_rfc3339().as_bytes());

        // Serialize reason to canonical JSON for hashing
        let reason_json = serde_json::to_string(reason).unwrap_or_default();
        hasher.update(reason_json.as_bytes());

        hasher.update(issuer.as_bytes());

        let result = hasher.finalize();
        format!("{:x}", result)
    }

    /// Verify the proof hash is valid for this receipt
    #[must_use]
    pub fn verify_proof(&self) -> bool {
        let expected_hash =
            Self::compute_proof_hash(&self.packet_id, &self.timestamp, &self.reason, &self.issuer);
        self.proof_hash == expected_hash
    }

    /// Get a human-readable summary of the refusal
    #[must_use]
    pub fn summary(&self) -> String {
        match &self.reason {
            RefusalReason::WipCapExceeded {
                current,
                limit,
                message,
            } => {
                format!("WIP capacity exceeded ({}/{}): {}", current, limit, message)
            }
            RefusalReason::AuthenticationFailed { code, message } => {
                format!("Authentication failed ({:?}): {}", code, message)
            }
            RefusalReason::GuardFailed {
                guard_id, message, ..
            } => {
                format!("Guard '{}' failed: {}", guard_id, message)
            }
            RefusalReason::TypeCheckFailed { code, message, .. } => {
                format!("Type check failed ({:?}): {}", code, message)
            }
        }
    }
}

/// Helper constructors for common refusal reasons
impl RefusalReason {
    /// Create a WIP capacity exceeded refusal reason
    #[must_use]
    pub fn wip_cap_exceeded(current: usize, limit: usize) -> Self {
        Self::WipCapExceeded {
            current,
            limit,
            message: format!("WIP limit reached: {}/{} slots occupied", current, limit),
        }
    }

    /// Create an authentication failed refusal reason
    #[must_use]
    pub fn auth_failed(code: AuthErrorCode, message: impl Into<String>) -> Self {
        Self::AuthenticationFailed {
            code,
            message: message.into(),
        }
    }

    /// Create a guard failed refusal reason
    #[must_use]
    pub fn guard_failed(
        guard_id: impl Into<String>,
        condition: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::GuardFailed {
            guard_id: guard_id.into(),
            condition: condition.into(),
            message: message.into(),
        }
    }

    /// Create a type check failed refusal reason
    #[must_use]
    pub fn type_check_failed(
        code: TypeCheckErrorCode,
        attempted_type: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::TypeCheckFailed {
            code,
            attempted_type: attempted_type.into(),
            message: message.into(),
            errors: Vec::new(),
        }
    }

    /// Create a type check failed refusal reason with validation errors
    #[must_use]
    pub fn type_check_failed_with_errors(
        code: TypeCheckErrorCode,
        attempted_type: impl Into<String>,
        message: impl Into<String>,
        errors: Vec<String>,
    ) -> Self {
        Self::TypeCheckFailed {
            code,
            attempted_type: attempted_type.into(),
            message: message.into(),
            errors,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refusal_receipt_creation() {
        let reason = RefusalReason::wip_cap_exceeded(5, 5);
        let receipt = RefusalReceipt::new("pkt-123", reason, "gateway-1");

        assert_eq!(receipt.packet_id, "pkt-123");
        assert_eq!(receipt.issuer, "gateway-1");
        assert!(!receipt.proof_hash.is_empty());
        assert!(receipt.verify_proof());
    }

    #[test]
    fn test_refusal_receipt_with_retry() {
        let reason = RefusalReason::wip_cap_exceeded(5, 5);
        let receipt = RefusalReceipt::with_retry_after("pkt-123", reason, "gateway-1", "PT30S");

        assert_eq!(receipt.retry_after, Some("PT30S".to_string()));
        assert!(receipt.verify_proof());
    }

    #[test]
    fn test_refusal_receipt_context() {
        let reason = RefusalReason::wip_cap_exceeded(5, 5);
        let mut receipt = RefusalReceipt::new("pkt-123", reason, "gateway-1");

        receipt.add_context("service", "osiris-edge");
        receipt.add_context("version", "0.1.0");

        assert_eq!(
            receipt.context.get("service"),
            Some(&"osiris-edge".to_string())
        );
        assert_eq!(receipt.context.get("version"), Some(&"0.1.0".to_string()));
    }

    #[test]
    fn test_proof_hash_verification() {
        let reason = RefusalReason::auth_failed(
            AuthErrorCode::InvalidSignature,
            "JWT signature verification failed",
        );
        let receipt = RefusalReceipt::new("pkt-456", reason, "gateway-2");

        // Proof should verify
        assert!(receipt.verify_proof());

        // Create a tampered receipt
        let mut tampered = receipt.clone();
        tampered.packet_id = "pkt-999".to_string();

        // Tampered receipt should not verify
        assert!(!tampered.verify_proof());
    }

    #[test]
    fn test_refusal_reason_serialization() {
        let reason = RefusalReason::WipCapExceeded {
            current: 10,
            limit: 10,
            message: "At capacity".to_string(),
        };

        let json = serde_json::to_string(&reason).unwrap();
        let deserialized: RefusalReason = serde_json::from_str(&json).unwrap();

        assert_eq!(reason, deserialized);
    }

    #[test]
    fn test_auth_error_codes() {
        let codes = vec![
            AuthErrorCode::MissingToken,
            AuthErrorCode::InvalidTokenFormat,
            AuthErrorCode::InvalidSignature,
            AuthErrorCode::TokenExpired,
            AuthErrorCode::InvalidIssuer,
            AuthErrorCode::InvalidAudience,
            AuthErrorCode::MissingClaim,
            AuthErrorCode::InsufficientPermissions,
        ];

        for code in codes {
            let json = serde_json::to_string(&code).unwrap();
            let deserialized: AuthErrorCode = serde_json::from_str(&json).unwrap();
            assert_eq!(code, deserialized);
        }
    }

    #[test]
    fn test_type_check_error_codes() {
        let codes = vec![
            TypeCheckErrorCode::TypeNotInSigma,
            TypeCheckErrorCode::SchemaViolation,
            TypeCheckErrorCode::MalformedPayload,
            TypeCheckErrorCode::SourcePayloadMismatch,
        ];

        for code in codes {
            let json = serde_json::to_string(&code).unwrap();
            let deserialized: TypeCheckErrorCode = serde_json::from_str(&json).unwrap();
            assert_eq!(code, deserialized);
        }
    }

    #[test]
    fn test_refusal_summary() {
        let reason = RefusalReason::wip_cap_exceeded(5, 5);
        let receipt = RefusalReceipt::new("pkt-123", reason, "gateway-1");
        let summary = receipt.summary();
        assert!(summary.contains("WIP capacity exceeded"));
        assert!(summary.contains("5/5"));

        let reason =
            RefusalReason::auth_failed(AuthErrorCode::TokenExpired, "Token expired at 2026-02-09");
        let receipt = RefusalReceipt::new("pkt-456", reason, "gateway-1");
        let summary = receipt.summary();
        assert!(summary.contains("Authentication failed"));
        assert!(summary.contains("TokenExpired"));
    }

    #[test]
    fn test_guard_failed_reason() {
        let reason = RefusalReason::guard_failed(
            "precondition-auth",
            "RequiresPrior(AuthToken)",
            "Authentication token must be submitted before data packet",
        );

        let receipt = RefusalReceipt::new("pkt-data-1", reason, "gateway-1");
        assert!(receipt.verify_proof());
        assert!(
            receipt
                .summary()
                .contains("Guard 'precondition-auth' failed")
        );
    }

    #[test]
    fn test_type_check_failed_with_errors() {
        let reason = RefusalReason::type_check_failed_with_errors(
            TypeCheckErrorCode::SchemaViolation,
            "EmailPacket",
            "Packet schema validation failed",
            vec![
                "Missing required field: 'from'".to_string(),
                "Field 'to' must be non-empty array".to_string(),
            ],
        );

        if let RefusalReason::TypeCheckFailed { errors, .. } = &reason {
            assert_eq!(errors.len(), 2);
            assert!(errors[0].contains("from"));
            assert!(errors[1].contains("to"));
        } else {
            panic!("Expected TypeCheckFailed variant");
        }
    }
}
