# Protocol Method Stations

This module provides typed Station implementations for all A2A Protocol v0.3.0 methods.

## Overview

The `methods` module demonstrates the **Protocol Realization Theorem**: every A2A method specified in the protocol has a corresponding typed Station implementation with guards and typed input/output.

## Two Complementary Abstractions

### 1. Method Signatures (`methods/mod.rs`)

Lightweight trait-based abstraction showing protocol coverage:

```rust
pub trait Station {
    const METHOD_NAME: &'static str;
    type Input: Clone + Serialize + Deserialize;
    type Output: Clone + Serialize + Deserialize;

    fn validate(input: &Self::Input) -> Result<(), RefusalReceipt>;
    fn description() -> &'static str;
}
```

**Purpose:** Compile-time proof that all protocol methods are defined with typed signatures.

**Use Case:** Type checking, protocol coverage verification, documentation generation.

### 2. Ontology-Based Stations (`station.rs`)

Full stateful Station implementations with ontology integration:

```rust
pub trait Station {
    type Input: DeserializeOwned;
    type Output;

    fn admit(ontology: &Ontology, input: &Self::Input) -> Result<(), RefusalReceipt>;
    fn step(&mut self, ontology: &mut Ontology, input: Self::Input)
        -> Result<Self::Output, RefusalReceipt>;
}
```

**Purpose:** Runtime execution of protocol methods with deterministic state transitions.

**Use Case:** Actual request processing, state management, refusal control.

## Complete Coverage

Both abstractions provide 100% coverage of A2A v0.3.0:

| Method | Signature Station | Ontology Station | Status |
|--------|------------------|------------------|--------|
| message/send | SendMessageStation | SendMessageStation | ✅ |
| message/stream | SendStreamingMessageStation | SendStreamingMessageStation | ✅ |
| tasks/get | GetTaskStation | GetTaskStation | ✅ |
| tasks/list | ListTasksStation | ListTasksStation | ✅ |
| tasks/cancel | CancelTaskStation | CancelTaskStation | ✅ |
| tasks/resubscribe | TaskResubscribeStation | TaskResubscribeStation | ✅ |
| tasks/pushNotificationConfig/set | SetPushNotificationConfigStation | SetPushNotificationConfigStation | ✅ |
| tasks/pushNotificationConfig/get | GetPushNotificationConfigStation | GetPushNotificationConfigStation | ✅ |
| tasks/pushNotificationConfig/list | ListPushNotificationConfigsStation | ListPushNotificationConfigsStation | ✅ |
| tasks/pushNotificationConfig/delete | DeletePushNotificationConfigStation | DeletePushNotificationConfigStation | ✅ |
| agent/getAuthenticatedExtendedCard | GetAuthenticatedExtendedCardStation | GetAuthenticatedExtendedCardStation | ✅ |

**Total: 11/11 methods (100% coverage)**

## Documentation

- **Coverage Analysis:** [`COVERAGE.md`](./COVERAGE.md) - Full proof of protocol completeness
- **Implementation:** [`../station.rs`](../station.rs) - Runtime station implementations
- **Method Signatures:** [`mod.rs`](./mod.rs) - Type-level protocol coverage

## Protocol Realization Theorem

**Claim:** For every method M in the A2A specification, there exists a typed Station S.

**Proof:** The coverage checklist in mod.rs demonstrates bijection between spec methods and Station types through:

1. Exhaustive enumeration of all 11 spec methods
2. Corresponding Station struct for each method
3. Typed Input/Output for each station
4. Guard implementations for precondition checking
5. Tests verifying the bijection

See `coverage_proof` module in mod.rs for executable proof.

## Usage

### Type-Level Validation

```rust
use a2a_rs::construct::methods::{Station, SendMessageStation, StationRegistry};

// Check if method is supported
assert!(StationRegistry::is_supported("message/send"));

// Get method description
let desc = StationRegistry::description("message/send").unwrap();
println!("{}", desc);

// Validate input at type level
let params = MessageSendParams { /* ... */ };
SendMessageStation::validate(&params)?;
```

### Runtime Execution

```rust
use a2a_rs::construct::station::{Station, SendMessageStation};
use a2a_rs::construct::ontology::OntologyState;

let mut ontology = OntologyState::new();
let mut station = SendMessageStation;

// Admission control (pure function)
SendMessageStation::admit(&ontology, &request)?;

// State transition (deterministic)
let response = station.step(&mut ontology, request)?;
```

## Design Principles

1. **Type Safety:** All boundaries use concrete types, no `serde_json::Value`
2. **Refusal Determinism:** All failures return typed `RefusalReceipt`
3. **Separation of Concerns:** Validation (`validate`/`admit`) separate from execution (`step`)
4. **Protocol Completeness:** Exhaustive coverage of all A2A methods
5. **Testability:** Guards are pure functions, stations are deterministic

## Files

- `mod.rs` - Method signature trait and Station type definitions
- `COVERAGE.md` - Complete coverage analysis with line numbers
- `README.md` - This file

## Relationship to Other Modules

```
spec/*.json (A2A Protocol)
    │
    ├─> construct/types.rs (Typed packets)
    │       └─> construct/methods/mod.rs (Signature validation)
    │
    └─> construct/station.rs (Runtime stations)
            └─> construct/ontology/ (State management)
```

## Testing

```bash
# Run method signature tests
cargo test -p a2a-rs --lib construct::methods

# Run station implementation tests
cargo test -p a2a-rs --lib construct::station
```

## Contributing

When adding a new A2A protocol method:

1. Add to spec/*.json
2. Add typed request/response to construct/types.rs
3. Add method Station to construct/methods/mod.rs
4. Add runtime Station implementation to construct/station.rs
5. Register in StationRegistry
6. Add tests
7. Update COVERAGE.md

## References

- A2A Protocol Specification: `/home/user/a2a-rs/spec/`
- CONSTRUCT.md: `/home/user/a2a-rs/CONSTRUCT.md`
- Protocol Coverage: `/home/user/a2a-rs/a2a-rs/src/construct/coverage.md`
