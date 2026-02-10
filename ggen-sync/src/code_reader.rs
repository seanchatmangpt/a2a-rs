//! Rust code parser for extracting struct and enum definitions
//!
//! Uses the `syn` crate to parse .rs files and extract type information
//! for comparison with ontology definitions.

use crate::types::{CodeNode, FieldDef};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use syn::{Attribute, Field, Fields, Item, Type};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodeReaderError {
    #[error("Failed to read file: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse Rust code: {0}")]
    Parse(#[from] syn::Error),
}

pub type Result<T> = std::result::Result<T, CodeReaderError>;

/// Parse a single Rust file and extract struct/enum definitions
pub fn parse_file(path: &Path) -> Result<HashMap<String, CodeNode>> {
    let content = fs::read_to_string(path)?;
    parse_code(&content)
}

/// Parse Rust code from a string and extract struct/enum definitions
pub fn parse_code(code: &str) -> Result<HashMap<String, CodeNode>> {
    let syntax_tree = syn::parse_file(code)?;
    let mut nodes = HashMap::new();

    for item in syntax_tree.items {
        match item {
            Item::Struct(item_struct) => {
                let name = item_struct.ident.to_string();
                let fields = extract_struct_fields(&item_struct.fields);
                nodes.insert(name.clone(), CodeNode::new(name, fields));
            }
            Item::Enum(item_enum) => {
                let name = item_enum.ident.to_string();
                let fields = extract_enum_variants(&item_enum);
                nodes.insert(name.clone(), CodeNode::new(name, fields));
            }
            // Skip other items (impl blocks, functions, etc.) per 80/20 approach
            _ => {}
        }
    }

    Ok(nodes)
}

/// Extract fields from a struct definition
fn extract_struct_fields(fields: &Fields) -> Vec<FieldDef> {
    match fields {
        Fields::Named(fields_named) => fields_named
            .named
            .iter()
            .filter_map(|field| {
                let name = field.ident.as_ref()?.to_string();
                let field_name = extract_serde_rename(field).unwrap_or(name);
                let field_type = type_to_string(&field.ty);
                Some(FieldDef::new(field_name, field_type))
            })
            .collect(),
        Fields::Unnamed(fields_unnamed) => fields_unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(idx, field)| {
                let name = format!("_{}", idx);
                let field_type = type_to_string(&field.ty);
                FieldDef::new(name, field_type)
            })
            .collect(),
        Fields::Unit => Vec::new(),
    }
}

/// Extract variants from an enum definition
fn extract_enum_variants(item_enum: &syn::ItemEnum) -> Vec<FieldDef> {
    item_enum
        .variants
        .iter()
        .map(|variant| {
            let name = variant.ident.to_string();
            let field_type = match &variant.fields {
                Fields::Unit => "()".to_string(),
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                    type_to_string(&fields.unnamed[0].ty)
                }
                Fields::Unnamed(fields) => {
                    let types: Vec<String> = fields
                        .unnamed
                        .iter()
                        .map(|f| type_to_string(&f.ty))
                        .collect();
                    format!("({})", types.join(", "))
                }
                Fields::Named(fields) => {
                    let field_strs: Vec<String> = fields
                        .named
                        .iter()
                        .filter_map(|f| {
                            let name = f.ident.as_ref()?.to_string();
                            let ty = type_to_string(&f.ty);
                            Some(format!("{}: {}", name, ty))
                        })
                        .collect();
                    format!("{{ {} }}", field_strs.join(", "))
                }
            };
            FieldDef::new(name, field_type)
        })
        .collect()
}

