//! Receipt chain system for replayable proofs of agent state transitions.
//!
//! This module provides cryptographic receipts that bind input observations to output
//! behaviors and state changes. Receipts form a tamper-proof chain that enables proving
//! "the system did what it did" without AI explanations.
//!
//! # Design
//!
//! A Receipt captures a single state transition with three components:
//! - **Observation (O_i)**: The input state or trigger
//! - **Action (A_i)**: The output behavior or response
//! - **Delta (Δ_i)**: The state change or effect
//!
//! Each receipt computes: `hash(O_i) || hash(A_i) || hash(Δ_i)`
//!
//! # Features
//!
//! - SHA-256 hashing for deterministic receipt generation
//! - Optional ed25519 signing for tamper-proof chains
//! - Sequential chain linking with integrity verification
//! - Replay verification to detect tampering
//!
//! # Examples
//!
//! ```rust
//! # #[cfg(feature = "receipts")]
//! # {
//! use a2a_rs::construct::receipts::{Receipt, ReceiptChain};
//!
//! // Create a receipt for a state transition
//! let receipt = Receipt::new(
//!     b"user query: what is 2+2?",
//!     b"assistant response: 4",
//!     b"state: query_count += 1",
//! );
//!
//! // Create a chain and add receipts
//! let mut chain = ReceiptChain::new();
//! chain.add_receipt(receipt);
//!
//! // Verify chain integrity
//! assert!(chain.verify_integrity().is_ok());
//! # }
//! ```

#[cfg(feature = "receipts")]
use chrono::{DateTime, Utc};
#[cfg(feature = "receipts")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "receipts")]
use sha2::{Digest, Sha256};
#[cfg(feature = "receipts")]
use std::fmt;

#[cfg(feature = "receipts-signing")]
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

#[cfg(feature = "receipts")]
use thiserror::Error;

/// Errors that can occur during receipt operations
#[cfg(feature = "receipts")]
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ReceiptError {
    /// Chain integrity check failed - receipts don't link properly
    #[error("Chain integrity violation at index {index}: {reason}")]
    ChainIntegrityViolation { index: usize, reason: String },

    /// Signature verification failed
    #[cfg(feature = "receipts-signing")]
    #[error("Signature verification failed for receipt {receipt_hash}: {reason}")]
    SignatureVerificationFailed {
        receipt_hash: String,
        reason: String,
    },

    /// Invalid signature format
    #[cfg(feature = "receipts-signing")]
    #[error("Invalid signature format: {0}")]
    InvalidSignature(String),

    /// Invalid public key format
    #[cfg(feature = "receipts-signing")]
    #[error("Invalid public key format: {0}")]
    InvalidPublicKey(String),

    /// Chain is empty
    #[error("Cannot verify empty chain")]
    EmptyChain,

    /// Hex decoding error
    #[error("Hex decode error: {0}")]
    HexDecode(String),
}

/// A cryptographic receipt binding observation, action, and state delta.
///
/// Receipt format: `hash(O_i) || hash(A_i) || hash(Δ_i)`
/// where || denotes concatenation and hash is SHA-256.
#[cfg(feature = "receipts")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    /// Unique sequence number for ordering
    pub sequence: u64,

    /// Timestamp when the receipt was created
    pub timestamp: DateTime<Utc>,

    /// Hash of the observation (input state)
    pub observation_hash: String,

    /// Hash of the action (output behavior)
    pub action_hash: String,

    /// Hash of the state delta (change)
    pub delta_hash: String,

    /// Combined receipt hash: hash(observation_hash || action_hash || delta_hash)
    pub receipt_hash: String,

    /// Hash of the previous receipt in the chain (None for genesis receipt)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_hash: Option<String>,

    /// Optional signature over the receipt_hash
    #[cfg(feature = "receipts-signing")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,

    /// Optional public key for signature verification
    #[cfg(feature = "receipts-signing")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,

    /// Optional metadata for additional context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[cfg(feature = "receipts")]
