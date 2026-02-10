//! Cryptographic receipt validation system for A2A protocol
//!
//! This module provides a comprehensive receipt validation system with:
//! - Receipt generation and verification using ed25519 signatures
//! - Receipt chains with hash pointers for immutability
//! - Merkle tree for efficient batch verification
//! - Replay validation for deterministic build verification

#[cfg(feature = "crypto")]
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
#[cfg(feature = "crypto")]
use sha2::{Digest, Sha256};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "crypto")]
use bon::Builder;

/// Errors that can occur during receipt operations
#[derive(Error, Debug)]
pub enum ReceiptError {
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("Invalid hash: expected {expected}, got {actual}")]
    InvalidHash { expected: String, actual: String },

    #[error("Receipt chain broken at index {index}: {reason}")]
    ChainBroken { index: usize, reason: String },

    #[error("Merkle tree verification failed: {0}")]
    MerkleVerificationFailed(String),

    #[error("Replay validation failed: {0}")]
    ReplayValidationFailed(String),

    #[error("Empty receipt chain")]
    EmptyChain,

    #[error("Invalid receipt data: {0}")]
    InvalidData(String),

    #[error("Cryptographic error: {0}")]
    CryptoError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Result type for receipt operations
pub type ReceiptResult<T> = Result<T, ReceiptError>;

/// A cryptographic receipt linking ontology hash to output hash
///
/// Receipts provide tamper-evident proof that a specific output was generated
/// from a specific ontology at a specific time, signed by a specific key.
#[cfg(feature = "crypto")]
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    /// Hash of the ontology (input specification)
    pub ontology_hash: String,

    /// Hash of the generated output
    pub output_hash: String,

    /// Timestamp of receipt generation
    pub timestamp: DateTime<Utc>,

    /// Ed25519 signature over the receipt data
    pub signature: String,

    /// Optional metadata
    pub metadata: Option<serde_json::Value>,
}

#[cfg(feature = "crypto")]
impl Receipt {
    /// Create a new receipt and sign it
    pub fn new(
        ontology_hash: String,
        output_hash: String,
        signing_key: &SigningKey,
        metadata: Option<serde_json::Value>,
    ) -> ReceiptResult<Self> {
        let timestamp = Utc::now();
        let data = Self::build_signing_data(&ontology_hash, &output_hash, &timestamp)?;
        let signature = signing_key.sign(&data);

        Ok(Self {
            ontology_hash,
            output_hash,
            timestamp,
            signature: hex::encode(signature.to_bytes()),
            metadata,
        })
    }

    /// Verify the receipt signature
    pub fn verify(&self, verifying_key: &VerifyingKey) -> ReceiptResult<()> {
        let data =
            Self::build_signing_data(&self.ontology_hash, &self.output_hash, &self.timestamp)?;

        let sig_bytes = hex::decode(&self.signature)
            .map_err(|e| ReceiptError::InvalidSignature(e.to_string()))?;

        let sig_array: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| ReceiptError::InvalidSignature("Invalid signature length".to_string()))?;

        let signature = Signature::from_bytes(&sig_array);

        verifying_key
            .verify(&data, &signature)
            .map_err(|e| ReceiptError::InvalidSignature(e.to_string()))
    }

    /// Compute the hash of this receipt for chaining
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.ontology_hash.as_bytes());
        hasher.update(self.output_hash.as_bytes());
        hasher.update(self.timestamp.to_rfc3339().as_bytes());
        hasher.update(self.signature.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Build the data to be signed
    fn build_signing_data(
        ontology_hash: &str,
        output_hash: &str,
        timestamp: &DateTime<Utc>,
    ) -> ReceiptResult<Vec<u8>> {
        let data = serde_json::json!({
            "ontologyHash": ontology_hash,
            "outputHash": output_hash,
            "timestamp": timestamp.to_rfc3339(),
        });
        Ok(serde_json::to_vec(&data)?)
    }

    /// Compute hash of arbitrary data
    pub fn hash_data(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }
}

/// A chain of receipts linked by hash pointers
///
/// Each receipt in the chain contains a hash of the previous receipt,
/// creating a tamper-evident chain similar to a blockchain.
#[cfg(feature = "crypto")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptChain {
    /// Ordered list of receipts
    receipts: Vec<Receipt>,

    /// Hash pointers linking each receipt to its predecessor
    hash_pointers: Vec<String>,
}

#[cfg(feature = "crypto")]
impl ReceiptChain {
    /// Create a new empty receipt chain
    pub fn new() -> Self {
        Self {
            receipts: Vec::new(),
            hash_pointers: Vec::new(),
        }
    }

