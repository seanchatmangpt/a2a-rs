//! Reverse sync: code → ontology
//!
//! Takes diffs between code and ontology and generates RDF triples to update
//! ontology files with types that exist in code but not in the ontology.
//!
//! 80/20 approach: Basic type creation, append to TTL files, don't reorganize.

#[cfg(test)]
use crate::types::FieldDef;
use crate::types::{CodeNode, FieldChange, SyncDiff};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReverseSyncError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Type not found in code nodes: {0}")]
    TypeNotFound(String),

    #[error("Invalid field type: {0}")]
    InvalidFieldType(String),
}

type Result<T> = std::result::Result<T, ReverseSyncError>;

// Namespace constants
const A2A_NS: &str = "https://ggen.io/ontology/a2a/";

/// Apply reverse sync: update ontology files based on code differences
///
/// # Arguments
/// * `diffs` - Vector of detected differences between code and ontology
/// * `code_nodes` - Map of type name to code definitions
/// * `ontology_dir` - Path to ontology directory (contains *.ttl files)
///
/// # Behavior
/// - `SyncDiff::Removed` → Create new entity and properties in ontology
/// - `SyncDiff::Modified` with added fields → Add new property definitions
/// - Appends to appropriate TTL files (creates new file if needed)
pub fn apply_reverse_sync(
    diffs: &[SyncDiff],
    code_nodes: &HashMap<String, CodeNode>,
    ontology_dir: &Path,
) -> Result<()> {
    for diff in diffs {
        match diff {
            SyncDiff::Removed { type_name } => {
                // Type exists in code but not in ontology - create it
                let code_node = code_nodes
                    .get(type_name)
                    .ok_or_else(|| ReverseSyncError::TypeNotFound(type_name.clone()))?;

                create_entity_in_ontology(code_node, ontology_dir)?;
                println!("  Created entity '{}' in ontology", type_name);
            }
            SyncDiff::Modified {
                type_name,
                field_changes,
            } => {
                // Type exists in both, but has field differences
                for field_change in field_changes {
                    if let FieldChange::Removed { name, field_type } = field_change {
                        // Field exists in code but not in ontology - add it
                        add_property_to_entity(type_name, name, field_type, ontology_dir)?;
                        println!("  Added property '{}.{}' to ontology", type_name, name);
                    }
                }
            }
            SyncDiff::Added { .. } => {
                // Type exists in ontology but not in code - this is forward sync
                // Skip for reverse sync
            }
        }
    }

    Ok(())
}

/// Create a new entity definition in the ontology
fn create_entity_in_ontology(code_node: &CodeNode, ontology_dir: &Path) -> Result<()> {
    // Determine which TTL file to append to (use a default or create one)
    let ttl_file = ontology_dir.join("a2a-generated.ttl");

    // Create file if it doesn't exist, with header
    if !ttl_file.exists() {
        create_ttl_file_with_header(&ttl_file)?;
    }

    // Open file in append mode
    let mut file = OpenOptions::new().append(true).open(&ttl_file)?;

    // Generate entity definition
    let entity_ttl = generate_entity_ttl(code_node)?;

    // Write to file
    writeln!(file)?;
    writeln!(
        file,
        "# =============================================================================
"
    )?;
    writeln!(file, "# {} Entity", code_node.name)?;
    writeln!(
        file,
        "# =============================================================================
"
    )?;
    writeln!(file)?;
    writeln!(file, "{}", entity_ttl)?;

    Ok(())
}

/// Add a property to an existing entity
fn add_property_to_entity(
    type_name: &str,
    field_name: &str,
    field_type: &str,
    ontology_dir: &Path,
) -> Result<()> {
    // Append to the same file as entity creation
    let ttl_file = ontology_dir.join("a2a-generated.ttl");

    if !ttl_file.exists() {
        create_ttl_file_with_header(&ttl_file)?;
    }

    let mut file = OpenOptions::new().append(true).open(&ttl_file)?;

    // Generate property definition
    let property_ttl = generate_property_ttl(type_name, field_name, field_type)?;

    writeln!(file)?;
    writeln!(file, "# --- Property: {}.{} ---", type_name, field_name)?;
    writeln!(file)?;
    writeln!(file, "{}", property_ttl)?;

    Ok(())
}

