# Rust Implementer Agent Memory

## Quick Links
- [SSE Streaming](./sse-streaming.md) - SSE resumable streaming patterns (2026-02-09)

## Recent Work

### Deterministic Orderer (osiris-compiler, 2026-02-09)
Implemented Λ (lambda) deterministic orderer for compiler operations:
- Law-based resolution (priority → timestamp → UUID tiebreaker)
- Total order guarantee with repeatability across runs
- Files: `osiris-compiler/src/port/orderer.rs`, `adapter/lambda_orderer.rs`, `domain/operation.rs`
- All 10 tests passing (7 adapter + 3 domain)
- See details below in "Deterministic Ordering Pattern"

### SSE Resumable Streaming (a2a-mcp, 2026-02-09)
Implemented SSE manager with MCP-compliant resumability:
- Event IDs, Last-Event-ID support, redelivery window, broadcast + replay pattern
- Files: `a2a-mcp/src/adapter/sse_manager.rs`, integration in `server.rs`
- Key learnings: tokio-stream `sync` feature, a2a-rs public API usage, Axum error handling
- See [sse-streaming.md](./sse-streaming.md) for full details

## Key Patterns

### Hexagonal Architecture Implementation

When implementing new features in a2a-rs workspace:

1. **Domain first**: Create pure types with validation in `domain/`
2. **Port traits**: Define async traits with `#[async_trait]` in `port/`
3. **Adapter implementations**: Concrete implementations in `adapter/`
4. **Module exports**: Update `mod.rs` files to export new types/traits
5. **Lib.rs integration**: Add public re-exports and prelude module

### Deterministic Ordering Pattern (Λ-Laws)

Implemented in osiris-compiler for compiler operation ordering:

**Core Principle**: Establish total order through law-based resolution, not negotiation

**Ordering Laws**:
```rust
A < B ⟺ priority(A) > priority(B)  ∨
        (priority(A) = priority(B) ∧ timestamp(A) < timestamp(B))  ∨
        (priority(A) = priority(B) ∧ timestamp(A) = timestamp(B) ∧ id(A) < id(B))
```

**Implementation**:
- Domain type `Operation` with `Ord` impl (priority, timestamp, UUID)
- Port trait `DeterministicOrderer` with `order()`, `validate()`, `conflicts()`
- Adapter `LambdaOrderer` with configurable validation and conflict detection
- Rust's stable `sort()` ensures deterministic results

**Guarantees**:
1. Determinism: same inputs → same output order
2. Totality: all operation pairs comparable
3. Transitivity: A < B ∧ B < C → A < C
4. Repeatability: same across runs and systems

**Key files**:
- `domain/operation.rs` - Operation type with Ord implementation
- `domain/error.rs` - OrderingError types
- `port/orderer.rs` - DeterministicOrderer trait
- `adapter/lambda_orderer.rs` - LambdaOrderer implementation with config

**Testing**: Property-based tests for ordering stability, priority/timestamp precedence

### CONSTRUCT8 Bounded Writer Pattern

Successfully implemented bounded state mutations with:

- Domain validation before backend execution (fail fast)
- `MAX_MUTATION_UNITS` constant (8) enforced at type level
- Pluggable backend via `StorageBackend` + `Transaction` traits
- In-memory implementation for testing, production backends via traits
- Atomic commits with explicit rollback on error

**Key files:**
- `domain/patch.rs` - Patch, PatchSet, validation logic
- `domain/triple.rs` - RDF triple types
- `port/bounded_writer.rs` - BoundedWriter trait with CommitResult
- `adapter/in_memory_writer.rs` - Testing implementation
- `adapter/construct8_writer.rs` - Production writer with pluggable backend

### Testing Strategy

- Domain validation tests in domain module (unit tests)
- Adapter tests with mock backends (integration-style)
- Run specific test modules: `cargo test -- module::path`
- Use `#[tokio::test]` for async tests

### Common Issues

- **Cargo.toml features**: Some dependencies (e.g., firestore) define their own feature flags that override expected values, causing `unexpected_cfgs` warnings. These are warnings only and don't affect build success.
- **Workspace members**: Always verify member is in workspace `Cargo.toml` before building
- **Module exports**: Must update all `mod.rs` files in the chain (domain → port → lib)

## CONSTRUCT Semantics

- Delete before insert (SPARQL CONSTRUCT order)
- Atomic execution (all-or-nothing)
- Mutation count = additions.len() + deletions.len()
- Each triple = 1 mutation unit

## Next Steps

When implementing Firestore/Spanner backends:
- Implement `StorageBackend` trait
- Implement `Transaction` trait with actual client
- Feature-gate with `firestore-backend` or `spanner-backend`
- Test atomicity with real transactions
