//! Example demonstrating Merkle tree-based receipt storage.
//!
//! This example shows how to:
//! - Store receipts in a Merkle tree
//! - Generate verification proofs (O(log N) complexity)
//! - Verify proofs independently
//! - Detect tampering via root hash changes
//! - Use both in-memory and persistent backends

use osiris_compiler::prelude::*;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Merkle Tree Receipt Storage Demo ===\n");

    // Create in-memory Merkle storage
    let storage = InMemoryMerkleStorage::new();
    println!("Created in-memory Merkle storage\n");

    // Create some sample receipts
    let mut receipts = Vec::new();
    for i in 0..8 {
        let operation = Operation::new(
            OperationKind::Parse {
                input: format!("operation_{}", i),
            },
            1,
        );

        let receipt = Receipt {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            operation_id: operation.id,
            operation_hash: format!("op_hash_{}", i),
            attestation_hash: format!("op_hash_{}", i),
            signature: Some(format!("sig_{}", i)),
            replay_pointers: vec![],
            result: OperationResult::Success {
                output_hash: format!("output_{}", i),
                output: None,
            },
            refusal: None,
            metadata: HashMap::new(),
        };

        receipts.push(receipt);
    }

    println!("Created {} sample receipts", receipts.len());

    // Store receipts one by one (demonstrates incremental updates)
    println!("\n--- Incremental Storage ---");
    for (i, receipt) in receipts.iter().enumerate() {
        let root = storage.store_receipt(receipt).await?;
        println!(
            "Stored receipt {} - New root: {}... (leaf count: {})",
            i + 1,
            &root.hash[..16],
            root.leaf_count
        );
    }

    // Get current root hash
    let root = storage.get_root().await?.unwrap();
    println!("\n--- Final State ---");
    println!("Root hash: {}...", &root.hash[..16]);
    println!("Total receipts: {}", root.leaf_count);

    // Generate and verify proofs for all receipts
    println!("\n--- Proof Generation and Verification ---");
    for (i, receipt) in receipts.iter().enumerate() {
        let proof = storage.generate_proof(receipt.id).await?;
        let is_valid = storage.verify_proof(&proof).await.is_ok();

        println!(
            "Receipt {}: Proof size = {} steps (log2({}) ≈ {}), Valid = {}",
            i + 1,
            proof.len(),
            receipts.len(),
            (receipts.len() as f64).log2().ceil() as usize,
            is_valid
        );
    }

    // Demonstrate independent verification
    println!("\n--- Independent Verification ---");
    let proof = storage.generate_proof(receipts[0].id).await?;
    println!(
        "Proof for receipt 0 can be verified independently: {}",
        proof.verify()
    );

    // Retrieve a receipt
    println!("\n--- Receipt Retrieval ---");
    let retrieved = storage.get_receipt(receipts[0].id).await?;
    println!("Retrieved receipt: {}", retrieved.id);
    println!("  Operation: {}", retrieved.operation_id);
    println!("  Hash: {}", retrieved.operation_hash);

    // Verify tree integrity
    println!("\n--- Tree Integrity Check ---");
    match storage.verify_tree_integrity().await {
        Ok(()) => println!("✓ Tree integrity verified successfully"),
        Err(e) => println!("✗ Tree integrity check failed: {}", e),
    }

    // Demonstrate batch storage with persistent backend
    println!("\n--- Persistent Storage Demo ---");
    let backend = InMemoryBackend::new();
    let persistent = PersistentMerkleStorage::new(backend).await?;

    // Create more receipts for batch storage
    let mut batch_receipts = Vec::new();
    for i in 0..5 {
        let operation = Operation::new(
            OperationKind::CodeGen {
                template: format!("template_{}", i),
                target: format!("target_{}", i),
            },
            2,
        );

        let receipt = Receipt {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            operation_id: operation.id,
            operation_hash: format!("batch_hash_{}", i),
            attestation_hash: format!("batch_hash_{}", i),
            signature: Some(format!("batch_sig_{}", i)),
            replay_pointers: vec![],
            result: OperationResult::Success {
                output_hash: format!("batch_output_{}", i),
                output: None,
            },
            refusal: None,
            metadata: HashMap::new(),
        };

        batch_receipts.push(receipt);
    }

    // Batch store (more efficient)
    let root = persistent.store_receipts(&batch_receipts).await?;
    println!(
        "Batch stored {} receipts - Root: {}... (leaf count: {})",
        batch_receipts.len(),
        &root.hash[..16],
        root.leaf_count
    );

    // Reload from backend
    persistent.reload().await?;
    println!("✓ Successfully reloaded tree from backend");

    // Generate proofs from persistent storage
    let proof = persistent.generate_proof(batch_receipts[0].id).await?;
    println!(
        "Generated proof from persistent storage: {} steps",
        proof.len()
    );

    // Demonstrate proof serialization
    println!("\n--- Proof Serialization ---");
    let proof_json = serde_json::to_string_pretty(&proof)?;
    println!("Proof as JSON:\n{}", proof_json);

    // Demonstrate tamper detection
    println!("\n--- Tamper Detection ---");
    let tree = storage.get_tree().await?;
    let original_root = tree.root().unwrap().clone();
    println!("Original root: {}...", &original_root.hash[..16]);

    // Add one more receipt
    let tamper_operation = Operation::new(
        OperationKind::Parse {
            input: "tamper_test".into(),
        },
        1,
    );
    let tamper_receipt = Receipt {
        id: uuid::Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        operation_id: tamper_operation.id,
        operation_hash: "tamper_hash".to_string(),
        attestation_hash: "tamper_hash".to_string(),
        signature: Some("tamper_sig".to_string()),
        replay_pointers: vec![],
        result: OperationResult::Success {
            output_hash: "tamper_output".to_string(),
            output: None,
        },
        refusal: None,
        metadata: HashMap::new(),
    };

    let new_root = storage.store_receipt(&tamper_receipt).await?;
    println!("Root after modification: {}...", &new_root.hash[..16]);
    println!(
        "Roots are different: {} (tamper detected!)",
        original_root.hash != new_root.hash
    );

    println!("\n=== Demo Complete ===");
    Ok(())
}