    /// Add a receipt to the chain
    pub fn add_receipt(&mut self, receipt: Receipt) {
        let prev_hash = self
            .receipts
            .last()
            .map(|r| r.compute_hash())
            .unwrap_or_else(|| String::from("genesis"));

        self.hash_pointers.push(prev_hash);
        self.receipts.push(receipt);
    }

    /// Verify the entire chain
    pub fn verify_chain(&self, verifying_key: &VerifyingKey) -> ReceiptResult<()> {
        if self.receipts.is_empty() {
            return Err(ReceiptError::EmptyChain);
        }

        // Verify each receipt's signature
        for (idx, receipt) in self.receipts.iter().enumerate() {
            receipt
                .verify(verifying_key)
                .map_err(|e| ReceiptError::ChainBroken {
                    index: idx,
                    reason: format!("Signature verification failed: {}", e),
                })?;
        }

        // Verify hash pointers
        for idx in 1..self.receipts.len() {
            let expected_hash = self.receipts[idx - 1].compute_hash();
            let actual_hash = &self.hash_pointers[idx];

            if expected_hash != *actual_hash {
                return Err(ReceiptError::ChainBroken {
                    index: idx,
                    reason: format!(
                        "Hash pointer mismatch: expected {}, got {}",
                        expected_hash, actual_hash
                    ),
                });
            }
        }

        Ok(())
    }

    /// Get all receipts in the chain
    pub fn receipts(&self) -> &[Receipt] {
        &self.receipts
    }

    /// Get the length of the chain
    pub fn len(&self) -> usize {
        self.receipts.len()
    }

    /// Check if the chain is empty
    pub fn is_empty(&self) -> bool {
        self.receipts.is_empty()
    }

    /// Get a specific receipt by index
    pub fn get(&self, index: usize) -> Option<&Receipt> {
        self.receipts.get(index)
    }
}

#[cfg(feature = "crypto")]
impl Default for ReceiptChain {
    fn default() -> Self {
        Self::new()
    }
}

/// A Merkle tree node for batch receipt verification
#[cfg(feature = "crypto")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MerkleNode {
    hash: String,
    left: Option<Box<MerkleNode>>,
    right: Option<Box<MerkleNode>>,
}

/// Merkle tree for efficient batch receipt verification
///
/// Allows verification of a large set of receipts with a single root hash,
/// and verification of individual receipts with a Merkle proof.
#[cfg(feature = "crypto")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MerkleTree {
    root: Option<MerkleNode>,
    leaves: Vec<String>,
}

#[cfg(feature = "crypto")]
impl MerkleTree {
    /// Build a Merkle tree from a list of receipts
    pub fn from_receipts(receipts: &[Receipt]) -> Self {
        let leaves: Vec<String> = receipts.iter().map(|r| r.compute_hash()).collect();
        let root = Self::build_tree(&leaves);

        Self { root, leaves }
    }

    /// Build a Merkle tree from leaf hashes
    fn build_tree(leaves: &[String]) -> Option<MerkleNode> {
        if leaves.is_empty() {
            return None;
        }

        if leaves.len() == 1 {
            return Some(MerkleNode {
                hash: leaves[0].clone(),
                left: None,
                right: None,
            });
        }

        let mid = leaves.len() / 2;
        let left = Self::build_tree(&leaves[..mid]);
        let right = Self::build_tree(&leaves[mid..]);

        let combined_hash = match (&left, &right) {
            (Some(l), Some(r)) => Self::hash_pair(&l.hash, &r.hash),
            (Some(l), None) => l.hash.clone(),
            (None, Some(r)) => r.hash.clone(),
            (None, None) => return None,
        };

        Some(MerkleNode {
            hash: combined_hash,
            left: left.map(Box::new),
            right: right.map(Box::new),
        })
    }

    /// Get the root hash of the tree
    pub fn root_hash(&self) -> Option<String> {
        self.root.as_ref().map(|node| node.hash.clone())
    }

    /// Generate a Merkle proof for a specific receipt
    pub fn generate_proof(&self, receipt: &Receipt) -> ReceiptResult<Vec<(String, bool)>> {
        let target_hash = receipt.compute_hash();
        let index = self
            .leaves
            .iter()
            .position(|h| h == &target_hash)
            .ok_or_else(|| ReceiptError::InvalidData("Receipt not found in tree".to_string()))?;

        let mut proof = Vec::new();
        if let Some(root) = &self.root {
            self.collect_proof_from_node(root, index, 0, self.leaves.len(), &mut proof);
        }
        Ok(proof)
    }

