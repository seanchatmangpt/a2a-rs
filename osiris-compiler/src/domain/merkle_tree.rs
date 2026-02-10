//! Merkle tree domain types for receipt storage.
//!
//! Receipts form a Merkle tree where each receipt hash becomes a leaf.
//! This enables O(log N) verification proofs and efficient incremental updates.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

/// A node in the Merkle tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MerkleNode {
    /// Leaf node containing a receipt hash
    Leaf {
        /// Receipt ID
        receipt_id: Uuid,
        /// Hash of the receipt
        hash: String,
        /// Position in the tree
        index: usize,
    },
    /// Internal node with two children
    Internal {
        /// Hash of left child
        left_hash: String,
        /// Hash of right child
        right_hash: String,
        /// Combined hash of this node
        hash: String,
    },
}

impl MerkleNode {
    /// Returns the hash of this node.
    pub fn hash(&self) -> &str {
        match self {
            MerkleNode::Leaf { hash, .. } => hash,
            MerkleNode::Internal { hash, .. } => hash,
        }
    }

    /// Creates a leaf node from a receipt hash.
    pub fn leaf(receipt_id: Uuid, receipt_hash: String, index: usize) -> Self {
        MerkleNode::Leaf {
            receipt_id,
            hash: receipt_hash,
            index,
        }
    }

    /// Creates an internal node from two child hashes.
    pub fn internal(left_hash: String, right_hash: String) -> Self {
        let combined = format!("{}{}", left_hash, right_hash);
        let mut hasher = Sha256::new();
        hasher.update(combined.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        MerkleNode::Internal {
            left_hash,
            right_hash,
            hash,
        }
    }
}

/// A Merkle tree for receipt storage.
///
/// The tree is built from receipt hashes as leaves, with internal nodes
/// computed by hashing the concatenation of child hashes. This structure
/// enables efficient verification: a proof of inclusion requires only
/// O(log N) sibling hashes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MerkleTree {
    /// All leaves in the tree (receipt hashes)
    leaves: Vec<MerkleNode>,
    /// Root hash of the tree
    root: Option<MerkleRoot>,
    /// Index mapping receipt IDs to leaf positions
    #[serde(skip)]
    index: HashMap<Uuid, usize>,
}

impl MerkleTree {
    /// Creates an empty Merkle tree.
    pub fn new() -> Self {
        Self {
            leaves: Vec::new(),
            root: None,
            index: HashMap::new(),
        }
    }

    /// Adds a receipt hash to the tree.
    ///
    /// This is an incremental operation - the tree is rebuilt from scratch
    /// after adding the new leaf. For large trees, consider batching additions.
    pub fn add_receipt(&mut self, receipt_id: Uuid, receipt_hash: String) {
        let index = self.leaves.len();
        let leaf = MerkleNode::leaf(receipt_id, receipt_hash, index);
        self.leaves.push(leaf);
        self.index.insert(receipt_id, index);
        self.rebuild_tree();
    }

