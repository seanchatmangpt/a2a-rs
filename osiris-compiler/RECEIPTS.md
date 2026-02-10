# Receipt Builder with KMS Signing

Implementation of cryptographic receipt system for proof chains in osiris-compiler.

## Overview

Receipts provide cryptographic proof of all operations in the system, forming an auditable proof chain where each receipt can reference prior receipts. The core invariant is:

```
hash(A) = hash(μ(O))
```

Where:
- `A` is the attestation
- `μ(O)` is the canonical representation of operation `O`
- Both are hashed using SHA-256

## Architecture

Following hexagonal architecture principles:

### 1. Domain (`src/domain/receipt.rs`)

Pure domain types with no external dependencies:

- **`Receipt`** - Core receipt type with:
  - Unique ID and timestamp
  - Operation ID and hash
  - Attestation hash (must equal operation hash)
  - Digital signature
  - Replay pointers (references to prior receipts)
  - Operation result (Success/Rejected/Pending)
  - Optional refusal information
  - Metadata

- **`ReplayPointer`** - References to prior receipts:
  - Receipt ID and hash
  - Dependency relation type (RequiresCompletion, Modifies, Supersedes, etc.)
  - Optional reason

- **`OperationResult`** - Result of operation execution:
  - Success (with output hash)
  - Rejected (with reason and code)
  - Pending (with expected completion time)

- **`RefusalInfo`** - Details about operation refusal:
  - Category (TypeNotInSigma, GuardViolation, SchemaViolation, etc.)
  - Reason and optional retry time
  - Policy ID that caused refusal
  - Additional context

- **`DependencyRelation`** - Types of causal relationships between operations

### 2. Ports (`src/port/receipt_builder.rs`)

Trait definitions for receipt building and storage:

- **`ReceiptBuilder`** - Main interface for building receipts:
  - `build_receipt()` - Creates receipt for successful operation
  - `build_refusal_receipt()` - Creates receipt for rejected operation
  - `verify_receipt()` - Verifies receipt signature and hash invariant
  - `compute_operation_hash()` - Computes canonical operation hash
  - `sign()` - Signs data using configured authority

- **`ReceiptStorage`** - Interface for storing/retrieving receipts:
  - `store_receipt()` - Persists a receipt
  - `get_receipt()` - Retrieves receipt by ID
  - `get_receipts_for_operation()` - Gets all receipts for an operation
  - `list_receipts()` - Lists receipts in time range

### 3. Adapters

#### Receipt Builder (`src/adapter/receipt_builder.rs`)

Standard implementation with pluggable signing:

- **`StandardReceiptBuilder`** - Main implementation:
  - Uses SHA-256 for hashing
  - Pluggable `Signer` trait for signatures
  - Validates hash invariant before returning receipts

- **`Signer`** trait - Interface for signing mechanisms:
  - `sign()` - Signs data
  - `verify()` - Verifies signature
  - `signer_id()` - Returns signer identifier

- **`LocalSigner`** - In-memory signing (development only):
  - Uses HMAC-SHA256 with key ID
  - Not suitable for production

#### KMS Signer (`src/adapter/kms_signer.rs`)

Production-grade signing using Google Cloud KMS:

- **`KmsSigner`** - Cloud KMS implementation:
  - Stores keys in hardware security modules (HSMs)
  - Keys never leave HSM
  - Configurable via environment variables or config struct
  - Feature-gated behind `kms` feature

- **`KmsConfig`** - Configuration:
  - GCP project, location, key ring, key name, key version
  - Optional service account key path

Environment variables:
```bash
GCP_PROJECT_ID=my-project
KMS_LOCATION=global
KMS_KEY_RING=my-keyring
KMS_KEY_NAME=receipt-signing-key
KMS_KEY_VERSION=1
GOOGLE_APPLICATION_CREDENTIALS=/path/to/key.json  # optional
```

#### Receipt Storage (`src/adapter/receipt_storage.rs`)

Storage implementations:

- **`InMemoryReceiptStorage`** - In-memory storage for testing
- **`CloudStorageReceiptStorage`** - Google Cloud Storage (feature-gated):
  - Stores receipts as JSON in GCS buckets
  - Path: `gs://{bucket}/{prefix}/receipts/{receipt_id}.json`
  - Feature-gated behind `storage` feature

## Usage

### Basic Example

```rust
use osiris_compiler::prelude::*;
use std::sync::Arc;

// Create signer
let signer = Arc::new(LocalSigner::new("my-key"));
let builder = StandardReceiptBuilder::new(signer);

// Create operation
let operation = Operation::new(
    OperationKind::Parse { input: "code".into() },
    1 // priority
);

// Build receipt
let receipt = builder.build_receipt(
    &operation,
    OperationResult::Success {
        output_hash: "abc123".to_string(),
        output: None,
    },
    vec![], // no replay pointers
    HashMap::new(),
).await?;

// Verify
builder.verify_receipt(&receipt).await?;
```

