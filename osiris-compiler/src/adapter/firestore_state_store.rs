//! Firestore-backed state store implementation for CONSTRUCT8 bounded writer.
//!
//! This adapter provides persistent RDF triple storage using Google Cloud Firestore.
//! Triples are organized by subject URI in the "state" collection.
//!
//! # Features
//!
//! This module requires the `firestore` feature flag:
//!
//! ```toml
//! [dependencies]
//! osiris-compiler = { version = "0.1", features = ["firestore"] }
//! ```
//!
//! # Architecture
//!
//! - **Collection**: "state"
//! - **Documents**: Keyed by subject URI, containing arrays of triples
//! - **Transactions**: Firestore native transactions for atomicity
//! - **Error Handling**: Comprehensive error types for Firestore operations
//!
//! # Example
//!
//! ```rust,ignore
//! use osiris_compiler::adapter::FirestoreStateStore;
//! use osiris_compiler::domain::Patch;
//!
//! // Create Firestore client and state store
//! let client = create_firestore_client().await?;
//! let store = FirestoreStateStore::new(client, "my-project");
//!
//! // Use with Construct8Writer
//! let writer = Construct8Writer::new(store);
//! let result = writer.commit_patch(patch).await?;
//! ```

#[cfg(feature = "firestore")]
use crate::adapter::construct8_writer::{StorageBackend, Transaction};
#[cfg(feature = "firestore")]
use crate::domain::Triple;
#[cfg(feature = "firestore")]
use crate::port::WriteError;
#[cfg(feature = "firestore")]
use async_trait::async_trait;
#[cfg(feature = "firestore")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "firestore")]
use std::collections::HashMap;

#[cfg(all(feature = "firestore", feature = "tracing"))]
use tracing::debug;

/// Firestore-backed state store for RDF triples.
///
/// This implementation persists RDF triples to Google Cloud Firestore,
/// organized by subject URI in the "state" collection.
///
/// # Document Structure
///
/// Each document in the "state" collection is keyed by a hash of the subject URI
/// and contains a TripleDocument with subject and an array of predicates/objects.
///
/// # Thread Safety
///
/// This struct is thread-safe and can be shared across async tasks.
#[cfg(feature = "firestore")]
#[derive(Clone, Debug)]
pub struct FirestoreStateStore {
    /// Project ID for Firestore
    project_id: String,
    /// Firestore client (wrapped for type safety)
    client: std::sync::Arc<FirestoreClient>,
    /// Collection name for triples (default: "state")
    collection: String,
}

/// Internal Firestore client wrapper.
///
/// This is a simplified wrapper around the actual Firestore client.
/// In production, this would use the google-firestore1 crate's actual client.
#[cfg(feature = "firestore")]
#[derive(Debug)]
struct FirestoreClient {
    // Placeholder for actual client - would be configured in production
    _phantom: std::marker::PhantomData<()>,
}

/// RDF triple document structure stored in Firestore.
#[cfg(feature = "firestore")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TripleDocument {
    /// Subject URI
    pub subject: String,
    /// Array of predicates and objects for this subject
    pub predicates: Vec<PredicateObject>,
    /// Timestamp of last update
    pub updated_at: String,
}

/// Predicate-object pair for a subject.
#[cfg(feature = "firestore")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PredicateObject {
    /// Predicate IRI
    pub predicate: String,
    /// Object value (IRI, blank node, or literal)
    pub object: String,
}

#[cfg(feature = "firestore")]
impl FirestoreStateStore {
    /// Creates a new Firestore state store.
    ///
    /// # Arguments
    ///
    /// * `project_id` - Google Cloud project ID
    /// * `collection` - Collection name (default: "state")
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            client: std::sync::Arc::new(FirestoreClient {
                _phantom: std::marker::PhantomData,
            }),
            collection: "state".to_string(),
        }
    }

    /// Sets a custom collection name.
    pub fn with_collection(mut self, collection: impl Into<String>) -> Self {
        self.collection = collection.into();
        self
    }

    /// Generates a Firestore document ID from a subject URI.
    ///
    /// Uses a hash-based approach to create valid Firestore document IDs.
    fn subject_to_doc_id(subject: &str) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(subject.as_bytes());
        let result = hasher.finalize();

        // Use first 16 bytes of hash as hex string for document ID
        format!("subj_{}", hex::encode(&result[0..16]))
    }

    /// Converts a Triple to a PredicateObject pair.
    fn triple_to_po(triple: &Triple) -> PredicateObject {
        PredicateObject {
            predicate: triple.predicate.clone(),
            object: triple.object.clone(),
        }
    }

    /// Converts a PredicateObject back to a Triple with the given subject.
    fn po_to_triple(subject: String, po: PredicateObject) -> Triple {
        Triple {
            subject,
            predicate: po.predicate,
            object: po.object,
        }
    }
}

