//! Forward sync: ontology → generated code
//!
//! Takes detected sync diffs and ontology definitions to generate/update
//! Rust struct definitions in the generated/ directory.

use crate::types::{FieldChange, OntologyNode, SyncDiff};
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ForwardSyncError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to generate code for type '{0}': {1}")]
    CodeGen(String, String),

    #[error("Ontology node not found: {0}")]
    NodeNotFound(String),
}

pub type Result<T> = std::result::Result<T, ForwardSyncError>;

/// Apply forward sync diffs to generate/update Rust code
pub fn apply_forward_sync(
    diffs: &[SyncDiff],
    ontology: &HashMap<String, OntologyNode>,
    output_dir: &Path,
) -> Result<()> {
    for diff in diffs {
        match diff {
            SyncDiff::Added { type_name } => {
                // Generate new struct from ontology
                let node = ontology
                    .get(type_name)
                    .ok_or_else(|| ForwardSyncError::NodeNotFound(type_name.clone()))?;

                let code = generate_struct(node)?;
                write_struct_file(output_dir, type_name, &code)?;

                println!("  [+] Generated new struct: {}", type_name);
            }

            SyncDiff::Removed { type_name } => {
                // For removed types, we could delete the file or warn
                // For safety, just warn - manual intervention recommended
                println!(
                    "  [-] Type removed from ontology: {} (manual cleanup recommended)",
                    type_name
                );
            }

            SyncDiff::Modified {
                type_name,
                field_changes,
            } => {
                // Update existing struct with field changes
                let node = ontology
                    .get(type_name)
                    .ok_or_else(|| ForwardSyncError::NodeNotFound(type_name.clone()))?;

                let code = generate_struct(node)?;
                write_struct_file(output_dir, type_name, &code)?;

                println!(
                    "  [~] Updated struct: {} ({} field changes)",
                    type_name,
                    field_changes.len()
                );
                for change in field_changes {
                    match change {
                        FieldChange::Added { name, field_type } => {
                            println!("      + Added field: {}: {}", name, field_type);
                        }
                        FieldChange::Removed { name, .. } => {
                            println!("      - Removed field: {}", name);
                        }
                        FieldChange::TypeMismatch {
                            name,
                            ontology_type,
                            code_type,
                        } => {
                            println!(
                                "      ~ Changed field {}: {} -> {}",
                                name, code_type, ontology_type
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Generate a Rust struct definition from an ontology node
fn generate_struct(node: &OntologyNode) -> Result<String> {
    let struct_name =
        parse_ident(&node.name).map_err(|e| ForwardSyncError::CodeGen(node.name.clone(), e))?;

    let mut field_tokens = Vec::new();

    for field in &node.fields {
        let field_name = parse_ident(&field.name)
            .map_err(|e| ForwardSyncError::CodeGen(field.name.clone(), e))?;

        let field_type = parse_type(&field.field_type)
            .map_err(|e| ForwardSyncError::CodeGen(field.field_type.clone(), e))?;

        // Generate field with pub visibility
        field_tokens.push(quote! {
            pub #field_name: #field_type
        });
    }

    // Generate the complete struct with derives and serde attributes
    let tokens = quote! {
        /// Auto-generated struct from ontology
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct #struct_name {
            #(#field_tokens),*
        }
    };

    Ok(tokens.to_string())
}

/// Parse a string as a valid Rust identifier
fn parse_ident(s: &str) -> std::result::Result<Ident, String> {
    syn::parse_str::<Ident>(s).map_err(|e| format!("Invalid identifier '{}': {}", s, e))
}

/// Parse a type string into a TokenStream
fn parse_type(type_str: &str) -> std::result::Result<TokenStream, String> {
    // Handle common type conversions
    let normalized = normalize_type(type_str);

    syn::parse_str::<syn::Type>(&normalized)
        .map(|ty| quote! { #ty })
        .map_err(|e| format!("Invalid type '{}': {}", type_str, e))
}

/// Normalize type strings from ontology to Rust types
fn normalize_type(type_str: &str) -> String {
    match type_str {
        "string" => "String".to_string(),
        "integer" => "i64".to_string(),
        "boolean" => "bool".to_string(),
        "float" => "f64".to_string(),
        "dateTime" => "chrono::DateTime<chrono::Utc>".to_string(),
        "JsonValue" => "serde_json::Value".to_string(),
        other => other.to_string(),
    }
}

/// Write a generated struct to a file in the output directory
fn write_struct_file(output_dir: &Path, type_name: &str, code: &str) -> Result<()> {
    // Ensure output directory exists
    fs::create_dir_all(output_dir)?;

    // Convert struct name to snake_case for filename
    let filename = to_snake_case(type_name);
    let file_path = output_dir.join(format!("{}.rs", filename));

    // Add file header comment
    let content = format!(
        "//! Auto-generated from ontology - DO NOT EDIT MANUALLY\n\
         //! Generated by ggen-sync forward sync\n\
         \n\
         {}\n",
        code
    );

    fs::write(&file_path, content)?;

    Ok(())
}

/// Convert PascalCase to snake_case
/// For acronyms like "HTTPResponse", each letter gets separated: "h_t_t_p_response"
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();

    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            // Add underscore before each uppercase letter except the first
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FieldDef;

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("AgentCard"), "agent_card");
        assert_eq!(to_snake_case("TaskStatus"), "task_status");
        assert_eq!(to_snake_case("HTTPResponse"), "h_t_t_p_response");
    }

    #[test]
    fn test_normalize_type() {
        assert_eq!(normalize_type("string"), "String");
        assert_eq!(normalize_type("integer"), "i64");
        assert_eq!(normalize_type("boolean"), "bool");
        assert_eq!(normalize_type("CustomType"), "CustomType");
    }

    #[test]
    fn test_generate_simple_struct() {
        let node = OntologyNode::new(
            "TestStruct",
            vec![
                FieldDef::new("name", "String"),
                FieldDef::new("count", "i64"),
            ],
        );

        let result = generate_struct(&node);
        assert!(result.is_ok());

        let code = result.unwrap();
        assert!(code.contains("pub struct TestStruct"));
        assert!(code.contains("pub name : String"));
        assert!(code.contains("pub count : i64"));
    }
}