/// Convert a syn Type to a string representation
fn type_to_string(ty: &Type) -> String {
    match ty {
        Type::Path(type_path) => {
            let segments: Vec<String> = type_path
                .path
                .segments
                .iter()
                .map(|seg| {
                    let ident = seg.ident.to_string();
                    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                        let args_str: Vec<String> = args
                            .args
                            .iter()
                            .filter_map(|arg| match arg {
                                syn::GenericArgument::Type(ty) => Some(type_to_string(ty)),
                                syn::GenericArgument::Lifetime(lt) => {
                                    Some(format!("'{}", lt.ident))
                                }
                                _ => None,
                            })
                            .collect();
                        if args_str.is_empty() {
                            ident
                        } else {
                            format!("{}<{}>", ident, args_str.join(", "))
                        }
                    } else {
                        ident
                    }
                })
                .collect();
            segments.join("::")
        }
        Type::Reference(type_ref) => {
            let mut s = "&".to_string();
            if let Some(lifetime) = &type_ref.lifetime {
                s.push('\'');
                s.push_str(&lifetime.ident.to_string());
                s.push(' ');
            }
            if type_ref.mutability.is_some() {
                s.push_str("mut ");
            }
            s.push_str(&type_to_string(&type_ref.elem));
            s
        }
        Type::Tuple(type_tuple) => {
            let elems: Vec<String> = type_tuple.elems.iter().map(type_to_string).collect();
            if elems.is_empty() {
                "()".to_string()
            } else {
                format!("({})", elems.join(", "))
            }
        }
        Type::Array(type_array) => {
            format!(
                "[{}; {}]",
                type_to_string(&type_array.elem),
                quote::quote!(#type_array.len).to_string()
            )
        }
        Type::Slice(type_slice) => {
            format!("[{}]", type_to_string(&type_slice.elem))
        }
        Type::Ptr(type_ptr) => {
            let mut s = "*".to_string();
            if type_ptr.mutability.is_some() {
                s.push_str("mut ");
            } else {
                s.push_str("const ");
            }
            s.push_str(&type_to_string(&type_ptr.elem));
            s
        }
        _ => {
            // For other complex types, use quote to get a string representation
            quote::quote!(#ty).to_string()
        }
    }
}

/// Extract serde rename attribute from a field if present
fn extract_serde_rename(field: &Field) -> Option<String> {
    for attr in &field.attrs {
        if let Some(rename) = parse_serde_rename(attr) {
            return Some(rename);
        }
    }
    None
}

/// Parse a serde attribute to extract rename value
fn parse_serde_rename(attr: &Attribute) -> Option<String> {
    // Check if this is a serde attribute
    if !attr.path().is_ident("serde") {
        return None;
    }

    // Parse the attribute tokens to find rename = "..."
    if let syn::Meta::List(meta_list) = &attr.meta {
        // Parse the nested meta items
        let nested = meta_list.tokens.to_string();

        // Simple string parsing for rename = "value"
        // This handles common cases like: #[serde(rename = "camelCase")]
        if let Some(rename_idx) = nested.find("rename") {
            let after_rename = &nested[rename_idx..];
            if let Some(eq_idx) = after_rename.find('=') {
                let after_eq = &after_rename[eq_idx + 1..].trim_start();
                if after_eq.starts_with('"') {
                    let end_quote = after_eq[1..].find('"')?;
                    return Some(after_eq[1..=end_quote].to_string());
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_struct() {
        let code = r#"
            pub struct Person {
                pub name: String,
                pub age: u32,
            }
        "#;

        let nodes = parse_code(code).unwrap();
        assert_eq!(nodes.len(), 1);

        let person = nodes.get("Person").unwrap();
        assert_eq!(person.name, "Person");
        assert_eq!(person.fields.len(), 2);

        let name_field = person.field("name").unwrap();
        assert_eq!(name_field.field_type, "String");

        let age_field = person.field("age").unwrap();
        assert_eq!(age_field.field_type, "u32");
    }

    #[test]
    fn test_parse_struct_with_generic() {
        let code = r#"
            pub struct Container<T> {
                pub value: Vec<T>,
                pub optional: Option<T>,
            }
        "#;

        let nodes = parse_code(code).unwrap();
        let container = nodes.get("Container").unwrap();

        let value_field = container.field("value").unwrap();
        assert_eq!(value_field.field_type, "Vec<T>");

        let optional_field = container.field("optional").unwrap();
        assert_eq!(optional_field.field_type, "Option<T>");
    }

    #[test]
    fn test_parse_enum() {
        let code = r#"
            pub enum Status {
                Active,
                Inactive,
                Pending(String),
            }
        "#;

        let nodes = parse_code(code).unwrap();
        let status = nodes.get("Status").unwrap();
        assert_eq!(status.fields.len(), 3);

        let active = status.field("Active").unwrap();
        assert_eq!(active.field_type, "()");

        let pending = status.field("Pending").unwrap();
        assert_eq!(pending.field_type, "String");
    }

    #[test]
    fn test_parse_serde_rename() {
        let code = r#"
            pub struct Message {
                #[serde(rename = "messageId")]
                pub message_id: String,
                pub content: String,
            }
        "#;

        let nodes = parse_code(code).unwrap();
        let message = nodes.get("Message").unwrap();

        let id_field = message.field("messageId").unwrap();
        assert_eq!(id_field.field_type, "String");
    }

    #[test]
    fn test_parse_tuple_struct() {
        let code = r#"
            pub struct Point(pub f64, pub f64);
        "#;

        let nodes = parse_code(code).unwrap();
        let point = nodes.get("Point").unwrap();
        assert_eq!(point.fields.len(), 2);

        let x = point.field("_0").unwrap();
        assert_eq!(x.field_type, "f64");
    }

    #[test]
    fn test_skip_impl_blocks() {
        let code = r#"
            pub struct Person {
                pub name: String,
            }

            impl Person {
                pub fn new(name: String) -> Self {
                    Self { name }
                }
            }
        "#;

        let nodes = parse_code(code).unwrap();
        // Should only have the struct, impl block is skipped
        assert_eq!(nodes.len(), 1);
        assert!(nodes.contains_key("Person"));
    }

    #[test]
    fn test_complex_types() {
        let code = r#"
            pub struct Complex {
                pub vec: Vec<String>,
                pub map: HashMap<String, i32>,
                pub tuple: (u32, String),
                pub reference: &'static str,
            }
        "#;

        let nodes = parse_code(code).unwrap();
        let complex = nodes.get("Complex").unwrap();

        let vec_field = complex.field("vec").unwrap();
        assert_eq!(vec_field.field_type, "Vec<String>");

        let map_field = complex.field("map").unwrap();
        assert_eq!(map_field.field_type, "HashMap<String, i32>");

        let tuple_field = complex.field("tuple").unwrap();
        assert_eq!(tuple_field.field_type, "(u32, String)");

        let ref_field = complex.field("reference").unwrap();
        assert_eq!(ref_field.field_type, "&'static str");
    }
}
