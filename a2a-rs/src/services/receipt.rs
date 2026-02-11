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

#[cfg(feature = "crypto")]
use crate::domain::core::message::Artifact;

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

    /// Create a receipt from an artifact
    ///
    /// This integrates receipts with the artifact system by computing
    /// hashes from the artifact's serialized representation.
    pub fn from_artifact(
        artifact: &Artifact,
        ontology_hash: String,
        signing_key: &SigningKey,
    ) -> ReceiptResult<Self> {
        let artifact_json = serde_json::to_vec(artifact)
            .map_err(|e| ReceiptError::InvalidData(e.to_string()))?;

        let output_hash = Self::hash_data(&artifact_json);

        Self::new(ontology_hash, output_hash, signing_key, None)
    }

    /// Verify this receipt matches an artifact's hash
    pub fn verify_artifact(&self, artifact: &Artifact) -> ReceiptResult<bool> {
        let artifact_json = serde_json::to_vec(artifact)
            .map_err(|e| ReceiptError::InvalidData(e.to_string()))?;

        let computed_hash = Self::hash_data(&artifact_json);
        Ok(self.output_hash == computed_hash)
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

    /// Add a receipt from an artifact to the chain
    pub fn add_artifact_receipt(
        &mut self,
        artifact: &Artifact,
        ontology_hash: String,
        signing_key: &SigningKey,
    ) -> ReceiptResult<()> {
        let receipt = Receipt::from_artifact(artifact, ontology_hash, signing_key)?;
        self.add_receipt(receipt);
        Ok(())
    }

    /// Verify that a receipt chain contains valid artifact receipts
    pub fn verify_artifact_chain(
        &self,
        artifacts: &[Artifact],
        verifying_key: &VerifyingKey,
    ) -> ReceiptResult<()> {
        // First verify the chain signatures
        self.verify_chain(verifying_key)?;

        // Then verify each receipt matches its artifact
        for (receipt, artifact) in self.receipts.iter().zip(artifacts.iter()) {
            if !receipt.verify_artifact(artifact)? {
                return Err(ReceiptError::InvalidHash {
                    expected: receipt.output_hash.clone(),
                    actual: format!(
                        "{}",
                        &receipt.compute_hash()[..16]
                    ),
                });
            }
        }

        Ok(())
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

/// Agent card signing utilities (crypto feature)
///
/// Provides functions for signing agent cards with receipts to establish
/// cryptographic provenance and integrity verification.
#[cfg(feature = "crypto")]
pub struct AgentCardSigner;

#[cfg(feature = "crypto")]
impl AgentCardSigner {
    /// Sign an agent card with a receipt proving its artifact history
    ///
    /// This creates a receipt that links the agent card's hash to
    /// its artifact generation history, establishing verifiable provenance.
    pub fn sign_agent_card(
        agent_card_hash: String,
        artifact_hashes: Vec<String>,
        signing_key: &SigningKey,
    ) -> ReceiptResult<Receipt> {
        // Compute combined hash of all artifacts
        let mut combined_hasher = Sha256::new();
        for hash in &artifact_hashes {
            combined_hasher.update(hash.as_bytes());
        }
        let artifacts_hash = hex::encode(combined_hasher.finalize());

        Receipt::new(agent_card_hash, artifacts_hash, signing_key, None)
    }

    /// Verify an agent card's receipt chain
    ///
    /// Ensures that the agent card has valid receipts proving
    /// its artifact generation history.
    pub fn verify_agent_card(
        agent_card_hash: &str,
        receipts: &[Receipt],
        verifying_key: &VerifyingKey,
    ) -> ReceiptResult<()> {
        if receipts.is_empty() {
            return Err(ReceiptError::InvalidData(
                "Agent card has no receipts for verification".to_string(),
            ));
        }

        // The first receipt should link the agent card hash to artifact history
        let first_receipt = &receipts[0];

        if first_receipt.ontology_hash != agent_card_hash {
            return Err(ReceiptError::InvalidHash {
                expected: agent_card_hash.to_string(),
                actual: first_receipt.ontology_hash.clone(),
            });
        }

        // Verify the first receipt's signature
        first_receipt.verify(verifying_key)?;

        // If there are more receipts, verify the full chain
        if receipts.len() > 1 {
            let chain = ReceiptChain {
                receipts: receipts.to_vec(),
                hash_pointers: receipts
                    .windows(2)
                    .map(|w| w[0].compute_hash())
                    .collect(),
            };
            chain.verify_chain(verifying_key)?;
        }

        Ok(())
    }
}

#[cfg(all(test, feature = "crypto"))]
mod tests {
    use super::*;
    use crate::domain::core::message::{Artifact, Part};
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

    #[test]
    fn test_receipt_from_artifact() {
        let (signing_key, verifying_key) = generate_keypair();

        // Create an artifact
        let artifact = Artifact {
            artifact_id: "artifact-123".to_string(),
            name: Some("Test Artifact".to_string()),
            description: Some("A test artifact for receipt verification".to_string()),
            parts: vec![Part::Text {
                text: "Test content".to_string(),
                metadata: None,
            }],
            metadata: None,
            extensions: None,
        };

        // Create receipt from artifact
        let ontology_hash = Receipt::hash_data(b"test-ontology");
        let receipt = Receipt::from_artifact(&artifact, ontology_hash, &signing_key)
            .expect("Failed to create receipt from artifact");

        // Verify the receipt
        assert!(receipt.verify(&verifying_key).is_ok());

        // Verify receipt matches artifact
        assert!(receipt.verify_artifact(&artifact).expect("Failed to verify artifact"));

        // Tamper with artifact and verify detection
        let tampered_artifact = Artifact {
            artifact_id: artifact.artifact_id.clone(),
            name: artifact.name.clone(),
            description: artifact.description.clone(),
            parts: vec![Part::Text {
                text: "Tampered content".to_string(),
                metadata: None,
            }],
            metadata: None,
            extensions: None,
        };

        assert!(!receipt
            .verify_artifact(&tampered_artifact)
            .expect("Failed to verify tampered artifact"));
    }

    #[test]
    fn test_receipt_chain_with_artifacts() {
        let (signing_key, verifying_key) = generate_keypair();
        let mut chain = ReceiptChain::new();

        // Create multiple artifacts
        let mut artifacts = Vec::new();
        for i in 0..3 {
            let artifact = Artifact {
                artifact_id: format!("artifact-{}", i),
                name: Some(format!("Artifact {}", i)),
                description: Some(format!("Test artifact {}", i)),
                parts: vec![Part::Text {
                    text: format!("Content {}", i),
                    metadata: None,
                }],
                metadata: None,
                extensions: None,
            };

            let ontology_hash = Receipt::hash_data(format!("ontology-{}", i).as_bytes());
            chain
                .add_artifact_receipt(&artifact, ontology_hash, &signing_key)
                .expect("Failed to add artifact receipt");
            artifacts.push(artifact);
        }

        // Verify the chain with artifacts
        assert!(chain.verify_artifact_chain(&artifacts, &verifying_key).is_ok());
    }

    #[test]
    fn test_agent_card_signing() {
        let (signing_key, verifying_key) = generate_keypair();

        // Simulate agent card and artifact hashes
        let agent_card_hash = Receipt::hash_data(b"agent-card-data");
        let artifact_hashes = vec![
            Receipt::hash_data(b"artifact-1"),
            Receipt::hash_data(b"artifact-2"),
            Receipt::hash_data(b"artifact-3"),
        ];

        // Sign the agent card
        let receipt = AgentCardSigner::sign_agent_card(
            agent_card_hash.clone(),
            artifact_hashes.clone(),
            &signing_key,
        )
        .expect("Failed to sign agent card");

        // Verify the signature
        assert!(receipt.verify(&verifying_key).is_ok());

        // Verify the agent card with receipt
        let receipts = vec![receipt];
        assert!(AgentCardSigner::verify_agent_card(&agent_card_hash, &receipts, &verifying_key).is_ok());
    }

    #[test]
    fn test_agent_card_verification_fails_on_tamper() {
        let (signing_key, verifying_key) = generate_keypair();

        let agent_card_hash = Receipt::hash_data(b"original-agent-card");
        let artifact_hashes = vec![Receipt::hash_data(b"artifact-1")];

        let receipt = AgentCardSigner::sign_agent_card(
            agent_card_hash.clone(),
            artifact_hashes,
            &signing_key,
        )
        .expect("Failed to sign agent card");

        let receipts = vec![receipt];

        // Try to verify with wrong agent card hash
        let tampered_hash = Receipt::hash_data(b"tampered-agent-card");
        assert!(AgentCardSigner::verify_agent_card(&tampered_hash, &receipts, &verifying_key).is_err());
    }

    #[test]
    fn test_merkle_tree_with_artifacts() {
        let (signing_key, _verifying_key) = generate_keypair();
        let mut receipts = Vec::new();

        // Create receipts from artifacts
        for i in 0..4 {
            let artifact = Artifact {
                artifact_id: format!("artifact-{}", i),
                name: Some(format!("Artifact {}", i)),
                description: None,
                parts: vec![Part::Text {
                    text: format!("Content {}", i),
                    metadata: None,
                }],
                metadata: None,
                extensions: None,
            };

            let ontology_hash = Receipt::hash_data(format!("ontology-{}", i).as_bytes());
            let receipt = Receipt::from_artifact(&artifact, ontology_hash, &signing_key)
                .expect("Failed to create receipt");
            receipts.push(receipt);
        }

        // Build Merkle tree
        let tree = MerkleTree::from_receipts(&receipts);
        let root_hash = tree.root_hash().expect("Should have root hash");

        // Verify each receipt with Merkle proof
        for receipt in &receipts {
            let proof = tree.generate_proof(receipt).expect("Failed to generate proof");
            assert!(MerkleTree::verify_proof(receipt, &proof, &root_hash).is_ok());
        }
    }
}