impl Receipt {
    /// Creates a new receipt from observation, action, and delta.
    ///
    /// # Arguments
    ///
    /// * `observation` - The input state or trigger
    /// * `action` - The output behavior or response
    /// * `delta` - The state change or effect
    ///
    /// # Returns
    ///
    /// A new Receipt with computed hashes and current timestamp.
    pub fn new(observation: &[u8], action: &[u8], delta: &[u8]) -> Self {
        let observation_hash = compute_hash(observation);
        let action_hash = compute_hash(action);
        let delta_hash = compute_hash(delta);

        let receipt_hash = compute_receipt_hash(&observation_hash, &action_hash, &delta_hash);

        Self {
            sequence: 0, // Will be set by chain
            timestamp: Utc::now(),
            observation_hash,
            action_hash,
            delta_hash,
            receipt_hash,
            previous_hash: None,
            #[cfg(feature = "receipts-signing")]
            signature: None,
            #[cfg(feature = "receipts-signing")]
            public_key: None,
            metadata: None,
        }
    }

    /// Creates a new receipt with a specific sequence number and previous hash.
    ///
    /// This is used internally by ReceiptChain to maintain proper linking.
    pub fn with_chain_context(
        observation: &[u8],
        action: &[u8],
        delta: &[u8],
        sequence: u64,
        previous_hash: Option<String>,
    ) -> Self {
        let mut receipt = Self::new(observation, action, delta);
        receipt.sequence = sequence;
        receipt.previous_hash = previous_hash;
        receipt
    }

    /// Signs the receipt with the provided signing key.
    ///
    /// The signature is computed over the receipt_hash.
    #[cfg(feature = "receipts-signing")]
    pub fn sign(&mut self, signing_key: &SigningKey) {
        let signature = signing_key.sign(self.receipt_hash.as_bytes());
        self.signature = Some(hex::encode(signature.to_bytes()));
        self.public_key = Some(hex::encode(signing_key.verifying_key().to_bytes()));
    }

    /// Verifies the receipt's signature.
    ///
    /// Returns Ok(()) if the signature is valid, or an error if verification fails.
    #[cfg(feature = "receipts-signing")]
    pub fn verify_signature(&self) -> Result<(), ReceiptError> {
        let signature =
            self.signature
                .as_ref()
                .ok_or_else(|| ReceiptError::SignatureVerificationFailed {
                    receipt_hash: self.receipt_hash.clone(),
                    reason: "No signature present".to_string(),
                })?;

        let public_key =
            self.public_key
                .as_ref()
                .ok_or_else(|| ReceiptError::SignatureVerificationFailed {
                    receipt_hash: self.receipt_hash.clone(),
                    reason: "No public key present".to_string(),
                })?;

        let signature_bytes =
            hex::decode(signature).map_err(|e| ReceiptError::InvalidSignature(e.to_string()))?;
        let signature =
            Signature::from_bytes(&signature_bytes.try_into().map_err(|_| {
                ReceiptError::InvalidSignature("Invalid signature length".to_string())
            })?);

        let public_key_bytes =
            hex::decode(public_key).map_err(|e| ReceiptError::InvalidPublicKey(e.to_string()))?;
        let verifying_key =
            VerifyingKey::from_bytes(&public_key_bytes.try_into().map_err(|_| {
                ReceiptError::InvalidPublicKey("Invalid public key length".to_string())
            })?)
            .map_err(|e| ReceiptError::InvalidPublicKey(e.to_string()))?;

        verifying_key
            .verify(self.receipt_hash.as_bytes(), &signature)
            .map_err(|e| ReceiptError::SignatureVerificationFailed {
                receipt_hash: self.receipt_hash.clone(),
                reason: e.to_string(),
            })
    }

