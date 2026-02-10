//! Example: Generate database migrations from schema changes
//!
//! This example demonstrates how to detect breaking changes between
//! schema versions and generate SQLx-compatible migration files.

use ggen_sync::{
    BreakingChange, DatabaseBackend, FieldChange, FieldDef, OntologyNode, SyncDiff,
    detect_breaking_changes, generate_migrations,
};
use std::collections::HashMap;

fn main() {
    println!("=== Schema Migration Generator Example ===\n");

    // Simulate some schema changes
    let diffs = create_sample_diffs();
    let ontology = create_sample_ontology();

    // 1. Detect breaking changes
    println!("Step 1: Detecting breaking changes...");
    let breaking = detect_breaking_changes(&diffs, &ontology);

    if breaking.is_empty() {
        println!("  No breaking changes detected!");
        return;
    }

    println!("  Found {} breaking change(s):\n", breaking.len());
    for (i, change) in breaking.iter().enumerate() {
        println!("  {}. {:?}", i + 1, change);
    }

    // 2. Generate migrations for different database backends
    println!("\n\nStep 2: Generating migrations...\n");

    for backend in [
        DatabaseBackend::Sqlite,
        DatabaseBackend::Postgres,
        DatabaseBackend::Mysql,
    ] {
        println!("--- {:?} Backend ---", backend);
        let migrations = generate_migrations(&breaking, backend);

        for migration in migrations {
            println!("\nMigration: {}", migration.description);
            println!(
                "Files: {} / {}",
                migration.up_filename(),
                migration.down_filename()
            );
            println!("\nUP SQL:");
            println!("{}", migration.up_sql);
            println!("DOWN SQL:");
            println!("{}", migration.down_sql);
        }
        println!();
    }
}

/// Create sample schema differences
fn create_sample_diffs() -> Vec<SyncDiff> {
    vec![
        // Type removed from ontology
        SyncDiff::Removed {
            type_name: "OldTask".to_string(),
        },
        // Type with field changes
        SyncDiff::Modified {
            type_name: "User".to_string(),
            field_changes: vec![
                // Field removed
                FieldChange::Removed {
                    name: "legacy_id".to_string(),
                    field_type: "i32".to_string(),
                },
                // Field type changed
                FieldChange::TypeMismatch {
                    name: "score".to_string(),
                    ontology_type: "f64".to_string(),
                    code_type: "i32".to_string(),
                },
                // Required field added
                FieldChange::Added {
                    name: "email".to_string(),
                    field_type: "String".to_string(), // Required (not Option)
                },
                // Optional field added (not breaking)
                FieldChange::Added {
                    name: "phone".to_string(),
                    field_type: "Option<String>".to_string(),
                },
            ],
        },
    ]
}

/// Create sample ontology
fn create_sample_ontology() -> HashMap<String, OntologyNode> {
    let mut ontology = HashMap::new();

    ontology.insert(
        "User".to_string(),
        OntologyNode::new(
            "User",
            vec![
                FieldDef::new("id", "i64"),
                FieldDef::new("name", "String"),
                FieldDef::new("score", "f64"),
                FieldDef::new("email", "String"),
                FieldDef::new("phone", "Option<String>"),
            ],
        ),
    );

    ontology
}
