//! Receipt domain types for proof chains.
//!
//! Receipts provide cryptographic proof of all operations in the system.
//! Each receipt follows the pattern: hash(A) = hash(μ(O))
//! where A is the attestation and μ(O) is the canonical representation of operation O.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A receipt is cryptographic proof that an operation was processed.
///
/// Receipts form a proof chain where each receipt references prior receipts,
/// creating an auditable history of all operations. The core invariant is:
/// hash(attestation) = hash(canonical(operation))
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    /// Unique receipt identifier
    pub id: Uuid,

    /// Timestamp when this receipt was issued
    pub timestamp: DateTime<Utc>,

    /// The operation this receipt attests to
    pub operation_id: Uuid,

    /// SHA-256 hash of the canonical operation representation
    /// This is hash(μ(O)) in the receipt equation
    pub operation_hash: String,

    /// Attestation hash - should equal operation_hash
    /// This is hash(A) in the receipt equation
    pub attestation_hash: String,

    /// Digital signature from KMS or other signing authority
    pub signature: Option<String>,

    /// Replay pointers: references to prior receipts this depends on
    pub replay_pointers: Vec<ReplayPointer>,

    /// Result of the operation
    pub result: OperationResult,

    /// Optional refusal information if operation was rejected
    pub refusal: Option<RefusalInfo>,

    /// Additional metadata
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// A pointer to a prior receipt in the proof chain.
///
/// Replay pointers establish causal relationships between operations
/// and enable reconstruction of the complete execution history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ReplayPointer {
    /// ID of the receipt being referenced
    pub receipt_id: Uuid,

    /// Hash of the referenced receipt for integrity
    pub receipt_hash: String,

    /// Type of dependency relationship
    pub relation: DependencyRelation,

    /// Optional description of why this dependency exists
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Types of dependency relationships between operations.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum DependencyRelation {
    /// This operation requires the referenced operation to complete first
    RequiresCompletion,

    /// This operation modifies the result of the referenced operation
    Modifies,

    /// This operation supersedes the referenced operation
    Supersedes,

    /// This operation references data from the referenced operation
    References,

    /// Custom causal relationship
    Causal,
}

/// Result of an operation execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum OperationResult {
    /// Operation completed successfully
    #[serde(rename_all = "camelCase")]
    Success {
        /// Hash of the output
        output_hash: String,

        /// Optional output data (may be stored separately)
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<serde_json::Value>,
    },

    /// Operation was rejected or failed
    #[serde(rename_all = "camelCase")]
    Rejected {
        /// Reason for rejection
        reason: String,

        /// Error code
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },

    /// Operation is pending
    #[serde(rename_all = "camelCase")]
    Pending {
        /// Expected completion time
        #[serde(skip_serializing_if = "Option::is_none")]
        expected_completion: Option<DateTime<Utc>>,
    },
}

/// Information about why an operation was refused.
///
/// Refusal information is included in receipts to provide proof that
/// an operation was properly rejected according to policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RefusalInfo {
    /// Category of refusal
    pub category: RefusalCategory,

    /// Human-readable reason
    pub reason: String,

    /// Optional retry information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<DateTime<Utc>>,

    /// Policy or guard that caused the refusal
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<String>,

    /// Additional context
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub context: HashMap<String, serde_json::Value>,
}

/// Categories of operation refusal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RefusalCategory {
    /// Type not in Σ (closed type system)
    TypeNotInSigma,

    /// H-guard violated (temporal constraint)
    GuardViolation,

    /// Schema validation failed
    SchemaViolation,

    /// Insufficient permissions
    PermissionDenied,

    /// Resource quota exceeded
    QuotaExceeded,

    /// Invalid operation state
    InvalidState,

    /// Dependency not satisfied
    DependencyNotSatisfied,

    /// Other policy violation
    PolicyViolation,
}

impl Receipt {
    /// Validates that the receipt's attestation hash matches the operation hash.
    ///
    /// This verifies the core receipt invariant: hash(A) = hash(μ(O))
    pub fn validate_hash_invariant(&self) -> Result<(), ReceiptError> {
        if self.attestation_hash != self.operation_hash {
            return Err(ReceiptError::HashMismatch {
                expected: self.operation_hash.clone(),
                actual: self.attestation_hash.clone(),
            });
        }
        Ok(())
    }

