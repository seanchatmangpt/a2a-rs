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

### Determinism Testing Checklist
1. Record execution with state snapshots
2. Replay on fresh runtime instance
3. Assert identical receipts (ignore timestamps)
4. Verify receipt chain integrity
5. Test scheduler produces same order regardless of insertion order
