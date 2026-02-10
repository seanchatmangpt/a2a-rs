# Workflow Persistence & Checkpoint Recovery

## Overview

The `WorkflowStore` port provides a complete framework for persisting workflow instances to Firestore with checkpoint-based recovery and replay capabilities. This enables:

1. **Checkpointing** - Save workflow state at critical points
2. **Recovery** - Restore workflows from checkpoints after failures
3. **Replay** - Reconstruct execution history from checkpoints
4. **Querying** - Find and list checkpoints with flexible filtering

## Architecture

### Port Trait: `WorkflowStore`

Located in `src/port/workflow_store.rs`, this async trait defines the persistence contract:

```rust
#[async_trait]
pub trait WorkflowStore: Send + Sync {
    // Checkpoint creation
    async fn create_checkpoint(
        &self,
        instance: &WorkflowInstance,
        description: Option<String>,
        tags: Vec<String>,
    ) -> WorkflowStoreResult<CheckpointMetadata>;

    // Checkpoint restoration
    async fn restore_checkpoint(&self, checkpoint_id: &str) -> WorkflowStoreResult<Checkpoint>;

    // Query and recovery operations
    async fn query_checkpoints(&self, query: &CheckpointQuery) -> WorkflowStoreResult<Vec<CheckpointMetadata>>;
    async fn recover_to_latest(&self, instance_id: &str) -> WorkflowStoreResult<RecoverySummary>;

    // Checkpoint management
    async fn delete_checkpoint(&self, checkpoint_id: &str) -> WorkflowStoreResult<()>;
    async fn prune_old_checkpoints(&self, instance_id: &str, keep_count: usize) -> WorkflowStoreResult<usize>;

    // Export/Import for backup
    async fn export_checkpoint(&self, checkpoint_id: &str) -> WorkflowStoreResult<String>;
    async fn import_checkpoint(&self, json: &str) -> WorkflowStoreResult<CheckpointMetadata>;
}
```

### Domain Types

#### `CheckpointMetadata`
Lightweight metadata for a checkpoint:
```rust
pub struct CheckpointMetadata {
    pub checkpoint_id: String,
    pub instance_id: String,
    pub workflow_id: WorkflowId,
    pub state: InstanceState,
    pub created_at: DateTime<Utc>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub active_node_count: usize,
    pub context_size_bytes: usize,
    pub history_count: usize,
}
```

#### `Checkpoint`
Complete snapshot of workflow state:
```rust
pub struct Checkpoint {
    pub metadata: CheckpointMetadata,
    pub instance: WorkflowInstance,    // Full instance snapshot
    pub extra_context: HashMap<String, serde_json::Value>,
}
```

#### `CheckpointQuery`
Flexible filtering for checkpoint searches:
```rust
pub struct CheckpointQuery {
    pub instance_id: Option<String>,
    pub workflow_id: Option<WorkflowId>,
    pub state: Option<InstanceState>,
    pub tags: Vec<String>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}
```

#### `RecoverySummary`
Summary of recovery operation:
```rust
pub struct RecoverySummary {
    pub checkpoint_id: String,
    pub instance_id: String,
    pub events_replayed: usize,
    pub recovery_time_ms: u64,
    pub success: bool,
    pub error: Option<String>,
}
```

## Firestore Adapter

Located in `src/adapter/workflow_persistence.rs`, the `FirestoreWorkflowStore` implementation provides:

### Configuration

```rust
pub struct FirestoreConfig {
    pub project_id: String,
    pub instances_collection: String,    // Default: "workflows"
    pub checkpoints_collection: String,   // Default: "checkpoints"
    pub max_checkpoints_per_instance: usize,  // Auto-prune threshold
    pub auto_prune: bool,  // Auto-cleanup old checkpoints
}

// Builder pattern
let config = FirestoreConfig::new("my-project")
    .with_collections("workflows", "checkpoints")
    .with_max_checkpoints(10)
    .with_auto_prune(true);
```

### Implementation Features

#### 1. **Checkpoint Creation**
```rust
let metadata = store.create_checkpoint(
    &instance,
    Some("Post-approval checkpoint".to_string()),
    vec!["approval".to_string(), "critical".to_string()],
).await?;
```