    /// Builds the tree from current leaves.
    ///
    /// This computes all internal nodes and the root hash.
    /// Time complexity: O(N) where N is the number of leaves.
    fn rebuild_tree(&mut self) {
        if self.leaves.is_empty() {
            self.root = None;
            return;
        }

        if self.leaves.len() == 1 {
            self.root = Some(MerkleRoot {
                hash: self.leaves[0].hash().to_string(),
                leaf_count: 1,
            });
            return;
        }

        // Build tree level by level
        let mut current_level: Vec<String> =
            self.leaves.iter().map(|n| n.hash().to_string()).collect();

        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    let node = MerkleNode::internal(chunk[0].clone(), chunk[1].clone());
                    node.hash().to_string()
                } else {
                    // Odd number of nodes - duplicate the last one
                    let node = MerkleNode::internal(chunk[0].clone(), chunk[0].clone());
                    node.hash().to_string()
                };
                next_level.push(hash);
            }

            current_level = next_level;
        }

        self.root = Some(MerkleRoot {
            hash: current_level[0].clone(),
            leaf_count: self.leaves.len(),
        });
    }

    /// Returns the root hash of the tree.
    pub fn root(&self) -> Option<&MerkleRoot> {
        self.root.as_ref()
    }

    /// Returns the number of leaves in the tree.
    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }

    /// Generates a Merkle proof for a receipt.
    ///
    /// The proof contains the sibling hashes needed to verify that a receipt
    /// is included in the tree. Verification is O(log N).
    ///
    /// Returns None if the receipt is not in the tree.
    pub fn generate_proof(&self, receipt_id: Uuid) -> Option<MerkleProof> {
        let leaf_index = *self.index.get(&receipt_id)?;
        let leaf_hash = self.leaves[leaf_index].hash().to_string();

        let mut proof_hashes = Vec::new();
        let mut current_level: Vec<String> =
            self.leaves.iter().map(|n| n.hash().to_string()).collect();
        let mut index = leaf_index;

        // Collect sibling hashes at each level
        while current_level.len() > 1 {
            let sibling_index = if index % 2 == 0 { index + 1 } else { index - 1 };

            // Handle odd number of nodes
            let sibling_hash = if sibling_index < current_level.len() {
                current_level[sibling_index].clone()
            } else {
                current_level[index].clone()
            };

            proof_hashes.push(ProofStep {
                hash: sibling_hash,
                is_left: index % 2 != 0,
            });

            // Build next level
            let mut next_level = Vec::new();
            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    let node = MerkleNode::internal(chunk[0].clone(), chunk[1].clone());
                    node.hash().to_string()
                } else {
                    let node = MerkleNode::internal(chunk[0].clone(), chunk[0].clone());
                    node.hash().to_string()
                };
                next_level.push(hash);
            }

            current_level = next_level;
            index /= 2;
        }

        Some(MerkleProof {
            receipt_id,
            leaf_hash,
            leaf_index,
            proof_hashes,
            root_hash: self.root.as_ref()?.hash.clone(),
        })
    }

    /// Verifies that a proof is valid for this tree.
    pub fn verify_proof(&self, proof: &MerkleProof) -> bool {
        let expected_root = match &self.root {
            Some(r) => &r.hash,
            None => return false,
        };

        // Recompute root from leaf and proof
        let mut current_hash = proof.leaf_hash.clone();

        for step in &proof.proof_hashes {
            let (left, right) = if step.is_left {
                (&step.hash, &current_hash)
            } else {
                (&current_hash, &step.hash)
            };

            let combined = format!("{}{}", left, right);
            let mut hasher = Sha256::new();
            hasher.update(combined.as_bytes());
            current_hash = format!("{:x}", hasher.finalize());
        }

        current_hash == *expected_root
    }

    /// Batch adds multiple receipts.
    ///
    /// More efficient than calling add_receipt multiple times as it only
    /// rebuilds the tree once.
    pub fn add_receipts(&mut self, receipts: Vec<(Uuid, String)>) {
        for (receipt_id, receipt_hash) in receipts {
            let index = self.leaves.len();
            let leaf = MerkleNode::leaf(receipt_id, receipt_hash, index);
            self.leaves.push(leaf);
            self.index.insert(receipt_id, index);
        }
        self.rebuild_tree();
    }

    /// Returns all leaf nodes.
    pub fn leaves(&self) -> &[MerkleNode] {
        &self.leaves
    }

    /// Checks if a receipt is in the tree.
    pub fn contains(&self, receipt_id: Uuid) -> bool {
        self.index.contains_key(&receipt_id)
    }
}

impl Default for MerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Root of a Merkle tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MerkleRoot {
    /// Root hash
    pub hash: String,
    /// Number of leaves in the tree
    pub leaf_count: usize,
}

