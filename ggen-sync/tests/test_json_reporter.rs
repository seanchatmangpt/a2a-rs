//! Test JSON serialization for web viewer

use ggen_sync::{FieldChange, SyncDiff, report_sync_json};

#[test]
fn test_json_serialization_round_trip() {
    let diffs = vec![
        SyncDiff::Added {
            type_name: "NewType".to_string(),
        },
        SyncDiff::Modified {
            type_name: "ExistingType".to_string(),
            field_changes: vec![
                FieldChange::Added {
                    name: "field1".to_string(),
                    field_type: "String".to_string(),
                },
                FieldChange::TypeMismatch {
                    name: "field2".to_string(),
                    ontology_type: "i32".to_string(),
                    code_type: "u32".to_string(),
                },
            ],
        },
        SyncDiff::Removed {
            type_name: "OldType".to_string(),
        },
    ];

    // Serialize to JSON
    let json = report_sync_json(&diffs).expect("Failed to serialize");

    // Deserialize back
    let parsed: Vec<SyncDiff> = serde_json::from_str(&json).expect("Failed to deserialize");

    // Should match original
    assert_eq!(parsed, diffs);
}

#[test]
fn test_empty_diffs_json() {
    let diffs: Vec<SyncDiff> = vec![];
    let json = report_sync_json(&diffs).expect("Failed to serialize empty vec");
    assert_eq!(json, "[]");
}

#[test]
fn test_json_contains_expected_fields() {
    let diffs = vec![SyncDiff::Modified {
        type_name: "TestType".to_string(),
        field_changes: vec![FieldChange::Added {
            name: "test_field".to_string(),
            field_type: "bool".to_string(),
        }],
    }];

    let json = report_sync_json(&diffs).expect("Failed to serialize");

    // Verify JSON structure
    assert!(json.contains("Modified"));
    assert!(json.contains("TestType"));
    assert!(json.contains("test_field"));
    assert!(json.contains("bool"));
}
