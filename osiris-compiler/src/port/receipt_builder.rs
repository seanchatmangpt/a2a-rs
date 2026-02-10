//! Port trait for receipt building with cryptographic signing.
//!
//! This port defines the interface for building receipts with signatures
//! from various signing authorities (KMS, local keys, etc.).

use crate::domain::{
    Operation, OperationResult, Receipt, ReceiptError, RefusalInfo, ReplayPointer,
};
use async_trait::async_trait;
use std::collections::HashMap;
use uuid::Uuid;

/// Port trait for building signed receipts.
///
/// Implementations of this trait can use different signing mechanisms
/// (Cloud KMS, local keys, HSMs, etc.) while maintaining the same interface.
#[async_trait]
pub trait ReceiptBuilder: Send + Sync {
    /// Builds a receipt for a successful operation.
    ///
    /// This method:
    /// 1. Computes hash(μ(O)) - the canonical hash of the operation
    /// 2. Creates an attestation with hash(A)
    /// 3. Verifies hash(A) = hash(μ(O))
    /// 4. Signs the receipt using the configured signing authority
    /// 5. Returns the complete receipt with signature
    ///
    /// # Arguments
    /// * `operation` - The operation to create a receipt for
    /// * `result` - The result of executing the operation
    /// * `replay_pointers` - References to prior receipts this depends on
    /// * `metadata` - Additional metadata to include in the receipt
    ///
    /// # Returns
    /// A signed receipt proving the operation was processed
    async fn build_receipt(
        &self,
        operation: &Operation,
        result: OperationResult,
        replay_pointers: Vec<ReplayPointer>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Result<Receipt, ReceiptError>;

    /// Builds a refusal receipt for a rejected operation.
    ///
    /// Refusal receipts provide cryptographic proof that an operation
    /// was properly rejected according to policy (Σ violations, H-guard
    /// failures, etc.).
    ///
    /// # Arguments
    /// * `operation` - The operation that was rejected
    /// * `refusal` - Information about why the operation was refused
    /// * `replay_pointers` - References to prior receipts (if any)
    /// * `metadata` - Additional metadata
    ///
    /// # Returns
    /// A signed refusal receipt
    async fn build_refusal_receipt(
        &self,
        operation: &Operation,
        refusal: RefusalInfo,
        replay_pointers: Vec<ReplayPointer>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Result<Receipt, ReceiptError>;

    /// Verifies a receipt's signature and hash invariant.
    ///
    /// This checks:
    /// 1. hash(A) = hash(μ(O)) - the core receipt invariant
    /// 2. The signature is valid and from a trusted authority
    ///
    /// # Arguments
    /// * `receipt` - The receipt to verify
    ///
    /// # Returns
    /// `Ok(())` if the receipt is valid, `Err` otherwise
    async fn verify_receipt(&self, receipt: &Receipt) -> Result<(), ReceiptError>;

    /// Computes the canonical hash of an operation.
    ///
    /// This is hash(μ(O)) in the receipt equation, where μ(O) is
    /// the canonical serialization of operation O.
    ///
    /// # Arguments
    /// * `operation` - The operation to hash
    ///
    /// # Returns
    /// SHA-256 hash of the canonical operation representation
    fn compute_operation_hash(&self, operation: &Operation) -> Result<String, ReceiptError>;

    /// Signs data using the configured signing authority.
    ///
    /// # Arguments
    /// * `data` - The data to sign
    ///
    /// # Returns
    /// Base64-encoded signature
    async fn sign(&self, data: &[u8]) -> Result<String, ReceiptError>;
}

/// Port trait for receipt storage.
///
/// Implementations can store receipts in various backends
/// (Cloud Storage, S3, local filesystem, databases, etc.).
#[async_trait]
pub trait ReceiptStorage: Send + Sync {
    /// Stores a receipt.
    ///
    /// # Arguments
    /// * `receipt` - The receipt to store
    ///
    /// # Returns
    /// The storage location/key where the receipt was stored
    async fn store_receipt(&self, receipt: &Receipt) -> Result<String, ReceiptError>;

    /// Retrieves a receipt by ID.
    ///
    /// # Arguments
    /// * `receipt_id` - The ID of the receipt to retrieve
    ///
    /// # Returns
    /// The receipt if found
    async fn get_receipt(&self, receipt_id: Uuid) -> Result<Receipt, ReceiptError>;

    /// Retrieves all receipts for a given operation.
    ///
    /// # Arguments
    /// * `operation_id` - The operation ID to query
    ///
    /// # Returns
    /// All receipts associated with the operation
    async fn get_receipts_for_operation(
        &self,
        operation_id: Uuid,
    ) -> Result<Vec<Receipt>, ReceiptError>;

    /// Lists receipts in a time range.
    ///
    /// # Arguments
    /// * `start` - Start time (inclusive)
    /// * `end` - End time (inclusive)
    ///
    /// # Returns
    /// All receipts in the time range
    async fn list_receipts(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Receipt>, ReceiptError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::OperationKind;

    // Mock implementation for testing
    struct MockReceiptBuilder;

    #[async_trait]
    impl ReceiptBuilder for MockReceiptBuilder {
        async fn build_receipt(
            &self,
            operation: &Operation,
            result: OperationResult,
            replay_pointers: Vec<ReplayPointer>,
            metadata: HashMap<String, serde_json::Value>,
        ) -> Result<Receipt, ReceiptError> {
            let operation_hash = self.compute_operation_hash(operation)?;
            Ok(Receipt {
                id: Uuid::new_v4(),
                timestamp: chrono::Utc::now(),
                operation_id: operation.id,
                operation_hash: operation_hash.clone(),
                attestation_hash: operation_hash,
                signature: Some("mock_signature".to_string()),
                replay_pointers,
                result,
                refusal: None,
                metadata,
            })
        }

        async fn build_refusal_receipt(
            &self,
            operation: &Operation,
            refusal: RefusalInfo,
            replay_pointers: Vec<ReplayPointer>,
            metadata: HashMap<String, serde_json::Value>,
        ) -> Result<Receipt, ReceiptError> {
            let operation_hash = self.compute_operation_hash(operation)?;
            Ok(Receipt {
                id: Uuid::new_v4(),
                timestamp: chrono::Utc::now(),
                operation_id: operation.id,
                operation_hash: operation_hash.clone(),
                attestation_hash: operation_hash,
                signature: Some("mock_signature".to_string()),
                replay_pointers,
                result: OperationResult::Rejected {
                    reason: refusal.reason.clone(),
                    code: None,
                },
                refusal: Some(refusal),
                metadata,
            })
        }

        async fn verify_receipt(&self, receipt: &Receipt) -> Result<(), ReceiptError> {
            receipt.validate_hash_invariant()
        }

        fn compute_operation_hash(&self, operation: &Operation) -> Result<String, ReceiptError> {
            use sha2::{Digest, Sha256};

            let canonical = serde_json::to_string(operation)
                .map_err(|e| ReceiptError::SerializationError(e.to_string()))?;

            let mut hasher = Sha256::new();
            hasher.update(canonical.as_bytes());
            let hash = hasher.finalize();

            Ok(format!("{:x}", hash))
        }

        async fn sign(&self, _data: &[u8]) -> Result<String, ReceiptError> {
            Ok("mock_signature".to_string())
        }
    }

    #[tokio::test]
    async fn test_mock_receipt_builder() {
        let builder = MockReceiptBuilder;
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
        assert!(builder.verify_receipt(&receipt).await.is_ok());
    }
}
