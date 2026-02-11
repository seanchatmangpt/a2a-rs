//! Unit tests for MemoryStore port trait
//!
//! Tests the contract and behavior of the MemoryStore port trait
//! using mock implementations.

use a2a_rs::domain::error::A2AError;
use a2a_rs::port::memory_store::{MemoryEntry, MemoryQuery, MemoryStats, MemoryStore};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock implementation of MemoryStore for testing
#[derive(Debug, Clone)]
struct MockMemoryStore {
    entries: Arc<RwLock<HashMap<String, MemoryEntry>>>,
}

impl MockMemoryStore {
    fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn count(&self) -> usize {
        self.entries.read().await.len()
    }
}

#[async_trait]
impl MemoryStore for MockMemoryStore {
    async fn create(&self, entry: MemoryEntry) -> Result<MemoryEntry, A2AError> {
        let mut entries = self.entries.write().await;
        entries.insert(entry.id.clone(), entry.clone());
        Ok(entry)
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>, A2AError> {
        let mut entries = self.entries.write().await;

        if let Some(entry) = entries.get_mut(id) {
            // Increment access count and update last accessed
            entry.access_count += 1;
            entry.last_accessed_at = Utc::now();
            Ok(Some(entry.clone()))
        } else {
            Ok(None)
        }
    }

    async fn update(&self, entry: MemoryEntry) -> Result<MemoryEntry, A2AError> {
        let mut entries = self.entries.write().await;

        if !entries.contains_key(&entry.id) {
            return Err(A2AError::TaskNotFound(format!("Memory {} not found", entry.id)));
        }

        entries.insert(entry.id.clone(), entry.clone());
        Ok(entry)
    }

    async fn delete(&self, id: &str) -> Result<bool, A2AError> {
        let mut entries = self.entries.write().await;
        Ok(entries.remove(id).is_some())
    }

    async fn search(&self, query: MemoryQuery) -> Result<Vec<MemoryEntry>, A2AError> {
        let entries = self.entries.read().await;
        let mut results: Vec<MemoryEntry> = entries
            .values()
            .filter(|entry| {
                // Filter by project
                if let Some(ref project) = query.project {
                    if &entry.project != project {
                        return false;
                    }
                }

                // Filter by topic
                if let Some(ref topic) = query.topic {
                    if &entry.topic != topic {
                        return false;
                    }
                }

                // Filter by search text
                if let Some(ref search_text) = query.search_text {
                    if !entry.content.contains(search_text) {
                        return false;
                    }
                }

                // Filter by tags
                if !query.tags.is_empty() {
                    if !query.tags.iter().any(|tag| entry.tags.contains(tag)) {
                        return false;
                    }
                }

                // Filter by minimum importance
                if let Some(min_importance) = query.min_importance {
                    if entry.importance < min_importance {
                        return false;
                    }
                }

                // Filter expired
                if query.exclude_expired {
                    if let Some(expires_at) = entry.expires_at {
                        if Utc::now() > expires_at {
                            return false;
                        }
                    }
                }

                true
            })
            .cloned()
            .collect();

        // Sort results
        if let Some(sort_by) = query.sort_by {
            results.sort_by(|a, b| {
                let comparison = match sort_by.as_str() {
                    "importance" => {
                        a.importance
                            .partial_cmp(&b.importance)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    }
                    "access_count" => a.access_count.cmp(&b.access_count),
                    "created_at" => a.created_at.cmp(&b.created_at),
                    _ => std::cmp::Ordering::Equal,
                };

                if query.sort_desc {
                    comparison.reverse()
                } else {
                    comparison
                }
            });
        }

        // Apply limit and offset
        if let Some(offset) = query.offset {
            if offset < results.len() {
                results = results.into_iter().skip(offset).collect();
            } else {
                results.clear();
            }
        }

        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    async fn stats(
        &self,
        project: Option<&str>,
        topic: Option<&str>,
    ) -> Result<MemoryStats, A2AError> {
        let entries = self.entries.read().await;
        let filtered: Vec<_> = entries
            .values()
            .filter(|entry| {
                if let Some(proj) = project {
                    if &entry.project != proj {
                        return false;
                    }
                }
                if let Some(top) = topic {
                    if &entry.topic != top {
                        return false;
                    }
                }
                true
            })
            .collect();

        if filtered.is_empty() {
            return Ok(MemoryStats {
                total_count: 0,
                expired_count: 0,
                avg_importance: 0.0,
                total_accesses: 0,
                most_accessed_id: None,
                oldest_created_at: None,
                newest_created_at: None,
            });
        }

        let total_count = filtered.len() as u64;
        let now = Utc::now();
        let expired_count = filtered
            .iter()
            .filter(|e| e.expires_at.map_or(false, |exp| now > exp))
            .count() as u64;

        let total_importance: f64 = filtered.iter().map(|e| e.importance).sum();
        let avg_importance = total_importance / total_count as f64;

        let total_accesses: u64 = filtered.iter().map(|e| e.access_count).sum();

        let most_accessed = filtered
            .iter()
            .max_by_key(|e| e.access_count)
            .map(|e| e.id.clone());

        let oldest_created = filtered
            .iter()
            .map(|e| e.created_at)
            .min()
            .unwrap_or_else(|| Utc::now());

        let newest_created = filtered
            .iter()
            .map(|e| e.created_at)
            .max()
            .unwrap_or_else(|| Utc::now());

        Ok(MemoryStats {
            total_count,
            expired_count,
            avg_importance,
            total_accesses,
            most_accessed_id: most_accessed,
            oldest_created_at: Some(oldest_created),
            newest_created_at: Some(newest_created),
        })
    }

    async fn delete_expired(&self) -> Result<u64, A2AError> {
        let mut entries = self.entries.write().await;
        let now = Utc::now();
        let initial_count = entries.len();

        entries.retain(|_, entry| {
            entry
                .expires_at
                .map_or(true, |exp| now <= exp)
        });

        Ok((initial_count - entries.len()) as u64)
    }

    async fn summarize(&self, project: &str, topic: &str) -> Result<Option<String>, A2AError> {
        let entries = self.entries.read().await;

        let summaries: Vec<_> = entries
            .values()
            .filter(|e| e.project == project && e.topic == topic)
            .map(|e| e.content.clone())
            .collect();

        if summaries.is_empty() {
            Ok(None)
        } else {
            Ok(Some(format!(
                "Summary of {} memories for {}/{}",
                summaries.len(),
                project,
                topic
            )))
        }
    }

    async fn list_projects(&self) -> Result<Vec<String>, A2AError> {
        let entries = self.entries.read().await;
        let projects: std::collections::HashSet<_> =
            entries.values().map(|e| e.project.clone()).collect();
        let mut sorted: Vec<_> = projects.into_iter().collect();
        sorted.sort();
        Ok(sorted)
    }

    async fn list_topics(&self, project: &str) -> Result<Vec<String>, A2AError> {
        let entries = self.entries.read().await;
        let topics: std::collections::HashSet<_> = entries
            .values()
            .filter(|e| e.project == project)
            .map(|e| e.topic.clone())
            .collect();
        let mut sorted: Vec<_> = topics.into_iter().collect();
        sorted.sort();
        Ok(sorted)
    }
}

fn create_test_entry(id: &str, project: &str, topic: &str, content: &str) -> MemoryEntry {
    MemoryEntry {
        id: id.to_string(),
        project: project.to_string(),
        topic: topic.to_string(),
        content: content.to_string(),
        importance: 0.5,
        access_count: 0,
        created_at: Utc::now(),
        last_accessed_at: Utc::now(),
        expires_at: None,
        tags: vec![],
        metadata: None,
    }
}

#[tokio::test]
async fn test_create_memory() {
    let store = MockMemoryStore::new();

    let entry = create_test_entry("mem-1", "proj-a", "topic-x", "Content 1");

    let result = store.create(entry.clone()).await;

    assert!(result.is_ok());
    assert_eq!(store.count().await, 1);
}

#[tokio::test]
async fn test_get_memory() {
    let store = MockMemoryStore::new();

    let entry = create_test_entry("mem-2", "proj-a", "topic-x", "Content 2");
    store.create(entry.clone()).await.unwrap();

    let result = store.get("mem-2").await;

    assert!(result.is_ok());
    let retrieved = result.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().content, "Content 2");
}

#[tokio::test]
async fn test_get_memory_not_found() {
    let store = MockMemoryStore::new();

    let result = store.get("nonexistent").await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_get_increments_access_count() {
    let store = MockMemoryStore::new();

    let mut entry = create_test_entry("mem-3", "proj-a", "topic-x", "Content 3");
    entry.access_count = 5;
    store.create(entry).await.unwrap();

    // First access
    let result1 = store.get("mem-3").await.unwrap().unwrap();
    assert_eq!(result1.access_count, 6);

    // Second access
    let result2 = store.get("mem-3").await.unwrap().unwrap();
    assert_eq!(result2.access_count, 7);
}

#[tokio::test]
async fn test_update_memory() {
    let store = MockMemoryStore::new();

    let entry = create_test_entry("mem-4", "proj-a", "topic-x", "Original");
    store.create(entry).await.unwrap();

    let mut updated = create_test_entry("mem-4", "proj-a", "topic-x", "Updated");
    updated.importance = 0.9;

    let result = store.update(updated.clone()).await;

    assert!(result.is_ok());
    let retrieved = store.get("mem-4").await.unwrap().unwrap();
    assert_eq!(retrieved.content, "Updated");
    assert_eq!(retrieved.importance, 0.9);
}

#[tokio::test]
async fn test_update_memory_not_found() {
    let store = MockMemoryStore::new();

    let entry = create_test_entry("mem-5", "proj-a", "topic-x", "Content");

    let result = store.update(entry).await;

    assert!(matches!(result, Err(A2AError::TaskNotFound(_))));
}

#[tokio::test]
async fn test_delete_memory() {
    let store = MockMemoryStore::new();

    let entry = create_test_entry("mem-6", "proj-a", "topic-x", "Content");
    store.create(entry).await.unwrap();

    let result = store.delete("mem-6").await;

    assert!(result.is_ok());
    assert!(result.unwrap());
    assert_eq!(store.count().await, 0);
}

#[tokio::test]
async fn test_delete_memory_not_found() {
    let store = MockMemoryStore::new();

    let result = store.delete("nonexistent").await;

    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[tokio::test]
async fn test_search_all() {
    let store = MockMemoryStore::new();

    store
        .create(create_test_entry("mem-7", "proj-a", "topic-x", "Content 1"))
        .await
        .unwrap();
    store
        .create(create_test_entry("mem-8", "proj-b", "topic-y", "Content 2"))
        .await
        .unwrap();

    let query = MemoryQuery::default();
    let result = store.search(query).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 2);
}

#[tokio::test]
async fn test_search_by_project() {
    let store = MockMemoryStore::new();

    store
        .create(create_test_entry("mem-9", "proj-a", "topic-x", "Content 1"))
        .await
        .unwrap();
    store
        .create(create_test_entry("mem-10", "proj-b", "topic-y", "Content 2"))
        .await
        .unwrap();

    let query = MemoryQuery {
        project: Some("proj-a".to_string()),
        ..Default::default()
    };

    let result = store.search(query).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1);
}

#[tokio::test]
async fn test_search_by_topic() {
    let store = MockMemoryStore::new();

    store
        .create(create_test_entry("mem-11", "proj-a", "topic-x", "Content 1"))
        .await
        .unwrap();
    store
        .create(create_test_entry("mem-12", "proj-a", "topic-y", "Content 2"))
        .await
        .unwrap();

    let query = MemoryQuery {
        topic: Some("topic-x".to_string()),
        ..Default::default()
    };

    let result = store.search(query).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1);
}

