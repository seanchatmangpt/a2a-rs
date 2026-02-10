# Rust Implementer Memory

## Key Patterns

### CONSTRUCT System Architecture
The CONSTRUCT module (`src/construct/`) implements deterministic runtime execution:
- `runtime/` - Core execution engine (μ function)
- `ontology/` - State model using BTreeMap for determinism
- `receipts/` - Cryptographic proof chain
- `guards/` - Refusal determinism predicates
- `scheduler.rs` - Deterministic task ordering

### Replay Testing Pattern
For determinism verification, use record-replay pattern:
1. `ExecutionRecorder` - Captures state snapshots + receipts at each step
2. `ExecutionReplayer` - Replays operations from recorded state
3. `StateSnapshot` - Lightweight state representation using BTreeMap for deterministic serialization
4. Compare outputs bit-for-bit to verify determinism

### Domain Types Don't Derive PartialEq/Eq
`Task`, `Message`, `Artifact` only derive `Debug, Clone, Serialize, Deserialize` (per domain layer rules).
For comparison in tests:
- Serialize to JSON and compare strings (deterministic due to BTreeMap)
- Don't add `PartialEq`/`Eq` to structs containing domain types
- Use custom comparison logic when needed

### Scheduler API (runtime/scheduler.rs)
The scheduler is named `Scheduler`, not `DeterministicScheduler`:
- `submit(task)` - Add task to pending queue
- `next()` - Get next task (deterministic selection)
- `pending_tasks()` - Get pending task IDs in deterministic order
- Uses BTreeMap + stable sorting for replay consistency

### Receipt Chain Verification
With `receipts` feature enabled:
- `Receipt::new(observation, action, delta)` - Create cryptographic receipt
- `ReceiptChain` - Maintains linked chain with integrity verification
- `verify_integrity()` - Validates chain hasn't been tampered with
- Receipts form tamper-proof audit trail for all state transitions

### Observability System (construct/observability.rs)
Comprehensive tracing and metrics for CONSTRUCT runtime:
- `RuntimeMetrics` - Thread-safe atomic counters for all operations (guards, invariants, scheduler, runtime)
- `ObservabilityContext` - Correlation context with execution_id, policy_epoch, timing
- `InstrumentedGuard<G>` - Wrapper adding metrics/tracing to any Guard (requires `#[derive(Debug, Clone)]`)
- `InstrumentedInvariant<I>` - Wrapper adding metrics/tracing to any Invariant (requires `#[derive(Debug, Clone)]`)
- `MetricsSnapshot` - Point-in-time metrics with calculated rates (rejection rate, failure rate, completion rate)
- `OperationTiming` - Per-stage timing breakdown with slowest_stage() analysis
- All behind `tracing` feature flag for zero-cost when disabled
- Creates structured spans: runtime_execution, runtime_stage, guard_evaluation, invariant_check, scheduler_operation
- Preserves determinism: metrics are side-effect free, timing is observability-only

### Receipt Store (Persistent Storage)
With `sqlx-storage` + `receipts` features enabled:
- `ReceiptStore` - SQLx-based persistent receipt storage
- `append(receipt)` - Append receipt with sequence/prev_hash validation
- `get_chain()` - Retrieve entire chain from database
- `verify_chain()` - Verify integrity of stored chain
- `replay_from(seq)` - Replay operations from specific sequence
- Table: receipts (sequence, timestamp, observation_hash, action_hash, delta_hash, receipt_hash, previous_hash, signature, public_key, metadata)
- Supports SQLite/PostgreSQL/MySQL via SQLx
- Auto-runs migrations on creation

### SQLx Multi-Backend Pattern
For storage implementations supporting both Postgres and SQLite:
- Use conditional pool types: `#[cfg(feature = "postgres")] pool: PgPool` vs `#[cfg(feature = "sqlite")] pool: SqlitePool`
- Separate `row_to_task` methods with concrete row types (PgRow vs SqliteRow) to satisfy SQLx trait bounds
- Don't use `impl Row` - it doesn't satisfy ColumnIndex/Decode/Type trait bounds
- Postgres uses JSONB, SQLite uses TEXT for JSON storage
- Both use separate CREATE TABLE IF NOT EXISTS blocks with database-specific SQL

## Project Structure

### Workspace Layout
- `a2a-rs/` - Core library (hexagonal architecture)
- `a2a-agents/` - Example agent implementations
- `a2a-client/` - Web UI
- `a2a-ap2/` - Payments extension

### Hexagonal Architecture Layers
```
domain/ <- port/ <- adapter/ <- application/ <- services/
```
- Domain: Pure types, zero dependencies
- Port: Async trait definitions
- Adapter: Implementations (feature-gated)
- Application: JSON-RPC routing
- Services: High-level wrappers

### Test Organization
Tests live in module-specific directories:
- `src/construct/tests/` - CONSTRUCT module tests
  - `proptest.rs` - Property-based tests
  - `compliance.rs` - Spec compliance tests
  - `replay.rs` - Determinism verification (new)
- Always add new test files to `mod.rs` with `#[cfg(test)]`

## Common Issues

### BTreeMap vs HashMap
Always use `BTreeMap` for deterministic ordering in:
- State snapshots
- Any data structure used in replay/comparison
- Collections that need consistent serialization order

