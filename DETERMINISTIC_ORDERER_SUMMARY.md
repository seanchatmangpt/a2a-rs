# Deterministic Orderer Implementation

## Overview

Successfully implemented a Λ (lambda) deterministic orderer in `osiris-compiler` following hexagonal architecture principles. The orderer establishes total order over compiler operations through law-based resolution rather than negotiation.

## Architecture

### 1. Domain Layer (`domain/operation.rs`)

**Operation Type** - Core domain type representing an admissible compiler operation:

```rust
pub struct Operation {
    pub id: Uuid,              // Unique identifier
    pub timestamp: DateTime<Utc>,  // Creation time
    pub priority: u32,         // Priority level (higher = more important)
    pub kind: OperationKind,   // Operation payload
    pub source: Option<String>, // Source identifier for conflict resolution
}
```

**OperationKind** - Types of compiler operations:
- `Parse { input: String }`
- `TypeCheck { module_id: String }`
- `Optimize { ir_id: String, level: u8 }`
- `CodeGen { target: String }`
- `Link { modules: Vec<String> }`

**Ordering Implementation** - `Ord` trait establishes deterministic total order:

```rust
impl Ord for Operation {
    fn cmp(&self, other: &Self) -> Ordering {
        // 1. Higher priority comes first
        match other.priority.cmp(&self.priority) {
            Ordering::Equal => {
                // 2. Earlier timestamp comes first
                match self.timestamp.cmp(&other.timestamp) {
                    Ordering::Equal => {
                        // 3. UUID as stable tiebreaker
                        self.id.cmp(&other.id)
                    }
                    ord => ord,
                }
            }
            ord => ord,
        }
    }
}
```

**Error Types** (`domain/error.rs`):
- `OrderingError::InvalidOperation` - Operation validation failure
- `OrderingError::CircularDependency` - Cycle detected
- `OrderingError::Conflict` - Unresolvable operation conflicts
- `OrderingError::OrderingFailed` - Generic ordering failure

### 2. Port Layer (`port/orderer.rs`)

**DeterministicOrderer Trait** - Contract for deterministic operation ordering:

```rust
pub trait DeterministicOrderer {
    /// Order operations deterministically
    fn order(&self, operations: Vec<Operation>) -> Result<Vec<Operation>, OrderingError>;
    
    /// Validate operation admissibility
    fn validate(&self, operation: &Operation) -> Result<(), OrderingError>;
    
    /// Check if two operations conflict
    fn conflicts(&self, a: &Operation, b: &Operation) -> bool;
}
```

**Guarantees**:
1. **Determinism**: `order(ops)` always returns the same sequence
2. **Completeness**: All input operations appear in output
3. **Uniqueness**: No operation appears more than once
4. **Repeatability**: Order is reproducible across runs and systems

**Optional Async Variant** (feature-gated):

```rust
#[cfg(feature = "async")]
#[async_trait]
pub trait AsyncDeterministicOrderer {
    async fn order(&self, operations: Vec<Operation>) -> Result<Vec<Operation>, OrderingError>;
    async fn validate(&self, operation: &Operation) -> Result<(), OrderingError>;
    async fn conflicts(&self, a: &Operation, b: &Operation) -> bool;
}
```

### 3. Adapter Layer (`adapter/lambda_orderer.rs`)

**LambdaOrderer** - Concrete implementation using Λ-laws:

```rust
pub struct LambdaOrderer {
    config: LambdaOrdererConfig,
}

pub struct LambdaOrdererConfig {
    pub max_priority: u32,        // Maximum allowed priority (validation)
    pub strict_sources: bool,     // Enforce source identifier requirement
    pub detect_conflicts: bool,   // Enable conflict detection
}
```

**Implementation Strategy**:

1. **Validation** - Check all operations before ordering:
   - Priority within bounds (`<= max_priority`)
   - Source identifier present (if `strict_sources` enabled)

2. **Cycle Detection** - Detect circular dependencies:
   - Check for duplicate source identifiers
   - Placeholder for dependency graph analysis

3. **Conflict Detection** - Identify unresolvable conflicts:
   - Same operation type with same target
   - Example: Two `TypeCheck` operations on same module

4. **Deterministic Ordering** - Apply Rust's stable `sort()`:
   - Uses `Operation::Ord` implementation
   - Guarantees stable, deterministic results

**Law-Based Resolution** (Λ-Laws):

```
For operations A and B:

A < B ⟺ priority(A) > priority(B)  ∨
        (priority(A) = priority(B) ∧ timestamp(A) < timestamp(B))  ∨
        (priority(A) = priority(B) ∧ timestamp(A) = timestamp(B) ∧ id(A) < id(B))
```