#[tokio::test]
async fn test_search_by_text() {
    let store = MockMemoryStore::new();

    store
        .create(create_test_entry("mem-13", "proj-a", "topic-x", "Rust is great"))
        .await
        .unwrap();
    store
        .create(create_test_entry("mem-14", "proj-a", "topic-y", "Python is nice"))
        .await
        .unwrap();

    let query = MemoryQuery {
        search_text: Some("Rust".to_string()),
        ..Default::default()
    };

    let result = store.search(query).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1);
}

#[tokio::test]
async fn test_search_by_tags() {
    let store = MockMemoryStore::new();

    let mut entry1 = create_test_entry("mem-15", "proj-a", "topic-x", "Content 1");
    entry1.tags = vec!["rust".to_string(), "testing".to_string()];
    store.create(entry1).await.unwrap();

    let mut entry2 = create_test_entry("mem-16", "proj-a", "topic-y", "Content 2");
    entry2.tags = vec!["python".to_string()];
    store.create(entry2).await.unwrap();

    let query = MemoryQuery {
        tags: vec!["rust".to_string()],
        ..Default::default()
    };

    let result = store.search(query).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1);
}

#[tokio::test]
async fn test_search_by_min_importance() {
    let store = MockMemoryStore::new();

    let mut entry1 = create_test_entry("mem-17", "proj-a", "topic-x", "Content 1");
    entry1.importance = 0.3;
    store.create(entry1).await.unwrap();

    let mut entry2 = create_test_entry("mem-18", "proj-a", "topic-y", "Content 2");
    entry2.importance = 0.8;
    store.create(entry2).await.unwrap();

    let query = MemoryQuery {
        min_importance: Some(0.5),
        ..Default::default()
    };

    let result = store.search(query).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1);
}

