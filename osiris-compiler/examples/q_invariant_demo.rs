//! Demonstration of Q invariant verification with jidoka stop-the-line mechanism.
//!
//! This example shows:
//! 1. Registering Q invariants
//! 2. Verifying state snapshots against invariants
//! 3. Proving preserve(Q) across state transitions
//! 4. Blocking commits that violate invariants
//! 5. Emitting refusal receipts

use osiris_compiler::prelude::*;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Q Invariant Verifier Demo ===\n");

    // Create the Q invariant verifier
    let mut verifier = QInvariantVerifier::new();

    // Define invariants for a banking system
    println!("1. Registering Q invariants...");

    // Invariant 1: Balance must be non-negative (critical)
    let balance_invariant = QInvariant {
        id: "inv-balance-nonnegative".to_string(),
        name: "Balance must be non-negative".to_string(),
        description: Some("Account balance cannot go below zero".to_string()),
        predicate: InvariantPredicate::StateComparison {
            field: "balance".to_string(),
            operator: ComparisonOperator::Ge,
            value: serde_json::json!(0),
        },
        severity: InvariantSeverity::Critical,
        enabled: true,
    };

    verifier.register_invariant(balance_invariant).await?;
    println!("   ✓ Registered: Balance must be non-negative (Critical)");

    // Invariant 2: Status must be valid (error level)
    let status_invariant = QInvariant {
        id: "inv-status-valid".to_string(),
        name: "Status must be valid".to_string(),
        description: Some("Account status must be one of: active, suspended, closed".to_string()),
        predicate: InvariantPredicate::Or {
            predicates: vec![
                InvariantPredicate::StateEquals {
                    field: "status".to_string(),
                    expected: serde_json::json!("active"),
                },
                InvariantPredicate::StateEquals {
                    field: "status".to_string(),
                    expected: serde_json::json!("suspended"),
                },
                InvariantPredicate::StateEquals {
                    field: "status".to_string(),
                    expected: serde_json::json!("closed"),
                },
            ],
        },
        severity: InvariantSeverity::Error,
        enabled: true,
    };

    verifier.register_invariant(status_invariant).await?;
    println!("   ✓ Registered: Status must be valid (Error)");

    // Invariant 3: Transaction count must be non-negative
    let tx_count_invariant = QInvariant {
        id: "inv-txcount-nonnegative".to_string(),
        name: "Transaction count must be non-negative".to_string(),
        description: Some("Transaction count cannot be negative".to_string()),
        predicate: InvariantPredicate::StateComparison {
            field: "tx_count".to_string(),
            operator: ComparisonOperator::Ge,
            value: serde_json::json!(0),
        },
        severity: InvariantSeverity::Error,
        enabled: true,
    };

    verifier.register_invariant(tx_count_invariant).await?;
    println!("   ✓ Registered: Transaction count must be non-negative (Error)\n");

    // Test Case 1: Valid commit
    println!("2. Test Case 1: Valid commit (should succeed)");
    let valid_commit = create_commit(
        "commit-valid",
        ("active", 1000, 5),
        ("active", 900, 6), // Withdrew 100, incremented tx_count
    );

    let result = verifier.verify_commit(&valid_commit).await?;
    println!(
        "   Result: {} invariants checked",
        result.invariant_results.len()
    );
    println!("   Allowed: {}", result.is_allowed());
    if result.is_allowed() {
        println!("   ✓ Commit would be accepted\n");
    }

    // Test Case 2: Commit that violates balance invariant
    println!("3. Test Case 2: Overdraft attempt (should be blocked)");
    let overdraft_commit = create_commit(
        "commit-overdraft",
        ("active", 100, 10),
        ("active", -50, 11), // Tried to withdraw 150 when only 100 available
    );

    let result = verifier.verify_commit(&overdraft_commit).await?;
    println!(
        "   Result: {} invariants checked",
        result.invariant_results.len()
    );
    println!("   Allowed: {}", result.is_allowed());
    println!("   Violations: {:?}", result.blocking_violations);

    if result.is_blocked() {
        println!("   ✓ Commit blocked by jidoka mechanism");

        // Emit refusal receipt
        let receipt = verifier.block_commit(&overdraft_commit, &result).await?;
        println!("   Refusal receipt generated:");
        println!("     Receipt ID: {}", receipt.receipt_id);
        println!("     Packet ID: {}", receipt.packet_id);
        println!("     Reason: {:?}", receipt.reason);
        println!();
    }

    // Test Case 3: Commit with invalid status
    println!("4. Test Case 3: Invalid status (should be blocked)");
    let invalid_status_commit = create_commit(
        "commit-invalid-status",
        ("active", 1000, 5),
        ("pending", 1000, 5), // "pending" is not a valid status
    );

    let result = verifier.verify_commit(&invalid_status_commit).await?;
    println!(
        "   Result: {} invariants checked",
        result.invariant_results.len()
    );
    println!("   Allowed: {}", result.is_allowed());
    println!("   Violations: {:?}", result.blocking_violations);

    if result.is_blocked() {
        println!("   ✓ Commit blocked by jidoka mechanism");
        let receipt = verifier
            .block_commit(&invalid_status_commit, &result)
            .await?;
        println!("   Refusal receipt: {}", receipt.receipt_id);
        println!();
    }

    // Test Case 4: Multiple violations
    println!("5. Test Case 4: Multiple violations (should be blocked)");
    let multi_violation_commit = create_commit(
        "commit-multi-violation",
        ("active", 500, 10),
        ("invalid", -100, -5), // Invalid status, negative balance, negative tx_count
    );

    let result = verifier.verify_commit(&multi_violation_commit).await?;
    println!(
        "   Result: {} invariants checked",
        result.invariant_results.len()
    );
    println!("   Allowed: {}", result.is_allowed());
    println!(
        "   Violations: {} invariants violated",
        result.blocking_violations.len()
    );
    println!("   Violation IDs: {:?}", result.blocking_violations);

    if result.is_blocked() {
        println!("   ✓ Commit blocked by jidoka mechanism");
        println!();
    }

    // Test Case 5: Disable an invariant and retry
    println!("6. Test Case 5: Disabling invariants");
    verifier
        .set_invariant_enabled("inv-balance-nonnegative", false)
        .await?;
    println!("   Disabled: Balance must be non-negative");

    let result = verifier.verify_commit(&overdraft_commit).await?;
    println!("   Re-checking overdraft commit...");
    println!("   Allowed: {}", result.is_allowed());
    if result.is_allowed() {
        println!("   ✓ Commit now allowed (invariant disabled)\n");
    }

    // Re-enable for final summary
    verifier
        .set_invariant_enabled("inv-balance-nonnegative", true)
        .await?;

    // Summary
    println!("=== Summary ===");
    let all_invariants = verifier.list_invariants().await;
    println!("Total registered invariants: {}", all_invariants.len());
    for inv in all_invariants {
        println!(
            "  - {} [{}] (enabled: {})",
            inv.name, inv.severity as i32, inv.enabled
        );
    }

    println!("\n✓ Q invariant verifier demonstration complete!");
    println!("The jidoka 'stop-the-line' mechanism successfully blocked invalid commits.");

    Ok(())
}