**Properties**:
- **No negotiation** - Operations don't negotiate their order
- **No randomness** - Order is fully deterministic
- **No external state** - Order depends only on operation properties
- **Repeatability** - Same inputs always produce same outputs

## Test Coverage

All tests passing (10 total):

### Domain Tests (3)
- `test_operation_ordering_by_priority` - Higher priority operations come first
- `test_operation_ordering_by_timestamp` - Earlier timestamps come first (same priority)
- `test_operation_ordering_stability` - Multiple sorts produce identical results

### Adapter Tests (7)
- `test_deterministic_ordering` - Same input produces same output
- `test_priority_ordering` - Priority-based precedence
- `test_timestamp_ordering` - Timestamp-based ordering (same priority)
- `test_validation_max_priority` - Rejects operations exceeding max priority
- `test_validation_strict_sources` - Enforces source identifier requirement
- `test_conflict_detection` - Detects conflicting operations
- `test_repeatability` - Multiple invocations produce identical results

## Usage Example

```rust
use osiris_compiler::{
    adapter::LambdaOrderer,
    domain::{Operation, OperationKind},
    port::DeterministicOrderer,
};

// Create orderer with default configuration
let orderer = LambdaOrderer::default();

// Create operations
let operations = vec![
    Operation::new(OperationKind::Parse { input: "main.rs".into() }, 1),
    Operation::new(OperationKind::TypeCheck { module_id: "main".into() }, 2),
    Operation::new(OperationKind::CodeGen { target: "x86_64".into() }, 1),
];

// Order deterministically
let ordered = orderer.order(operations)?;

// TypeCheck (priority 2) comes before Parse and CodeGen (priority 1)
assert_eq!(ordered[0].priority, 2);
```

## File Structure

```
osiris-compiler/
├── src/
│   ├── domain/
│   │   ├── operation.rs      # Operation type with Ord implementation
│   │   ├── error.rs          # OrderingError types
│   │   └── mod.rs            # Domain module exports
│   ├── port/
│   │   ├── orderer.rs        # DeterministicOrderer trait
│   │   └── mod.rs            # Port module exports
│   ├── adapter/
│   │   ├── lambda_orderer.rs # LambdaOrderer implementation
│   │   └── mod.rs            # Adapter module exports
│   └── lib.rs                # Public API and prelude
└── Cargo.toml                # Package manifest
```

## Integration

The orderer is fully integrated into the osiris-compiler public API:

```rust
// In lib.rs
pub use domain::{Operation, OperationKind, OrderingError, ...};
pub use port::{DeterministicOrderer, ...};
pub use adapter::{LambdaOrderer, LambdaOrdererConfig, ...};

pub mod prelude {
    pub use crate::adapter::{LambdaOrderer, LambdaOrdererConfig, ...};
    pub use crate::domain::{Operation, OperationKind, OrderingError, ...};
    pub use crate::port::{DeterministicOrderer, ...};
}
```

## Key Design Decisions

1. **Law-based rather than negotiation-based** - Operations follow deterministic rules, not runtime negotiation, ensuring repeatability

2. **Three-tier ordering** - Priority → Timestamp → UUID provides complete determinism without external state

3. **Stable sort** - Rust's `sort()` is stable, guaranteeing identical results for identical inputs

4. **Configurable validation** - `LambdaOrdererConfig` allows tuning strictness without changing implementation

5. **Conflict detection is optional** - Enabled via configuration for use cases that need it

6. **Async variant feature-gated** - `AsyncDeterministicOrderer` available for async contexts without forcing async on all users

## Future Enhancements

Potential areas for expansion:

1. **Dependency graph analysis** - Full cycle detection based on operation dependencies
2. **Custom ordering strategies** - Additional orderer implementations (e.g., topological sort)
3. **Operation batching** - Group operations by priority or type
4. **Parallel execution planning** - Identify operations that can run concurrently
5. **Ordering visualization** - Debug output showing ordering decisions

## References

- Port trait: `/home/user/a2a-rs/osiris-compiler/src/port/orderer.rs`
- Adapter implementation: `/home/user/a2a-rs/osiris-compiler/src/adapter/lambda_orderer.rs`
- Domain types: `/home/user/a2a-rs/osiris-compiler/src/domain/operation.rs`
- Error types: `/home/user/a2a-rs/osiris-compiler/src/domain/error.rs`

## Running Tests

```bash
# Run all orderer tests
cargo test -p osiris-compiler lambda_orderer --lib

# Run domain operation tests
cargo test -p osiris-compiler domain::operation::tests --lib

# Run all tests
cargo test -p osiris-compiler --lib
```

All 10 deterministic orderer tests pass successfully.