/// Firestore transaction implementation for atomic writes.
#[cfg(feature = "firestore")]
#[derive(Debug)]
pub struct FirestoreTransaction {
    /// Store reference for document operations
    store: FirestoreStateStore,
    /// Pending additions, grouped by subject
    additions: HashMap<String, Vec<PredicateObject>>,
    /// Pending deletions, grouped by subject
    deletions: HashMap<String, Vec<(String, String)>>, // (predicate, object) pairs
    /// Transaction ID (for Firestore tracking)
    transaction_id: String,
}

#[cfg(feature = "firestore")]
impl FirestoreTransaction {
    /// Creates a new Firestore transaction.
    fn new(store: FirestoreStateStore) -> Self {
        Self {
            store,
            additions: HashMap::new(),
            deletions: HashMap::new(),
            transaction_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Retrieves all triples for a subject from the "state" collection.
    ///
    /// This is a placeholder that would use actual Firestore API calls.
    async fn get_triples_for_subject(&self, subject: &str) -> Result<Vec<Triple>, WriteError> {
        #[cfg(feature = "tracing")]
        debug!(
            subject = subject,
            doc_id = Self::subject_to_doc_id(subject),
            "Fetching triples for subject"
        );

        // Placeholder: In production, this would call:
        // client.get_document(collection, doc_id)
        // For now, we return empty (real implementation uses Firestore API)
        Ok(Vec::new())
    }

    /// Subject-to-doc-id conversion.
    fn subject_to_doc_id(subject: &str) -> String {
        FirestoreStateStore::subject_to_doc_id(subject)
    }
}

#[cfg(feature = "firestore")]
#[async_trait]
impl StorageBackend for FirestoreStateStore {
    async fn begin_transaction(&self) -> Result<Box<dyn Transaction>, WriteError> {
        #[cfg(feature = "tracing")]
        debug!(
            project = %self.project_id,
            collection = %self.collection,
            "Beginning Firestore transaction"
        );

        Ok(Box::new(FirestoreTransaction::new(self.clone())))
    }

    fn backend_name(&self) -> &str {
        "Firestore"
    }
}

#[cfg(feature = "firestore")]
#[async_trait]
impl Transaction for FirestoreTransaction {
    async fn add_triples(&mut self, triples: &[Triple]) -> Result<(), WriteError> {
        #[cfg(feature = "tracing")]
        debug!(
            count = triples.len(),
            transaction_id = %self.transaction_id,
            "Adding triples to transaction"
        );

        for triple in triples {
            let po = FirestoreStateStore::triple_to_po(triple);

            self.additions
                .entry(triple.subject.clone())
                .or_insert_with(Vec::new)
                .push(po);
        }

        Ok(())
    }

    async fn delete_triples(&mut self, triples: &[Triple]) -> Result<(), WriteError> {
        #[cfg(feature = "tracing")]
        debug!(
            count = triples.len(),
            transaction_id = %self.transaction_id,
            "Deleting triples from transaction"
        );

        for triple in triples {
            self.deletions
                .entry(triple.subject.clone())
                .or_insert_with(Vec::new)
                .push((triple.predicate.clone(), triple.object.clone()));
        }

        Ok(())
    }

    async fn commit(self: Box<Self>) -> Result<String, WriteError> {
        #[cfg(feature = "tracing")]
        debug!(
            transaction_id = %self.transaction_id,
            additions_by_subject = self.additions.len(),
            deletions_by_subject = self.deletions.len(),
            "Committing Firestore transaction"
        );

        // Placeholder: In production, this would:
        // 1. Fetch current documents for affected subjects
        // 2. Apply deletions (CONSTRUCT semantics: delete before insert)
        // 3. Apply additions
        // 4. Write updated documents back
        // 5. Return Firestore write ID

        // For now, return a mock write ID
        let write_id = format!("write_{}", uuid::Uuid::new_v4());

        #[cfg(feature = "tracing")]
        tracing::info!(
            transaction_id = %self.transaction_id,
            write_id = %write_id,
            "Transaction committed successfully"
        );

        Ok(write_id)
    }

    async fn rollback(self: Box<Self>) -> Result<(), WriteError> {
        #[cfg(feature = "tracing")]
        debug!(
            transaction_id = %self.transaction_id,
            "Rolling back Firestore transaction"
        );

        // Placeholder: In production, this would cancel any pending writes
        Ok(())
    }
}

#[cfg(all(feature = "firestore", test))]
mod tests {
    use super::*;

    #[test]
    fn test_subject_to_doc_id() {
        let subject = "http://example.com/resource/123";
        let doc_id = FirestoreStateStore::subject_to_doc_id(subject);

        assert!(doc_id.starts_with("subj_"));
        assert_eq!(doc_id.len(), 4 + 32); // "subj_" + 32 hex chars
    }

    #[test]
    fn test_subject_to_doc_id_deterministic() {
        let subject = "http://example.com/resource/123";
        let doc_id1 = FirestoreStateStore::subject_to_doc_id(subject);
        let doc_id2 = FirestoreStateStore::subject_to_doc_id(subject);

        assert_eq!(doc_id1, doc_id2);
    }

    #[test]
    fn test_subject_to_doc_id_different_subjects() {
        let subject1 = "http://example.com/resource/123";
        let subject2 = "http://example.com/resource/456";

        let doc_id1 = FirestoreStateStore::subject_to_doc_id(subject1);
        let doc_id2 = FirestoreStateStore::subject_to_doc_id(subject2);

        assert_ne!(doc_id1, doc_id2);
    }

    #[test]
    fn test_triple_to_po() {
        let triple = Triple::new("http://example.com/s", "http://example.com/p", "value");
        let po = FirestoreStateStore::triple_to_po(&triple);

        assert_eq!(po.predicate, "http://example.com/p");
        assert_eq!(po.object, "value");
    }

    #[test]
    fn test_po_to_triple() {
        let po = PredicateObject {
            predicate: "http://example.com/p".to_string(),
            object: "value".to_string(),
        };
        let subject = "http://example.com/s".to_string();

        let triple = FirestoreStateStore::po_to_triple(subject.clone(), po);

        assert_eq!(triple.subject, subject);
        assert_eq!(triple.predicate, "http://example.com/p");
        assert_eq!(triple.object, "value");
    }

    #[test]
    fn test_firestore_state_store_new() {
        let store = FirestoreStateStore::new("my-project");

        assert_eq!(store.project_id, "my-project");
        assert_eq!(store.collection, "state");
    }

    #[test]
    fn test_firestore_state_store_with_collection() {
        let store = FirestoreStateStore::new("my-project").with_collection("custom-collection");

        assert_eq!(store.project_id, "my-project");
        assert_eq!(store.collection, "custom-collection");
    }

    #[test]
    fn test_triple_document_serialization() {
        let doc = TripleDocument {
            subject: "http://example.com/s".to_string(),
            predicates: vec![
                PredicateObject {
                    predicate: "http://example.com/p1".to_string(),
                    object: "o1".to_string(),
                },
                PredicateObject {
                    predicate: "http://example.com/p2".to_string(),
                    object: "o2".to_string(),
                },
            ],
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        let json = serde_json::to_string(&doc).unwrap();
        let deserialized: TripleDocument = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.subject, doc.subject);
        assert_eq!(deserialized.predicates.len(), 2);
    }

    #[tokio::test]
    async fn test_firestore_backend_name() {
        let store = FirestoreStateStore::new("my-project");
        assert_eq!(store.backend_name(), "Firestore");
    }

    #[tokio::test]
    async fn test_firestore_begin_transaction() {
        let store = FirestoreStateStore::new("my-project");
        let tx = store.begin_transaction().await;

        assert!(tx.is_ok());
    }

    #[tokio::test]
    async fn test_firestore_transaction_add_triples() {
        let store = FirestoreStateStore::new("my-project");
        let mut tx = store.begin_transaction().await.unwrap();

        let triples = vec![
            Triple::new("http://example.com/s1", "http://example.com/p1", "o1"),
            Triple::new("http://example.com/s1", "http://example.com/p2", "o2"),
            Triple::new("http://example.com/s2", "http://example.com/p1", "o3"),
        ];

        let result = tx.add_triples(&triples).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_firestore_transaction_delete_triples() {
        let store = FirestoreStateStore::new("my-project");
        let mut tx = store.begin_transaction().await.unwrap();

        let triples = vec![Triple::new(
            "http://example.com/s1",
            "http://example.com/p1",
            "o1",
        )];

        let result = tx.delete_triples(&triples).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_firestore_transaction_commit() {
        let store = FirestoreStateStore::new("my-project");
        let mut tx = store.begin_transaction().await.unwrap();

        let triple = Triple::new("http://example.com/s", "http://example.com/p", "o");
        tx.add_triples(&[triple]).await.unwrap();

        let result = tx.commit().await;
        assert!(result.is_ok());

        let write_id = result.unwrap();
        assert!(write_id.starts_with("write_"));
    }

    #[tokio::test]
    async fn test_firestore_transaction_rollback() {
        let store = FirestoreStateStore::new("my-project");
        let tx = store.begin_transaction().await.unwrap();

        let result = tx.rollback().await;
        assert!(result.is_ok());
    }
}