/// Helper function to create a commit with given states.
fn create_commit(
    commit_id: &str,
    pre_state: (&str, i64, i64),
    post_state: (&str, i64, i64),
) -> Commit {
    let (pre_status, pre_balance, pre_tx_count) = pre_state;
    let (post_status, post_balance, post_tx_count) = post_state;

    let mut pre_state_data = HashMap::new();
    pre_state_data.insert("status".to_string(), serde_json::json!(pre_status));
    pre_state_data.insert("balance".to_string(), serde_json::json!(pre_balance));
    pre_state_data.insert("tx_count".to_string(), serde_json::json!(pre_tx_count));

    let mut post_state_data = HashMap::new();
    post_state_data.insert("status".to_string(), serde_json::json!(post_status));
    post_state_data.insert("balance".to_string(), serde_json::json!(post_balance));
    post_state_data.insert("tx_count".to_string(), serde_json::json!(post_tx_count));

    Commit {
        commit_id: commit_id.to_string(),
        pre_state: StateSnapshot {
            snapshot_id: format!("{}-pre", commit_id),
            state: pre_state_data,
            #[cfg(feature = "timestamps")]
            timestamp: chrono::Utc::now(),
            #[cfg(not(feature = "timestamps"))]
            timestamp: "2026-02-09T12:00:00Z".to_string(),
            metadata: HashMap::new(),
        },
        post_state: StateSnapshot {
            snapshot_id: format!("{}-post", commit_id),
            state: post_state_data,
            #[cfg(feature = "timestamps")]
            timestamp: chrono::Utc::now(),
            #[cfg(not(feature = "timestamps"))]
            timestamp: "2026-02-09T12:00:01Z".to_string(),
            metadata: HashMap::new(),
        },
        description: Some(format!("Test commit: {}", commit_id)),
        metadata: HashMap::new(),
    }
}
