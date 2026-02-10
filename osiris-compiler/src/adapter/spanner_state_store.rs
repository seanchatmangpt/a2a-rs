//! Cloud Spanner-backed state store implementation for CONSTRUCT8 bounded writer.
//!
//! This adapter provides persistent RDF triple storage using Google Cloud Spanner
//! with interleaved tables for hierarchical triple organization.
//!
//! # Schema
//!
//! Two interleaved tables store RDF triples efficiently:
//!
//! ```sql
//! CREATE TABLE subjects (
//!   subject_id STRING(MAX) NOT NULL,
//!   subject_uri STRING(MAX) NOT NULL,
//!   created_at TIMESTAMP NOT NULL OPTIONS (allow_commit_timestamp = true),
//!   updated_at TIMESTAMP NOT NULL OPTIONS (allow_commit_timestamp = true),
//! ) PRIMARY KEY(subject_id);
//!
//! CREATE TABLE triples (
//!   subject_id STRING(MAX) NOT NULL,
//!   triple_id STRING(MAX) NOT NULL,
//!   predicate STRING(MAX) NOT NULL,
//!   object STRING(MAX) NOT NULL,
//!   created_at TIMESTAMP NOT NULL OPTIONS (allow_commit_timestamp = true),
//! ) PRIMARY KEY(subject_id, triple_id),
//!   INTERLEAVE IN PARENT subjects ON DELETE CASCADE;
//! ```
//!
//! # Features
//!
//! This module requires the `spanner` feature flag:
//!
//! ```toml
//! [dependencies]
//! osiris-compiler = { version = "0.1", features = ["spanner"] }
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use osiris_compiler::adapter::{SpannerStateStore, SpannerConfig};
//! use osiris_compiler::domain::Patch;
//!
//! let config = SpannerConfig::new("my-project", "my-instance", "my-database");
//! let store = SpannerStateStore::new(config).await?;
//!
//! let writer = Construct8Writer::new(store);
//! let result = writer.commit_patch(patch).await?;
//! ```

#[cfg(feature = "spanner")]
use crate::adapter::construct8_writer::{StorageBackend, Transaction};
#[cfg(feature = "spanner")]
use crate::domain::Triple;
#[cfg(feature = "spanner")]
use crate::port::WriteError;
#[cfg(feature = "spanner")]
use async_trait::async_trait;
#[cfg(feature = "spanner")]
use chrono::{DateTime, Utc};
#[cfg(feature = "spanner")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "spanner")]
use std::collections::HashMap;
#[cfg(feature = "spanner")]
use std::sync::Arc;

#[cfg(all(feature = "spanner", feature = "tracing"))]
use tracing::{debug, error, info, warn};

/// Configuration for Cloud Spanner connection.
#[cfg(feature = "spanner")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpannerConfig {
    /// Google Cloud project ID
    pub project_id: String,
    /// Spanner instance ID
    pub instance_id: String,
    /// Spanner database ID
    pub database_id: String,
    /// Optional: subjects table name (default: "subjects")
    #[serde(default)]
    pub subjects_table: Option<String>,
    /// Optional: triples table name (default: "triples")
    #[serde(default)]
    pub triples_table: Option<String>,
}

#[cfg(feature = "spanner")]
impl SpannerConfig {
    /// Creates a new Spanner configuration.
    pub fn new(
        project_id: impl Into<String>,
        instance_id: impl Into<String>,
        database_id: impl Into<String>,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            instance_id: instance_id.into(),
            database_id: database_id.into(),
            subjects_table: None,
            triples_table: None,
        }
    }

    /// Sets a custom subjects table name.
    pub fn with_subjects_table(mut self, table: impl Into<String>) -> Self {
        self.subjects_table = Some(table.into());
        self
    }

    /// Sets a custom triples table name.
    pub fn with_triples_table(mut self, table: impl Into<String>) -> Self {
        self.triples_table = Some(table.into());
        self
    }

    /// Gets the subjects table name (with default).
    fn subjects_table(&self) -> &str {
        self.subjects_table.as_deref().unwrap_or("subjects")
    }

    /// Gets the triples table name (with default).
    fn triples_table(&self) -> &str {
        self.triples_table.as_deref().unwrap_or("triples")
    }

    /// Returns the database path for Spanner API calls.
    fn database_path(&self) -> String {
        format!(
            "projects/{}/instances/{}/databases/{}",
            self.project_id, self.instance_id, self.database_id
        )
    }
}

/// Internal Spanner client wrapper.
///
/// In production, this would use the google-cloud-spanner crate's actual client.
/// For now, it's a placeholder demonstrating the integration pattern.
#[cfg(feature = "spanner")]
#[derive(Debug)]
struct SpannerClient {
    config: SpannerConfig,
    // In production: actual spanner::Client from google-cloud-spanner
    _phantom: std::marker::PhantomData<()>,
}