    /// Computes the canonical hash of this receipt for chaining.
    ///
    /// Returns SHA-256 hash of the receipt's canonical representation.
    pub fn compute_receipt_hash(&self) -> Result<String, ReceiptError> {
        use sha2::{Digest, Sha256};

        // Create canonical representation
        let canonical = serde_json::to_string(self)
            .map_err(|e| ReceiptError::SerializationError(e.to_string()))?;

        // Compute SHA-256 hash
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        let hash = hasher.finalize();

        // Encode as hex string
        Ok(format!("{:x}", hash))
    }

    /// Checks if this receipt represents a successful operation.
    pub fn is_success(&self) -> bool {
        matches!(self.result, OperationResult::Success { .. })
    }

    /// Checks if this receipt represents a refused operation.
    pub fn is_refused(&self) -> bool {
        self.refusal.is_some() || matches!(self.result, OperationResult::Rejected { .. })
    }
}

/// Errors that can occur during receipt operations.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ReceiptError {
    /// Hash mismatch between attestation and operation
    #[error("Receipt hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    /// Signature verification failed
    #[error("Signature verification failed: {0}")]
    SignatureError(String),

    /// Invalid receipt format
    #[error("Invalid receipt format: {0}")]
    InvalidFormat(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Missing required field
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Invalid replay pointer
    #[error("Invalid replay pointer: {0}")]
    InvalidReplayPointer(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_receipt_hash_validation() {
        let receipt = Receipt {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation_id: Uuid::new_v4(),
            operation_hash: "abc123".to_string(),
            attestation_hash: "abc123".to_string(),
            signature: None,
            replay_pointers: vec![],
            result: OperationResult::Success {
                output_hash: "def456".to_string(),
                output: None,
            },
            refusal: None,
            metadata: HashMap::new(),
        };

        assert!(receipt.validate_hash_invariant().is_ok());
        assert!(receipt.is_success());
        assert!(!receipt.is_refused());
    }

    #[test]
    fn test_receipt_hash_mismatch() {
        let receipt = Receipt {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation_id: Uuid::new_v4(),
            operation_hash: "abc123".to_string(),
            attestation_hash: "xyz789".to_string(),
            signature: None,
            replay_pointers: vec![],
            result: OperationResult::Success {
                output_hash: "def456".to_string(),
                output: None,
            },
            refusal: None,
            metadata: HashMap::new(),
        };

        assert!(receipt.validate_hash_invariant().is_err());
    }

    #[test]
    fn test_receipt_compute_hash() {
        let receipt = Receipt {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation_id: Uuid::new_v4(),
            operation_hash: "abc123".to_string(),
            attestation_hash: "abc123".to_string(),
            signature: None,
            replay_pointers: vec![],
            result: OperationResult::Success {
                output_hash: "def456".to_string(),
                output: None,
            },
            refusal: None,
            metadata: HashMap::new(),
        };

        let hash = receipt.compute_receipt_hash();
        assert!(hash.is_ok());
        assert_eq!(hash.unwrap().len(), 64); // SHA-256 hex is 64 chars
    }

    #[test]
    fn test_receipt_with_refusal() {
        let receipt = Receipt {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation_id: Uuid::new_v4(),
            operation_hash: "abc123".to_string(),
            attestation_hash: "abc123".to_string(),
            signature: None,
            replay_pointers: vec![],
            result: OperationResult::Rejected {
                reason: "Type not in Sigma".to_string(),
                code: Some("TYPE_NOT_IN_SIGMA".to_string()),
            },
            refusal: Some(RefusalInfo {
                category: RefusalCategory::TypeNotInSigma,
                reason: "Packet type not registered".to_string(),
                retry_after: None,
                policy_id: Some("sigma-policy".to_string()),
                context: HashMap::new(),
            }),
            metadata: HashMap::new(),
        };

        assert!(!receipt.is_success());
        assert!(receipt.is_refused());
    }

    #[test]
    fn test_replay_pointer_serialization() {
        let pointer = ReplayPointer {
            receipt_id: Uuid::new_v4(),
            receipt_hash: "abc123".to_string(),
            relation: DependencyRelation::RequiresCompletion,
            reason: Some("Depends on prior operation".to_string()),
        };

        let json = serde_json::to_string(&pointer).unwrap();
        let deserialized: ReplayPointer = serde_json::from_str(&json).unwrap();
        assert_eq!(pointer, deserialized);
    }
}