/// Create a new TTL file with standard header
fn create_ttl_file_with_header(path: &Path) -> Result<()> {
    let mut file = File::create(path)?;

    writeln!(
        file,
        "# =============================================================================
"
    )?;
    writeln!(
        file,
        "# Generated Types (Reverse Sync from Code)
"
    )?;
    writeln!(
        file,
        "# Types that exist in generated code but not in the ontology
"
    )?;
    writeln!(
        file,
        "# =============================================================================
"
    )?;
    writeln!(file)?;
    writeln!(file, "@prefix a2a: <{}> .", A2A_NS)?;
    writeln!(
        file,
        "@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> ."
    )?;
    writeln!(
        file,
        "@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> ."
    )?;
    writeln!(file, "@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .")?;
    writeln!(file)?;

    Ok(())
}

/// Generate RDF/Turtle for an entity definition
fn generate_entity_ttl(code_node: &CodeNode) -> Result<String> {
    let mut ttl = String::new();

    // Entity definition
    ttl.push_str(&format!("a2a:{} a a2a:Entity ;\n", code_node.name));
    ttl.push_str(&format!("    a2a:name \"{}\" ;\n", code_node.name));
    ttl.push_str(&format!(
        "    rdfs:comment \"Generated from code via reverse sync.\" ;\n"
    ));

    // Add hasProperty declarations
    for field in &code_node.fields {
        ttl.push_str(&format!(
            "    a2a:hasProperty a2a:{}_{} ;\n",
            code_node.name, field.name
        ));
    }

    // Remove trailing semicolon and newline, add period
    if ttl.ends_with(" ;\n") {
        ttl.truncate(ttl.len() - 3);
        ttl.push_str(" .\n");
    }

    // Add property definitions
    ttl.push('\n');
    for field in &code_node.fields {
        let property_ttl = generate_property_ttl(&code_node.name, &field.name, &field.field_type)?;
        ttl.push_str(&property_ttl);
        ttl.push('\n');
    }

    Ok(ttl)
}

/// Generate RDF/Turtle for a property definition
fn generate_property_ttl(type_name: &str, field_name: &str, field_type: &str) -> Result<String> {
    let mut ttl = String::new();

    // Parse the Rust type to determine RDF attributes
    let type_info = parse_rust_type(field_type)?;

    // Property definition
    ttl.push_str(&format!(
        "a2a:{}_{} a a2a:Property ;\n",
        type_name, field_name
    ));
    ttl.push_str(&format!("    a2a:name \"{}\" ;\n", field_name));
    ttl.push_str(&format!("    a2a:type \"{}\" ;\n", type_info.base_type));
    ttl.push_str(&format!(
        "    a2a:required {} ;\n",
        if type_info.required { "true" } else { "false" }
    ));

    if type_info.is_array {
        ttl.push_str("    a2a:isArray true ;\n");
    }

    if let Some(ref_entity) = type_info.ref_entity {
        ttl.push_str(&format!("    a2a:refEntity \"{}\" ;\n", ref_entity));
    }

    ttl.push_str(&format!(
        "    rdfs:comment \"Generated from code type: {}\" .\n",
        field_type
    ));

    Ok(ttl)
}

/// Information about a parsed Rust type
#[derive(Debug)]
struct RustTypeInfo {
    base_type: String,
    required: bool,
    is_array: bool,
    ref_entity: Option<String>,
}

