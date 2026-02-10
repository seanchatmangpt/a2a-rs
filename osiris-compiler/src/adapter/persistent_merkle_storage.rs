//! Persistent Merkle tree-based receipt storage with pluggable backends.
//!
//! Supports various storage backends (SQLite, Postgres, Cloud Storage)
//! while maintaining Merkle tree verification capabilities.

use crate::domain::{MerkleError, MerkleProof, MerkleRoot, MerkleTree, Receipt, ReceiptError};
use crate::port::{MerkleReceiptStorage, PersistentMerkleBackend};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Persistent Merkle tree-based receipt storage.
///
/// Uses a pluggable backend for persistence while maintaining an
/// in-memory cache of the Merkle tree for fast proof generation.
pub struct PersistentMerkleStorage<B: PersistentMerkleBackend> {
    /// The backend storage implementation
    backend: Arc<B>,
    /// In-memory cache of the Merkle tree
    tree_cache: Arc<RwLock<MerkleTree>>,
}

impl<B: PersistentMerkleBackend> PersistentMerkleStorage<B> {
    /// Creates a new persistent storage with the given backend.
    pub async fn new(backend: B) -> Result<Self, MerkleError> {
        let tree = backend
            .load_tree()
            .await
            .unwrap_or_else(|_| MerkleTree::new());

        Ok(Self {
            backend: Arc::new(backend),
            tree_cache: Arc::new(RwLock::new(tree)),
        })
    }

    /// Creates storage with explicit tree (for testing).
    pub fn with_tree(backend: B, tree: MerkleTree) -> Self {
        Self {
            backend: Arc::new(backend),
            tree_cache: Arc::new(RwLock::new(tree)),
        }
    }

    /// Persists the current tree state to the backend.
    async fn persist_tree(&self) -> Result<(), MerkleError> {
        let tree = self.tree_cache.read().await;
        self.backend.save_tree(&tree).await
    }

    /// Reloads the tree from the backend.
    pub async fn reload(&self) -> Result<(), MerkleError> {
        let tree = self.backend.load_tree().await?;
        let mut cache = self.tree_cache.write().await;
        *cache = tree;
        Ok(())
    }

    /// Returns a reference to the backend.
    pub fn backend(&self) -> &B {
        &self.backend
    }
}

#[async_trait]
impl<B: PersistentMerkleBackend> MerkleReceiptStorage for PersistentMerkleStorage<B> {
    async fn store_receipt(&self, receipt: &Receipt) -> Result<MerkleRoot, ReceiptError> {
        // Compute receipt hash
        let receipt_hash = receipt.compute_receipt_hash()?;

        // Save receipt to backend
        self.backend.save_receipt(receipt).await?;

        // Update tree cache
        let mut tree = self.tree_cache.write().await;
        tree.add_receipt(receipt.id, receipt_hash);

        // Persist updated tree
        self.backend
            .save_tree(&tree)
            .await
            .map_err(|e| ReceiptError::SerializationError(e.to_string()))?;

        // Return new root
        tree.root()
            .cloned()
            .ok_or_else(|| ReceiptError::InvalidFormat("Failed to compute root".to_string()))
    }

    async fn store_receipts(&self, receipts: &[Receipt]) -> Result<MerkleRoot, ReceiptError> {
        let mut receipt_hashes = Vec::new();

        // Compute all hashes and save receipts
        for receipt in receipts {
            let hash = receipt.compute_receipt_hash()?;
            receipt_hashes.push((receipt.id, hash));
            self.backend.save_receipt(receipt).await?;
        }

        // Batch update tree cache
        let mut tree = self.tree_cache.write().await;
        tree.add_receipts(receipt_hashes);

        // Persist updated tree
        self.backend
            .save_tree(&tree)
            .await
            .map_err(|e| ReceiptError::SerializationError(e.to_string()))?;

        // Return new root
        tree.root()
            .cloned()
            .ok_or_else(|| ReceiptError::InvalidFormat("Failed to compute root".to_string()))
    }

    async fn get_receipt(&self, receipt_id: Uuid) -> Result<Receipt, ReceiptError> {
        self.backend.load_receipt(receipt_id).await
    }