    /// Verifies that the receipt's internal hashes are consistent.
    ///
    /// This checks that the receipt_hash correctly combines the component hashes.
    pub fn verify_hashes(&self) -> Result<(), ReceiptError> {
        let expected_receipt_hash =
            compute_receipt_hash(&self.observation_hash, &self.action_hash, &self.delta_hash);

        if expected_receipt_hash != self.receipt_hash {
            return Err(ReceiptError::ChainIntegrityViolation {
                index: self.sequence as usize,
                reason: format!(
                    "Receipt hash mismatch: expected {}, got {}",
                    expected_receipt_hash, self.receipt_hash
                ),
            });
        }

        Ok(())
    }

    /// Adds metadata to the receipt.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

#[cfg(feature = "receipts")]
impl fmt::Display for Receipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Receipt(seq={}, hash={}, prev={:?})",
            self.sequence,
            &self.receipt_hash[..16],
            self.previous_hash.as_ref().map(|h| &h[..16])
        )
    }
}

/// A chain of receipts forming a tamper-proof audit trail.
///
/// Each receipt links to the previous one via cryptographic hashes, creating
/// a structure similar to a blockchain. The chain can be verified for integrity
/// and optionally for cryptographic signatures.
#[cfg(feature = "receipts")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptChain {
    /// The receipts in the chain, in sequential order
    pub receipts: Vec<Receipt>,

    /// Chain metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[cfg(feature = "receipts")]
impl ReceiptChain {
    /// Creates a new empty receipt chain.
    pub fn new() -> Self {
        Self {
            receipts: Vec::new(),
            metadata: None,
        }
    }

    /// Adds a receipt to the chain.
    ///
    /// The receipt will be assigned the next sequence number and linked to
    /// the previous receipt's hash.
    pub fn add_receipt(&mut self, mut receipt: Receipt) {
        let sequence = self.receipts.len() as u64;
        let previous_hash = self.receipts.last().map(|r| r.receipt_hash.clone());

        receipt.sequence = sequence;
        receipt.previous_hash = previous_hash;

        self.receipts.push(receipt);
    }

    /// Creates and adds a new receipt from raw components.
    ///
    /// This is a convenience method that creates a Receipt and adds it in one step.
    pub fn add_transition(&mut self, observation: &[u8], action: &[u8], delta: &[u8]) -> &Receipt {
        let sequence = self.receipts.len() as u64;
        let previous_hash = self.receipts.last().map(|r| r.receipt_hash.clone());

        let receipt =
            Receipt::with_chain_context(observation, action, delta, sequence, previous_hash);

        self.receipts.push(receipt);
        self.receipts.last().unwrap()
    }

    /// Creates and adds a signed receipt from raw components.
    #[cfg(feature = "receipts-signing")]
    pub fn add_signed_transition(
        &mut self,
        observation: &[u8],
        action: &[u8],
        delta: &[u8],
        signing_key: &SigningKey,
    ) -> &Receipt {
        let sequence = self.receipts.len() as u64;
        let previous_hash = self.receipts.last().map(|r| r.receipt_hash.clone());

        let mut receipt =
            Receipt::with_chain_context(observation, action, delta, sequence, previous_hash);
        receipt.sign(signing_key);

        self.receipts.push(receipt);
        self.receipts.last().unwrap()
    }