    /// Collect proof hashes for a specific index from bottom to top
    /// Proof elements are tuples of (hash, is_right_sibling)
    fn collect_proof_from_node(
        &self,
        node: &MerkleNode,
        target_index: usize,
        start: usize,
        end: usize,
        proof: &mut Vec<(String, bool)>,
    ) {
        if end - start == 1 {
            return;
        }

        let mid = (start + end) / 2;

        if target_index < mid {
            // Target is in left subtree
            if let Some(left) = &node.left {
                self.collect_proof_from_node(left, target_index, start, mid, proof);
            }
            // Add right sibling hash to proof
            if let Some(right) = &node.right {
                proof.push((right.hash.clone(), true));
            }
        } else {
            // Target is in right subtree
            if let Some(right) = &node.right {
                self.collect_proof_from_node(right, target_index, mid, end, proof);
            }
            // Add left sibling hash to proof
            if let Some(left) = &node.left {
                proof.push((left.hash.clone(), false));
            }
        }
    }

    /// Verify a receipt using a Merkle proof
    pub fn verify_proof(
        receipt: &Receipt,
        proof: &[(String, bool)],
        root_hash: &str,
    ) -> ReceiptResult<()> {
        let mut current_hash = receipt.compute_hash();

        for (sibling_hash, is_right) in proof {
            current_hash = if *is_right {
                // Sibling is on the right, current is on the left
                Self::hash_pair(&current_hash, sibling_hash)
            } else {
                // Sibling is on the left, current is on the right
                Self::hash_pair(sibling_hash, &current_hash)
            };
        }

        if current_hash == root_hash {
            Ok(())
        } else {
            Err(ReceiptError::MerkleVerificationFailed(format!(
                "Computed hash {} does not match root hash {}",
                current_hash, root_hash
            )))
        }
    }

    /// Hash a pair of strings
    fn hash_pair(left: &str, right: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(left.as_bytes());
        hasher.update(right.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Replay validator for deterministic build verification
///
/// Ensures that regenerating output from the same ontology produces
/// identical results, proving determinism and reproducibility.
#[cfg(feature = "crypto")]
#[derive(Debug, Clone)]
pub struct ReplayValidator {
    recorded_receipts: Vec<Receipt>,
}

#[cfg(feature = "crypto")]
impl ReplayValidator {
    /// Create a new replay validator
    pub fn new() -> Self {
        Self {
            recorded_receipts: Vec::new(),
        }
    }

    /// Record a receipt for later replay validation
    pub fn record(&mut self, receipt: Receipt) {
        self.recorded_receipts.push(receipt);
    }

    /// Validate that a new receipt matches a recorded one
    pub fn validate_replay(&self, index: usize, new_receipt: &Receipt) -> ReceiptResult<()> {
        let recorded = self.recorded_receipts.get(index).ok_or_else(|| {
            ReceiptError::ReplayValidationFailed(format!("No recorded receipt at index {}", index))
        })?;

        // Verify ontology hash matches (same input)
        if recorded.ontology_hash != new_receipt.ontology_hash {
            return Err(ReceiptError::ReplayValidationFailed(format!(
                "Ontology hash mismatch at index {}: expected {}, got {}",
                index, recorded.ontology_hash, new_receipt.ontology_hash
            )));
        }

        // Verify output hash matches (deterministic output)
        if recorded.output_hash != new_receipt.output_hash {
            return Err(ReceiptError::ReplayValidationFailed(format!(
                "Output hash mismatch at index {}: expected {}, got {}. Build is not deterministic!",
                index, recorded.output_hash, new_receipt.output_hash
            )));
        }

        Ok(())
    }

    /// Validate an entire replay sequence
    pub fn validate_replay_sequence(&self, new_receipts: &[Receipt]) -> ReceiptResult<()> {
        if new_receipts.len() != self.recorded_receipts.len() {
            return Err(ReceiptError::ReplayValidationFailed(format!(
                "Receipt count mismatch: expected {}, got {}",
                self.recorded_receipts.len(),
                new_receipts.len()
            )));
        }

        for (idx, new_receipt) in new_receipts.iter().enumerate() {
            self.validate_replay(idx, new_receipt)?;
        }

        Ok(())
    }

    /// Get all recorded receipts
    pub fn recorded_receipts(&self) -> &[Receipt] {
        &self.recorded_receipts
    }

    /// Clear all recorded receipts
    pub fn clear(&mut self) {
        self.recorded_receipts.clear();
    }
}

#[cfg(feature = "crypto")]
impl Default for ReplayValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "crypto"))]
mod tests {
    use super::*;
    use rand::RngCore;

