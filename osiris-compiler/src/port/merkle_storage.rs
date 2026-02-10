//! Port trait for Merkle tree-based receipt storage.
//!
//! Provides efficient verification proofs (O(log N)) and incremental updates
//! for receipt storage using Merkle trees.

use crate::domain::{MerkleError, MerkleProof, MerkleRoot, MerkleTree, Receipt, ReceiptError};
use async_trait::async_trait;
use uuid::Uuid;

/// Port trait for Merkle tree-based receipt storage.
///
/// This trait defines operations for storing receipts in a Merkle tree
/// structure, enabling efficient verification proofs and tamper detection.
#[async_trait]
pub trait MerkleReceiptStorage: Send + Sync {
    /// Stores a receipt and updates the Merkle tree.
    ///
    /// This operation:
    /// 1. Computes the receipt hash
    /// 2. Adds the receipt as a leaf to the Merkle tree
    /// 3. Rebuilds the tree and updates the root hash
    /// 4. Persists both the receipt and updated tree state
    ///
    /// # Arguments
    /// * `receipt` - The receipt to store
    ///
    /// # Returns
    /// The new root hash after adding the receipt
    async fn store_receipt(&self, receipt: &Receipt) -> Result<MerkleRoot, ReceiptError>;

    /// Batch stores multiple receipts.
    ///
    /// More efficient than calling store_receipt multiple times as it only
    /// rebuilds the tree once.
    ///
    /// # Arguments
    /// * `receipts` - The receipts to store
    ///
    /// # Returns
    /// The new root hash after adding all receipts
    async fn store_receipts(&self, receipts: &[Receipt]) -> Result<MerkleRoot, ReceiptError>;

    /// Retrieves a receipt by ID.
    ///
    /// # Arguments
    /// * `receipt_id` - The ID of the receipt to retrieve
    ///
    /// # Returns
    /// The receipt if found
    async fn get_receipt(&self, receipt_id: Uuid) -> Result<Receipt, ReceiptError>;

    /// Generates a Merkle proof for a receipt.
    ///
    /// The proof contains the sibling hashes needed to verify that the receipt
    /// is included in the tree. Verification complexity is O(log N).
    ///
    /// # Arguments
    /// * `receipt_id` - The ID of the receipt to prove
    ///
    /// # Returns
    /// A Merkle proof that can be verified independently
    async fn generate_proof(&self, receipt_id: Uuid) -> Result<MerkleProof, MerkleError>;

    /// Verifies a Merkle proof against the current tree.
    ///
    /// # Arguments
    /// * `proof` - The proof to verify
    ///
    /// # Returns
    /// `Ok(())` if the proof is valid, `Err` otherwise
    async fn verify_proof(&self, proof: &MerkleProof) -> Result<(), MerkleError>;

    /// Returns the current root hash of the Merkle tree.
    ///
    /// The root hash changes every time a receipt is added, providing
    /// tamper detection for the entire history.
    ///
    /// # Returns
    /// The current root, or None if the tree is empty
    async fn get_root(&self) -> Result<Option<MerkleRoot>, MerkleError>;

    /// Returns the complete Merkle tree.
    ///
    /// This is useful for debugging, auditing, or creating snapshots.
    /// For large trees, consider using pagination or streaming.
    ///
    /// # Returns
    /// The complete tree structure
    async fn get_tree(&self) -> Result<MerkleTree, MerkleError>;

    /// Returns the number of receipts in the tree.
    async fn receipt_count(&self) -> Result<usize, MerkleError>;

    /// Verifies the integrity of the entire tree.
    ///
    /// This checks that:
    /// 1. All internal node hashes are correctly computed
    /// 2. The root hash matches the computed value
    /// 3. All receipts are correctly indexed
    ///
    /// # Returns
    /// `Ok(())` if the tree is valid, `Err` with details otherwise
    async fn verify_tree_integrity(&self) -> Result<(), MerkleError>;
}

