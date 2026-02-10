//! Example demonstrating packet verification with type checker and H-guards.
//!
//! This example shows:
//! 1. Setting up a closed type system Σ
//! 2. Rejecting packets not in Σ
//! 3. Configuring H-guards (inadmissible-before constraints)
//! 4. Producing refusal receipts for violations
//!
//! Run with: cargo run --example packet_verification

use osiris_compiler::prelude::*;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== Osiris Packet Verification Demo ===\n");

    // Step 1: Define the closed type system Σ
    println!("1. Setting up closed type system Σ");
    let mut sigma = Sigma::new();

    // Register admissible packet types
    let auth_type = PacketType::new("osiris", "AuthRequest", "1.0");
    let data_type = PacketType::new("osiris", "DataPacket", "1.0");

    sigma.register(auth_type.clone());
    sigma.register_with_schema(
        data_type.clone(),
        TypeSchema {
            schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": { "type": "string" }
                }
            }),
            required_fields: vec!["content".to_string()],
        },
    );

    println!("   ✓ Registered packet types:");
    println!("     - osiris.AuthRequest.1.0");
    println!("     - osiris.DataPacket.1.0 (with schema)");
    println!();

    // Step 2: Initialize type checker
    println!("2. Initializing SigmaTypeChecker");
    let mut type_checker = SigmaTypeChecker::with_sigma(sigma);
    println!("   ✓ Type checker ready\n");

    // Step 3: Test packet rejection (not in Σ)
    println!("3. Testing packet rejection (type not in Σ)");
    let invalid_packet = Packet {
        id: "pkt-001".to_string(),
        packet_type: PacketType::new("osiris", "UnknownType", "1.0"),
        payload: serde_json::json!({}),
        metadata: HashMap::new(),
    };

    match type_checker.check(&invalid_packet).await? {
        TypeCheckResult::TypeNotInSigma {
            packet_id,
            attempted_type,
            reason,
        } => {
            println!("   ✗ Packet {} REJECTED:", packet_id);
            println!("     Type: {}", attempted_type.fqn());
            println!("     Reason: {}", reason);

            // Generate refusal receipt
            let receipt = SigmaTypeChecker::create_refusal_receipt(
                packet_id.clone(),
                RefusalReason::TypeNotInSigma {
                    attempted_type,
                    message: reason,
                },
            )?;
            println!("     Receipt ID: {}", receipt.receipt_id);
        }
        _ => println!("   Unexpected result!"),
    }
    println!();

    // Step 4: Test valid packet
    println!("4. Testing valid packet (in Σ)");
    let valid_packet = Packet {
        id: "pkt-002".to_string(),
        packet_type: data_type.clone(),
        payload: serde_json::json!({"content": "Hello, Osiris!"}),
        metadata: HashMap::new(),
    };

    match type_checker.check(&valid_packet).await? {
        TypeCheckResult::Valid {
            packet_id,
            packet_type,
        } => {
            println!("   ✓ Packet {} ACCEPTED:", packet_id);
            println!("     Type: {}", packet_type.fqn());
        }
        _ => println!("   Unexpected result!"),
    }
    println!();

    // Step 5: Test schema violation
    println!("5. Testing schema violation");
    let malformed_packet = Packet {
        id: "pkt-003".to_string(),
        packet_type: data_type.clone(),
        payload: serde_json::json!({"wrong_field": "value"}),
        metadata: HashMap::new(),
    };

    match type_checker.check(&malformed_packet).await? {
        TypeCheckResult::SchemaViolation {
            packet_id,
            packet_type,
            errors,
        } => {
            println!("   ✗ Packet {} REJECTED (schema violation):", packet_id);
            println!("     Type: {}", packet_type.fqn());
            println!("     Errors:");
            for error in &errors {
                println!("       - {}", error);
            }

            // Generate refusal receipt
            let receipt = SigmaTypeChecker::create_refusal_receipt(
                packet_id.clone(),
                RefusalReason::SchemaViolation {
                    packet_type,
                    errors,
                },
            )?;
            println!("     Receipt ID: {}", receipt.receipt_id);
        }
        _ => println!("   Unexpected result!"),
    }
    println!();

    // Step 6: Set up H-guards
    println!("6. Setting up H-guards (inadmissible-before constraints)");
    let mut guard_evaluator = HGuardEvaluatorAdapter::new();

    // H-guard: DataPacket requires prior AuthRequest
    let auth_guard = HGuard {
        id: "guard-auth-required".to_string(),
        packet_type: data_type.clone(),
        condition: GuardCondition::RequiresPrior {
            packet_type: auth_type.clone(),
            packet_id: None,
        },
        description: Some("Data packets require prior authentication".to_string()),
    };

    guard_evaluator.register_guard(auth_guard).await?;
    println!("   ✓ Registered H-guard: guard-auth-required");
    println!("     Condition: DataPacket requires prior AuthRequest");
    println!();

    // Step 7: Test guard violation
    println!("7. Testing H-guard violation");
    let data_packet = Packet {
        id: "pkt-004".to_string(),
        packet_type: data_type.clone(),
        payload: serde_json::json!({"content": "Data"}),
        metadata: HashMap::new(),
    };

    let results = guard_evaluator.evaluate(&data_packet).await?;
    for result in results {
        match result {
            GuardEvaluationResult::Violated {
                guard_id,
                reason,
                retry_after,
            } => {
                println!("   ✗ H-guard VIOLATED:");
                println!("     Guard: {}", guard_id);
                println!("     Reason: {}", reason);
                if let Some(retry) = retry_after {
                    println!("     Retry after: {}", retry);
                }

                // Generate refusal receipt
                let receipt = SigmaTypeChecker::create_refusal_receipt(
                    data_packet.id.clone(),
                    RefusalReason::GuardViolation {
                        guard_id,
                        guard_condition: "RequiresPrior".to_string(),
                        message: reason,
                        retry_after,
                    },
                )?;
                println!("     Receipt ID: {}", receipt.receipt_id);
            }
            _ => println!("   Unexpected result!"),
        }
    }
    println!();

    // Step 8: Satisfy guard by processing auth packet
    println!("8. Processing authentication packet");
    let auth_packet = Packet {
        id: "pkt-005".to_string(),
        packet_type: auth_type.clone(),
        payload: serde_json::json!({"user": "alice"}),
        metadata: HashMap::new(),
    };

    guard_evaluator.record_packet(auth_packet.clone()).await?;
    println!("   ✓ Auth packet processed: {}", auth_packet.id);
    println!();

    // Step 9: Re-evaluate guard (should pass now)
    println!("9. Re-evaluating H-guard after authentication");
    let results = guard_evaluator.evaluate(&data_packet).await?;
    for result in results {
        match result {
            GuardEvaluationResult::Satisfied { guard_id } => {
                println!("   ✓ H-guard SATISFIED:");
                println!("     Guard: {}", guard_id);
            }
            _ => println!("   Unexpected result!"),
        }
    }
    println!();

    // Step 10: Full verification pipeline
    println!("10. Full verification pipeline (type check + H-guards)");
    let final_packet = Packet {
        id: "pkt-006".to_string(),
        packet_type: data_type.clone(),
        payload: serde_json::json!({"content": "Final data"}),
        metadata: HashMap::new(),
    };

    // Check type
    match type_checker.check(&final_packet).await? {
        TypeCheckResult::Valid { .. } => {
            println!("   ✓ Type check: PASSED");

            // Check guards
            let guard_results = guard_evaluator.evaluate(&final_packet).await?;
            let all_satisfied = guard_results
                .iter()
                .all(|r| matches!(r, GuardEvaluationResult::Satisfied { .. }));

            if all_satisfied {
                println!("   ✓ H-guard check: PASSED");
                println!(
                    "   ✓ Packet {} ACCEPTED (full verification)",
                    final_packet.id
                );
            } else {
                println!("   ✗ H-guard check: FAILED");
            }
        }
        result => {
            println!("   ✗ Type check: FAILED - {:?}", result);
        }
    }

    println!("\n=== Demo complete ===");
    Ok(())
}