    async fn generate_proof(&self, receipt_id: Uuid) -> Result<MerkleProof, MerkleError> {
        let tree = self.tree_cache.read().await;
        tree.generate_proof(receipt_id)
            .ok_or(MerkleError::ReceiptNotFound(receipt_id))
    }

    async fn verify_proof(&self, proof: &MerkleProof) -> Result<(), MerkleError> {
        let tree = self.tree_cache.read().await;
        if tree.verify_proof(proof) {
            Ok(())
        } else {
            Err(MerkleError::InvalidProof(proof.receipt_id))
        }
    }

    async fn get_root(&self) -> Result<Option<MerkleRoot>, MerkleError> {
        let tree = self.tree_cache.read().await;
        Ok(tree.root().cloned())
    }

    async fn get_tree(&self) -> Result<MerkleTree, MerkleError> {
        let tree = self.tree_cache.read().await;
        Ok(tree.clone())
    }

    async fn receipt_count(&self) -> Result<usize, MerkleError> {
        let tree = self.tree_cache.read().await;
        Ok(tree.leaf_count())
    }

    async fn verify_tree_integrity(&self) -> Result<(), MerkleError> {
        let tree = self.tree_cache.read().await;

        // Verify all receipts in tree can be loaded from backend
        for leaf in tree.leaves() {
            if let crate::domain::MerkleNode::Leaf { receipt_id, .. } = leaf {
                let receipt = self
                    .backend
                    .load_receipt(*receipt_id)
                    .await
                    .map_err(|_| MerkleError::ReceiptNotFound(*receipt_id))?;

                // Verify receipt hash matches leaf
                let computed_hash = receipt
                    .compute_receipt_hash()
                    .map_err(|e| MerkleError::SerializationError(e.to_string()))?;

                if computed_hash != leaf.hash() {
                    return Err(MerkleError::InvalidProof(*receipt_id));
                }
            }
        }

        // Verify all proofs
        for leaf in tree.leaves() {
            if let crate::domain::MerkleNode::Leaf { receipt_id, .. } = leaf {
                let proof = tree
                    .generate_proof(*receipt_id)
                    .ok_or(MerkleError::ReceiptNotFound(*receipt_id))?;

                if !tree.verify_proof(&proof) {
                    return Err(MerkleError::InvalidProof(*receipt_id));
                }
            }
        }

        Ok(())
    }
}

/// In-memory backend for testing.
///
/// Implements PersistentMerkleBackend using in-memory storage.
#[derive(Clone)]
pub struct InMemoryBackend {
    tree: Arc<RwLock<Option<MerkleTree>>>,
    receipts: Arc<RwLock<std::collections::HashMap<Uuid, Receipt>>>,
}

