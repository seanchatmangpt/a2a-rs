---
name: impl
description: Implement a feature following hexagonal architecture with CONSTRUCT-first, port-second design
disable-model-invocation: true
allowed-tools: Read, Write, Edit, Glob, Grep, Bash(cargo *), Bash(ggen *)
argument-hint: [feature-description]
---

Implement $ARGUMENTS following the hexagonal architecture of this codebase.

## Current feature flags
!`grep -A1 '^\[features\]' a2a-rs/Cargo.toml | head -20`

## Available ontology files
!`ls ggen/ontology/*.ttl`

## Architecture (CONSTRUCT-first, port-second, then adapter)

### Step 1: Domain types — decide CONSTRUCT vs hand-code

Determine whether the feature's domain types can be derived from the RDF ontology via ggen CONSTRUCT.

**If YES (ontology-derivable):** follow Steps 1a-1c.
**If NO (e.g., adapter-specific logic, integration glue, runtime state not modeled in ontology):** skip to Step 1d.

#### Step 1a: Add type to the ontology
Define the new type as an RDF class with properties in the appropriate `.ttl` file under `ggen/ontology/`. Use existing ontology files as reference for namespace conventions and property patterns.

#### Step 1b: Write a CONSTRUCT query
Create a SPARQL CONSTRUCT query in `ggen/queries/` that selects the type and its properties from the ontology graph. The query output shapes the generated Rust struct.

#### Step 1c: Create or update a Tera template and run ggen
- Add or update a Tera template in `ggen/templates/` that maps the CONSTRUCT result to Rust code (structs with derives, serde attributes, builder patterns).
- Run `ggen` to generate the domain types into `a2a-rs/src/domain/core/`.
- Generated types automatically get `Debug, Clone, Serialize, Deserialize` derives.
- Review the generated output for correctness before proceeding.

#### Step 1d: Hand-implement domain types (non-ontology path)
If the types cannot be CONSTRUCT-generated, add them manually to `a2a-rs/src/domain/core/`:
- Derive `Debug, Clone, Serialize, Deserialize`
- Use `#[serde(rename_all = "camelCase")]` for JSON field mapping
- Use `bon::Builder` for types with 3+ fields
- Add validation methods on the types themselves

### Step 2: Port trait
Define the interface in `a2a-rs/src/port/`:
- Use `#[async_trait]` for async methods
- Parameters and return types are domain types only (whether CONSTRUCT-generated or hand-coded)
- Use `Result<T, A2AError>` for fallible operations
- Export from `a2a-rs/src/port/mod.rs`

### Step 3: Adapter implementation (always hand-written)
Implement the port in `a2a-rs/src/adapter/`:
- Feature-gate with `#[cfg(feature = "...")]`
- Add new dependency to `Cargo.toml` as optional
- Map adapter errors to domain errors via `From`
- No `unwrap()` or `expect()`
- Adapter logic is never ontology-derived; it is always hand-implemented

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
