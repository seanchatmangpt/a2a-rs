//! Differ module for detecting drift between ontology and code

use crate::types::{CodeNode, FieldChange, OntologyNode, SyncDiff};
use std::collections::{HashMap, HashSet};

/// Detect differences between ontology and generated code
pub fn detect_diffs(
    ontology: &HashMap<String, OntologyNode>,
    code: &HashMap<String, CodeNode>,
) -> Vec<SyncDiff> {
    let mut diffs = Vec::new();

    let ontology_types: HashSet<_> = ontology.keys().collect();
    let code_types: HashSet<_> = code.keys().collect();

    // Find types only in ontology (added)
    for type_name in ontology_types.difference(&code_types) {
        diffs.push(SyncDiff::Added {
            type_name: (*type_name).clone(),
        });
    }

    // Find types only in code (removed)
    for type_name in code_types.difference(&ontology_types) {
        diffs.push(SyncDiff::Removed {
            type_name: (*type_name).clone(),
        });
    }

    // Find types in both with differences (modified)
    for type_name in ontology_types.intersection(&code_types) {
        let onto_node = &ontology[*type_name];
        let code_node = &code[*type_name];

        let field_changes = detect_field_changes(onto_node, code_node);
        if !field_changes.is_empty() {
            diffs.push(SyncDiff::Modified {
                type_name: (*type_name).clone(),
                field_changes,
            });
        }
    }

    diffs
}