    /// Verifies the integrity of the entire receipt chain.
    ///
    /// This checks:
    /// 1. All receipts have correct internal hashes
    /// 2. All receipts link properly to their predecessors
    /// 3. Sequence numbers are consecutive
    /// 4. If signatures are present, they are valid
    ///
    /// Returns Ok(()) if the chain is valid, or the first error encountered.
    pub fn verify_integrity(&self) -> Result<(), ReceiptError> {
        if self.receipts.is_empty() {
            return Err(ReceiptError::EmptyChain);
        }

        for (i, receipt) in self.receipts.iter().enumerate() {
            // Verify sequence number
            if receipt.sequence != i as u64 {
                return Err(ReceiptError::ChainIntegrityViolation {
                    index: i,
                    reason: format!(
                        "Sequence mismatch: expected {}, got {}",
                        i, receipt.sequence
                    ),
                });
            }

            // Verify internal hashes
            receipt.verify_hashes()?;

            // Verify chain linking
            if i == 0 {
                if receipt.previous_hash.is_some() {
                    return Err(ReceiptError::ChainIntegrityViolation {
                        index: i,
                        reason: "Genesis receipt should not have previous hash".to_string(),
                    });
                }
            } else {
                let expected_prev = &self.receipts[i - 1].receipt_hash;
                match &receipt.previous_hash {
                    Some(prev) if prev == expected_prev => {}
                    Some(prev) => {
                        return Err(ReceiptError::ChainIntegrityViolation {
                            index: i,
                            reason: format!(
                                "Previous hash mismatch: expected {}, got {}",
                                expected_prev, prev
                            ),
                        });
                    }
                    None => {
                        return Err(ReceiptError::ChainIntegrityViolation {
                            index: i,
                            reason: "Missing previous hash".to_string(),
                        });
                    }
                }
            }

            // Verify signature if present
            #[cfg(feature = "receipts-signing")]
            if receipt.signature.is_some() {
                receipt.verify_signature()?;
            }
        }

        Ok(())
    }

    /// Returns the number of receipts in the chain.
    pub fn len(&self) -> usize {
        self.receipts.len()
    }

    /// Returns true if the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.receipts.is_empty()
    }

    /// Gets a receipt by sequence number.
    pub fn get(&self, sequence: u64) -> Option<&Receipt> {
        self.receipts.get(sequence as usize)
    }

    /// Gets the most recent receipt in the chain.
    pub fn latest(&self) -> Option<&Receipt> {
        self.receipts.last()
    }

    /// Returns an iterator over all receipts.
    pub fn iter(&self) -> impl Iterator<Item = &Receipt> {
        self.receipts.iter()
    }

    /// Adds metadata to the chain.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Exports the chain as JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Imports a chain from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(feature = "receipts")]
impl Default for ReceiptChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "receipts")]
impl fmt::Display for ReceiptChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ReceiptChain(length={})", self.receipts.len())
    }
}

/// Computes the SHA-256 hash of the input data.
///
/// # Arguments
///
/// * `data` - The data to hash
///
/// # Returns
///
/// Hex-encoded hash string
#[cfg(feature = "receipts")]
pub fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Computes the combined receipt hash from component hashes.
///
/// Receipt hash = hash(observation_hash || action_hash || delta_hash)
///
/// # Arguments
///
/// * `observation_hash` - Hex-encoded hash of the observation
/// * `action_hash` - Hex-encoded hash of the action
/// * `delta_hash` - Hex-encoded hash of the delta
///
/// # Returns
///
/// Hex-encoded combined receipt hash
#[cfg(feature = "receipts")]
pub fn compute_receipt_hash(observation_hash: &str, action_hash: &str, delta_hash: &str) -> String {
    let combined = format!("{}{}{}", observation_hash, action_hash, delta_hash);
    compute_hash(combined.as_bytes())
}

#[cfg(all(test, feature = "receipts"))]
mod tests {
    use super::*;

    #[test]
    fn test_receipt_creation() {
        let receipt = Receipt::new(b"observation", b"action", b"delta");

        assert_eq!(receipt.sequence, 0);
        assert!(receipt.previous_hash.is_none());
        assert!(!receipt.observation_hash.is_empty());
        assert!(!receipt.action_hash.is_empty());
        assert!(!receipt.delta_hash.is_empty());
        assert!(!receipt.receipt_hash.is_empty());
    }

    #[test]
    fn test_receipt_hash_verification() {
        let receipt = Receipt::new(b"observation", b"action", b"delta");
        assert!(receipt.verify_hashes().is_ok());
    }