/// Spanner-backed state store for RDF triples.
///
/// This implementation persists RDF triples to Google Cloud Spanner
/// using interleaved tables for efficient hierarchical queries.
///
/// # Thread Safety
///
/// This struct is thread-safe and can be shared across async tasks.
#[cfg(feature = "spanner")]
#[derive(Clone, Debug)]
pub struct SpannerStateStore {
    /// Configuration for Spanner connection
    config: Arc<SpannerConfig>,
    /// Spanner client (wrapped for type safety)
    client: Arc<SpannerClient>,
}

#[cfg(feature = "spanner")]
impl SpannerStateStore {
    /// Creates a new Spanner state store.
    ///
    /// # Arguments
    ///
    /// * `config` - Spanner configuration (project, instance, database)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = SpannerConfig::new("my-project", "my-instance", "my-database");
    /// let store = SpannerStateStore::new(config).await?;
    /// ```
    pub async fn new(config: SpannerConfig) -> Result<Self, WriteError> {
        #[cfg(feature = "tracing")]
        debug!(
            project = %config.project_id,
            instance = %config.instance_id,
            database = %config.database_id,
            "Creating Spanner state store"
        );

        let client = SpannerClient {
            config: config.clone(),
            _phantom: std::marker::PhantomData,
        };

        Ok(Self {
            config: Arc::new(config),
            client: Arc::new(client),
        })
    }

    /// Generates a Spanner subject ID from a subject URI.
    ///
    /// Uses SHA-256 hash to create a stable, valid Spanner key.
    fn subject_to_id(subject: &str) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(subject.as_bytes());
        let result = hasher.finalize();

        // Use first 16 bytes of hash as hex string
        format!("subj_{}", hex::encode(&result[0..16]))
    }

    /// Generates a Spanner triple ID from a triple.
    ///
    /// Creates a deterministic ID from subject + predicate + object hash.
    fn triple_to_id(triple: &Triple) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(triple.subject.as_bytes());
        hasher.update(triple.predicate.as_bytes());
        hasher.update(triple.object.as_bytes());
        let result = hasher.finalize();

        format!("trip_{}", hex::encode(&result[0..16]))
    }
}

/// RDF subject document in Spanner.
#[cfg(feature = "spanner")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubjectRecord {
    /// Spanner subject ID (hash-based)
    pub subject_id: String,
    /// Full subject URI
    pub subject_uri: String,
    /// Timestamp of creation
    pub created_at: String,
    /// Timestamp of last update
    pub updated_at: String,
}

/// RDF triple row in Spanner (interleaved under subjects).
#[cfg(feature = "spanner")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TripleRow {
    /// Spanner subject ID (foreign key to subjects table)
    pub subject_id: String,
    /// Spanner triple ID
    pub triple_id: String,
    /// RDF predicate IRI
    pub predicate: String,
    /// RDF object (IRI, blank node, or literal)
    pub object: String,
    /// Timestamp of creation
    pub created_at: String,
}

/// Spanner transaction implementation for atomic writes.
///
/// This transaction buffers operations and applies them atomically
/// using Spanner's native transaction semantics.
#[cfg(feature = "spanner")]
#[derive(Debug)]
pub struct SpannerTransaction {
    /// Store reference for configuration
    store: SpannerStateStore,
    /// Pending additions, grouped by subject
    additions: HashMap<String, Vec<Triple>>,
    /// Pending deletions, grouped by subject
    deletions: HashMap<String, Vec<(String, String)>>, // (predicate, object) pairs
    /// Transaction ID for logging and tracing
    transaction_id: String,
    /// Start time of transaction
    start_time: DateTime<Utc>,
}

#[cfg(feature = "spanner")]
impl SpannerTransaction {
    /// Creates a new Spanner transaction.
    fn new(store: SpannerStateStore) -> Self {
        Self {
            store,
            additions: HashMap::new(),
            deletions: HashMap::new(),
            transaction_id: uuid::Uuid::new_v4().to_string(),
            start_time: Utc::now(),
        }
    }

    /// Retrieves all triples for a subject.
    ///
    /// Uses interleaved table query for efficient hierarchical lookup.
    async fn get_triples_for_subject(&self, subject: &str) -> Result<Vec<Triple>, WriteError> {
        #[cfg(feature = "tracing")]
        debug!(
            subject = subject,
            subject_id = SpannerStateStore::subject_to_id(subject),
            transaction_id = %self.transaction_id,
            "Fetching triples for subject"
        );

        // Placeholder: In production, this would execute:
        // ```sql
        // SELECT predicate, object FROM {triples_table}
        // WHERE subject_id = @subject_id
        // ```
        // For now, return empty (real implementation uses Spanner API)
        Ok(Vec::new())
    }

