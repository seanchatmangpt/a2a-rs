//! Debug example for Merkle tree

use a2a_rs::{MerkleTree, Receipt};
use ed25519_dalek::SigningKey;
use rand::RngCore;
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let signing_key = generate_keypair().0;

    // Create just 4 receipts for easier debugging
    let mut receipts = Vec::new();
    for i in 0..4 {
        let ontology = format!("ont-{}", i);
        let output = format!("out-{}", i);
        let ont_hash = hash_data(ontology.as_bytes());
        let out_hash = hash_data(output.as_bytes());
        let receipt = Receipt::new(ont_hash, out_hash, &signing_key, None)?;
        println!("Receipt {}: hash = {}", i, &receipt.compute_hash()[..16]);
        receipts.push(receipt);
    }

    let tree = MerkleTree::from_receipts(&receipts);
    let root_hash = tree.root_hash().expect("Should have root");
    println!("\nRoot hash: {}", &root_hash[..16]);

    // Try to verify first receipt
    println!("\nVerifying receipt 0...");
    let proof = tree.generate_proof(&receipts[0])?;
    println!("Proof has {} elements:", proof.len());
    for (i, (hash, is_right)) in proof.iter().enumerate() {
        println!(
            "  {}: {} ({})",
            i,
            &hash[..16],
            if *is_right { "right" } else { "left" }
        );
    }

    // Manual verification
    let mut current = receipts[0].compute_hash();
    println!("\nStarting with: {}", &current[..16]);
    for (hash, is_right) in &proof {
        let next = if *is_right {
            hash_pair(&current, hash)
        } else {
            hash_pair(hash, &current)
        };
        println!(
            "  Combined with {} ({}) -> {}",
            &hash[..16],
            if *is_right { "right" } else { "left" },
            &next[..16]
        );
        current = next;
    }
    println!("Final: {}", &current[..16]);
    println!("Root:  {}", &root_hash[..16]);
    println!("Match: {}", current == root_hash);

    Ok(())
}

fn generate_keypair() -> (SigningKey, ed25519_dalek::VerifyingKey) {
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

fn hash_pair(left: &str, right: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(left.as_bytes());
    hasher.update(right.as_bytes());
    hex::encode(hasher.finalize())
}
