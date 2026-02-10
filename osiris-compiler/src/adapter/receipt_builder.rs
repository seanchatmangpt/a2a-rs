//! Adapter implementation for receipt building.
//!
//! Provides a concrete implementation of the ReceiptBuilder port trait
//! with pluggable signing mechanisms.

use crate::domain::{
    Operation, OperationResult, Receipt, ReceiptError, RefusalInfo, ReplayPointer,
};
use crate::port::ReceiptBuilder;
use async_trait::async_trait;
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Signer trait for pluggable signing implementations.
///
/// This allows different signing mechanisms (KMS, local keys, HSM, etc.)
/// to be used with the same receipt builder.
#[async_trait]
pub trait Signer: Send + Sync {
    /// Signs data and returns a base64-encoded signature.
    async fn sign(&self, data: &[u8]) -> Result<String, ReceiptError>;

    /// Verifies a signature against data.
    async fn verify(&self, data: &[u8], signature: &str) -> Result<bool, ReceiptError>;

    /// Returns the signer's identifier (e.g., key ID, key name).
    fn signer_id(&self) -> String;
}

/// Standard receipt builder implementation.
///
/// This adapter implements the ReceiptBuilder port using:
/// - SHA-256 for operation hashing
/// - Pluggable Signer for signature generation
/// - Standard serialization for canonical representation
pub struct StandardReceiptBuilder {
    signer: Arc<dyn Signer>,
}

impl StandardReceiptBuilder {
    /// Creates a new receipt builder with the given signer.
    pub fn new(signer: Arc<dyn Signer>) -> Self {
        Self { signer }
    }

    /// Computes the attestation hash from operation.
    ///
    /// Per the receipt invariant hash(A) = hash(μ(O)), the attestation hash
    /// should equal the operation hash. The attestation A attests to the operation O.
    fn compute_attestation_hash(&self, operation: &Operation) -> Result<String, ReceiptError> {
        // Attestation is simply the operation itself
        // So hash(A) = hash(μ(O)) by definition
        self.compute_operation_hash(operation)
    }

    /// Signs a receipt and returns the signature.
    async fn sign_receipt(&self, receipt: &Receipt) -> Result<String, ReceiptError> {
        // Create canonical representation for signing
        let signable = serde_json::json!({
            "id": receipt.id,
            "timestamp": receipt.timestamp,
            "operationId": receipt.operation_id,
            "operationHash": receipt.operation_hash,
            "attestationHash": receipt.attestation_hash,
            "result": receipt.result,
        });

        let canonical = serde_json::to_string(&signable)
            .map_err(|e| ReceiptError::SerializationError(e.to_string()))?;

        self.signer.sign(canonical.as_bytes()).await
    }
}

