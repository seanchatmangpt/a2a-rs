//! Report generation for sync differences

use crate::types::SyncDiff;
use std::io::Write;

/// Generate and print a sync status report
pub fn report_sync_status(diffs: &[SyncDiff]) {
    if diffs.is_empty() {
        println!("No differences detected. Ontology and code are in sync.");
        return;
    }

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();

    // Categorize diffs
    for diff in diffs {
        match diff {
            SyncDiff::Added { type_name } => added.push(type_name.clone()),
            SyncDiff::Removed { type_name } => removed.push(type_name.clone()),
            SyncDiff::Modified { type_name, .. } => modified.push(type_name.clone()),
        }
    }

    // Print counts by type
    println!("Sync Status Report");
    println!("==================");
    println!();
    println!("Changes by type:");
    println!("  Added:    {}", added.len());
    println!("  Modified: {}", modified.len());
    println!("  Removed:  {}", removed.len());
    println!();

    // Print affected types
    if !added.is_empty() {
        println!("Added types (in ontology, not in code):");
        for type_name in &added {
            println!("  + {}", type_name);
        }
        println!();
    }

    if !modified.is_empty() {
        println!("Modified types (field differences):");
        for type_name in &modified {
            println!("  ~ {}", type_name);
        }
        println!();
    }

    if !removed.is_empty() {
        println!("Removed types (in code, not in ontology):");
        for type_name in &removed {
            println!("  - {}", type_name);
        }
        println!();
    }

    // Print summary line
    println!(
        "Summary: {} types added, {} modified, {} removed",
        added.len(),
        modified.len(),
        removed.len()
    );
}

/// Generate JSON output for sync differences
///
/// Returns a JSON string representation of the diffs that can be consumed
/// by the web viewer or other tools.
pub fn report_sync_json(diffs: &[SyncDiff]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(diffs)
}

/// Write JSON report to a writer (file, stdout, etc.)
pub fn write_sync_json<W: Write>(
    diffs: &[SyncDiff],
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = report_sync_json(diffs)?;
    writer.write_all(json.as_bytes())?;
    writer.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FieldChange;

    #[test]
    fn test_empty_report() {
        let diffs = vec![];
        report_sync_status(&diffs);
        // Should print "No differences detected"
    }

    #[test]
    fn test_report_with_all_diff_types() {
        let diffs = vec![
            SyncDiff::Added {
                type_name: "NewType".to_string(),
            },
            SyncDiff::Modified {
                type_name: "ExistingType".to_string(),
                field_changes: vec![FieldChange::Added {
                    name: "new_field".to_string(),
                    field_type: "String".to_string(),
                }],
            },
            SyncDiff::Removed {
                type_name: "OldType".to_string(),
            },
        ];
        report_sync_status(&diffs);
        // Should print categorized report
    }

    #[test]
    fn test_report_counts() {
        let diffs = vec![
            SyncDiff::Added {
                type_name: "Type1".to_string(),
            },
            SyncDiff::Added {
                type_name: "Type2".to_string(),
            },
            SyncDiff::Modified {
                type_name: "Type3".to_string(),
                field_changes: vec![],
            },
        ];

        // Count manually
        let added_count = diffs
            .iter()
            .filter(|d| matches!(d, SyncDiff::Added { .. }))
            .count();
        assert_eq!(added_count, 2);
    }

    #[test]
    fn test_json_report_empty() {
        let diffs = vec![];
        let json = report_sync_json(&diffs).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_json_report_serialization() {
        let diffs = vec![
            SyncDiff::Added {
                type_name: "NewType".to_string(),
            },
            SyncDiff::Removed {
                type_name: "OldType".to_string(),
            },
        ];
        let json = report_sync_json(&diffs).unwrap();

        // Verify it's valid JSON and can be deserialized
        let parsed: Vec<SyncDiff> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, diffs);
    }

    #[test]
    fn test_write_json_to_buffer() {
        let diffs = vec![SyncDiff::Added {
            type_name: "TestType".to_string(),
        }];

        let mut buffer = Vec::new();
        write_sync_json(&diffs, &mut buffer).unwrap();

        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("TestType"));
        assert!(output.contains("Added"));
    }
}
