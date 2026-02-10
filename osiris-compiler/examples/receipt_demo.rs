//! Receipt Builder Demo
//!
//! This example demonstrates how to use the receipt builder system
//! to create cryptographic proof chains for operations.

use osiris_compiler::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create a local signer (for development)
    // In production, use KmsSigner instead
    let signer = Arc::new(LocalSigner::new("demo-key"));
    let builder = StandardReceiptBuilder::new(signer);

    // 2. Create an operation
    let operation = Operation::new(
        OperationKind::Parse {
            input: "example code".into(),
        },
        1, // priority
    );

    println!("Created operation: {:?}", operation.id);

    // 3. Build a successful receipt
    let receipt = builder
        .build_receipt(
            &operation,
            OperationResult::Success {
                output_hash: "abc123def456".to_string(),
                output: None,
            },
            vec![], // no replay pointers yet
            HashMap::new(),
        )
        .await?;

    println!("\n=== Successful Receipt ===");
    println!("Receipt ID: {}", receipt.id);
    println!("Operation ID: {}", receipt.operation_id);
    println!("Operation Hash: {}", receipt.operation_hash);
    println!("Attestation Hash: {}", receipt.attestation_hash);
    println!(
        "Hash Invariant Valid: {}",
        receipt.operation_hash == receipt.attestation_hash
    );
    println!("Signature: {:?}", receipt.signature);
    println!("Timestamp: {}", receipt.timestamp);

    // 4. Verify the receipt
    builder.verify_receipt(&receipt).await?;
    println!("\n✓ Receipt verified successfully!");

    // 5. Create a second operation that depends on the first
    let operation2 = Operation::new(
        OperationKind::TypeCheck {
            module_id: "module-1".into(),
        },
        1,
    );

    // 6. Create replay pointer to first operation
    let replay_pointer = ReplayPointer {
        receipt_id: receipt.id,
        receipt_hash: receipt.compute_receipt_hash()?,
        relation: DependencyRelation::RequiresCompletion,
        reason: Some("Requires parsing to complete first".to_string()),
    };

    // 7. Build receipt with replay pointer
    let receipt2 = builder
        .build_receipt(
            &operation2,
            OperationResult::Success {
                output_hash: "xyz789abc123".to_string(),
                output: None,
            },
            vec![replay_pointer],
            HashMap::new(),
        )
        .await?;

    println!("\n=== Second Receipt (with Replay Pointer) ===");
    println!("Receipt ID: {}", receipt2.id);
    println!("Replay Pointers: {}", receipt2.replay_pointers.len());
    println!(
        "  -> References: {}",
        receipt2.replay_pointers[0].receipt_id
    );
    println!("  -> Relation: {:?}", receipt2.replay_pointers[0].relation);

    // 8. Demonstrate refusal receipt
    let operation3 = Operation::new(
        OperationKind::CodeGen {
            target: "x86_64".into(),
        },
        1,
    );

    let refusal = RefusalInfo {
        category: RefusalCategory::GuardViolation,
        reason: "Type checking not completed".to_string(),
        retry_after: None,
        policy_id: Some("h-guard-policy".to_string()),
        context: HashMap::new(),
    };

    let refusal_receipt = builder
        .build_refusal_receipt(&operation3, refusal, vec![], HashMap::new())
        .await?;

    println!("\n=== Refusal Receipt ===");
    println!("Receipt ID: {}", refusal_receipt.id);
    println!("Operation ID: {}", refusal_receipt.operation_id);
    println!("Refused: {}", refusal_receipt.is_refused());
    if let Some(ref refusal_info) = refusal_receipt.refusal {
        println!("Category: {:?}", refusal_info.category);
        println!("Reason: {}", refusal_info.reason);
    }

    // 9. Store receipts (in-memory storage for demo)
    let storage = InMemoryReceiptStorage::new();

    storage.store_receipt(&receipt).await?;
    storage.store_receipt(&receipt2).await?;
    storage.store_receipt(&refusal_receipt).await?;

    println!("\n✓ Stored {} receipts", 3);

    // 10. Retrieve receipts for an operation
    let op_receipts = storage.get_receipts_for_operation(operation.id).await?;
    println!(
        "Found {} receipts for operation {}",
        op_receipts.len(),
        operation.id
    );

    // 11. List receipts in time range
    let start = receipt.timestamp - chrono::Duration::hours(1);
    let end = receipt.timestamp + chrono::Duration::hours(1);
    let time_receipts = storage.list_receipts(start, end).await?;
    println!("Found {} receipts in time range", time_receipts.len());

    println!("\n=== Demo Complete ===");
    println!("\nKey Concepts Demonstrated:");
    println!("1. Receipt creation with hash(A) = hash(μ(O)) invariant");
    println!("2. Digital signatures via pluggable Signer trait");
    println!("3. Replay pointers for operation dependencies");
    println!("4. Refusal receipts for rejected operations");
    println!("5. Receipt storage and retrieval");
    println!("\nFor production use:");
    println!("- Use KmsSigner with Cloud KMS for signing");
    println!("- Use CloudStorageReceiptStorage for persistent storage");
    println!("- Enable 'kms' feature for KMS support");
    println!("- Enable 'storage' feature for Cloud Storage support");

    Ok(())
}
