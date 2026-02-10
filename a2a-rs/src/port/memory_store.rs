//! Memory storage port for persistent agent learnings
//!
//! This port defines the interface for storing and retrieving agent memories
//! across sessions. Memories are keyed by project and topic, enabling contextual
//! recall and continuous improvement (Kaizen principle).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::A2AError;

/// A single memory entry representing a learning or insight
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntry {
    /// Unique identifier for the memory
    pub id: String,

    /// Project or workspace identifier
    pub project: String,

    /// Topic or category (e.g., "debugging", "patterns", "tps-metrics")
    pub topic: String,

    /// The actual content/learning
    pub content: String,

    /// Importance score (0.0 to 1.0, higher = more important)
    #[serde(default)]
    pub importance: f64,

    /// Number of times this memory has been accessed
    #[serde(default)]
    pub access_count: u64,

    /// When this memory was created
    pub created_at: DateTime<Utc>,

    /// When this memory was last accessed
    pub last_accessed_at: DateTime<Utc>,

    /// When this memory should expire (TTL)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,

    /// Tags for additional categorization
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Query filters for searching memories
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryQuery {
    /// Filter by project
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,

    /// Filter by topic
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,

    /// Full-text search in content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_text: Option<String>,

    /// Filter by tags (any of these tags)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Minimum importance score
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_importance: Option<f64>,

    /// Only return non-expired memories
    #[serde(default)]
    pub exclude_expired: bool,

    /// Maximum number of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,

    /// Offset for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,

    /// Sort by field (e.g., "importance", "access_count", "created_at")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,

    /// Sort descending (default: true)
    #[serde(default = "default_true")]
    pub sort_desc: bool,
}

fn default_true() -> bool {
    true
}

/// Memory statistics for a project or topic
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStats {
    /// Total number of memories
    pub total_count: u64,

    /// Total number of expired memories
    pub expired_count: u64,

    /// Average importance score
    pub avg_importance: f64,

    /// Total access count across all memories
    pub total_accesses: u64,

    /// Most accessed memory ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub most_accessed_id: Option<String>,

    /// Oldest memory creation date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_created_at: Option<DateTime<Utc>>,

    /// Newest memory creation date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newest_created_at: Option<DateTime<Utc>>,
}

/// Memory storage operations
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Create a new memory entry
    async fn create(&self, entry: MemoryEntry) -> Result<MemoryEntry, A2AError>;

    /// Retrieve a memory by ID (increments access count)
    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>, A2AError>;

    /// Update an existing memory
    async fn update(&self, entry: MemoryEntry) -> Result<MemoryEntry, A2AError>;

    /// Delete a memory by ID
    async fn delete(&self, id: &str) -> Result<bool, A2AError>;

    /// Search memories with filters
    async fn search(&self, query: MemoryQuery) -> Result<Vec<MemoryEntry>, A2AError>;

    /// Get statistics for a project or topic
    async fn stats(
        &self,
        project: Option<&str>,
        topic: Option<&str>,
    ) -> Result<MemoryStats, A2AError>;

    /// Delete expired memories (cleanup operation)
    async fn delete_expired(&self) -> Result<u64, A2AError>;

    /// Summarize memories for a project/topic (AI-driven compression)
    async fn summarize(&self, project: &str, topic: &str) -> Result<Option<String>, A2AError>;

    /// Get all unique projects
    async fn list_projects(&self) -> Result<Vec<String>, A2AError>;

    /// Get all unique topics for a project
    async fn list_topics(&self, project: &str) -> Result<Vec<String>, A2AError>;
}