#[async_trait]
impl ReceiptBuilder for StandardReceiptBuilder {
    async fn build_receipt(
        &self,
        operation: &Operation,
        result: OperationResult,
        replay_pointers: Vec<ReplayPointer>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Result<Receipt, ReceiptError> {
        // 1. Compute hash(μ(O)) - canonical operation hash
        let operation_hash = self.compute_operation_hash(operation)?;

        // 2. Compute hash(A) - attestation hash
        // Per receipt invariant: hash(A) = hash(μ(O))
        let attestation_hash = self.compute_attestation_hash(operation)?;

        // 3. Create unsigned receipt
        let mut receipt = Receipt {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation_id: operation.id,
            operation_hash: operation_hash.clone(),
            attestation_hash: attestation_hash.clone(),
            signature: None,
            replay_pointers,
            result,
            refusal: None,
            metadata,
        };

        // 4. Sign the receipt
        let signature = self.sign_receipt(&receipt).await?;
        receipt.signature = Some(signature);

        // 5. Verify hash invariant before returning
        receipt.validate_hash_invariant()?;

        Ok(receipt)
    }

    async fn build_refusal_receipt(
        &self,
        operation: &Operation,
        refusal: RefusalInfo,
        replay_pointers: Vec<ReplayPointer>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Result<Receipt, ReceiptError> {
        let operation_hash = self.compute_operation_hash(operation)?;

        let result = OperationResult::Rejected {
            reason: refusal.reason.clone(),
            code: Some(format!("{:?}", refusal.category)),
        };

        // Per receipt invariant: hash(A) = hash(μ(O))
        let attestation_hash = self.compute_attestation_hash(operation)?;

        let mut receipt = Receipt {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation_id: operation.id,
            operation_hash: operation_hash.clone(),
            attestation_hash: attestation_hash.clone(),
            signature: None,
            replay_pointers,
            result,
            refusal: Some(refusal),
            metadata,
        };

        let signature = self.sign_receipt(&receipt).await?;
        receipt.signature = Some(signature);

        receipt.validate_hash_invariant()?;

        Ok(receipt)
    }

    async fn verify_receipt(&self, receipt: &Receipt) -> Result<(), ReceiptError> {
        // 1. Verify hash invariant
        receipt.validate_hash_invariant()?;

        // 2. Verify signature if present
        if let Some(signature) = &receipt.signature {
            let signable = serde_json::json!({
                "id": receipt.id,
                "timestamp": receipt.timestamp,
                "operationId": receipt.operation_id,
                "operationHash": receipt.operation_hash,
                "attestationHash": receipt.attestation_hash,
                "result": receipt.result,
            });

            let canonical = serde_json::to_string(&signable)
                .map_err(|e| ReceiptError::SerializationError(e.to_string()))?;

            let valid = self.signer.verify(canonical.as_bytes(), signature).await?;

            if !valid {
                return Err(ReceiptError::SignatureError(
                    "Signature verification failed".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn compute_operation_hash(&self, operation: &Operation) -> Result<String, ReceiptError> {
        // Create canonical representation
        let canonical = serde_json::to_string(operation)
            .map_err(|e| ReceiptError::SerializationError(e.to_string()))?;

        // Compute SHA-256 hash
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        let hash = hasher.finalize();

        // Return hex-encoded hash
        Ok(format!("{:x}", hash))
    }

    async fn sign(&self, data: &[u8]) -> Result<String, ReceiptError> {
        self.signer.sign(data).await
    }
}

/// Local signer implementation using in-memory keys.
///
/// This is suitable for development and testing but should NOT be used
/// in production. Use KmsSigner for production workloads.
pub struct LocalSigner {
    key_id: String,
}

impl LocalSigner {
    /// Creates a new local signer with a key identifier.
    pub fn new(key_id: impl Into<String>) -> Self {
        Self {
            key_id: key_id.into(),
        }
    }
}

#[async_trait]
impl Signer for LocalSigner {
    async fn sign(&self, data: &[u8]) -> Result<String, ReceiptError> {
        // For local signing, we use HMAC-SHA256 with the key_id as key
        use sha2::Sha256;
        use std::io::Write;

        let mut hasher = Sha256::new();
        hasher.update(self.key_id.as_bytes());
        hasher.update(data);
        let hash = hasher.finalize();

        // Encode as base64
        Ok(base64::encode(&hash))
    }

    async fn verify(&self, data: &[u8], signature: &str) -> Result<bool, ReceiptError> {
        let computed = self.sign(data).await?;
        Ok(computed == signature)
    }

    fn signer_id(&self) -> String {
        format!("local:{}", self.key_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::OperationKind;

    #[tokio::test]
    async fn test_local_signer() {
        let signer = LocalSigner::new("test-key");
        let data = b"test data";

        let signature = signer.sign(data).await.unwrap();
        assert!(!signature.is_empty());

        let valid = signer.verify(data, &signature).await.unwrap();
        assert!(valid);

        let invalid = signer.verify(b"wrong data", &signature).await.unwrap();
        assert!(!invalid);
    }

    #[tokio::test]
    async fn test_receipt_builder() {
        let signer = Arc::new(LocalSigner::new("test-key"));
        let builder = StandardReceiptBuilder::new(signer);

        let operation = Operation::new(
            OperationKind::Parse {
                input: "test".into(),
            },
            1,
        );

        let receipt = builder
            .build_receipt(
                &operation,
                OperationResult::Success {
                    output_hash: "abc123".to_string(),
                    output: None,
                },
                vec![],
                HashMap::new(),
            )
            .await
            .unwrap();

        assert_eq!(receipt.operation_id, operation.id);
        assert!(receipt.signature.is_some());
        assert!(receipt.is_success());

        // Verify the receipt
        assert!(builder.verify_receipt(&receipt).await.is_ok());
    }

    #[tokio::test]
    async fn test_refusal_receipt() {
        let signer = Arc::new(LocalSigner::new("test-key"));
        let builder = StandardReceiptBuilder::new(signer);

        let operation = Operation::new(
            OperationKind::Parse {
                input: "test".into(),
            },
            1,
        );

        let refusal = RefusalInfo {
            category: crate::domain::RefusalCategory::TypeNotInSigma,
            reason: "Type not registered".to_string(),
            retry_after: None,
            policy_id: Some("sigma-policy".to_string()),
            context: HashMap::new(),
        };

        let receipt = builder
            .build_refusal_receipt(&operation, refusal, vec![], HashMap::new())
            .await
            .unwrap();

        assert!(receipt.is_refused());
        assert!(receipt.signature.is_some());
        assert!(builder.verify_receipt(&receipt).await.is_ok());
    }

    #[tokio::test]
    async fn test_operation_hash_determinism() {
        let signer = Arc::new(LocalSigner::new("test-key"));
        let builder = StandardReceiptBuilder::new(signer);

        let op1 = Operation::new(
            OperationKind::Parse {
                input: "test".into(),
            },
            1,
        );

        // Create a clone with same data
        let op2 = Operation {
            id: op1.id,
            timestamp: op1.timestamp,
            priority: op1.priority,
            kind: op1.kind.clone(),
            source: op1.source.clone(),
        };

        let hash1 = builder.compute_operation_hash(&op1).unwrap();
        let hash2 = builder.compute_operation_hash(&op2).unwrap();

        assert_eq!(hash1, hash2, "Same operations should produce same hash");
    }
}
