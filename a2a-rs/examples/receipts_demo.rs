//! Receipt Chain System Demo
//!
//! This example demonstrates the cryptographic receipt system for
//! creating tamper-proof audit trails of state transitions.

#[cfg(feature = "receipts")]
use a2a_rs::construct::receipts::{Receipt, ReceiptChain, compute_hash};

#[cfg(feature = "receipts")]
fn main() {
    println!("=== Receipt Chain Demo ===\n");

    // Create a receipt chain to track state transitions
    let mut chain = ReceiptChain::new();

    println!("1. Creating receipt for first state transition:");
    println!("   Observation: User query 'What is 2+2?'");
    println!("   Action: Assistant response '4'");
    println!("   Delta: query_count = 1\n");

    chain.add_transition(
        b"observation: user query 'What is 2+2?'",
        b"action: assistant response '4'",
        b"delta: query_count = 1",
    );

    let first_receipt = chain.get(0).unwrap();
    println!("   Receipt Hash: {}", &first_receipt.receipt_hash[..32]);
    println!("   Sequence: {}", first_receipt.sequence);
    println!("   Previous: None (genesis)\n");

    println!("2. Adding second transition:");
    println!("   Observation: User query 'What is 3+3?'");
    println!("   Action: Assistant response '6'");
    println!("   Delta: query_count = 2\n");

    chain.add_transition(
        b"observation: user query 'What is 3+3?'",
        b"action: assistant response '6'",
        b"delta: query_count = 2",
    );

    let second_receipt = chain.get(1).unwrap();
    println!("   Receipt Hash: {}", &second_receipt.receipt_hash[..32]);
    println!("   Sequence: {}", second_receipt.sequence);
    println!(
        "   Previous: {}...\n",
        &second_receipt.previous_hash.as_ref().unwrap()[..16]
    );

    println!("3. Adding third transition:");
    println!("   Observation: User query 'What is 5+5?'");
    println!("   Action: Assistant response '10'");
    println!("   Delta: query_count = 3\n");

    chain.add_transition(
        b"observation: user query 'What is 5+5?'",
        b"action: assistant response '10'",
        b"delta: query_count = 3",
    );

    println!("4. Chain Summary:");
    println!("   Total Receipts: {}", chain.len());
    println!("   Latest Sequence: {}", chain.latest().unwrap().sequence);
    println!();

    // Verify chain integrity
    println!("5. Verifying Chain Integrity:");
    match chain.verify_integrity() {
        Ok(_) => println!("   ✓ Chain integrity verified successfully!"),
        Err(e) => println!("   ✗ Integrity check failed: {}", e),
    }
    println!();

    // Export to JSON
    println!("6. Exporting Chain to JSON:");
    match chain.to_json() {
        Ok(json) => {
            println!("   Chain exported ({} bytes)", json.len());
            if json.len() < 500 {
                println!("   JSON Preview:\n{}", &json[..json.len().min(500)]);
            } else {
                println!("   JSON Preview (first 500 chars):\n{}", &json[..500]);
            }
        }
        Err(e) => println!("   Export failed: {}", e),
    }
    println!();

    // Demonstrate tampering detection
    println!("7. Testing Tamper Detection:");
    let mut tampered_chain = chain.clone();
    if let Some(receipt) = tampered_chain.receipts.get_mut(1) {
        println!("   Tampering with receipt #1...");
        receipt.observation_hash = "00000000tampered".to_string();
    }

    match tampered_chain.verify_integrity() {
        Ok(_) => println!("   ✗ Tampering NOT detected (unexpected!)"),
        Err(e) => println!("   ✓ Tampering detected: {}", e),
    }
    println!();

    // Demonstrate deterministic hashing
    println!("8. Demonstrating Deterministic Hashing:");
    let data = b"test data";
    let hash1 = compute_hash(data);
    let hash2 = compute_hash(data);
    println!("   Hash 1: {}", &hash1[..32]);
    println!("   Hash 2: {}", &hash2[..32]);
    println!(
        "   {}",
        if hash1 == hash2 {
            "✓ Hashes match (deterministic)"
        } else {
            "✗ Hashes don't match (non-deterministic)"
        }
    );

    #[cfg(feature = "receipts-signing")]
    demonstrate_signing();

    println!("\n=== Demo Complete ===");
}

#[cfg(feature = "receipts-signing")]
fn demonstrate_signing() {
    use a2a_rs::construct::receipts::ReceiptChain;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    println!("\n9. Demonstrating Cryptographic Signing:");

    let mut rng = OsRng;
    let signing_key = SigningKey::generate(&mut rng);

    println!("   Generated signing key");
    println!(
        "   Public key: {}...",
        hex::encode(signing_key.verifying_key().to_bytes())[..16].to_string()
    );

    let mut signed_chain = ReceiptChain::new();

    println!("   Adding signed transitions...");
    signed_chain.add_signed_transition(
        b"observation: signed event 1",
        b"action: response 1",
        b"delta: state = 1",
        &signing_key,
    );

    signed_chain.add_signed_transition(
        b"observation: signed event 2",
        b"action: response 2",
        b"delta: state = 2",
        &signing_key,
    );

    println!(
        "   Created chain with {} signed receipts",
        signed_chain.len()
    );

    match signed_chain.verify_integrity() {
        Ok(_) => println!("   ✓ Signed chain verified successfully!"),
        Err(e) => println!("   ✗ Verification failed: {}", e),
    }
}

#[cfg(not(feature = "receipts"))]
fn main() {
    println!("This example requires the 'receipts' feature to be enabled.");
    println!("Run with: cargo run --example receipts_demo --features receipts");
}