    #[test]
    fn test_receipt_chain_creation() {
        let mut chain = ReceiptChain::new();
        assert!(chain.is_empty());

        chain.add_transition(b"obs1", b"act1", b"delta1");
        chain.add_transition(b"obs2", b"act2", b"delta2");

        assert_eq!(chain.len(), 2);
        assert!(chain.verify_integrity().is_ok());
    }

    #[test]
    fn test_receipt_chain_linking() {
        let mut chain = ReceiptChain::new();

        chain.add_transition(b"obs1", b"act1", b"delta1");
        chain.add_transition(b"obs2", b"act2", b"delta2");
        chain.add_transition(b"obs3", b"act3", b"delta3");

        // Verify first receipt has no previous
        assert!(chain.get(0).unwrap().previous_hash.is_none());

        // Verify subsequent receipts link properly
        let receipt1_hash = &chain.get(0).unwrap().receipt_hash;
        assert_eq!(
            chain.get(1).unwrap().previous_hash.as_ref().unwrap(),
            receipt1_hash
        );

        let receipt2_hash = &chain.get(1).unwrap().receipt_hash;
        assert_eq!(
            chain.get(2).unwrap().previous_hash.as_ref().unwrap(),
            receipt2_hash
        );
    }

    #[test]
    fn test_chain_integrity_verification() {
        let mut chain = ReceiptChain::new();

        chain.add_transition(b"obs1", b"act1", b"delta1");
        chain.add_transition(b"obs2", b"act2", b"delta2");

        assert!(chain.verify_integrity().is_ok());
    }

    #[test]
    fn test_chain_tamper_detection() {
        let mut chain = ReceiptChain::new();

        chain.add_transition(b"obs1", b"act1", b"delta1");
        chain.add_transition(b"obs2", b"act2", b"delta2");

        // Tamper with a receipt
        chain.receipts[1].observation_hash = "tampered".to_string();

        // Verification should fail
        assert!(chain.verify_integrity().is_err());
    }

    #[cfg(feature = "receipts-signing")]
    #[test]
    fn test_receipt_signing() {
        use ed25519_dalek::SigningKey;
        use rand::{RngCore, rngs::OsRng};

        let mut rng = OsRng;
        let mut secret_bytes = [0u8; 32];
        rng.fill_bytes(&mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);

        let mut receipt = Receipt::new(b"observation", b"action", b"delta");
        receipt.sign(&signing_key);

        assert!(receipt.signature.is_some());
        assert!(receipt.public_key.is_some());
        assert!(receipt.verify_signature().is_ok());
    }

    #[cfg(feature = "receipts-signing")]
    #[test]
    fn test_signed_chain() {
        use ed25519_dalek::SigningKey;
        use rand::{RngCore, rngs::OsRng};

        let mut rng = OsRng;
        let mut secret_bytes = [0u8; 32];
        rng.fill_bytes(&mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);

        let mut chain = ReceiptChain::new();
        chain.add_signed_transition(b"obs1", b"act1", b"delta1", &signing_key);
        chain.add_signed_transition(b"obs2", b"act2", b"delta2", &signing_key);

        assert!(chain.verify_integrity().is_ok());
    }

    #[test]
    fn test_chain_json_serialization() {
        let mut chain = ReceiptChain::new();
        chain.add_transition(b"obs1", b"act1", b"delta1");
        chain.add_transition(b"obs2", b"act2", b"delta2");

        let json = chain.to_json().unwrap();
        let deserialized = ReceiptChain::from_json(&json).unwrap();

        assert_eq!(chain.len(), deserialized.len());
        assert!(deserialized.verify_integrity().is_ok());
    }

    #[test]
    fn test_deterministic_hashing() {
        let hash1 = compute_hash(b"test data");
        let hash2 = compute_hash(b"test data");

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_uniqueness() {
        let hash1 = compute_hash(b"data1");
        let hash2 = compute_hash(b"data2");

        assert_ne!(hash1, hash2);
    }
}
