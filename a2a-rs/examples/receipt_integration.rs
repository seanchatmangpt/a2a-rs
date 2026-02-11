//! Example demonstrating integrated crypto features with tasks and artifacts
//!
//! This example shows the complete integration of:
//! - Receipt generation from task artifacts
//! - Receipt chains for artifact history
//! - Agent card signing with receipts
//! - End-to-end verification workflow

use a2a_rs::{
    AgentCard, AgentCardSigner, AgentCapabilities, Artifact, MerkleTree, Part, Receipt,
    ReceiptChain, Role, Task, TaskState,
};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::RngCore;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Integrated Crypto Features Demo ===\n");

    let (signing_key, verifying_key) = generate_keypair();
    println!("1. Generated ed25519 keypair\n");

    // Create a task for processing
    println!("2. Creating task with artifacts...");
    let mut task = Task::new("task-001".to_string(), "ctx-001".to_string());
    task.update_status(TaskState::Working, None);

    // Build receipt chain for artifact history
    let mut receipt_chain = ReceiptChain::new();

    // Add processing artifacts
    println!("   Adding artifacts with cryptographic receipts:");
    for step in 0..=3 {
        let artifact = create_processing_artifact(step)?;
        task.add_artifact(artifact.clone());

        let ontology_hash = Receipt::hash_data(format!("processing-step-{}", step).as_bytes());
        receipt_chain.add_artifact_receipt(&artifact, ontology_hash, &signing_key)?;

        println!(
            "   - Step {}: {} (receipt: {}...)",
            step,
            &artifact.artifact_id,
            &receipt_chain.get(step).unwrap().compute_hash()[..16]
        );
    }

    println!("\n3. Verifying artifact receipt chain...");
    let artifacts = task.artifacts.as_ref().unwrap();
    receipt_chain.verify_artifact_chain(artifacts, &verifying_key)?;
    println!("   ✓ Artifact chain verified with {} receipts", receipt_chain.len());

    // Add receipts to task
    println!("\n4. Adding receipts to task...");
    for receipt in receipt_chain.receipts() {
        task.add_receipt(receipt.clone());
    }
    println!("   ✓ Task now has {} receipts", task.receipts().unwrap().len());

    // Create Merkle tree for efficient verification
    println!("\n5. Building Merkle tree for batch verification...");
    let tree = MerkleTree::from_receipts(receipt_chain.receipts());
    let root_hash = tree.root_hash().unwrap();
    println!("   Merkle root: {}...", &root_hash[..32]);

    // Verify with Merkle proofs
    println!("   Verifying receipts with Merkle proofs:");
    for (i, receipt) in receipt_chain.receipts().iter().enumerate() {
        let proof = tree.generate_proof(receipt)?;
        MerkleTree::verify_proof(receipt, &proof, &root_hash)?;
        println!("   - Receipt {}: ✓ (proof size: {})", i, proof.len());
    }

    // Create and sign agent card
    println!("\n6. Creating agent card with signed receipts...");
    let mut agent_card = AgentCard::builder()
        .name("Crypto-Enabled Agent".to_string())
        .description("Demonstrates full crypto integration".to_string())
        .url("https://crypto-agent.example.com".to_string())
        .version("1.0.0".to_string())
        .capabilities(AgentCapabilities::default())
        .skills(vec![])
        .build();

    // Attach receipt chain to agent card
    for receipt in receipt_chain.receipts() {
        agent_card.add_receipt(receipt.clone());
    }

    // Sign the agent card
    let agent_hash = agent_card.compute_hash();
    let receipt_hashes: Vec<String> = receipt_chain.receipts()
        .iter()
        .map(|r| r.compute_hash())
        .collect();

    let signing_receipt = AgentCardSigner::sign_agent_card(
        agent_hash.clone(),
        receipt_hashes,
        &signing_key,
    )?;

    println!("   Agent card hash: {}...", &agent_hash[..32]);
    println!("   Signing receipt: {}...", &signing_receipt.signature[..32]);

    // Verify the signed agent card
    println!("\n7. Verifying signed agent card...");
    let receipts = vec![signing_receipt];
    AgentCardSigner::verify_agent_card(&agent_hash, &receipts, &verifying_key)?;
    println!("   ✓ Agent card signature verified");
    println!("   ✓ Receipt chain integrity confirmed");

    // Display summary
    println!("\n=== Integration Summary ===");
    println!("Task ID: {}", task.id);
    println!("Artifacts: {}", task.artifacts.as_ref().unwrap().len());
    println!("Receipts: {}", task.receipts().unwrap().len());
    println!("Chain verified: ✓");
    println!("Merkle root: {}...", &root_hash[..32]);
    println!("Agent card signed: ✓");

    println!("\n=== All crypto features integrated successfully! ===");
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

fn create_processing_artifact(step: usize) -> Result<Artifact, Box<dyn std::error::Error>> {
    let descriptions = [
        "Input data validated",
        "Processing completed",
        "Output generated",
        "Results verified",
    ];

    Ok(Artifact {
        artifact_id: format!("artifact-{:03}", step),
        name: Some(format!("Processing Step {}", step)),
        description: Some(descriptions[step].to_string()),
        parts: vec![Part::Text {
            text: format!("Step {} result: {}", step, descriptions[step]),
            metadata: None,
        }],
        metadata: None,
        extensions: None,
    })
}
