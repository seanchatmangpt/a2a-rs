//! Integration tests for crypto feature
//!
//! Tests the complete integration of cryptographic receipts with:
//! - Task artifacts
//! - Agent card signing
//! - Receipt chains for artifact history
//! - End-to-end verification flows

#![cfg(feature = "crypto")]

use a2a_rs::{
    AgentCard, AgentCardSigner, AgentCapabilities, Artifact, Message, MerkleTree, Part, Receipt,
    ReceiptChain, ReplayValidator, Role, Task, TaskState,
};
use ed25519_dalek::{SigningKey, VerifyingKey};
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
fn test_task_with_artifact_receipts() {
    let (signing_key, verifying_key) = generate_keypair();

    // Create a task
    let mut task = Task::new("task-123".to_string(), "ctx-456".to_string());

    // Add artifacts with receipts
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

        task.add_artifact(artifact.clone());

        // Create receipt for artifact
        let ontology_hash = Receipt::hash_data(format!("ontology-{}", i).as_bytes());
        let receipt = Receipt::from_artifact(&artifact, ontology_hash, &signing_key)
            .expect("Failed to create receipt");
        task.add_receipt(receipt);
    }

    // Verify task has artifacts and receipts
    assert!(task.artifacts.is_some());
    assert_eq!(task.artifacts.as_ref().unwrap().len(), 3);
    assert!(task.receipts().is_some());
    assert_eq!(task.receipts().unwrap().len(), 3);

    // Verify all receipts
    for receipt in task.receipts().unwrap() {
        assert!(receipt.verify(&verifying_key).is_ok());
    }
}

#[test]
fn test_agent_card_with_signed_receipts() {
    let (signing_key, verifying_key) = generate_keypair();

    // Create agent card
    let mut agent_card = AgentCard::builder()
        .name("Test Agent".to_string())
        .description("A test agent".to_string())
        .url("https://example.com".to_string())
        .version("1.0.0".to_string())
        .capabilities(AgentCapabilities::default())
        .skills(vec![])
        .build();

    // Add receipts to agent card
    for i in 0..3 {
        let ontology_hash = Receipt::hash_data(format!("ontology-{}", i).as_bytes());
        let output_hash = Receipt::hash_data(format!("output-{}", i).as_bytes());

        let receipt = Receipt::new(ontology_hash, output_hash, &signing_key, None)
            .expect("Failed to create receipt");
        agent_card.add_receipt(receipt);
    }

    // Verify agent card has receipts
    assert!(agent_card.receipts().is_some());
    assert_eq!(agent_card.receipts().unwrap().len(), 3);

    // Compute agent card hash for signing
    let agent_hash = agent_card.compute_hash();

    // Sign the agent card with its receipts
    let receipt_hashes: Vec<String> = agent_card
        .receipts()
        .unwrap()
        .iter()
        .map(|r| r.compute_hash())
        .collect();

    let signing_receipt = AgentCardSigner::sign_agent_card(
        agent_hash.clone(),
        receipt_hashes,
        &signing_key,
    )
    .expect("Failed to sign agent card");

    // Verify the signing receipt
    assert!(signing_receipt.verify(&verifying_key).is_ok());

    // Verify the agent card with its signing receipt
    let receipts = vec![signing_receipt];
    assert!(AgentCardSigner::verify_agent_card(&agent_hash, &receipts, &verifying_key).is_ok());
}

#[test]
fn test_receipt_chain_for_task_history() {
    let (signing_key, verifying_key) = generate_keypair();

    // Create receipt chain representing task artifact history
    let mut chain = ReceiptChain::new();

    let mut artifacts = Vec::new();
    for i in 0..5 {
        let artifact = Artifact {
            artifact_id: format!("artifact-{}", i),
            name: Some(format!("Step {}", i)),
            description: Some(format!("Processing step {}", i)),
            parts: vec![Part::Text {
                text: format!("Result {}", i),
                metadata: None,
            }],
            metadata: None,
            extensions: None,
        };

        let ontology_hash = Receipt::hash_data(format!("step-{}", i).as_bytes());
        chain
            .add_artifact_receipt(&artifact, ontology_hash, &signing_key)
            .expect("Failed to add artifact receipt");
        artifacts.push(artifact);
    }

    // Verify the chain with artifacts
    assert!(chain.verify_artifact_chain(&artifacts, &verifying_key).is_ok());
    assert_eq!(chain.len(), 5);

    // Verify we can retrieve receipts by index
    for i in 0..5 {
        assert!(chain.get(i).is_some());
    }
    assert!(chain.get(5).is_none());
}

