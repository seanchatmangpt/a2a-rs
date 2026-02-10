//! TTL ontology parser using sophia crate
//!
//! Parses RDF/Turtle ontology files to extract entity and property definitions.
//! Focuses on the 80/20: extract what's needed for sync detection.

use crate::types::{FieldDef, OntologyNode};
use sophia_api::prelude::TripleSource;
use sophia_api::term::{SimpleTerm, Term, TermKind};
use sophia_turtle::parser::turtle::parse_str;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OntologyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("RDF parsing error: {0}")]
    Parse(String),
}

type Result<T> = std::result::Result<T, OntologyError>;

// Namespace constants
const A2A_NS: &str = "https://ggen.io/ontology/a2a/";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Parse a TTL file and extract ontology entities
pub fn parse_ttl_file(path: &Path) -> Result<HashMap<String, OntologyNode>> {
    let content = fs::read_to_string(path)?;
    parse_ttl_string(&content)
}

/// Parse TTL content from a string
pub fn parse_ttl_string(content: &str) -> Result<HashMap<String, OntologyNode>> {
    // Parse the RDF graph using simple parse_str function
    let triples: Vec<[SimpleTerm; 3]> = parse_str(content)
        .collect_triples()
        .map_err(|e| OntologyError::Parse(format!("{:?}", e)))?;

    // Build a simple in-memory index for lookups
    let mut entities = HashMap::new();
    let mut property_definitions = HashMap::new();
    let mut entity_properties: HashMap<String, Vec<String>> = HashMap::new();

    // First pass: identify entities and property definitions
    for triple in &triples {
        let subject = &triple[0];
        let predicate = &triple[1];
        let object = &triple[2];

        let pred_str = term_to_string(predicate);

        // Look for entities (classes with a2a:Entity type)
        if pred_str == RDF_TYPE {
            let obj_str = term_to_string(object);
            if obj_str == format!("{}Entity", A2A_NS) {
                let entity_name = extract_local_name(&term_to_string(subject));
                entities.insert(entity_name.clone(), OntologyNode::new(entity_name, vec![]));
            }
        }

        // Look for property linkages (a2a:hasProperty)
        if pred_str == format!("{}hasProperty", A2A_NS) {
            let entity_uri = term_to_string(subject);
            let entity_name = extract_local_name(&entity_uri);
            let property_uri = term_to_string(object);

            entity_properties
                .entry(entity_name)
                .or_default()
                .push(property_uri);
        }

        // Look for property definitions (a2a:Property type)
        if pred_str == RDF_TYPE {
            let obj_str = term_to_string(object);
            if obj_str == format!("{}Property", A2A_NS) {
                let property_uri = term_to_string(subject);
                property_definitions.insert(property_uri, PropertyDef::default());
            }
        }
    }

    // Second pass: extract property attributes
    for triple in &triples {
        let subject = &triple[0];
        let predicate = &triple[1];
        let object = &triple[2];

        let subj_str = term_to_string(subject);
        let pred_str = term_to_string(predicate);

        if let Some(prop_def) = property_definitions.get_mut(&subj_str) {
            match pred_str.as_str() {
                s if s == format!("{}name", A2A_NS) => {
                    prop_def.name = extract_literal_value(object);
                }
                s if s == format!("{}type", A2A_NS) => {
                    prop_def.field_type = extract_literal_value(object);
                }
                s if s == format!("{}required", A2A_NS) => {
                    prop_def.required = extract_literal_value(object) == "true";
                }
                s if s == format!("{}isArray", A2A_NS) => {
                    prop_def.is_array = extract_literal_value(object) == "true";
                }
                s if s == format!("{}refEntity", A2A_NS) => {
                    prop_def.ref_entity = Some(extract_literal_value(object));
                }
                _ => {}
            }
        }
    }

    // Third pass: build OntologyNode with fields
    for (entity_name, node) in entities.iter_mut() {
        if let Some(prop_uris) = entity_properties.get(entity_name) {
            for prop_uri in prop_uris {
                if let Some(prop_def) = property_definitions.get(prop_uri) {
                    let field_type = determine_field_type(prop_def);
                    if !prop_def.name.is_empty() {
                        node.fields
                            .push(FieldDef::new(prop_def.name.clone(), field_type));
                    }
                }
            }
        }
    }

    Ok(entities)
}

