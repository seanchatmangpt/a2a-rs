//! Core types for ontology-code comparison

use std::collections::HashMap;

/// A field definition with name and type
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDef {
    pub name: String,
    pub field_type: String,
}

impl FieldDef {
    pub fn new(name: impl Into<String>, field_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            field_type: field_type.into(),
        }
    }
}

/// A type defined in the RDF ontology
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OntologyNode {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

impl OntologyNode {
    pub fn new(name: impl Into<String>, fields: Vec<FieldDef>) -> Self {
        Self {
            name: name.into(),
            fields,
        }
    }

    /// Get field by name
    pub fn field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Get fields as a HashMap for easy lookup
    pub fn fields_map(&self) -> HashMap<String, String> {
        self.fields
            .iter()
            .map(|f| (f.name.clone(), f.field_type.clone()))
            .collect()
    }
}

/// A type found in generated Rust code
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeNode {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

impl CodeNode {
    pub fn new(name: impl Into<String>, fields: Vec<FieldDef>) -> Self {
        Self {
            name: name.into(),
            fields,
        }
    }

    /// Get field by name
    pub fn field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Get fields as a HashMap for easy lookup
    pub fn fields_map(&self) -> HashMap<String, String> {
        self.fields
            .iter()
            .map(|f| (f.name.clone(), f.field_type.clone()))
            .collect()
    }
}

/// A change to a field
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldChange {
    /// Field exists in ontology but not in code
    Added { name: String, field_type: String },
    /// Field exists in code but not in ontology
    Removed { name: String, field_type: String },
    /// Field type differs between ontology and code
    TypeMismatch {
        name: String,
        ontology_type: String,
        code_type: String,
    },
}

/// A detected difference between ontology and code
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncDiff {
    /// Type exists in ontology but not in generated code
    Added { type_name: String },
    /// Type exists in generated code but not in ontology
    Removed { type_name: String },
    /// Type exists in both but has field differences
    Modified {
        type_name: String,
        field_changes: Vec<FieldChange>,
    },
}