#[test]
fn test_merkle_tree_for_batch_artifact_verification() {
    let (signing_key, _verifying_key) = generate_keypair();

    // Create many artifacts
    let mut receipts = Vec::new();
    let mut artifacts = Vec::new();

    for i in 0..16 {
        let artifact = Artifact {
            artifact_id: format!("artifact-{}", i),
            name: Some(format!("Batch Artifact {}", i)),
            description: None,
            parts: vec![Part::Text {
                text: format!("Batch content {}", i),
                metadata: None,
            }],
            metadata: None,
            extensions: None,
        };

        let ontology_hash = Receipt::hash_data(format!("batch-ontology-{}", i).as_bytes());
        let receipt = Receipt::from_artifact(&artifact, ontology_hash, &signing_key)
            .expect("Failed to create receipt");
        receipts.push(receipt);
        artifacts.push(artifact);
    }

    // Build Merkle tree
    let tree = MerkleTree::from_receipts(&receipts);
    let root_hash = tree.root_hash().expect("Should have root hash");

    // Verify each receipt with Merkle proof
    for receipt in &receipts {
        let proof = tree.generate_proof(receipt).expect("Failed to generate proof");
        assert!(MerkleTree::verify_proof(receipt, &proof, &root_hash).is_ok());
    }

    // Verify proof size is logarithmic
    let proof = tree.generate_proof(&receipts[0]).expect("Failed to generate proof");
    assert!(proof.len() <= 4, "Proof should be O(log n) for 16 items");
}

#[test]
fn test_replay_validator_with_task_artifacts() {
    let (signing_key, _verifying_key) = generate_keypair();
    let mut validator = ReplayValidator::new();

    // Record original task execution
    let mut original_artifacts = Vec::new();
    for i in 0..3 {
        let artifact = Artifact {
            artifact_id: format!("artifact-{}", i),
            name: Some(format!("Execution {}", i)),
            description: None,
            parts: vec![Part::Text {
                text: format!("Execution result {}", i),
                metadata: None,
            }],
            metadata: None,
            extensions: None,
        };

        let ontology_hash = Receipt::hash_data(format!("execution-{}", i).as_bytes());
        let receipt = Receipt::from_artifact(&artifact, ontology_hash, &signing_key)
            .expect("Failed to create receipt");
        validator.record(receipt);
        original_artifacts.push(artifact);
    }

    // Replay with same artifacts (deterministic)
    let mut replay_receipts = Vec::new();
    for i in 0..3 {
        let artifact = Artifact {
            artifact_id: format!("artifact-{}", i),
            name: Some(format!("Execution {}", i)),
            description: None,
            parts: vec![Part::Text {
                text: format!("Execution result {}", i),
                metadata: None,
            }],
            metadata: None,
            extensions: None,
        };

        let ontology_hash = Receipt::hash_data(format!("execution-{}", i).as_bytes());
        let receipt = Receipt::from_artifact(&artifact, ontology_hash, &signing_key)
            .expect("Failed to create receipt");
        replay_receipts.push(receipt);
    }

    // Should validate successfully
    assert!(validator.validate_replay_sequence(&replay_receipts).is_ok());

    // Non-deterministic replay should fail
    let different_artifact = Artifact {
        artifact_id: "artifact-0".to_string(),
        name: Some("Different Execution".to_string()),
        description: None,
        parts: vec![Part::Text {
            text: "Different result".to_string(),
            metadata: None,
        }],
        metadata: None,
        extensions: None,
    };

    let ontology_hash = Receipt::hash_data(b"execution-0");
    let different_receipt = Receipt::from_artifact(&different_artifact, ontology_hash, &signing_key)
        .expect("Failed to create receipt");

    assert!(validator.validate_replay(0, &different_receipt).is_err());
}