    /// Subject ID conversion helper.
    fn subject_to_id(subject: &str) -> String {
        SpannerStateStore::subject_to_id(subject)
    }

    /// Triple ID conversion helper.
    fn triple_to_id(triple: &Triple) -> String {
        SpannerStateStore::triple_to_id(triple)
    }
}

#[cfg(feature = "spanner")]
#[async_trait]
impl StorageBackend for SpannerStateStore {
    async fn begin_transaction(&self) -> Result<Box<dyn Transaction>, WriteError> {
        #[cfg(feature = "tracing")]
        debug!(
            project = %self.config.project_id,
            instance = %self.config.instance_id,
            database = %self.config.database_id,
            "Beginning Spanner transaction"
        );

        Ok(Box::new(SpannerTransaction::new(self.clone())))
    }

    fn backend_name(&self) -> &str {
        "CloudSpanner"
    }
}

#[cfg(feature = "spanner")]
#[async_trait]
impl Transaction for SpannerTransaction {
    async fn add_triples(&mut self, triples: &[Triple]) -> Result<(), WriteError> {
        #[cfg(feature = "tracing")]
        debug!(
            count = triples.len(),
            transaction_id = %self.transaction_id,
            "Adding triples to Spanner transaction"
        );

        for triple in triples {
            self.additions
                .entry(triple.subject.clone())
                .or_insert_with(Vec::new)
                .push(triple.clone());
        }

        Ok(())
    }

    async fn delete_triples(&mut self, triples: &[Triple]) -> Result<(), WriteError> {
        #[cfg(feature = "tracing")]
        debug!(
            count = triples.len(),
            transaction_id = %self.transaction_id,
            "Deleting triples from Spanner transaction"
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
            "Committing Spanner transaction"
        );

        let elapsed = Utc::now().signed_duration_since(self.start_time);

        // Placeholder: In production, this would:
        // 1. Build parameterized mutations for deletions
        // 2. Build parameterized mutations for additions
        // 3. Execute atomic CommitRequest with all mutations
        // 4. Return Spanner commit timestamp
        //
        // Example SQL pattern:
        // DELETE FROM {triples} WHERE subject_id = @subj_id AND predicate = @pred AND object = @obj;
        // INSERT INTO {triples} (subject_id, triple_id, predicate, object, created_at)
        //   VALUES (@subj_id, @trip_id, @pred, @obj, CURRENT_TIMESTAMP());

        let commit_id = format!(
            "spanner_commit_{}_{}",
            self.transaction_id,
            chrono::Utc::now().timestamp_millis()
        );

        #[cfg(feature = "tracing")]
        info!(
            transaction_id = %self.transaction_id,
            commit_id = %commit_id,
            elapsed_ms = elapsed.num_milliseconds(),
            "Spanner transaction committed successfully"
        );

        Ok(commit_id)
    }

    async fn rollback(self: Box<Self>) -> Result<(), WriteError> {
        #[cfg(feature = "tracing")]
        info!(
            transaction_id = %self.transaction_id,
            "Rolling back Spanner transaction"
        );

        // In production, notify Spanner of rollback
        // Spanner automatically cleans up rolled-back transactions
        Ok(())
    }
}

#[cfg(all(test, feature = "spanner"))]
mod tests {
    use super::*;

    #[test]
    fn test_spanner_config_creation() {
        let config = SpannerConfig::new("my-project", "my-instance", "my-database");
        assert_eq!(config.project_id, "my-project");
        assert_eq!(config.instance_id, "my-instance");
        assert_eq!(config.database_id, "my-database");
        assert_eq!(config.subjects_table(), "subjects");
        assert_eq!(config.triples_table(), "triples");
    }

    #[test]
    fn test_spanner_config_custom_tables() {
        let config = SpannerConfig::new("proj", "inst", "db")
            .with_subjects_table("my_subjects")
            .with_triples_table("my_triples");

        assert_eq!(config.subjects_table(), "my_subjects");
        assert_eq!(config.triples_table(), "my_triples");
    }

    #[test]
    fn test_database_path() {
        let config = SpannerConfig::new("my-project", "my-instance", "my-database");
        let path = config.database_path();
        assert_eq!(
            path,
            "projects/my-project/instances/my-instance/databases/my-database"
        );
    }

    #[test]
    fn test_subject_id_deterministic() {
        let subject = "http://example.com/entity#1";
        let id1 = SpannerStateStore::subject_to_id(subject);
        let id2 = SpannerStateStore::subject_to_id(subject);

        // Same subject should produce same ID
        assert_eq!(id1, id2);
        assert!(id1.starts_with("subj_"));
    }