/// A step in a Merkle proof.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProofStep {
    /// Hash of the sibling node
    pub hash: String,
    /// True if the sibling is on the left
    pub is_left: bool,
}

/// A Merkle proof for a receipt.
///
/// Contains the minimum information needed to verify that a receipt
/// is included in a Merkle tree with a specific root hash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MerkleProof {
    /// Receipt ID being proved
    pub receipt_id: Uuid,
    /// Hash of the receipt (leaf)
    pub leaf_hash: String,
    /// Position of the leaf in the tree
    pub leaf_index: usize,
    /// Sibling hashes needed for verification
    pub proof_hashes: Vec<ProofStep>,
    /// Expected root hash
    pub root_hash: String,
}

impl MerkleProof {
    /// Verifies this proof independently.
    ///
    /// Returns true if the proof correctly computes the expected root hash.
    pub fn verify(&self) -> bool {
        let mut current_hash = self.leaf_hash.clone();

        for step in &self.proof_hashes {
            let (left, right) = if step.is_left {
                (&step.hash, &current_hash)
            } else {
                (&current_hash, &step.hash)
            };

            let combined = format!("{}{}", left, right);
            let mut hasher = Sha256::new();
            hasher.update(combined.as_bytes());
            current_hash = format!("{:x}", hasher.finalize());
        }

        current_hash == self.root_hash
    }

    /// Returns the length of the proof (number of steps).
    ///
    /// For a tree with N leaves, this is approximately log2(N).
    pub fn len(&self) -> usize {
        self.proof_hashes.len()
    }

    /// Checks if the proof is empty.
    pub fn is_empty(&self) -> bool {
        self.proof_hashes.is_empty()
    }
}

