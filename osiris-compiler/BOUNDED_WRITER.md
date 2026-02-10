# CONSTRUCT8 Bounded Writer

Implementation of bounded RDF state mutations using SPARQL CONSTRUCT semantics with a hard limit of 8 mutation units per commit.

## Overview

The CONSTRUCT8 bounded writer provides:

- **Bounded mutations**: Hard limit of ≤8 mutation units per commit
- **Atomic commits**: All-or-nothing transaction semantics
- **SPARQL CONSTRUCT semantics**: RDF-based state updates
- **Pluggable backends**: In-memory, Firestore, Spanner support
- **Size tracking**: Pre-commit mutation count validation

## Architecture

Following hexagonal architecture principles:

```
domain/          # Pure types (Patch, Triple, PatchSet)
  ├── patch.rs   # Bounded patch with validation
  └── triple.rs  # RDF triple types

port/            # Trait definitions
  └── bounded_writer.rs  # BoundedWriter trait

adapter/         # Implementations
  ├── in_memory_writer.rs   # In-memory implementation (testing)
  └── construct8_writer.rs  # Production writer with pluggable backend
```

## Usage

### Basic Example

```rust
use osiris_compiler::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create writer
    let writer = InMemoryWriter::new();
    
    // Create patch
    let mut patch = Patch::new();
    patch.add(Triple::new("subject", "predicate", "object"));
    
    // Validate before commit
    writer.validate_patch(&patch).await?;
    
    // Commit atomically
    let result = writer.commit_patch(patch).await?;
    println!("Committed {} additions", result.additions_count);
    
    Ok(())
}
```

### Validation Example

```rust
let mut patch = Patch::new();
for i in 0..9 {  // 9 triples exceeds limit
    patch.add(Triple::new(format!("s{}", i), "p", "o"));
}

match writer.validate_patch(&patch).await {
    Err(WriteError::ValidationFailed(PatchError::ExceedsLimit { actual, max })) => {
        println!("Patch exceeds limit: {} > {}", actual, max);
    }
    _ => {}
}
```

### Patch Set Example

```rust
let patch1 = Patch::with_additions(vec![
    Triple::new("s1", "p1", "o1"),
]);

let patch2 = Patch::with_additions(vec![
    Triple::new("s2", "p2", "o2"),
]);

// Commit multiple patches atomically
let patch_set = PatchSet::new(vec![patch1, patch2]);
writer.commit_patch_set(patch_set).await?;
```

## Domain Types

### Patch

Represents a bounded set of RDF triple mutations:

```rust
pub struct Patch {
    pub additions: Vec<Triple>,
    pub deletions: Vec<Triple>,
}

impl Patch {
    pub fn mutation_count(&self) -> usize;
    pub fn validate(&self) -> Result<(), PatchError>;
}
```

**Constraints:**
- Total mutations (additions + deletions) ≤ 8
- Cannot be empty
- Each triple addition/deletion = 1 mutation unit

### Triple

RDF triple (subject, predicate, object):

```rust
pub struct Triple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}
```

### PatchSet

Collection of patches committed atomically:

```rust
pub struct PatchSet {
    pub id: uuid::Uuid,
    pub patches: Vec<Patch>,
}

impl PatchSet {
    pub fn total_mutation_count(&self) -> usize;
    pub fn validate(&self) -> Result<(), PatchError>;
}
```

## Port Trait

### BoundedWriter

```rust
#[async_trait]
pub trait BoundedWriter: Send + Sync {
    async fn commit_patch(&self, patch: Patch) -> Result<CommitResult, WriteError>;
    async fn commit_patch_set(&self, patch_set: PatchSet) -> Result<CommitResult, WriteError>;
    async fn validate_patch(&self, patch: &Patch) -> Result<(), WriteError>;
    fn max_mutation_units(&self) -> usize;
}
```

## Adapters

### InMemoryWriter

In-memory implementation for testing and development:

```rust
let writer = InMemoryWriter::new();

// Query state
let count = writer.triple_count();
let history = writer.commit_history();

// Clear state
writer.clear();
```

**Use cases:**
- Unit tests
- Development/prototyping
- Single-node deployments without persistence

### Construct8Writer

Production writer with pluggable storage backend:

```rust
use osiris_compiler::adapter::{Construct8Writer, StorageBackend};

let backend = MyStorageBackend::new();  // Implement StorageBackend
let writer = Construct8Writer::new(backend);

let result = writer.commit_patch(patch).await?;
```

