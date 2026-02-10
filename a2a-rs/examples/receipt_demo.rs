//! Example demonstrating the cryptographic receipt validation system
//!
//! This example shows how to:
//! - Create and verify cryptographic receipts
//! - Build and verify receipt chains
//! - Use Merkle trees for batch verification
//! - Validate deterministic builds with replay validation

use a2a_rs::{MerkleTree, Receipt, ReceiptChain, ReplayValidator};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::RngCore;
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cryptographic Receipt Validation Demo ===\n");

    // Generate a keypair for signing receipts
    let (signing_key, verifying_key) = generate_keypair();
    println!("1. Generated ed25519 keypair for signing receipts");

    // Example 1: Create and verify a single receipt
    println!("\n2. Creating and verifying a single receipt...");
    let ontology_data = b"ontology specification v1.0";
    let output_data = b"generated output from ontology";

    let ontology_hash = hash_data(ontology_data);
    let output_hash = hash_data(output_data);

    let receipt = Receipt::new(
        ontology_hash.clone(),
        output_hash.clone(),
        &signing_key,
        Some(serde_json::json!({"version": "1.0", "generator": "a2a-rs"})),
    )?;

    println!("   Created receipt:");
    println!("   - Ontology hash: {}", &receipt.ontology_hash[..16]);
    println!("   - Output hash: {}", &receipt.output_hash[..16]);
    println!("   - Timestamp: {}", receipt.timestamp);
    println!("   - Signature: {}...", &receipt.signature[..16]);

    receipt.verify(&verifying_key)?;
    println!("   ✓ Receipt signature verified successfully!");

    // Example 2: Build and verify a receipt chain
    println!("\n3. Building a receipt chain...");
    let mut chain = ReceiptChain::new();

    for i in 0..5 {
        let ontology = format!("ontology-v{}", i);
        let output = format!("generated-output-v{}", i);

        let ont_hash = hash_data(ontology.as_bytes());
        let out_hash = hash_data(output.as_bytes());

        let receipt = Receipt::new(ont_hash, out_hash, &signing_key, None)?;
        chain.add_receipt(receipt);
    }

    println!("   Created chain with {} receipts", chain.len());

    chain.verify_chain(&verifying_key)?;
    println!("   ✓ Entire chain verified successfully!");
    println!("   ✓ All hash pointers validated!");

    // Example 3: Merkle tree for batch verification
    println!("\n4. Building Merkle tree for batch verification...");
    let mut receipts = Vec::new();

    for i in 0..8 {
        let ontology = format!("batch-ontology-{}", i);
        let output = format!("batch-output-{}", i);

        let ont_hash = hash_data(ontology.as_bytes());
        let out_hash = hash_data(output.as_bytes());

        let receipt = Receipt::new(ont_hash, out_hash, &signing_key, None)?;
        receipts.push(receipt);
    }

    let tree = MerkleTree::from_receipts(&receipts);
    let root_hash = tree.root_hash().expect("Tree should have a root");

    println!("   Created Merkle tree with {} leaves", receipts.len());
    println!("   Root hash: {}...", &root_hash[..16]);

    // Verify individual receipts with Merkle proofs
    for (i, receipt) in receipts.iter().enumerate() {
        let proof = tree.generate_proof(receipt)?;
        MerkleTree::verify_proof(receipt, &proof, &root_hash)?;
        println!(
            "   ✓ Receipt {} verified with Merkle proof ({} siblings)",
            i,
            proof.len()
        );
    }

    // Example 4: Replay validation for deterministic builds
    println!("\n5. Demonstrating replay validation...");
    let mut validator = ReplayValidator::new();

    // Record original build sequence
    println!("   Recording original build sequence...");
    for i in 0..3 {
        let ontology = format!("deterministic-ontology-{}", i);
        let output = format!("deterministic-output-{}", i);

        let ont_hash = hash_data(ontology.as_bytes());
        let out_hash = hash_data(output.as_bytes());

        let receipt = Receipt::new(ont_hash, out_hash, &signing_key, None)?;
        validator.record(receipt);
    }
    println!(
        "   Recorded {} receipts",
        validator.recorded_receipts().len()
    );

    // Replay with same inputs (should produce same outputs)
    println!("   Replaying build sequence...");
    let mut replay_receipts = Vec::new();
    for i in 0..3 {
        let ontology = format!("deterministic-ontology-{}", i);
        let output = format!("deterministic-output-{}", i); // Same output = deterministic

        let ont_hash = hash_data(ontology.as_bytes());
        let out_hash = hash_data(output.as_bytes());

        let receipt = Receipt::new(ont_hash, out_hash, &signing_key, None)?;
        replay_receipts.push(receipt);
    }

    validator.validate_replay_sequence(&replay_receipts)?;
    println!("   ✓ Replay validation passed - build is deterministic!");

    // Demonstrate detection of non-determinism
    println!("\n6. Demonstrating non-determinism detection...");
    let different_output = hash_data(b"different-output");
    let same_ontology = hash_data(format!("deterministic-ontology-0").as_bytes());
    let non_deterministic_receipt =
        Receipt::new(same_ontology, different_output, &signing_key, None)?;

    match validator.validate_replay(0, &non_deterministic_receipt) {
        Ok(_) => println!("   ✗ Should have detected non-determinism"),
        Err(e) => println!("   ✓ Non-determinism detected: {}", e),
    }

    println!("\n=== All examples completed successfully! ===");
    Ok(())
}

fn generate_keypair() -> (SigningKey, VerifyingKey) {
    let mut rng = rand::rngs::OsRng;
    let mut seed = [0u8; 32];
    rng.fill_bytes(&mut seed);
    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();
    (signing_key, verifying_key)
}

fn hash_data(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}
