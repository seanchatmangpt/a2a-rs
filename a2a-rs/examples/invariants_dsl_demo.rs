//! Demo of the declarative invariants DSL
//!
//! This example shows how to use the DSL to define invariants declaratively
//! instead of implementing the Invariant trait manually.
//!
//! Run with:
//! ```bash
//! cargo run --example invariants_dsl_demo
//! ```

use a2a_rs::construct::invariants::{InvariantRegistry, parse_invariant};
use a2a_rs::domain::{Task, TaskState};

fn main() {
    println!("=== Invariants DSL Demo ===\n");

    // Create a sample task
    let mut task = Task::new("task-123".to_string(), "ctx-456".to_string());
    task.artifacts = Some(vec![]);
    task.history = Some(vec![]);

    println!("Task ID: {}", task.id);
    println!("Context ID: {}", task.context_id);
    println!("State: {:?}\n", task.status.state);

    // Create a registry for invariants
    let mut registry = InvariantRegistry::new();

    // Parse and register various invariants
    println!("Registering invariants...\n");

    // Simple field comparison
    let inv1 = parse_invariant(r#"INVARIANT id == "task-123""#).unwrap();
    println!("✓ Invariant 1: {}", inv1.source);
    registry.register("id_check", Box::new(inv1));

    // Array length check
    let inv2 = parse_invariant("INVARIANT artifacts.length <= 100").unwrap();
    println!("✓ Invariant 2: {}", inv2.source);
    registry.register("artifacts_limit", Box::new(inv2));

    // Combined conditions
    let inv3 = parse_invariant("INVARIANT history.length >= 0 AND history.length <= 1000").unwrap();
    println!("✓ Invariant 3: {}", inv3.source);
    registry.register("history_bounds", Box::new(inv3));

    // Logical OR
    let inv4 = parse_invariant(r#"INVARIANT kind == "task" OR kind == "message""#).unwrap();
    println!("✓ Invariant 4: {}", inv4.source);
    registry.register("kind_check", Box::new(inv4));

    println!("\n--- Checking all invariants ---\n");

    // Check all invariants
    match registry.check_all(&task) {
        Ok(()) => {
            println!("✅ All invariants passed!");
        }
        Err(e) => {
            println!("❌ Invariant violation: {}", e);
        }
    }

    // Now break an invariant by adding too many items
    println!("\n--- Adding 101 artifacts to violate artifact limit ---\n");
    for i in 0..101 {
        task.artifacts
            .as_mut()
            .unwrap()
            .push(a2a_rs::domain::Artifact {
                artifact_id: format!("artifact-{}", i),
                content: serde_json::json!({"data": i}),
            });
    }

    match registry.check_all(&task) {
        Ok(()) => {
            println!("✅ All invariants passed!");
        }
        Err(e) => {
            println!("❌ Invariant violation: {}", e);
        }
    }

    // Show examples of parsing different expressions
    println!("\n--- DSL Expression Examples ---\n");

    let examples = vec![
        "INVARIANT x > 0",
        "INVARIANT name == \"example\"",
        "INVARIANT count >= 1 AND count <= 100",
        "INVARIANT status == \"active\" OR status == \"pending\"",
        "INVARIANT NOT disabled",
        "INVARIANT (x > 0 AND x < 10) OR x == 100",
        "INVARIANT items.length <= 50",
    ];

    for example in examples {
        match parse_invariant(example) {
            Ok(expr) => println!("✓ {}", expr.source),
            Err(e) => println!("✗ Failed to parse: {} ({})", example, e),
        }
    }

    println!("\n=== Demo Complete ===");
}
