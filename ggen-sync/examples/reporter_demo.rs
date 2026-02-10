//! Example demonstrating the sync status reporter

use ggen_sync::{FieldChange, SyncDiff, report_sync_status};

fn main() {
    println!("Example 1: No differences");
    println!("==========================");
    let no_diffs = vec![];
    report_sync_status(&no_diffs);

    println!("\n\nExample 2: All types of changes");
    println!("================================");
    let all_types = vec![
        SyncDiff::Added {
            type_name: "NewAgent".to_string(),
        },
        SyncDiff::Added {
            type_name: "NewTask".to_string(),
        },
        SyncDiff::Modified {
            type_name: "Message".to_string(),
            field_changes: vec![
                FieldChange::Added {
                    name: "timestamp".to_string(),
                    field_type: "DateTime<Utc>".to_string(),
                },
                FieldChange::Removed {
                    name: "old_field".to_string(),
                    field_type: "String".to_string(),
                },
            ],
        },
        SyncDiff::Modified {
            type_name: "AgentCard".to_string(),
            field_changes: vec![FieldChange::TypeMismatch {
                name: "version".to_string(),
                ontology_type: "String".to_string(),
                code_type: "u32".to_string(),
            }],
        },
        SyncDiff::Removed {
            type_name: "DeprecatedType".to_string(),
        },
    ];
    report_sync_status(&all_types);

    println!("\n\nExample 3: Only additions");
    println!("==========================");
    let additions_only = vec![
        SyncDiff::Added {
            type_name: "Feature1".to_string(),
        },
        SyncDiff::Added {
            type_name: "Feature2".to_string(),
        },
        SyncDiff::Added {
            type_name: "Feature3".to_string(),
        },
    ];
    report_sync_status(&additions_only);
}