Firestore document structure:
```json
{
  "checkpointId": "ckpt_a1b2c3d4e5f6g7h8",
  "instanceId": "inst-001",
  "workflowId": "wf-reimbursement",
  "state": "active",
  "createdAt": "2026-02-10T12:34:56Z",
  "description": "Post-approval checkpoint",
  "tags": ["approval", "critical"],
  "activeNodeCount": 3,
  "contextSizeBytes": 1024,
  "historyCount": 5
}
```

#### 2. **Checkpoint Restoration**
```rust
let checkpoint = store.restore_checkpoint("ckpt_a1b2c3d4e5f6g7h8").await?;

// Access full instance state
println!("Active nodes: {:?}", checkpoint.instance.active_nodes);
println!("Context: {:?}", checkpoint.instance.context);
```

#### 3. **Recovery to Latest**
```rust
let summary = store.recover_to_latest("inst-001").await?;
println!("Recovered from checkpoint: {}", summary.checkpoint_id);
println!("Replayed {} events in {}ms", summary.events_replayed, summary.recovery_time_ms);
```

#### 4. **Query with Filtering**
```rust
let query = CheckpointQuery {
    instance_id: Some("inst-001".to_string()),
    state: Some(InstanceState::Active),
    created_after: Some(Utc::now() - Duration::days(7)),
    limit: Some(10),
    ..Default::default()
};

let checkpoints = store.query_checkpoints(&query).await?;
```

#### 5. **Automatic Pruning**
```rust
// Keep only 5 most recent checkpoints for an instance
let deleted = store.prune_old_checkpoints("inst-001", 5).await?;
println!("Deleted {} old checkpoints", deleted);
```

#### 6. **Export/Import for Backups**
```rust
// Export to JSON
let json = store.export_checkpoint("ckpt_a1b2c3d4e5f6g7h8").await?;
std::fs::write("backup.json", &json)?;

// Import from JSON
let metadata = store.import_checkpoint(&json).await?;
```

## Firestore Document Structure

### Collections

#### **"workflows" Collection**
Stores active workflow instances:
```
collections/
  workflows/
    inst_a1b2c3d4e5f6g7h8/
      {instance data}
```

#### **"checkpoints" Collection**
Stores checkpoint snapshots:
```
collections/
  checkpoints/
    ckpt_a1b2c3d4e5f6g7h8/
      {checkpoint data with full instance}
```

### Composite Indexes (Firestore)

For efficient querying, create these composite indexes:

```
Collection: checkpoints
Fields:
  - instanceId (Ascending)
  - createdAt (Descending)

Collection: checkpoints
Fields:
  - state (Ascending)
  - createdAt (Descending)

Collection: checkpoints
Fields:
  - tags (Ascending, array)
  - createdAt (Descending)
```

## Error Handling

All operations return `WorkflowStoreResult<T>` which is `Result<T, WorkflowStoreError>`:

```rust
pub enum WorkflowStoreError {
    InstanceNotFound(String),
    CheckpointNotFound(String),
    SaveFailed(String),
    RestoreFailed(String),
    SerializationError(String),
    ReplayFailed(String),
    QueryError(String),
    InvalidState(String),
}
```

## Usage Examples

### Example 1: Basic Checkpoint & Recovery

```rust
use osiris_compiler::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create store
    let store = FirestoreWorkflowStore::new("my-project", "us-central1");

    // Create workflow instance
    let instance = create_workflow_instance();

    // Create checkpoint
    let metadata = store.create_checkpoint(
        &instance,
        Some("Initial checkpoint".to_string()),
        vec!["initialization".to_string()],
    ).await?;

    println!("Created checkpoint: {}", metadata.checkpoint_id);

    // Restore from checkpoint
    let checkpoint = store.restore_checkpoint(&metadata.checkpoint_id).await?;
    println!("Restored instance: {}", checkpoint.instance.instance_id);

    Ok(())
}
```

### Example 2: Recovery After Failure

```rust
async fn recovery_workflow(store: &FirestoreWorkflowStore) -> Result<(), Box<dyn std::error::Error>> {
    // Try to recover from latest checkpoint
    match store.recover_to_latest("inst-001").await {
        Ok(summary) => {
            if summary.success {
                println!("Recovery successful!");
                println!("Replayed {} events", summary.events_replayed);
                // Resume from recovered state
            } else {
                eprintln!("Recovery failed: {}", summary.error.unwrap_or_default());
                // Fallback to manual recovery
            }
        }
        Err(e) => {
            eprintln!("No checkpoints found: {}", e);
            // Start fresh workflow
        }
    }
    Ok(())
}
```

