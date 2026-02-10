# CONSTRUCT Storage

Persistent storage adapters for CONSTRUCT components.

## Overview

This module provides database-backed storage implementations for receipts, artifacts, and other CONSTRUCT data structures. All storage implementations use SQLx and support multiple database backends (SQLite, PostgreSQL, MySQL).

## Receipt Store

The `ReceiptStore` provides persistent storage for receipt chains, enabling audit trails that survive restarts and support replay verification.

### Features

- **Persistent Chains**: Store receipt chains in a database for long-term retention
- **Integrity Verification**: Verify chain integrity directly from storage
- **Replay Support**: Reconstruct state from any point in the chain
- **Multi-Backend**: Supports SQLite, PostgreSQL, and MySQL via SQLx
- **Auto-Migration**: Automatically creates required tables on initialization

### Database Schema

```sql
CREATE TABLE receipts (
    sequence INTEGER PRIMARY KEY,
    timestamp TEXT NOT NULL,
    observation_hash TEXT NOT NULL,
    action_hash TEXT NOT NULL,
    delta_hash TEXT NOT NULL,
    receipt_hash TEXT NOT NULL,
    previous_hash TEXT,
    signature TEXT,
    public_key TEXT,
    metadata TEXT
);

CREATE INDEX idx_receipt_hash ON receipts(receipt_hash);
```

### Usage

```rust
use a2a_rs::construct::{ReceiptStore, ReceiptChain};

// Create a store from a database URL
let store = ReceiptStore::from_url("sqlite:receipts.db").await?;

// Create receipts
let mut chain = ReceiptChain::new();
chain.add_transition(
    b"observation",
    b"action",
    b"delta"
);

// Store receipts
for receipt in &chain.receipts {
    store.append(receipt).await?;
}

// Verify stored chain
store.verify_chain().await?;

// Retrieve entire chain
let stored_chain = store.get_chain().await?;

// Replay from a specific point
let replay_chain = store.replay_from(5).await?;
```

### Methods

#### `ReceiptStore::new(pool: SqlitePool) -> Self`
Creates a new receipt store with the given database pool.

#### `ReceiptStore::from_url(database_url: &str) -> Result<Self>`
Creates a new receipt store from a database URL and runs migrations.

#### `append(&self, receipt: &Receipt) -> Result<()>`
Appends a receipt to the store. Validates:
- Sequence number is next in chain
- Previous hash matches last stored receipt (if not genesis)

#### `get_receipt(&self, sequence: u64) -> Result<Receipt>`
Gets a single receipt by sequence number.

#### `get_chain(&self) -> Result<ReceiptChain>`
Retrieves the entire receipt chain from storage.

#### `get_chain_from(&self, start_sequence: u64) -> Result<ReceiptChain>`
Gets a partial chain starting from a specific sequence number.

#### `verify_chain(&self) -> Result<()>`
Verifies the integrity of the stored chain:
- All receipts have correct internal hashes
- All receipts link properly to predecessors
- Sequence numbers are consecutive
- Signatures are valid (if present)

#### `replay_from(&self, start_sequence: u64) -> Result<ReceiptChain>`
Replays operations from a specific sequence number. Returns a chain containing all receipts from the replay point onward. Useful for rebuilding state after a specific point in history.

#### `get_chain_length(&self) -> Result<u64>`
Returns the current length of the receipt chain.

#### `get_latest(&self) -> Result<Option<Receipt>>`
Gets the most recent receipt in the chain.

### Error Handling

All methods return `Result<T, ReceiptStoreError>` where errors can be:
- `Database(String)` - Database connection or query error
- `NotFound(u64)` - Receipt not found at sequence
- `VerificationFailed(String)` - Chain integrity check failed
- `SequenceMismatch { expected, actual }` - Sequence number mismatch
- `Serialization(String)` - JSON serialization error
- `InvalidReplayPoint(String)` - Invalid replay sequence

### Feature Requirements

```toml
# Cargo.toml
[dependencies]
a2a-rs = { version = "0.1", features = ["sqlx-storage", "receipts", "sqlite"] }
```

Features:
- `receipts` - Core receipt types
- `sqlx-storage` - SQLx storage adapter
- `sqlite` / `postgres` / `mysql` - Database backend

### Examples

See `examples/receipt_store_demo.rs`:

```bash
cargo run --example receipt_store_demo --features "sqlx-storage,receipts,sqlite"
```

## Testing

Tests are included in each module and use in-memory SQLite databases:

```bash
# Run all storage tests
cargo test --features "sqlx-storage,receipts,sqlite"

# Run just receipt store tests
cargo test --features "sqlx-storage,receipts,sqlite" receipt_store
```

## Architecture

Receipt storage is an **adapter** in the hexagonal architecture:
- **Domain**: `Receipt`, `ReceiptChain` (pure types)
- **Port**: Future `ReceiptStorage` trait
- **Adapter**: `ReceiptStore` (SQLx implementation)

The storage layer is feature-gated and optional - the core receipt types work without persistence.
