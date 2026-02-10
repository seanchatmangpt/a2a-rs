//! Demonstration of persistent receipt storage using SQLx.
//!
//! This example shows how to:
//! 1. Create a receipt store
//! 2. Append receipts to the store
//! 3. Retrieve and verify the chain
//! 4. Replay from a specific point
//!
//! Run with: cargo run --example receipt_store_demo --features "sqlx-storage,receipts,sqlite"

#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
use a2a_rs::construct::{ReceiptChain, ReceiptStore};

#[cfg(all(feature = "sqlx-storage", feature = "receipts"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Receipt Store Demo ===\n");

    // Create an in-memory database for testing
    let store = ReceiptStore::from_url("sqlite::memory:").await?;
    println!("✓ Created receipt store");

    // Create a chain in memory
    let mut chain = ReceiptChain::new();
    println!("\n--- Building receipt chain ---");

    // Add some transitions
    chain.add_transition(
        b"User query: What is 2+2?",
        b"Assistant response: 4",
        b"State: query_count += 1",
    );
    println!("✓ Added receipt 0: Simple calculation");

    chain.add_transition(
        b"User query: Tell me a joke",
        b"Assistant response: Why did the developer quit? Insufficient cache!",
        b"State: query_count += 1, joke_count += 1",
    );
    println!("✓ Added receipt 1: Joke request");

    chain.add_transition(
        b"User query: Store this: password123",
        b"REFUSAL: Cannot store sensitive data",
        b"State: refusal_count += 1",
    );
    println!("✓ Added receipt 2: Refusal (sensitive data)");

    // Store all receipts
    println!("\n--- Storing receipts ---");
    for receipt in &chain.receipts {
        store.append(receipt).await?;
        println!("✓ Stored receipt {}", receipt.sequence);
    }

    // Verify the stored chain
    println!("\n--- Verifying chain integrity ---");
    store.verify_chain().await?;
    println!("✓ Chain integrity verified");

    // Retrieve the chain
    println!("\n--- Retrieving chain ---");
    let stored_chain = store.get_chain().await?;
    println!("✓ Retrieved {} receipts", stored_chain.len());

    // Display chain details
    println!("\n--- Chain details ---");
    for receipt in stored_chain.iter() {
        println!(
            "Receipt {}: hash={}...",
            receipt.sequence,
            &receipt.receipt_hash[..16]
        );
    }

    // Get latest receipt
    println!("\n--- Latest receipt ---");
    if let Some(latest) = store.get_latest().await? {
        println!("Latest receipt sequence: {}", latest.sequence);
        println!("Receipt hash: {}...", &latest.receipt_hash[..16]);
    }

    // Replay from a specific point
    println!("\n--- Replay from sequence 1 ---");
    let replay_chain = store.replay_from(1).await?;
    println!("✓ Replayed {} receipts", replay_chain.len());
    replay_chain.verify_integrity()?;
    println!("✓ Replay chain integrity verified");

    for receipt in replay_chain.iter() {
        println!(
            "Replayed receipt {}: hash={}...",
            receipt.sequence,
            &receipt.receipt_hash[..16]
        );
    }

    // Export chain as JSON
    println!("\n--- Exporting as JSON ---");
    let json = stored_chain.to_json()?;
    println!("✓ Exported chain ({} bytes)", json.len());
    println!("\nFirst 200 characters of JSON:");
    println!("{}", &json[..json.len().min(200)]);

    println!("\n=== Demo Complete ===");
    Ok(())
}

#[cfg(not(all(feature = "sqlx-storage", feature = "receipts")))]
fn main() {
    eprintln!("This example requires the 'sqlx-storage' and 'receipts' features.");
    eprintln!(
        "Run with: cargo run --example receipt_store_demo --features \"sqlx-storage,receipts,sqlite\""
    );
    std::process::exit(1);
}