/// Detect field-level changes between ontology and code nodes
fn detect_field_changes(onto: &OntologyNode, code: &CodeNode) -> Vec<FieldChange> {
    let mut changes = Vec::new();

    let onto_fields = onto.fields_map();
    let code_fields = code.fields_map();

    let onto_names: HashSet<_> = onto_fields.keys().collect();
    let code_names: HashSet<_> = code_fields.keys().collect();

    // Fields in ontology but not in code (added)
    for name in onto_names.difference(&code_names) {
        changes.push(FieldChange::Added {
            name: (*name).clone(),
            field_type: onto_fields[*name].clone(),
        });
    }

    // Fields in code but not in ontology (removed)
    for name in code_names.difference(&onto_names) {
        changes.push(FieldChange::Removed {
            name: (*name).clone(),
            field_type: code_fields[*name].clone(),
        });
    }

    // Fields in both with type mismatches
    for name in onto_names.intersection(&code_names) {
        let onto_type = &onto_fields[*name];
        let code_type = &code_fields[*name];

        if onto_type != code_type {
            changes.push(FieldChange::TypeMismatch {
                name: (*name).clone(),
                ontology_type: onto_type.clone(),
                code_type: code_type.clone(),
            });
        }
    }

    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FieldDef;

    #[test]
    fn test_no_differences() {
        let mut ontology = HashMap::new();
        let mut code = HashMap::new();

        ontology.insert(
            "Person".to_string(),
            OntologyNode::new(
                "Person",
                vec![FieldDef::new("name", "String"), FieldDef::new("age", "u32")],
            ),
        );

        code.insert(
            "Person".to_string(),
            CodeNode::new(
                "Person",
                vec![FieldDef::new("name", "String"), FieldDef::new("age", "u32")],
            ),
        );

        let diffs = detect_diffs(&ontology, &code);
        assert!(diffs.is_empty());
    }

    #[test]
    fn test_added_type() {
        let mut ontology = HashMap::new();
        let code = HashMap::new();

        ontology.insert(
            "Person".to_string(),
            OntologyNode::new("Person", vec![FieldDef::new("name", "String")]),
        );

        let diffs = detect_diffs(&ontology, &code);
        assert_eq!(diffs.len(), 1);

        match &diffs[0] {
            SyncDiff::Added { type_name } => {
                assert_eq!(type_name, "Person");
            }
            _ => panic!("Expected Added variant"),
        }
    }

    #[test]
    fn test_removed_type() {
        let ontology = HashMap::new();
        let mut code = HashMap::new();

        code.insert(
            "Person".to_string(),
            CodeNode::new("Person", vec![FieldDef::new("name", "String")]),
        );

        let diffs = detect_diffs(&ontology, &code);
        assert_eq!(diffs.len(), 1);

        match &diffs[0] {
            SyncDiff::Removed { type_name } => {
                assert_eq!(type_name, "Person");
            }
            _ => panic!("Expected Removed variant"),
        }
    }

    #[test]
    fn test_added_field() {
        let mut ontology = HashMap::new();
        let mut code = HashMap::new();

        ontology.insert(
            "Person".to_string(),
            OntologyNode::new(
                "Person",
                vec![FieldDef::new("name", "String"), FieldDef::new("age", "u32")],
            ),
        );

        code.insert(
            "Person".to_string(),
            CodeNode::new("Person", vec![FieldDef::new("name", "String")]),
        );

        let diffs = detect_diffs(&ontology, &code);
        assert_eq!(diffs.len(), 1);

        match &diffs[0] {
            SyncDiff::Modified {
                type_name,
                field_changes,
            } => {
                assert_eq!(type_name, "Person");
                assert_eq!(field_changes.len(), 1);

                match &field_changes[0] {
                    FieldChange::Added { name, field_type } => {
                        assert_eq!(name, "age");
                        assert_eq!(field_type, "u32");
                    }
                    _ => panic!("Expected Added field change"),
                }
            }
            _ => panic!("Expected Modified variant"),
        }
    }

    #[test]
    fn test_removed_field() {
        let mut ontology = HashMap::new();
        let mut code = HashMap::new();

        ontology.insert(
            "Person".to_string(),
            OntologyNode::new("Person", vec![FieldDef::new("name", "String")]),
        );

        code.insert(
            "Person".to_string(),
            CodeNode::new(
                "Person",
                vec![FieldDef::new("name", "String"), FieldDef::new("age", "u32")],
            ),
        );

        let diffs = detect_diffs(&ontology, &code);
        assert_eq!(diffs.len(), 1);

        match &diffs[0] {
            SyncDiff::Modified {
                type_name,
                field_changes,
            } => {
                assert_eq!(type_name, "Person");
                assert_eq!(field_changes.len(), 1);

                match &field_changes[0] {
                    FieldChange::Removed { name, field_type } => {
                        assert_eq!(name, "age");
                        assert_eq!(field_type, "u32");
                    }
                    _ => panic!("Expected Removed field change"),
                }
            }
            _ => panic!("Expected Modified variant"),
        }
    }

    #[test]
    fn test_field_type_mismatch() {
        let mut ontology = HashMap::new();
        let mut code = HashMap::new();

        ontology.insert(
            "Person".to_string(),
            OntologyNode::new("Person", vec![FieldDef::new("age", "u32")]),
        );

        code.insert(
            "Person".to_string(),
            CodeNode::new("Person", vec![FieldDef::new("age", "i32")]),
        );

        let diffs = detect_diffs(&ontology, &code);
        assert_eq!(diffs.len(), 1);

        match &diffs[0] {
            SyncDiff::Modified {
                type_name,
                field_changes,
            } => {
                assert_eq!(type_name, "Person");
                assert_eq!(field_changes.len(), 1);

                match &field_changes[0] {
                    FieldChange::TypeMismatch {
                        name,
                        ontology_type,
                        code_type,
                    } => {
                        assert_eq!(name, "age");
                        assert_eq!(ontology_type, "u32");
                        assert_eq!(code_type, "i32");
                    }
                    _ => panic!("Expected TypeMismatch field change"),
                }
            }
            _ => panic!("Expected Modified variant"),
        }
    }

    #[test]
    fn test_multiple_changes() {
        let mut ontology = HashMap::new();
        let mut code = HashMap::new();

        // Type with multiple field changes
        ontology.insert(
            "Person".to_string(),
            OntologyNode::new(
                "Person",
                vec![
                    FieldDef::new("name", "String"),
                    FieldDef::new("age", "u32"),
                    FieldDef::new("email", "String"),
                ],
            ),
        );

        code.insert(
            "Person".to_string(),
            CodeNode::new(
                "Person",
                vec![
                    FieldDef::new("name", "String"),
                    FieldDef::new("age", "i32"),      // Type mismatch
                    FieldDef::new("phone", "String"), // Removed from ontology
                ],
            ),
        );

        // Type only in ontology
        ontology.insert(
            "Task".to_string(),
            OntologyNode::new("Task", vec![FieldDef::new("id", "String")]),
        );

        // Type only in code
        code.insert(
            "Message".to_string(),
            CodeNode::new("Message", vec![FieldDef::new("text", "String")]),
        );

        let diffs = detect_diffs(&ontology, &code);
        assert_eq!(diffs.len(), 3);

        // Check we have one of each type
        let added_count = diffs
            .iter()
            .filter(|d| matches!(d, SyncDiff::Added { .. }))
            .count();
        let removed_count = diffs
            .iter()
            .filter(|d| matches!(d, SyncDiff::Removed { .. }))
            .count();
        let modified_count = diffs
            .iter()
            .filter(|d| matches!(d, SyncDiff::Modified { .. }))
            .count();

        assert_eq!(added_count, 1);
        assert_eq!(removed_count, 1);
        assert_eq!(modified_count, 1);

        // Check the modified type has 3 field changes
        let modified_diff = diffs
            .iter()
            .find(|d| matches!(d, SyncDiff::Modified { .. }))
            .unwrap();

        if let SyncDiff::Modified { field_changes, .. } = modified_diff {
            assert_eq!(field_changes.len(), 3); // email added, phone removed, age type mismatch
        }
    }
}