/// Errors related to Merkle tree operations.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum MerkleError {
    /// Receipt not found in tree
    #[error("Receipt {0} not found in Merkle tree")]
    ReceiptNotFound(Uuid),

    /// Invalid proof
    #[error("Invalid Merkle proof for receipt {0}")]
    InvalidProof(Uuid),

    /// Tree is empty
    #[error("Merkle tree is empty")]
    EmptyTree,

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_hash(n: usize) -> String {
        let mut hasher = Sha256::new();
        hasher.update(n.to_string().as_bytes());
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn test_empty_tree() {
        let tree = MerkleTree::new();
        assert_eq!(tree.leaf_count(), 0);
        assert!(tree.root().is_none());
    }

    #[test]
    fn test_single_leaf() {
        let mut tree = MerkleTree::new();
        let id = Uuid::new_v4();
        let hash = sample_hash(1);

        tree.add_receipt(id, hash.clone());

        assert_eq!(tree.leaf_count(), 1);
        assert!(tree.root().is_some());
        assert_eq!(tree.root().unwrap().hash, hash);
        assert!(tree.contains(id));
    }

    #[test]
    fn test_two_leaves() {
        let mut tree = MerkleTree::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let hash1 = sample_hash(1);
        let hash2 = sample_hash(2);

        tree.add_receipt(id1, hash1.clone());
        tree.add_receipt(id2, hash2.clone());

        assert_eq!(tree.leaf_count(), 2);
        assert!(tree.root().is_some());

        // Root should be hash of combined leaves
        let expected = MerkleNode::internal(hash1, hash2).hash().to_string();
        assert_eq!(tree.root().unwrap().hash, expected);
    }

    #[test]
    fn test_multiple_leaves() {
        let mut tree = MerkleTree::new();
        let mut ids = Vec::new();

        for i in 0..8 {
            let id = Uuid::new_v4();
            let hash = sample_hash(i);
            tree.add_receipt(id, hash);
            ids.push(id);
        }

        assert_eq!(tree.leaf_count(), 8);
        assert!(tree.root().is_some());

        // All receipts should be in tree
        for id in &ids {
            assert!(tree.contains(*id));
        }
    }

    #[test]
    fn test_proof_generation_and_verification() {
        let mut tree = MerkleTree::new();
        let id = Uuid::new_v4();
        let hash = sample_hash(1);

        tree.add_receipt(id, hash);

        // Generate proof
        let proof = tree.generate_proof(id);
        assert!(proof.is_some());

        let proof = proof.unwrap();
        assert_eq!(proof.receipt_id, id);

        // Verify proof
        assert!(tree.verify_proof(&proof));
        assert!(proof.verify());
    }

    #[test]
    fn test_proof_for_multiple_leaves() {
        let mut tree = MerkleTree::new();
        let mut ids = Vec::new();

        for i in 0..7 {
            let id = Uuid::new_v4();
            let hash = sample_hash(i);
            tree.add_receipt(id, hash);
            ids.push(id);
        }

        // Generate and verify proof for each receipt
        for id in &ids {
            let proof = tree.generate_proof(*id).unwrap();
            assert!(tree.verify_proof(&proof));
            assert!(proof.verify());

            // Proof length should be approximately log2(7) = 3
            assert!(proof.len() <= 3);
        }
    }

    #[test]
    fn test_proof_not_found() {
        let tree = MerkleTree::new();
        let id = Uuid::new_v4();

        let proof = tree.generate_proof(id);
        assert!(proof.is_none());
    }

    #[test]
    fn test_batch_add() {
        let mut tree = MerkleTree::new();
        let mut receipts = Vec::new();

        for i in 0..10 {
            receipts.push((Uuid::new_v4(), sample_hash(i)));
        }

        tree.add_receipts(receipts.clone());

        assert_eq!(tree.leaf_count(), 10);
        for (id, _) in receipts {
            assert!(tree.contains(id));
        }
    }

    #[test]
    fn test_incremental_updates() {
        let mut tree = MerkleTree::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        tree.add_receipt(id1, sample_hash(1));
        let root1 = tree.root().unwrap().hash.clone();

        tree.add_receipt(id2, sample_hash(2));
        let root2 = tree.root().unwrap().hash.clone();

        // Root should change after adding new receipt
        assert_ne!(root1, root2);

        // Both receipts should be provable
        assert!(tree.generate_proof(id1).is_some());
        assert!(tree.generate_proof(id2).is_some());
    }

    #[test]
    fn test_odd_number_of_leaves() {
        let mut tree = MerkleTree::new();

        for i in 0..5 {
            tree.add_receipt(Uuid::new_v4(), sample_hash(i));
        }

        assert_eq!(tree.leaf_count(), 5);
        assert!(tree.root().is_some());

        // Should be able to generate proofs for all leaves
        for leaf in tree.leaves() {
            if let MerkleNode::Leaf { receipt_id, .. } = leaf {
                let proof = tree.generate_proof(*receipt_id);
                assert!(proof.is_some());
            }
        }
    }

    #[test]
    fn test_deterministic_root() {
        let mut tree1 = MerkleTree::new();
        let mut tree2 = MerkleTree::new();

        let receipts: Vec<(Uuid, String)> =
            (0..10).map(|i| (Uuid::new_v4(), sample_hash(i))).collect();

        // Add same receipts to both trees
        for (id, hash) in &receipts {
            tree1.add_receipt(*id, hash.clone());
            tree2.add_receipt(*id, hash.clone());
        }

        // Roots should be identical
        assert_eq!(tree1.root(), tree2.root());
    }

    #[test]
    fn test_proof_serialization() {
        let mut tree = MerkleTree::new();
        let id = Uuid::new_v4();
        tree.add_receipt(id, sample_hash(1));

        let proof = tree.generate_proof(id).unwrap();

        // Serialize and deserialize
        let json = serde_json::to_string(&proof).unwrap();
        let deserialized: MerkleProof = serde_json::from_str(&json).unwrap();

        assert_eq!(proof, deserialized);
        assert!(deserialized.verify());
    }
}