**Backend Integration:**

Implement `StorageBackend` and `Transaction` traits:

```rust
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn begin_transaction(&self) -> Result<Box<dyn Transaction>, WriteError>;
    fn backend_name(&self) -> &str;
}

#[async_trait]
pub trait Transaction: Send + Sync {
    async fn add_triples(&mut self, triples: &[Triple]) -> Result<(), WriteError>;
    async fn delete_triples(&mut self, triples: &[Triple]) -> Result<(), WriteError>;
    async fn commit(self: Box<Self>) -> Result<String, WriteError>;
    async fn rollback(self: Box<Self>) -> Result<(), WriteError>;
}
```

**Planned backends:**
- Firestore (feature: `firestore-backend`)
- Spanner (feature: `spanner-backend`)

## Error Handling

```rust
pub enum WriteError {
    ValidationFailed(PatchError),
    ConflictError { reason: String },
    StorageError { message: String },
    RollbackError { reason: String },
    TimeoutError,
}

pub enum PatchError {
    ExceedsLimit { actual: usize, max: usize },
    EmptyPatch,
    InvalidTriple { reason: String },
}
```

## CONSTRUCT Semantics

The writer follows SPARQL CONSTRUCT semantics:

1. **Delete before insert**: Deletions applied first
2. **Atomic execution**: All mutations succeed or all fail
3. **Pattern matching**: Triples matched exactly (subject, predicate, object)

## Testing

Run tests:

```bash
# All tests
cargo test -p osiris-compiler

# Bounded writer tests only
cargo test -p osiris-compiler -- bounded_writer
cargo test -p osiris-compiler -- in_memory_writer

# Domain logic tests
cargo test -p osiris-compiler -- domain::patch
```

## Examples

Run the bounded writer example:

```bash
cargo run -p osiris-compiler --example bounded_writer_example
```

## Integration with Firestore/Spanner

To integrate with Firestore or Spanner, implement the `StorageBackend` trait:

```rust
struct FirestoreBackend {
    client: FirestoreClient,
    collection: String,
}

#[async_trait]
impl StorageBackend for FirestoreBackend {
    async fn begin_transaction(&self) -> Result<Box<dyn Transaction>, WriteError> {
        let tx = self.client.begin_transaction().await?;
        Ok(Box::new(FirestoreTransaction { tx, collection: self.collection.clone() }))
    }
    
    fn backend_name(&self) -> &str {
        "Firestore"
    }
}

struct FirestoreTransaction {
    tx: firestore::Transaction,
    collection: String,
}

#[async_trait]
impl Transaction for FirestoreTransaction {
    async fn add_triples(&mut self, triples: &[Triple]) -> Result<(), WriteError> {
        for triple in triples {
            self.tx.add(&self.collection, triple)?;
        }
        Ok(())
    }
    
    async fn delete_triples(&mut self, triples: &[Triple]) -> Result<(), WriteError> {
        for triple in triples {
            self.tx.delete(&self.collection, triple)?;
        }
        Ok(())
    }
    
    async fn commit(self: Box<Self>) -> Result<String, WriteError> {
        let result = self.tx.commit().await?;
        Ok(result.transaction_id)
    }
    
    async fn rollback(self: Box<Self>) -> Result<(), WriteError> {
        self.tx.rollback().await?;
        Ok(())
    }
}
```

## Design Rationale

### Why 8 mutations?

The CONSTRUCT8 limit ensures:
- **Bounded resource usage**: Predictable memory/storage per commit
- **Conflict minimization**: Smaller atomic units reduce contention
- **Audit granularity**: Fine-grained change tracking
- **CONSTRUCT semantics**: Aligns with SPARQL CONSTRUCT patterns

### Why hexagonal architecture?

- **Testability**: Domain logic testable without backends
- **Flexibility**: Easy backend swapping (in-memory → Firestore → Spanner)
- **Clear boundaries**: Adapter implementations isolated from domain rules

## Reference

- [SPARQL 1.1 CONSTRUCT](https://www.w3.org/TR/sparql11-query/#construct)
- [RDF Triples](https://www.w3.org/TR/rdf-concepts/#section-triples)
- [Hexagonal Architecture](https://alistair.cockburn.us/hexagonal-architecture/)