/// Parse multiple TTL files in a directory
pub fn parse_ttl_directory(dir: &Path) -> Result<HashMap<String, OntologyNode>> {
    let mut all_entities = HashMap::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("ttl") {
            let entities = parse_ttl_file(&path)?;
            all_entities.extend(entities);
        }
    }

    Ok(all_entities)
}

// Helper types

#[derive(Debug, Default, Clone)]
struct PropertyDef {
    name: String,
    field_type: String,
    required: bool,
    is_array: bool,
    ref_entity: Option<String>,
}

// Helper functions

fn term_to_string(term: &SimpleTerm) -> String {
    match term.kind() {
        TermKind::Iri => term.iri().unwrap().to_string(),
        TermKind::Literal => extract_literal_from_term(term),
        TermKind::BlankNode => format!("_:{:?}", term.bnode_id().unwrap()),
        _ => String::new(),
    }
}

fn extract_literal_from_term(term: &SimpleTerm) -> String {
    if let Some(lit) = term.lexical_form() {
        lit.to_string()
    } else {
        String::new()
    }
}

fn extract_literal_value(term: &SimpleTerm) -> String {
    extract_literal_from_term(term)
}

fn extract_local_name(uri: &str) -> String {
    uri.split(['/', '#']).last().unwrap_or(uri).to_string()
}

fn determine_field_type(prop_def: &PropertyDef) -> String {
    let base_type = if prop_def.field_type == "reference" {
        prop_def.ref_entity.as_deref().unwrap_or("Unknown")
    } else {
        match prop_def.field_type.as_str() {
            "string" => "String",
            "boolean" => "bool",
            "integer" => "i64",
            "object" => "serde_json::Value",
            _ => &prop_def.field_type,
        }
    };

    if prop_def.is_array {
        format!("Vec<{}>", base_type)
    } else if !prop_def.required {
        format!("Option<{}>", base_type)
    } else {
        base_type.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_entity() {
        let ttl = r#"
@prefix a2a: <https://ggen.io/ontology/a2a/> .
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .

a2a:TestEntity a a2a:Entity ;
    a2a:name "TestEntity" ;
    a2a:hasProperty a2a:TestEntity_name .

a2a:TestEntity_name a a2a:Property ;
    a2a:name "name" ;
    a2a:type "string" ;
    a2a:required true .
"#;

        let result = parse_ttl_string(ttl).unwrap();
        assert!(result.contains_key("TestEntity"));

        let entity = &result["TestEntity"];
        assert_eq!(entity.name, "TestEntity");
        assert_eq!(entity.fields.len(), 1);
        assert_eq!(entity.fields[0].name, "name");
        assert_eq!(entity.fields[0].field_type, "String");
    }
}

    #[test]
    #[ignore] // This is an integration test that requires the ontology files
    fn test_parse_real_agent_ontology() {
        use std::path::Path;
        
        let ontology_path = Path::new("/home/user/a2a-rs/ggen/ontology/a2a-agent.ttl");
        
        if !ontology_path.exists() {
            println!("Skipping test - ontology file not found");
            return;
        }
        
        let result = parse_ttl_file(ontology_path);
        match result {
            Ok(entities) => {
                println!("Parsed {} entities from a2a-agent.ttl", entities.len());
                for (name, entity) in entities.iter() {
                    println!("  Entity: {} ({} fields)", name, entity.fields.len());
                }
                assert!(!entities.is_empty(), "Should parse at least one entity");
            }
            Err(e) => {
                panic!("Failed to parse ontology: {}", e);
            }
        }
    }