/// Port trait for persistent Merkle tree backends.
///
/// Implementations can use various storage backends (SQLite, Postgres,
/// Cloud Storage, etc.) while maintaining the same interface.
#[async_trait]
pub trait PersistentMerkleBackend: Send + Sync {
    /// Saves the entire tree state.
    async fn save_tree(&self, tree: &MerkleTree) -> Result<(), MerkleError>;

    /// Loads the tree state.
    async fn load_tree(&self) -> Result<MerkleTree, MerkleError>;

    /// Saves a receipt.
    async fn save_receipt(&self, receipt: &Receipt) -> Result<(), ReceiptError>;

    /// Loads a receipt by ID.
    async fn load_receipt(&self, receipt_id: Uuid) -> Result<Receipt, ReceiptError>;

    /// Checks if the backend is healthy and reachable.
    async fn health_check(&self) -> Result<(), MerkleError>;

    /// Clears all data (for testing only).
    #[cfg(test)]
    async fn clear(&self) -> Result<(), MerkleError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Operation, OperationKind, OperationResult};
    use std::collections::HashMap;

    fn create_test_receipt() -> Receipt {
        let operation = Operation::new(
            OperationKind::Parse {
                input: "test".into(),
            },
            1,
        );

        Receipt {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            operation_id: operation.id,
            operation_hash: "abc123".to_string(),
            attestation_hash: "abc123".to_string(),
            signature: Some("sig".to_string()),
            replay_pointers: vec![],
            result: OperationResult::Success {
                output_hash: "def456".to_string(),
                output: None,
            },
            refusal: None,
            metadata: HashMap::new(),
        }
    }

    // Mock implementation for testing the trait interface
    struct MockMerkleStorage;

    #[async_trait]
    impl MerkleReceiptStorage for MockMerkleStorage {
        async fn store_receipt(&self, _receipt: &Receipt) -> Result<MerkleRoot, ReceiptError> {
            Ok(MerkleRoot {
                hash: "mock_root".to_string(),
                leaf_count: 1,
            })
        }

        async fn store_receipts(&self, receipts: &[Receipt]) -> Result<MerkleRoot, ReceiptError> {
            Ok(MerkleRoot {
                hash: "mock_root".to_string(),
                leaf_count: receipts.len(),
            })
        }

        async fn get_receipt(&self, receipt_id: Uuid) -> Result<Receipt, ReceiptError> {
            let mut receipt = create_test_receipt();
            receipt.id = receipt_id;
            Ok(receipt)
        }

        async fn generate_proof(&self, receipt_id: Uuid) -> Result<MerkleProof, MerkleError> {
            Ok(MerkleProof {
                receipt_id,
                leaf_hash: "mock_leaf".to_string(),
                leaf_index: 0,
                proof_hashes: vec![],
                root_hash: "mock_root".to_string(),
            })
        }

        async fn verify_proof(&self, _proof: &MerkleProof) -> Result<(), MerkleError> {
            Ok(())
        }

        async fn get_root(&self) -> Result<Option<MerkleRoot>, MerkleError> {
            Ok(Some(MerkleRoot {
                hash: "mock_root".to_string(),
                leaf_count: 1,
            }))
        }

        async fn get_tree(&self) -> Result<MerkleTree, MerkleError> {
            Ok(MerkleTree::new())
        }

        async fn receipt_count(&self) -> Result<usize, MerkleError> {
            Ok(1)
        }

        async fn verify_tree_integrity(&self) -> Result<(), MerkleError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_mock_merkle_storage() {
        let storage = MockMerkleStorage;
        let receipt = create_test_receipt();

        let root = storage.store_receipt(&receipt).await.unwrap();
        assert_eq!(root.leaf_count, 1);

        let retrieved = storage.get_receipt(receipt.id).await.unwrap();
        assert_eq!(retrieved.id, receipt.id);

        let proof = storage.generate_proof(receipt.id).await.unwrap();
        assert_eq!(proof.receipt_id, receipt.id);

        assert!(storage.verify_proof(&proof).await.is_ok());
        assert!(storage.verify_tree_integrity().await.is_ok());
    }
}