### Feature Gates
- Core types: No feature requirements
- Receipts: Requires `receipts` feature
- Signing: Requires `receipts-signing` feature
- Use `#[cfg(feature = "...")]` for conditional compilation

### Async Runtime Types
- Runtime module uses sync types (no async in domain)
- `ExecutionReceipt` is sync and Serialize/Deserialize
- Port traits use `#[async_trait]` for async operations

## Testing Workflows

### Running Tests
```bash
cargo test --all-features           # All tests
cargo test --all-features replay    # Just replay tests
cargo test -p a2a-rs                # Core library only
```

### Running Benchmarks
```bash
cargo bench --bench scheduler_bench           # Scheduler benchmarks
cargo bench --bench a2a_performance           # Core A2A benchmarks
cargo bench                                   # All benchmarks
```

### Benchmark Organization
Benchmarks live in `a2a-rs/benches/`:
- `a2a_performance.rs` - Core protocol operations (messages, tasks, serialization)
- `scheduler_bench.rs` - Scheduler performance and determinism cost analysis
- Each benchmark uses criterion with `harness = false` in Cargo.toml
- Use `black_box()` to prevent compiler optimizations
- Use `criterion::BatchSize::LargeInput` for expensive setup operations
- Group related benchmarks with `c.benchmark_group("group_name")`

### Benchmark Patterns
For scheduler benchmarks (`scheduler_bench.rs`):
- Test at multiple scales: 1k, 10k, 100k operations
- Compare BTreeMap vs HashMap to quantify determinism cost
- Use `Throughput::Elements` to measure ops/sec
- Test stable vs unstable sort to show sort overhead
- Use `iter_batched` with setup closure for complex scenarios
- Helper function `generate_tasks(count, num_stations)` creates test data

### Determinism Testing Checklist
1. Record execution with state snapshots
2. Replay on fresh runtime instance
3. Assert identical receipts (ignore timestamps)
4. Verify receipt chain integrity
5. Test scheduler produces same order regardless of insertion order

### Fuzz Testing
Located in `a2a-rs/fuzz/` using cargo-fuzz + libFuzzer:
- `fuzz_targets/station.rs` - Comprehensive station robustness testing
- Requires nightly Rust (has `rust-toolchain.toml`)
- Tests all station implementations for panic-freedom
- Validates guards properly reject invalid inputs with typed refusals
- Run: `cargo fuzz run station`
- Corpus in `corpus/station/`, crashes in `artifacts/station/`
- Key invariants: no panics, all errors return RefusalReceipt, state consistency

## ggen-sync (Code/Ontology Sync)

### Reverse Sync (Code → Ontology)
Located in `ggen-sync/src/reverse_sync.rs`:
- Takes `Vec<SyncDiff>` and `HashMap<String, CodeNode>` as input
- Generates RDF/Turtle triples for types that exist in code but not in ontology
- Appends to `ontology/a2a-generated.ttl` (80/20 approach - no reorganization)
- Maps Rust types to XSD types: String→"string", bool→"boolean", i32→"integer", etc.
- Handles Option<T> (required=false), Vec<T> (isArray=true), custom types (reference)
- Uses sophia 0.8 for RDF manipulation
- Function: `apply_reverse_sync(&[SyncDiff], &HashMap<String, CodeNode>, &Path)`

### Database Migrations (Schema Evolution)
Located in `ggen-sync/src/migrate.rs` - see [migrations.md](migrations.md) for detailed docs:
- Detects breaking changes from schema diffs
- Generates SQLx-compatible migration files for Postgres, MySQL, SQLite
- Supports up/down migrations with timestamps
- Key function: `apply_migrations(&[SyncDiff], &HashMap<String, OntologyNode>, DatabaseBackend, &Path)`
- Breaking changes: type removed, field removed, type changed, required field added
- Non-breaking: optional field added (Option<T>)

### Code Generation (ggen generate)
Located in `ggen-sync/src/generate.rs`:
- Integrates SPARQL CONSTRUCT queries with Tera template rendering
- Loads ggen.toml config with ontology sources, prefixes, rules
- Uses Oxigraph for RDF storage + SPARQL execution
- Executes CONSTRUCT queries to transform ontology into intermediate RDF graphs
- Applies Tera templates to generate Rust code from CONSTRUCT results
- Returns `GenerationResult` with list of generated files
- Key functions:
  - `load_config(path)` - Parse ggen.toml into GgenConfig
  - `generate(config_path)` - Full generation workflow
  - `execute_construct(store, query, rule_name)` - Run SPARQL CONSTRUCT
  - `graph_to_context(triples)` - Convert RDF graph to Tera context
- Dependencies: toml, tera, oxigraph, serde
- Important: Oxigraph's `load_from_reader` takes 2 args (format, reader), not 4
- CONSTRUCT returns Triple iterator (not Quad) via QueryResults::Graph

### Type Mapping Pattern
```rust
String → a2a:type "string"
bool → a2a:type "boolean"
i32/i64/u32/u64 → a2a:type "integer"
f32/f64 → a2a:type "number"
Option<T> → a2a:required false + recurse on T
Vec<T> → a2a:isArray true + recurse on T
CustomType → a2a:type "reference" + a2a:refEntity "CustomType"
```