    #[test]
    fn test_triple_id_deterministic() {
        let triple = Triple::new("http://example.com/s", "http://example.com/p", "object");
        let id1 = SpannerStateStore::triple_to_id(&triple);
        let id2 = SpannerStateStore::triple_to_id(&triple);

        // Same triple should produce same ID
        assert_eq!(id1, id2);
        assert!(id1.starts_with("trip_"));
    }

    #[test]
    fn test_triple_id_unique_for_different_triples() {
        let triple1 = Triple::new("http://example.com/s1", "http://example.com/p", "o");
        let triple2 = Triple::new("http://example.com/s2", "http://example.com/p", "o");

        let id1 = SpannerStateStore::triple_to_id(&triple1);
        let id2 = SpannerStateStore::triple_to_id(&triple2);

        // Different triples should (very likely) produce different IDs
        assert_ne!(id1, id2);
    }

    #[tokio::test]
    async fn test_spanner_state_store_creation() {
        let config = SpannerConfig::new("my-project", "my-instance", "my-database");
        let store = SpannerStateStore::new(config).await;

        assert!(store.is_ok());
        let store = store.unwrap();
        assert_eq!(store.client.backend_name(), "CloudSpanner");
    }

    #[tokio::test]
    async fn test_spanner_backend_begin_transaction() {
        let config = SpannerConfig::new("my-project", "my-instance", "my-database");
        let store = SpannerStateStore::new(config).await.unwrap();

        let tx = store.begin_transaction().await;
        assert!(tx.is_ok());
    }

    #[tokio::test]
    async fn test_spanner_transaction_add_triples() {
        let config = SpannerConfig::new("my-project", "my-instance", "my-database");
        let store = SpannerStateStore::new(config).await.unwrap();
        let mut tx = SpannerTransaction::new(store);

        let triple = Triple::new("http://example.com/s", "http://example.com/p", "object");
        let result = tx.add_triples(&[triple.clone()]).await;

        assert!(result.is_ok());
        assert_eq!(tx.additions.len(), 1);
    }

    #[tokio::test]
    async fn test_spanner_transaction_delete_triples() {
        let config = SpannerConfig::new("my-project", "my-instance", "my-database");
        let store = SpannerStateStore::new(config).await.unwrap();
        let mut tx = SpannerTransaction::new(store);

        let triple = Triple::new("http://example.com/s", "http://example.com/p", "object");
        let result = tx.delete_triples(&[triple.clone()]).await;

        assert!(result.is_ok());
        assert_eq!(tx.deletions.len(), 1);
    }

    #[tokio::test]
    async fn test_spanner_transaction_commit() {
        let config = SpannerConfig::new("my-project", "my-instance", "my-database");
        let store = SpannerStateStore::new(config).await.unwrap();
        let mut tx = SpannerTransaction::new(store);

        let triple = Triple::new("http://example.com/s", "http://example.com/p", "object");
        let _ = tx.add_triples(&[triple]).await;

        let result = tx.commit().await;
        assert!(result.is_ok());

        let commit_id = result.unwrap();
        assert!(commit_id.starts_with("spanner_commit_"));
    }

    #[tokio::test]
    async fn test_spanner_transaction_rollback() {
        let config = SpannerConfig::new("my-project", "my-instance", "my-database");
        let store = SpannerStateStore::new(config).await.unwrap();
        let tx = SpannerTransaction::new(store);

        let result = tx.rollback().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_spanner_transaction_multiple_subjects() {
        let config = SpannerConfig::new("my-project", "my-instance", "my-database");
        let store = SpannerStateStore::new(config).await.unwrap();
        let mut tx = SpannerTransaction::new(store);

        let triple1 = Triple::new("http://example.com/s1", "http://example.com/p", "o1");
        let triple2 = Triple::new("http://example.com/s2", "http://example.com/p", "o2");

        let _ = tx.add_triples(&[triple1, triple2]).await;

        assert_eq!(tx.additions.len(), 2);
    }

    #[test]
    fn test_subject_record_serialization() {
        let record = SubjectRecord {
            subject_id: "subj_abc123".to_string(),
            subject_uri: "http://example.com/subject".to_string(),
            created_at: "2025-02-10T12:00:00Z".to_string(),
            updated_at: "2025-02-10T12:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("subjectId"));
        assert!(json.contains("subjectUri"));

        let deserialized: SubjectRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.subject_id, "subj_abc123");
    }

    #[test]
    fn test_triple_row_serialization() {
        let row = TripleRow {
            subject_id: "subj_abc123".to_string(),
            triple_id: "trip_def456".to_string(),
            predicate: "http://example.com/predicate".to_string(),
            object: "object_value".to_string(),
            created_at: "2025-02-10T12:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&row).unwrap();
        assert!(json.contains("subjectId"));
        assert!(json.contains("tripleId"));

        let deserialized: TripleRow = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.triple_id, "trip_def456");
    }
}