    fn generate_keypair() -> (SigningKey, VerifyingKey) {
        let mut rng = rand::rngs::OsRng;
        let mut seed = [0u8; 32];
        rng.fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }

    #[test]
    fn test_receipt_creation_and_verification() {
        let (signing_key, verifying_key) = generate_keypair();
        let ontology_hash = Receipt::hash_data(b"ontology data");
        let output_hash = Receipt::hash_data(b"output data");

        let receipt = Receipt::new(ontology_hash, output_hash, &signing_key, None)
            .expect("Failed to create receipt");

        assert!(receipt.verify(&verifying_key).is_ok());
    }

    #[test]
    fn test_receipt_tamper_detection() {
        let (signing_key, verifying_key) = generate_keypair();
        let ontology_hash = Receipt::hash_data(b"ontology data");
        let output_hash = Receipt::hash_data(b"output data");

        let mut receipt = Receipt::new(ontology_hash, output_hash, &signing_key, None)
            .expect("Failed to create receipt");

        // Tamper with the output hash
        receipt.output_hash = Receipt::hash_data(b"tampered data");

        // Verification should fail
        assert!(receipt.verify(&verifying_key).is_err());
    }

    #[test]
    fn test_receipt_chain() {
        let (signing_key, verifying_key) = generate_keypair();
        let mut chain = ReceiptChain::new();

        for i in 0..5 {
            let ontology_hash = Receipt::hash_data(format!("ontology-{}", i).as_bytes());
            let output_hash = Receipt::hash_data(format!("output-{}", i).as_bytes());
            let receipt = Receipt::new(ontology_hash, output_hash, &signing_key, None)
                .expect("Failed to create receipt");
            chain.add_receipt(receipt);
        }

        assert_eq!(chain.len(), 5);
        assert!(chain.verify_chain(&verifying_key).is_ok());
    }

    #[test]
    fn test_merkle_tree() {
        let (signing_key, _verifying_key) = generate_keypair();
        let mut receipts = Vec::new();

        for i in 0..8 {
            let ontology_hash = Receipt::hash_data(format!("ontology-{}", i).as_bytes());
            let output_hash = Receipt::hash_data(format!("output-{}", i).as_bytes());
            let receipt = Receipt::new(ontology_hash, output_hash, &signing_key, None)
                .expect("Failed to create receipt");
            receipts.push(receipt);
        }

        let tree = MerkleTree::from_receipts(&receipts);
        let root_hash = tree.root_hash().expect("Root hash should exist");

        // Verify each receipt with a proof
        for receipt in &receipts {
            let proof = tree
                .generate_proof(receipt)
                .expect("Failed to generate proof");
            assert!(MerkleTree::verify_proof(receipt, &proof, &root_hash).is_ok());
        }
    }

    #[test]
    fn test_replay_validation() {
        let (signing_key, _verifying_key) = generate_keypair();
        let mut validator = ReplayValidator::new();

        // Record original receipts
        for i in 0..3 {
            let ontology_hash = Receipt::hash_data(format!("ontology-{}", i).as_bytes());
            let output_hash = Receipt::hash_data(format!("output-{}", i).as_bytes());
            let receipt = Receipt::new(ontology_hash, output_hash, &signing_key, None)
                .expect("Failed to create receipt");
            validator.record(receipt);
        }

        // Replay with same inputs should produce same outputs
        let mut new_receipts = Vec::new();
        for i in 0..3 {
            let ontology_hash = Receipt::hash_data(format!("ontology-{}", i).as_bytes());
            let output_hash = Receipt::hash_data(format!("output-{}", i).as_bytes());
            let receipt = Receipt::new(ontology_hash, output_hash, &signing_key, None)
                .expect("Failed to create receipt");
            new_receipts.push(receipt);
        }

        assert!(validator.validate_replay_sequence(&new_receipts).is_ok());
    }

    #[test]
    fn test_replay_validation_detects_nondeterminism() {
        let (signing_key, _verifying_key) = generate_keypair();
        let mut validator = ReplayValidator::new();

        // Record original receipt
        let ontology_hash = Receipt::hash_data(b"ontology data");
        let output_hash = Receipt::hash_data(b"output data");
        let receipt = Receipt::new(ontology_hash.clone(), output_hash, &signing_key, None)
            .expect("Failed to create receipt");
        validator.record(receipt);

        // Replay with different output (non-deterministic)
        let new_output_hash = Receipt::hash_data(b"different output");
        let new_receipt = Receipt::new(ontology_hash, new_output_hash, &signing_key, None)
            .expect("Failed to create receipt");

        assert!(validator.validate_replay(0, &new_receipt).is_err());
    }
}
