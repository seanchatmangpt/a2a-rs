//! Demonstration of the conflict resolver
//!
//! Shows how to use the resolver in both auto and interactive modes

use ggen_sync::resolver::{ResolutionStrategy, ResolverConfig, resolve_conflicts};
use ggen_sync::types::{FieldChange, SyncDiff};

fn main() {
    println!("=== Conflict Resolver Demo ===\n");

    // Create some sample diffs
    let diffs = vec![
        SyncDiff::Added {
            type_name: "NewType".to_string(),
        },
        SyncDiff::Removed {
            type_name: "OldType".to_string(),
        },
        SyncDiff::Modified {
            type_name: "ChangedType".to_string(),
            field_changes: vec![
                FieldChange::Added {
                    name: "new_field".to_string(),
                    field_type: "String".to_string(),
                },
                FieldChange::TypeMismatch {
                    name: "existing_field".to_string(),
                    ontology_type: "i64".to_string(),
                    code_type: "i32".to_string(),
                },
            ],
        },
    ];

    println!("Detected {} conflicts:\n", diffs.len());
    for diff in &diffs {
        match diff {
            SyncDiff::Added { type_name } => {
                println!("  [+] Added: {}", type_name);
            }
            SyncDiff::Removed { type_name } => {
                println!("  [-] Removed: {}", type_name);
            }
            SyncDiff::Modified {
                type_name,
                field_changes,
            } => {
                println!(
                    "  [~] Modified: {} ({} changes)",
                    type_name,
                    field_changes.len()
                );
            }
        }
    }

    println!("\n--- Auto Mode (Take Ontology) ---");
    let config = ResolverConfig::auto(ResolutionStrategy::TakeOntology);
    match resolve_conflicts(diffs.clone(), &config) {
        Ok(actions) => {
            println!("Resolved {} conflicts:", actions.len());
            for action in &actions {
                println!("  {:?}", action);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    println!("\n--- Auto Mode (Take Code) ---");
    let config = ResolverConfig::auto(ResolutionStrategy::TakeCode);
    match resolve_conflicts(diffs.clone(), &config) {
        Ok(actions) => {
            println!("Resolved {} conflicts:", actions.len());
            for action in &actions {
                println!("  {:?}", action);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    println!("\n--- Interactive Mode ---");
    println!("(Run this example with stdin to test interactive mode)");
    // Uncomment to test interactive mode:
    // let config = ResolverConfig::interactive();
    // match resolve_conflicts(diffs, &config) {
    //     Ok(actions) => {
    //         println!("Resolved {} conflicts:", actions.len());
    //         for action in &actions {
    //             println!("  {:?}", action);
    //         }
    //     }
    //     Err(e) => eprintln!("Error: {}", e),
    // }
}
