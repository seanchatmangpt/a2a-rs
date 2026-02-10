//! Example demonstrating JSON output for the web viewer

use ggen_sync::{FieldChange, SyncDiff, report_sync_json, write_sync_json};

fn main() {
    // Create sample diffs
    let diffs = vec![
        SyncDiff::Added {
            type_name: "NewAgentCapability".to_string(),
        },
        SyncDiff::Modified {
            type_name: "Message".to_string(),
            field_changes: vec![
                FieldChange::Added {
                    name: "timestamp".to_string(),
                    field_type: "DateTime<Utc>".to_string(),
                },
                FieldChange::Removed {
                    name: "deprecated_field".to_string(),
                    field_type: "String".to_string(),
                },
                FieldChange::TypeMismatch {
                    name: "version".to_string(),
                    ontology_type: "String".to_string(),
                    code_type: "u32".to_string(),
                },
            ],
        },
        SyncDiff::Removed {
            type_name: "LegacyType".to_string(),
        },
    ];

    println!("Example: JSON output for web viewer");
    println!("====================================\n");

    // Generate JSON string
    match report_sync_json(&diffs) {
        Ok(json) => {
            println!("JSON Output:");
            println!("{}", json);
            println!();

            // Write to file
            let output_path = "sync-status.json";
            match std::fs::File::create(output_path) {
                Ok(mut file) => {
                    if let Err(e) = write_sync_json(&diffs, &mut file) {
                        eprintln!("Failed to write JSON file: {}", e);
                    } else {
                        println!("JSON written to: {}", output_path);
                        println!("You can now open web/index.html and load this file.");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to create output file: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to generate JSON: {}", e);
        }
    }
}