#[tokio::test]
async fn test_search_exclude_expired() {
    let store = MockMemoryStore::new();

    let mut entry1 = create_test_entry("mem-19", "proj-a", "topic-x", "Valid content");
    entry1.expires_at = Some(Utc::now() + Duration::hours(1));
    store.create(entry1).await.unwrap();

    let mut entry2 = create_test_entry("mem-20", "proj-a", "topic-y", "Expired content");
    entry2.expires_at = Some(Utc::now() - Duration::hours(1));
    store.create(entry2).await.unwrap();

    let query = MemoryQuery {
        exclude_expired: true,
        ..Default::default()
    };

    let result = store.search(query).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1);
}

#[tokio::test]
async fn test_search_with_limit() {
    let store = MockMemoryStore::new();

    for i in 0..10 {
        store
            .create(create_test_entry(
                &format!("mem-{}", i),
                "proj-a",
                "topic-x",
                &format!("Content {}", i),
            ))
            .await
            .unwrap();
    }

    let query = MemoryQuery {
        limit: Some(5),
        ..Default::default()
    };

    let result = store.search(query).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 5);
}

#[tokio::test]
async fn test_search_with_offset() {
    let store = MockMemoryStore::new();

    for i in 0..5 {
        store
            .create(create_test_entry(
                &format!("mem-off-{}", i),
                "proj-a",
                "topic-x",
                &format!("Content {}", i),
            ))
            .await
            .unwrap();
    }

    let query = MemoryQuery {
        offset: Some(2),
        limit: Some(2),
        ..Default::default()
    };

    let result = store.search(query).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 2);
}

