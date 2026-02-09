---
name: impl
description: Implement a feature following hexagonal architecture with port-first design
disable-model-invocation: true
allowed-tools: Read, Write, Edit, Glob, Grep, Bash(cargo *)
argument-hint: [feature-description]
---

Implement $ARGUMENTS following the hexagonal architecture of this codebase.

## Current feature flags
!`grep -A1 '^\[features\]' a2a-rs/Cargo.toml | head -20`

## Architecture (port-first, then adapter)

### Step 1: Domain types
If the feature needs new types, add them to `a2a-rs/src/domain/core/`.
- Derive `Debug, Clone, Serialize, Deserialize`
- Use `#[serde(rename_all = "camelCase")]` for JSON field mapping
- Use `bon::Builder` for types with 3+ fields
- Add validation methods on the types themselves

### Step 2: Port trait
Define the interface in `a2a-rs/src/port/`:
- Use `#[async_trait]` for async methods
- Parameters and return types are domain types only
- Use `Result<T, A2AError>` for fallible operations
- Export from `a2a-rs/src/port/mod.rs`

### Step 3: Adapter implementation
Implement the port in `a2a-rs/src/adapter/`:
- Feature-gate with `#[cfg(feature = "...")]`
- Add new dependency to `Cargo.toml` as optional
- Map adapter errors to domain errors via `From`
- No `unwrap()` or `expect()`

### Step 4: Wire up
- Re-export public types from `a2a-rs/src/lib.rs`
- Add to application layer if it needs JSON-RPC routing

### Step 5: Verify
- `cargo check --all-features`
- `cargo clippy -- -D warnings`
- `cargo test --workspace`

For each file you create or modify, reference the supporting docs:
- [Architecture rules](../rules/architecture.md)
- [Rust conventions](../rules/rust-conventions.md)
