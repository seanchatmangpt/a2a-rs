//! RDF triple types for CONSTRUCT operations.

use serde::{Deserialize, Serialize};

/// Represents an RDF triple (subject, predicate, object).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Triple {
    /// The subject of the triple (IRI or blank node).
    pub subject: String,
    /// The predicate of the triple (IRI).
    pub predicate: String,
    /// The object of the triple (IRI, blank node, or literal).
    pub object: String,
}

impl Triple {
    /// Creates a new RDF triple.
    pub fn new(
        subject: impl Into<String>,
        predicate: impl Into<String>,
        object: impl Into<String>,
    ) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
        }
    }
}

/// Represents a triple pattern for CONSTRUCT queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriplePattern {
    /// Subject pattern (variable or constant).
    pub subject: String,
    /// Predicate pattern (variable or constant).
    pub predicate: String,
    /// Object pattern (variable or constant).
    pub object: String,
}

impl TriplePattern {
    /// Creates a new triple pattern.
    pub fn new(
        subject: impl Into<String>,
        predicate: impl Into<String>,
        object: impl Into<String>,
    ) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
        }
    }
}