#[tokio::test]
async fn test_search_sort_by_importance() {
    let store = MockMemoryStore::new();

    let mut entry1 = create_test_entry("mem-21", "proj-a", "topic-x", "Low importance");
    entry1.importance = 0.2;
    store.create(entry1).await.unwrap();

    let mut entry2 = create_test_entry("mem-22", "proj-a", "topic-x", "High importance");
    entry2.importance = 0.9;
    store.create(entry2).await.unwrap();

    let query = MemoryQuery {
        sort_by: Some("importance".to_string()),
        sort_desc: true,
        ..Default::default()
    };

    let result = store.search(query).await;

    assert!(result.is_ok());
    let results = result.unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].importance, 0.9);
    assert_eq!(results[1].importance, 0.2);
}

#[tokio::test]
async fn test_stats() {
    let store = MockMemoryStore::new();

    let mut entry1 = create_test_entry("mem-23", "proj-stats", "topic-stats", "Content 1");
    entry1.importance = 0.5;
    entry1.access_count = 10;
    store.create(entry1).await.unwrap();

    let mut entry2 = create_test_entry("mem-24", "proj-stats", "topic-stats", "Content 2");
    entry2.importance = 0.7;
    entry2.access_count = 5;
    store.create(entry2).await.unwrap();

    let result = store.stats(Some("proj-stats"), Some("topic-stats")).await;

    assert!(result.is_ok());
    let stats = result.unwrap();
    assert_eq!(stats.total_count, 2);
    assert_eq!(stats.total_accesses, 15);
    assert!((stats.avg_importance - 0.6).abs() < 0.01);
}

#[tokio::test]
async fn test_stats_empty() {
    let store = MockMemoryStore::new();

    let result = store.stats(Some("nonexistent"), Some("nonexistent")).await;

    assert!(result.is_ok());
    let stats = result.unwrap();
    assert_eq!(stats.total_count, 0);
    assert_eq!(stats.expired_count, 0);
    assert_eq!(stats.avg_importance, 0.0);
}

