//! In-memory Merkle tree-based receipt storage.
//!
//! Provides efficient O(log N) verification proofs for receipt storage
//! using a Merkle tree structure. Suitable for testing and development.

use crate::domain::{MerkleError, MerkleProof, MerkleRoot, MerkleTree, Receipt, ReceiptError};
use crate::port::MerkleReceiptStorage;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// In-memory Merkle tree-based receipt storage.
///
/// Stores receipts in a Merkle tree structure, enabling:
/// - O(log N) verification proofs
/// - Tamper detection via root hash
/// - Efficient incremental updates
///
/// Not suitable for production use as data is not persisted.
pub struct InMemoryMerkleStorage {
    /// The Merkle tree containing receipt hashes
    tree: Arc<RwLock<MerkleTree>>,
    /// Map from receipt ID to full receipt data
    receipts: Arc<RwLock<HashMap<Uuid, Receipt>>>,
}

impl InMemoryMerkleStorage {
    /// Creates a new in-memory Merkle storage.
    pub fn new() -> Self {
        Self {
            tree: Arc::new(RwLock::new(MerkleTree::new())),
            receipts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Creates storage with pre-populated receipts.
    pub fn with_receipts(receipts: Vec<Receipt>) -> Result<Self, ReceiptError> {
        let mut tree = MerkleTree::new();
        let mut receipt_map = HashMap::new();

        for receipt in receipts {
            let receipt_hash = receipt.compute_receipt_hash()?;
            tree.add_receipt(receipt.id, receipt_hash);
            receipt_map.insert(receipt.id, receipt);
        }

        Ok(Self {
            tree: Arc::new(RwLock::new(tree)),
            receipts: Arc::new(RwLock::new(receipt_map)),
        })
    }
}

impl Default for InMemoryMerkleStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MerkleReceiptStorage for InMemoryMerkleStorage {
    async fn store_receipt(&self, receipt: &Receipt) -> Result<MerkleRoot, ReceiptError> {
        // Compute receipt hash
        let receipt_hash = receipt.compute_receipt_hash()?;

        // Add to tree
        let mut tree = self.tree.write().await;
        tree.add_receipt(receipt.id, receipt_hash);

        // Store full receipt
        let mut receipts = self.receipts.write().await;
        receipts.insert(receipt.id, receipt.clone());

        // Return new root
        tree.root()
            .cloned()
            .ok_or_else(|| ReceiptError::InvalidFormat("Failed to compute root".to_string()))
    }

    async fn store_receipts(&self, receipts: &[Receipt]) -> Result<MerkleRoot, ReceiptError> {
        let mut receipt_hashes = Vec::new();

        // Compute all hashes first
        for receipt in receipts {
            let hash = receipt.compute_receipt_hash()?;
            receipt_hashes.push((receipt.id, hash));
        }

        // Batch add to tree
        let mut tree = self.tree.write().await;
        tree.add_receipts(receipt_hashes);

        // Store all receipts
        let mut receipt_map = self.receipts.write().await;
        for receipt in receipts {
            receipt_map.insert(receipt.id, receipt.clone());
        }

        // Return new root
        tree.root()
            .cloned()
            .ok_or_else(|| ReceiptError::InvalidFormat("Failed to compute root".to_string()))
    }

    async fn get_receipt(&self, receipt_id: Uuid) -> Result<Receipt, ReceiptError> {
        let receipts = self.receipts.read().await;
        receipts
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| ReceiptError::InvalidFormat(format!("Receipt {} not found", receipt_id)))
    }

    async fn generate_proof(&self, receipt_id: Uuid) -> Result<MerkleProof, MerkleError> {
        let tree = self.tree.read().await;
        tree.generate_proof(receipt_id)
            .ok_or(MerkleError::ReceiptNotFound(receipt_id))
    }

    async fn verify_proof(&self, proof: &MerkleProof) -> Result<(), MerkleError> {
        let tree = self.tree.read().await;
        if tree.verify_proof(proof) {
            Ok(())
        } else {
            Err(MerkleError::InvalidProof(proof.receipt_id))
        }
    }

    async fn get_root(&self) -> Result<Option<MerkleRoot>, MerkleError> {
        let tree = self.tree.read().await;
        Ok(tree.root().cloned())
    }

    async fn get_tree(&self) -> Result<MerkleTree, MerkleError> {
        let tree = self.tree.read().await;
        Ok(tree.clone())
    }

    async fn receipt_count(&self) -> Result<usize, MerkleError> {
        let tree = self.tree.read().await;
        Ok(tree.leaf_count())
    }

    async fn verify_tree_integrity(&self) -> Result<(), MerkleError> {
        let tree = self.tree.read().await;
        let receipts = self.receipts.read().await;

        // Check that tree and receipts have same count
        if tree.leaf_count() != receipts.len() {
            return Err(MerkleError::SerializationError(format!(
                "Tree has {} leaves but {} receipts stored",
                tree.leaf_count(),
                receipts.len()
            )));
        }

        // Verify all receipts are in tree
        for receipt_id in receipts.keys() {
            if !tree.contains(*receipt_id) {
                return Err(MerkleError::ReceiptNotFound(*receipt_id));
            }
        }

        // Verify proofs for all receipts
        for receipt_id in receipts.keys() {
            let proof = tree
                .generate_proof(*receipt_id)
                .ok_or(MerkleError::ReceiptNotFound(*receipt_id))?;

            if !tree.verify_proof(&proof) {
                return Err(MerkleError::InvalidProof(*receipt_id));
            }
        }

        Ok(())
    }
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

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let storage = InMemoryMerkleStorage::new();
        let receipt = create_test_receipt();

        let root = storage.store_receipt(&receipt).await.unwrap();
        assert_eq!(root.leaf_count, 1);

        let retrieved = storage.get_receipt(receipt.id).await.unwrap();
        assert_eq!(retrieved.id, receipt.id);
    }