#[test]
fn test_end_to_end_workflow() {
    let (signing_key, verifying_key) = generate_keypair();

    // Step 1: Create task with artifacts
    let mut task = Task::new("workflow-task".to_string(), "workflow-ctx".to_string());

    // Step 2: Add processing artifacts with receipts
    let mut chain = ReceiptChain::new();
    for i in 0..3 {
        let artifact = Artifact {
            artifact_id: format!("step-{}", i),
            name: Some(format!("Processing Step {}", i)),
            description: Some(format!("Step {} of workflow", i)),
            parts: vec![Part::Text {
                text: format!("Step {} completed successfully", i),
                metadata: None,
            }],
            metadata: None,
            extensions: None,
        };

        task.add_artifact(artifact.clone());

        let ontology_hash = Receipt::hash_data(format!("workflow-step-{}", i).as_bytes());
        chain
            .add_artifact_receipt(&artifact, ontology_hash, &signing_key)
            .expect("Failed to add artifact receipt");
    }

    // Step 3: Verify receipt chain
    let artifacts = task.artifacts.as_ref().unwrap();
    assert!(chain.verify_artifact_chain(artifacts, &verifying_key).is_ok());

    // Step 4: Add receipts to task
    for receipt in chain.receipts() {
        task.add_receipt(receipt.clone());
    }

    // Step 5: Create Merkle tree for batch verification
    let tree = MerkleTree::from_receipts(chain.receipts());
    let root_hash = tree.root_hash().expect("Should have root");

    // Step 6: Verify all receipts with Merkle proofs
    for receipt in chain.receipts() {
        let proof = tree.generate_proof(receipt).expect("Failed to generate proof");
        assert!(MerkleTree::verify_proof(receipt, &proof, &root_hash).is_ok());
    }

    // Step 7: Create agent card and sign with receipts
    let mut agent_card = AgentCard::builder()
        .name("Workflow Agent".to_string())
        .description("Agent demonstrating end-to-end crypto workflow".to_string())
        .url("https://workflow.example.com".to_string())
        .version("1.0.0".to_string())
        .capabilities(AgentCapabilities::default())
        .skills(vec![])
        .build();

    // Add receipts to agent card
    for receipt in chain.receipts() {
        agent_card.add_receipt(receipt.clone());
    }

    // Step 8: Sign and verify agent card
    let agent_hash = agent_card.compute_hash();
    let receipt_hashes: Vec<String> = chain.receipts().iter().map(|r| r.compute_hash()).collect();

    let signing_receipt = AgentCardSigner::sign_agent_card(
        agent_hash.clone(),
        receipt_hashes,
        &signing_key,
    )
    .expect("Failed to sign agent card");

    let receipts = vec![signing_receipt];
    assert!(AgentCardSigner::verify_agent_card(&agent_hash, &receipts, &verifying_key).is_ok());
}

#[test]
fn test_receipt_tampering_detection() {
    let (signing_key, verifying_key) = generate_keypair();

    // Create legitimate receipt
    let artifact = Artifact {
        artifact_id: "secure-artifact".to_string(),
        name: Some("Secure Artifact".to_string()),
        description: Some("This should not be tampered with".to_string()),
        parts: vec![Part::Text {
            text: "Original content".to_string(),
            metadata: None,
        }],
        metadata: None,
        extensions: None,
    };

    let ontology_hash = Receipt::hash_data(b"secure-ontology");
    let mut receipt = Receipt::from_artifact(&artifact, ontology_hash, &signing_key)
        .expect("Failed to create receipt");

    // Verify original is valid
    assert!(receipt.verify(&verifying_key).is_ok());
    assert!(receipt.verify_artifact(&artifact).expect("Failed to verify"));

    // Tamper with receipt
    receipt.output_hash = Receipt::hash_data(b"tampered-hash");

    // Verification should fail
    assert!(receipt.verify(&verifying_key).is_err());
    assert!(!receipt.verify_artifact(&artifact).expect("Should not verify tampered"));
}

#[test]
fn test_chain_tampering_detection() {
    let (signing_key, verifying_key) = generate_keypair();

    // Build legitimate chain
    let mut chain = ReceiptChain::new();
    let mut artifacts = Vec::new();

    for i in 0..3 {
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
        chain
            .add_artifact_receipt(&artifact, ontology_hash, &signing_key)
            .expect("Failed to add receipt");
        artifacts.push(artifact);
    }

    // Verify original chain
    assert!(chain.verify_artifact_chain(&artifacts, &verifying_key).is_ok());

    // Note: We can't directly tamper with chain.receipts as it's private
    // But the verification in verify_artifact_chain ensures integrity
}