#[tokio::test]
async fn test_delete_expired() {
    let store = MockMemoryStore::new();

    let mut entry1 = create_test_entry("mem-25", "proj-a", "topic-x", "Valid");
    entry1.expires_at = Some(Utc::now() + Duration::hours(1));
    store.create(entry1).await.unwrap();

    let mut entry2 = create_test_entry("mem-26", "proj-a", "topic-y", "Expired");
    entry2.expires_at = Some(Utc::now() - Duration::hours(1));
    store.create(entry2).await.unwrap();

    let result = store.delete_expired().await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);
    assert_eq!(store.count().await, 1);
}

#[tokio::test]
async fn test_summarize() {
    let store = MockMemoryStore::new();

    store
        .create(create_test_entry("mem-27", "proj-sum", "topic-sum", "Memory 1"))
        .await
        .unwrap();
    store
        .create(create_test_entry("mem-28", "proj-sum", "topic-sum", "Memory 2"))
        .await
        .unwrap();

    let result = store.summarize("proj-sum", "topic-sum").await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_some());
}

#[tokio::test]
async fn test_summarize_empty() {
    let store = MockMemoryStore::new();

    let result = store.summarize("nonexistent", "nonexistent").await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_list_projects() {
    let store = MockMemoryStore::new();

    store
        .create(create_test_entry("mem-29", "proj-b", "topic-x", "Content"))
        .await
        .unwrap();
    store
        .create(create_test_entry("mem-30", "proj-a", "topic-y", "Content"))
        .await
        .unwrap();
    store
        .create(create_test_entry("mem-31", "proj-c", "topic-z", "Content"))
        .await
        .unwrap();

    let result = store.list_projects().await;

    assert!(result.is_ok());
    let projects = result.unwrap();
    assert_eq!(projects, vec!["proj-a", "proj-b", "proj-c"]);
}

#[tokio::test]
async fn test_list_topics() {
    let store = MockMemoryStore::new();

    store
        .create(create_test_entry("mem-32", "proj-a", "topic-z", "Content"))
        .await
        .unwrap();
    store
        .create(create_test_entry("mem-33", "proj-a", "topic-y", "Content"))
        .await
        .unwrap();
    store
        .create(create_test_entry("mem-34", "proj-b", "topic-x", "Content"))
        .await
        .unwrap();

    let result = store.list_topics("proj-a").await;

    assert!(result.is_ok());
    let topics = result.unwrap();
    assert_eq!(topics, vec!["topic-y", "topic-z"]);
}

#[tokio::test]
async fn test_memory_with_metadata() {
    let store = MockMemoryStore::new();

    let mut entry = create_test_entry("mem-35", "proj-a", "topic-x", "Content");
    entry.metadata = Some(serde_json::json!({"source": "test", "priority": 1}));

    let result = store.create(entry).await;

    assert!(result.is_ok());
    let retrieved = store.get("mem-35").await.unwrap().unwrap();
    assert!(retrieved.metadata.is_some());
}

#[tokio::test]
async fn test_concurrent_operations() {
    let store = MockMemoryStore::new();

    // Create multiple entries concurrently
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let store = store.clone();
            tokio::spawn(async move {
                let entry = create_test_entry(
                    &format!("mem-concurrent-{}", i),
                    "proj-concurrent",
                    "topic-concurrent",
                    &format!("Content {}", i),
                );
                store.create(entry).await
            })
        })
        .collect();

    // Wait for all to complete
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }

    assert_eq!(store.count().await, 10);
}

#[tokio::test]
async fn test_complex_search() {
    let store = MockMemoryStore::new();

    let mut entry1 = create_test_entry("mem-36", "proj-a", "rust", "Rust testing patterns");
    entry1.importance = 0.9;
    entry1.tags = vec!["rust".to_string(), "testing".to_string()];
    entry1.access_count = 100;
    store.create(entry1).await.unwrap();

    let mut entry2 = create_test_entry("mem-37", "proj-b", "rust", "Rust async patterns");
    entry2.importance = 0.7;
    entry2.tags = vec!["rust".to_string(), "async".to_string()];
    entry2.access_count = 50;
    store.create(entry2).await.unwrap();

    let query = MemoryQuery {
        project: Some("proj-a".to_string()),
        topic: Some("rust".to_string()),
        search_text: Some("testing".to_string()),
        min_importance: Some(0.5),
        limit: Some(10),
        sort_by: Some("access_count".to_string()),
        sort_desc: true,
        ..Default::default()
    };

    let result = store.search(query).await;

    assert!(result.is_ok());
    let results = result.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].content, "Rust testing patterns");
}