### Example 3: Checkpoint Management

```rust
async fn cleanup_old_checkpoints(
    store: &FirestoreWorkflowStore,
) -> Result<(), Box<dyn std::error::Error>> {
    // Query checkpoints older than 30 days
    let query = CheckpointQuery {
        created_before: Some(Utc::now() - Duration::days(30)),
        limit: Some(1000),
        ..Default::default()
    };

    let old_checkpoints = store.query_checkpoints(&query).await?;

    // Delete old checkpoints
    for checkpoint in old_checkpoints {
        store.delete_checkpoint(&checkpoint.checkpoint_id).await?;
    }

    println!("Cleaned up {} old checkpoints", old_checkpoints.len());
    Ok(())
}
```

## Feature Gating

The implementation is feature-gated with the `firestore` flag:

```toml
[features]
firestore = ["google-firestore1", "hyper", "hyper-rustls", "yup-oauth2"]
```

Build with:
```bash
cargo build --features firestore
```

## Production Considerations

### 1. **Indexing**
Create the composite indexes mentioned above for optimal query performance.

### 2. **Cost Optimization**
- Set `max_checkpoints_per_instance` to limit storage
- Enable `auto_prune` to automatically clean old checkpoints
- Periodically delete completed workflow checkpoints

### 3. **Monitoring**
Add tracing for observability:
```rust
#[cfg(feature = "tracing")]
use tracing::{debug, warn};
```

### 4. **Caching**
The adapter uses an in-memory cache for recently accessed checkpoints:
```rust
cache: Arc<tokio::sync::RwLock<HashMap<String, CachedCheckpoint>>>
```

### 5. **Batch Operations**
For bulk checkpoint management:
```rust
// Delete all checkpoints for a workflow instance
store.delete_instance_checkpoints("inst-001").await?;

// Prune to keep only N recent checkpoints
store.prune_old_checkpoints("inst-001", 5).await?;
```

## Testing

The adapter includes comprehensive tests covering:

```rust
#[tokio::test]
async fn test_create_checkpoint()

#[tokio::test]
async fn test_checkpoint_metadata()

#[tokio::test]
async fn test_export_import_checkpoint()

#[tokio::test]
async fn test_checkpoint_not_found()

#[tokio::test]
async fn test_config_builder()

#[test]
fn test_generate_checkpoint_id_deterministic()
```

Run tests:
```bash
cargo test -p osiris-compiler --features firestore --lib adapter::workflow_persistence
```

## API Reference

### WorkflowStore Methods

| Method | Purpose |
|--------|---------|
| `create_checkpoint` | Save workflow state snapshot |
| `restore_checkpoint` | Load complete checkpoint data |
| `get_checkpoint_metadata` | Load metadata only (lighter weight) |
| `find_latest_checkpoint` | Get most recent checkpoint for instance |
| `query_checkpoints` | Flexible search with filters |
| `delete_checkpoint` | Remove single checkpoint |
| `delete_instance_checkpoints` | Remove all checkpoints for instance |
| `recover_to_latest` | Recovery workflow (find + restore + replay) |
| `checkpoint_exists` | Check if checkpoint exists |
| `get_total_size` | Total storage used |
| `get_checkpoint_count` | Count checkpoints for instance |
| `prune_old_checkpoints` | Keep only N recent checkpoints |
| `export_checkpoint` | Export as JSON |
| `import_checkpoint` | Import from JSON |

## See Also

- **Domain Types**: `src/domain/workflow.rs` - Workflow and checkpoint structures
- **Port Trait**: `src/port/workflow_store.rs` - Full trait definition
- **Adapter Implementation**: `src/adapter/workflow_persistence.rs` - Firestore backend
- **Workflow Kernel**: `src/adapter/workflow_kernel.rs` - Workflow execution engine

## Changelog

### 2026-02-10
- Initial implementation of WorkflowStore port and FirestoreWorkflowStore adapter
- Support for checkpoint creation, restoration, and recovery
- Query filtering with CheckpointQuery
- Export/import for backup and migration
- Automatic pruning and caching
- Comprehensive test suite