### With KMS Signing (Production)

```rust
#[cfg(feature = "kms")]
{
    use osiris_compiler::{KmsSigner, StandardReceiptBuilder};

    let kms_signer = KmsSigner::from_env().await?;
    let builder = StandardReceiptBuilder::new(Arc::new(kms_signer));

    // Use builder as normal
}
```

### With Replay Pointers

```rust
// First receipt
let receipt1 = builder.build_receipt(...).await?;

// Second receipt that depends on first
let replay_pointer = ReplayPointer {
    receipt_id: receipt1.id,
    receipt_hash: receipt1.compute_receipt_hash()?,
    relation: DependencyRelation::RequiresCompletion,
    reason: Some("Depends on prior operation".into()),
};

let receipt2 = builder.build_receipt(
    &operation2,
    result2,
    vec![replay_pointer],
    metadata,
).await?;
```

### Refusal Receipts

```rust
let refusal = RefusalInfo {
    category: RefusalCategory::GuardViolation,
    reason: "H-guard not satisfied".into(),
    retry_after: Some(Utc::now() + Duration::hours(1)),
    policy_id: Some("h-guard-policy".into()),
    context: HashMap::new(),
};

let refusal_receipt = builder.build_refusal_receipt(
    &operation,
    refusal,
    vec![],
    HashMap::new(),
).await?;
```

### Storage

```rust
// In-memory storage
let storage = InMemoryReceiptStorage::new();
storage.store_receipt(&receipt).await?;

let retrieved = storage.get_receipt(receipt.id).await?;
```

## Features

Enable features in `Cargo.toml`:

```toml
[dependencies]
osiris-compiler = { version = "0.1", features = ["kms", "storage"] }
```

- **`kms`** - Google Cloud KMS signing support
- **`storage`** - Google Cloud Storage persistence

## Security Considerations

### Development
- `LocalSigner` uses HMAC-SHA256 with key ID
- Keys stored in memory
- **NOT suitable for production**

### Production
- Use `KmsSigner` with Cloud KMS
- Keys stored in HSMs (FIPS 140-2 Level 3)
- Keys never leave HSM
- Audit logging via Cloud Logging
- IAM-based access control

## Testing

```bash
# Run all receipt tests
cargo test --lib receipt

# Run specific tests
cargo test --lib adapter::receipt_builder::tests
cargo test --lib adapter::receipt_storage::tests
cargo test --lib domain::receipt::tests

# Run example
cargo run --example receipt_demo
```

## Implementation Details

### Hash Computation

SHA-256 hashes are computed on canonical JSON representations:

```rust
let canonical = serde_json::to_string(&operation)?;
let mut hasher = Sha256::new();
hasher.update(canonical.as_bytes());
let hash = hasher.finalize();
format!("{:x}", hash) // hex-encoded
```

### Receipt Signing

Receipts are signed over:
- Receipt ID
- Timestamp
- Operation ID
- Operation hash
- Attestation hash
- Result

### Hash Invariant Validation

Every receipt validates `hash(A) = hash(μ(O))` before being returned:

```rust
receipt.validate_hash_invariant()?;
```

This ensures the core proof chain property.

## Future Enhancements

1. **Additional Signers**:
   - AWS KMS
   - Azure Key Vault
   - HSM via PKCS#11

2. **Storage Backends**:
   - AWS S3
   - Azure Blob Storage
   - Database backends (PostgreSQL, etc.)

3. **Indexing**:
   - Receipt indexing for efficient queries
   - Operation graph traversal
   - Proof chain verification

4. **Compression**:
   - Receipt compression for storage efficiency
   - Delta encoding for similar receipts

## Files Created

- `/home/user/a2a-rs/osiris-compiler/src/domain/receipt.rs` - Domain types
- `/home/user/a2a-rs/osiris-compiler/src/port/receipt_builder.rs` - Port traits
- `/home/user/a2a-rs/osiris-compiler/src/adapter/receipt_builder.rs` - Standard implementation
- `/home/user/a2a-rs/osiris-compiler/src/adapter/kms_signer.rs` - KMS signing
- `/home/user/a2a-rs/osiris-compiler/src/adapter/receipt_storage.rs` - Storage implementations
- `/home/user/a2a-rs/osiris-compiler/examples/receipt_demo.rs` - Complete demo

## Dependencies Added

```toml
sha2 = "0.10"       # SHA-256 hashing
base64 = "0.22"     # Base64 encoding

# Optional (KMS feature)
google-cloudkms1 = { version = "5.0", optional = true }
hyper = { version = "1.0", optional = true }
hyper-rustls = { version = "0.27", optional = true }
yup-oauth2 = { version = "11.0", optional = true }
```