    #[tokio::test]
    async fn test_proof_generation() {
        let storage = InMemoryMerkleStorage::new();
        let receipt = create_test_receipt();

        storage.store_receipt(&receipt).await.unwrap();

        let proof = storage.generate_proof(receipt.id).await.unwrap();
        assert_eq!(proof.receipt_id, receipt.id);

        assert!(storage.verify_proof(&proof).await.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_receipts() {
        let storage = InMemoryMerkleStorage::new();
        let mut receipts = Vec::new();

        for _ in 0..10 {
            let receipt = create_test_receipt();
            storage.store_receipt(&receipt).await.unwrap();
            receipts.push(receipt);
        }

        let count = storage.receipt_count().await.unwrap();
        assert_eq!(count, 10);

        for receipt in &receipts {
            let proof = storage.generate_proof(receipt.id).await.unwrap();
            assert!(storage.verify_proof(&proof).await.is_ok());
            assert!(proof.verify());
        }
    }

    #[tokio::test]
    async fn test_batch_store() {
        let storage = InMemoryMerkleStorage::new();
        let mut receipts = Vec::new();

        for _ in 0..5 {
            receipts.push(create_test_receipt());
        }

        let root = storage.store_receipts(&receipts).await.unwrap();
        assert_eq!(root.leaf_count, 5);

        for receipt in &receipts {
            let retrieved = storage.get_receipt(receipt.id).await.unwrap();
            assert_eq!(retrieved.id, receipt.id);
        }
    }

    #[tokio::test]
    async fn test_root_changes_on_update() {
        let storage = InMemoryMerkleStorage::new();
        let receipt1 = create_test_receipt();
        let receipt2 = create_test_receipt();

        let root1 = storage.store_receipt(&receipt1).await.unwrap();
        let root2 = storage.store_receipt(&receipt2).await.unwrap();

        assert_ne!(root1.hash, root2.hash);
        assert_eq!(root2.leaf_count, 2);
    }

    #[tokio::test]
    async fn test_proof_size_logarithmic() {
        let storage = InMemoryMerkleStorage::new();
        let mut receipts = Vec::new();

        for _ in 0..16 {
            let receipt = create_test_receipt();
            storage.store_receipt(&receipt).await.unwrap();
            receipts.push(receipt);
        }

        // For 16 leaves, proof should be exactly log2(16) = 4 steps
        for receipt in &receipts {
            let proof = storage.generate_proof(receipt.id).await.unwrap();
            assert_eq!(proof.len(), 4);
        }
    }

    #[tokio::test]
    async fn test_tree_integrity() {
        let storage = InMemoryMerkleStorage::new();

        for _ in 0..7 {
            let receipt = create_test_receipt();
            storage.store_receipt(&receipt).await.unwrap();
        }

        assert!(storage.verify_tree_integrity().await.is_ok());
    }

    #[tokio::test]
    async fn test_get_tree() {
        let storage = InMemoryMerkleStorage::new();
        let receipt = create_test_receipt();

        storage.store_receipt(&receipt).await.unwrap();

        let tree = storage.get_tree().await.unwrap();
        assert_eq!(tree.leaf_count(), 1);
        assert!(tree.root().is_some());
    }

    #[tokio::test]
    async fn test_empty_storage() {
        let storage = InMemoryMerkleStorage::new();

        let root = storage.get_root().await.unwrap();
        assert!(root.is_none());

        let count = storage.receipt_count().await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_proof_not_found() {
        let storage = InMemoryMerkleStorage::new();
        let id = Uuid::new_v4();

        let result = storage.generate_proof(id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_receipt_not_found() {
        let storage = InMemoryMerkleStorage::new();
        let id = Uuid::new_v4();

        let result = storage.get_receipt(id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_with_receipts_constructor() {
        let mut receipts = Vec::new();
        for _ in 0..5 {
            receipts.push(create_test_receipt());
        }

        let storage = InMemoryMerkleStorage::with_receipts(receipts.clone()).unwrap();

        let count = storage.receipt_count().await.unwrap();
        assert_eq!(count, 5);

        for receipt in &receipts {
            let retrieved = storage.get_receipt(receipt.id).await.unwrap();
            assert_eq!(retrieved.id, receipt.id);
        }
    }

    #[tokio::test]
    async fn test_deterministic_root() {
        let mut receipts = Vec::new();
        for _ in 0..5 {
            receipts.push(create_test_receipt());
        }

        let storage1 = InMemoryMerkleStorage::new();
        let storage2 = InMemoryMerkleStorage::new();

        for receipt in &receipts {
            storage1.store_receipt(receipt).await.unwrap();
            storage2.store_receipt(receipt).await.unwrap();
        }

        let root1 = storage1.get_root().await.unwrap().unwrap();
        let root2 = storage2.get_root().await.unwrap().unwrap();

        assert_eq!(root1.hash, root2.hash);
    }
}