/// Parse a Rust type string and extract RDF-relevant information
///
/// Maps:
/// - `String` → `"string"^^xsd:string`
/// - `bool` → `"boolean"^^xsd:boolean`
/// - `i32`, `i64`, `u32`, `u64` → `"integer"^^xsd:integer`
/// - `f32`, `f64` → `"number"^^xsd:decimal`
/// - `serde_json::Value` → `"object"`
/// - `Option<T>` → required = false, recurse on T
/// - `Vec<T>` → is_array = true, recurse on T
/// - Custom types → `"reference"` with refEntity
fn parse_rust_type(rust_type: &str) -> Result<RustTypeInfo> {
    let trimmed = rust_type.trim();

    // Check for Option<T>
    if let Some(inner) = strip_wrapper(trimmed, "Option") {
        let mut inner_info = parse_rust_type(inner)?;
        inner_info.required = false;
        return Ok(inner_info);
    }

    // Check for Vec<T>
    if let Some(inner) = strip_wrapper(trimmed, "Vec") {
        let mut inner_info = parse_rust_type(inner)?;
        inner_info.is_array = true;
        return Ok(inner_info);
    }

    // Map base types
    let (base_type, ref_entity) = match trimmed {
        "String" | "str" | "&str" => ("string".to_string(), None),
        "bool" => ("boolean".to_string(), None),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" => ("integer".to_string(), None),
        "f32" | "f64" => ("number".to_string(), None),
        "serde_json::Value" | "Value" => ("object".to_string(), None),
        _ => {
            // Assume it's a reference to another entity
            ("reference".to_string(), Some(trimmed.to_string()))
        }
    };

    Ok(RustTypeInfo {
        base_type,
        required: true,
        is_array: false,
        ref_entity,
    })
}

/// Strip a wrapper type like Option<T> or Vec<T>
fn strip_wrapper<'a>(s: &'a str, wrapper: &str) -> Option<&'a str> {
    let s = s.trim();
    if s.starts_with(wrapper) && s.ends_with('>') {
        let start = wrapper.len();
        if s.as_bytes().get(start) == Some(&b'<') {
            let inner = &s[start + 1..s.len() - 1];
            return Some(inner.trim());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_types() {
        let info = parse_rust_type("String").unwrap();
        assert_eq!(info.base_type, "string");
        assert!(info.required);
        assert!(!info.is_array);
        assert_eq!(info.ref_entity, None);

        let info = parse_rust_type("bool").unwrap();
        assert_eq!(info.base_type, "boolean");
        assert!(info.required);

        let info = parse_rust_type("i64").unwrap();
        assert_eq!(info.base_type, "integer");
        assert!(info.required);
    }

    #[test]
    fn test_parse_option_type() {
        let info = parse_rust_type("Option<String>").unwrap();
        assert_eq!(info.base_type, "string");
        assert!(!info.required);
        assert!(!info.is_array);
    }

    #[test]
    fn test_parse_vec_type() {
        let info = parse_rust_type("Vec<String>").unwrap();
        assert_eq!(info.base_type, "string");
        assert!(info.required);
        assert!(info.is_array);
    }

    #[test]
    fn test_parse_complex_type() {
        let info = parse_rust_type("Vec<Option<String>>").unwrap();
        assert_eq!(info.base_type, "string");
        assert!(!info.required);
        assert!(info.is_array);
    }

    #[test]
    fn test_parse_reference_type() {
        let info = parse_rust_type("AgentCard").unwrap();
        assert_eq!(info.base_type, "reference");
        assert_eq!(info.ref_entity, Some("AgentCard".to_string()));
        assert!(info.required);
        assert!(!info.is_array);
    }

    #[test]
    fn test_strip_wrapper() {
        assert_eq!(strip_wrapper("Option<String>", "Option"), Some("String"));
        assert_eq!(strip_wrapper("Vec<i32>", "Vec"), Some("i32"));
        assert_eq!(strip_wrapper("String", "Option"), None);
    }

    #[test]
    fn test_generate_property_ttl() {
        let ttl = generate_property_ttl("TestEntity", "name", "String").unwrap();
        assert!(ttl.contains("a2a:TestEntity_name a a2a:Property"));
        assert!(ttl.contains("a2a:name \"name\""));
        assert!(ttl.contains("a2a:type \"string\""));
        assert!(ttl.contains("a2a:required true"));
    }

    #[test]
    fn test_generate_entity_ttl() {
        let code_node = CodeNode::new(
            "TestEntity",
            vec![
                FieldDef::new("name", "String"),
                FieldDef::new("count", "i32"),
            ],
        );

        let ttl = generate_entity_ttl(&code_node).unwrap();
        assert!(ttl.contains("a2a:TestEntity a a2a:Entity"));
        assert!(ttl.contains("a2a:hasProperty a2a:TestEntity_name"));
        assert!(ttl.contains("a2a:hasProperty a2a:TestEntity_count"));
        assert!(ttl.contains("a2a:TestEntity_name a a2a:Property"));
        assert!(ttl.contains("a2a:TestEntity_count a a2a:Property"));
    }
}