impl InMemoryBackend {
    /// Creates a new in-memory backend.
    pub fn new() -> Self {
        Self {
            tree: Arc::new(RwLock::new(None)),
            receipts: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PersistentMerkleBackend for InMemoryBackend {
    async fn save_tree(&self, tree: &MerkleTree) -> Result<(), MerkleError> {
        let mut storage = self.tree.write().await;
        *storage = Some(tree.clone());
        Ok(())
    }

    async fn load_tree(&self) -> Result<MerkleTree, MerkleError> {
        let storage = self.tree.read().await;
        storage.clone().ok_or_else(|| MerkleError::EmptyTree)
    }

    async fn save_receipt(&self, receipt: &Receipt) -> Result<(), ReceiptError> {
        let mut storage = self.receipts.write().await;
        storage.insert(receipt.id, receipt.clone());
        Ok(())
    }

    async fn load_receipt(&self, receipt_id: Uuid) -> Result<Receipt, ReceiptError> {
        let storage = self.receipts.read().await;
        storage
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| ReceiptError::InvalidFormat(format!("Receipt {} not found", receipt_id)))
    }

    async fn health_check(&self) -> Result<(), MerkleError> {
        Ok(())
    }

    #[cfg(test)]
    async fn clear(&self) -> Result<(), MerkleError> {
        let mut tree = self.tree.write().await;
        *tree = None;
        let mut receipts = self.receipts.write().await;
        receipts.clear();
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
    async fn test_persistent_storage_basic() {
        let backend = InMemoryBackend::new();
        let storage = PersistentMerkleStorage::new(backend).await.unwrap();

        let receipt = create_test_receipt();
        let root = storage.store_receipt(&receipt).await.unwrap();
        assert_eq!(root.leaf_count, 1);

        let retrieved = storage.get_receipt(receipt.id).await.unwrap();
        assert_eq!(retrieved.id, receipt.id);
    }

    #[tokio::test]
    async fn test_persistent_storage_reload() {
        let backend = InMemoryBackend::new();
        let storage = PersistentMerkleStorage::new(backend.clone()).await.unwrap();

        let receipt = create_test_receipt();
        storage.store_receipt(&receipt).await.unwrap();

        // Create new storage with same backend
        let storage2 = PersistentMerkleStorage::new(backend).await.unwrap();

        // Should be able to retrieve receipt
        let retrieved = storage2.get_receipt(receipt.id).await.unwrap();
        assert_eq!(retrieved.id, receipt.id);

        // Proofs should work
        let proof = storage2.generate_proof(receipt.id).await.unwrap();
        assert!(storage2.verify_proof(&proof).await.is_ok());
    }

    #[tokio::test]
    async fn test_persistent_storage_batch() {
        let backend = InMemoryBackend::new();
        let storage = PersistentMerkleStorage::new(backend).await.unwrap();

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
    async fn test_persistent_storage_proof_generation() {
        let backend = InMemoryBackend::new();
        let storage = PersistentMerkleStorage::new(backend).await.unwrap();

        let mut receipts = Vec::new();
        for _ in 0..8 {
            let receipt = create_test_receipt();
            storage.store_receipt(&receipt).await.unwrap();
            receipts.push(receipt);
        }

        for receipt in &receipts {
            let proof = storage.generate_proof(receipt.id).await.unwrap();
            assert!(storage.verify_proof(&proof).await.is_ok());
            assert!(proof.verify());

            // Proof size should be log2(8) = 3
            assert_eq!(proof.len(), 3);
        }
    }

    #[tokio::test]
    async fn test_persistent_storage_integrity() {
        let backend = InMemoryBackend::new();
        let storage = PersistentMerkleStorage::new(backend).await.unwrap();

        for _ in 0..10 {
            let receipt = create_test_receipt();
            storage.store_receipt(&receipt).await.unwrap();
        }

        assert!(storage.verify_tree_integrity().await.is_ok());
    }

    #[tokio::test]
    async fn test_persistent_storage_reload_explicit() {
        let backend = InMemoryBackend::new();
        let storage = PersistentMerkleStorage::new(backend).await.unwrap();

        let receipt = create_test_receipt();
        storage.store_receipt(&receipt).await.unwrap();

        // Reload from backend
        storage.reload().await.unwrap();

        // Should still be able to generate proofs
        let proof = storage.generate_proof(receipt.id).await.unwrap();
        assert!(storage.verify_proof(&proof).await.is_ok());
    }

    #[tokio::test]
    async fn test_persistent_storage_root_persistence() {
        let backend = InMemoryBackend::new();
        let storage = PersistentMerkleStorage::new(backend.clone()).await.unwrap();

        let receipt = create_test_receipt();
        let root1 = storage.store_receipt(&receipt).await.unwrap();

        // Create new storage with same backend
        let storage2 = PersistentMerkleStorage::new(backend).await.unwrap();
        let root2 = storage2.get_root().await.unwrap().unwrap();

        assert_eq!(root1.hash, root2.hash);
    }

    #[tokio::test]
    async fn test_backend_health_check() {
        let backend = InMemoryBackend::new();
        assert!(backend.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_backend_clear() {
        let backend = InMemoryBackend::new();
        let storage = PersistentMerkleStorage::new(backend.clone()).await.unwrap();

        let receipt = create_test_receipt();
        storage.store_receipt(&receipt).await.unwrap();

        backend.clear().await.unwrap();

        // After clear, should not be able to load tree
        let result = backend.load_tree().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_empty_backend() {
        let backend = InMemoryBackend::new();
        let storage = PersistentMerkleStorage::new(backend).await.unwrap();

        let root = storage.get_root().await.unwrap();
        assert!(root.is_none());

        let count = storage.receipt_count().await.unwrap();
        assert_eq!(count, 0);
    }
}
