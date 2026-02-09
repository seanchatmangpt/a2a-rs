# Q Invariant Verifier

## Overview

The Q invariant verifier implements a jidoka "stop-the-line" mechanism for the Osiris compiler. It verifies that state invariants (Q predicates) are preserved across state transitions, blocking commits that violate critical or error-level invariants.

## Architecture

Following hexagonal architecture:

```
domain/invariants.rs (Pure types)
         ↑
port/invariant_verifier.rs (Trait interface)
         ↑
adapter/q_invariant_verifier.rs (Implementation)
```

## Key Concepts

### Q Invariants

A **Q invariant** is a predicate over system state that must hold before and after any state transition. The verifier enforces the **preserve(Q)** property:

```
∀ state transitions (S₁ → S₂): Q(S₁) ∧ Q(S₂)
```

If `preserve(Q)` cannot be proven, the commit is **blocked** and a **refusal receipt** is emitted.

### Jidoka Mechanism

Inspired by the Toyota Production System, the jidoka ("automation with a human touch") principle means the system automatically stops when a defect is detected:

1. **Register invariants** with severity levels (Critical, Error, Warning)
2. **Before commit**: Verify all enabled invariants in pre-state and post-state
3. **If violation detected**: Block commit (Critical/Error) or warn (Warning)
4. **Emit refusal receipt**: Cryptographic proof of rejection

## Domain Types

### QInvariant

Represents an invariant with a predicate, severity, and enabled flag.

```rust
pub struct QInvariant {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub predicate: InvariantPredicate,
    pub severity: InvariantSeverity,
    pub enabled: bool,
}
```

### InvariantPredicate

Recursive predicate type supporting:

- **StateEquals**: Field must equal a specific value
- **StateComparison**: Field must satisfy comparison (Eq, Ne, Lt, Le, Gt, Ge, Contains, Matches)
- **And**: Conjunction of predicates (all must hold)
- **Or**: Disjunction of predicates (at least one must hold)
- **Not**: Negation of a predicate
- **Relational**: Relationship between two fields
- **TypeInvariant**: State must conform to a schema
- **Custom**: Custom expression (extension point)

Example:

```rust
// Balance must be non-negative
InvariantPredicate::StateComparison {
    field: "balance".to_string(),
    operator: ComparisonOperator::Ge,
    value: serde_json::json!(0),
}

// Status must be "active" OR "suspended"
InvariantPredicate::Or {
    predicates: vec![
        InvariantPredicate::StateEquals {
            field: "status".to_string(),
            expected: serde_json::json!("active"),
        },
        InvariantPredicate::StateEquals {
            field: "status".to_string(),
            expected: serde_json::json!("suspended"),
        },
    ],
}
```

### StateSnapshot

Represents system state at a point in time:

```rust
pub struct StateSnapshot {
    pub snapshot_id: String,
    pub state: HashMap<String, serde_json::Value>,
    pub timestamp: DateTime<Utc>, // or String
    pub metadata: HashMap<String, serde_json::Value>,
}
```

### Commit

Represents a state transition requiring verification:

```rust
pub struct Commit {
    pub commit_id: String,
    pub pre_state: StateSnapshot,
    pub post_state: StateSnapshot,
    pub description: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

### InvariantSeverity

Determines whether violations block commits:

```rust
pub enum InvariantSeverity {
    Critical,  // Blocks commit
    Error,     // Blocks commit
    Warning,   // Logs but allows commit
}
```

## Port Trait: InvariantVerifier

Async trait defining the verifier interface:

```rust
#[async_trait]
pub trait InvariantVerifier: Send + Sync {
    // Registration
    async fn register_invariant(&mut self, invariant: QInvariant) -> Result<()>;
    async fn unregister_invariant(&mut self, invariant_id: &str) -> Result<()>;
    async fn get_invariant(&self, invariant_id: &str) -> Result<QInvariant>;
    async fn list_invariants(&self) -> Vec<QInvariant>;

    // Verification
    async fn check_invariant(&self, invariant_id: &str, state: &StateSnapshot)
        -> Result<InvariantCheckResult>;

    async fn verify_preservation(
        &self,
        invariant_id: &str,
        pre_state: &StateSnapshot,
        post_state: &StateSnapshot,
    ) -> Result<PreservationResult>;

    async fn verify_commit(&self, commit: &Commit)
        -> Result<CommitVerificationResult>;

    // Blocking
    async fn block_commit(
        &self,
        commit: &Commit,
        verification_result: &CommitVerificationResult,
    ) -> Result<RefusalReceipt>;

    // Control
    async fn set_invariant_enabled(&mut self, invariant_id: &str, enabled: bool)
        -> Result<()>;
}
```

## Adapter: QInvariantVerifier

Thread-safe implementation using `Arc<RwLock<HashMap>>`:

```rust
pub struct QInvariantVerifier {
    invariants: Arc<RwLock<HashMap<String, QInvariant>>>,
}
```

### Key Features

1. **Recursive Predicate Evaluation**: Handles And/Or/Not combinators
2. **Type-Safe Comparisons**: JSON value comparison with type checking
3. **Dynamic Control**: Enable/disable invariants at runtime
4. **Refusal Receipts**: Integration with existing RefusalReceipt types

### Implementation Details

#### Predicate Evaluation

```rust
fn evaluate_predicate(
    &self,
    predicate: &InvariantPredicate,
    state: &StateSnapshot,
) -> Result<bool, InvariantVerificationError>
```

- Recursively evaluates predicates against state
- Returns `true` if predicate holds, `false` if violated
- Returns error if evaluation fails (missing fields, type mismatches)

#### Preservation Verification

```rust
async fn verify_preservation(
    &self,
    invariant_id: &str,
    pre_state: &StateSnapshot,
    post_state: &StateSnapshot,
) -> Result<PreservationResult>
```

Checks invariant in both states:

1. Evaluate invariant in pre-state → `pre_result`
2. Evaluate invariant in post-state → `post_result`
3. `preserved = (pre_result == Satisfied) ∧ (post_result == Satisfied)`

#### Commit Verification

```rust
async fn verify_commit(&self, commit: &Commit)
    -> Result<CommitVerificationResult>
```

Verifies all enabled invariants:

1. List all registered invariants
2. For each enabled invariant:
   - Call `verify_preservation()`
   - Record result
3. Aggregate results into `CommitVerificationResult`
4. Determine if commit should be allowed (no Critical/Error violations)

## Usage Example

```rust
use osiris_compiler::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create verifier
    let mut verifier = QInvariantVerifier::new();

    // Register invariant: Balance must be non-negative
    let balance_invariant = QInvariant {
        id: "inv-balance-nonnegative".to_string(),
        name: "Balance must be non-negative".to_string(),
        description: Some("Account balance cannot go below zero".to_string()),
        predicate: InvariantPredicate::StateComparison {
            field: "balance".to_string(),
            operator: ComparisonOperator::Ge,
            value: serde_json::json!(0),
        },
        severity: InvariantSeverity::Critical,
        enabled: true,
    };

    verifier.register_invariant(balance_invariant).await?;

    // Create a commit that would violate the invariant
    let commit = Commit {
        commit_id: "commit-overdraft".to_string(),
        pre_state: StateSnapshot {
            snapshot_id: "snap-pre".to_string(),
            state: [("balance".to_string(), serde_json::json!(100))].into(),
            timestamp: chrono::Utc::now(),
            metadata: HashMap::new(),
        },
        post_state: StateSnapshot {
            snapshot_id: "snap-post".to_string(),
            state: [("balance".to_string(), serde_json::json!(-50))].into(),
            timestamp: chrono::Utc::now(),
            metadata: HashMap::new(),
        },
        description: Some("Attempted overdraft".to_string()),
        metadata: HashMap::new(),
    };

    // Verify commit
    let result = verifier.verify_commit(&commit).await?;

    if result.is_blocked() {
        println!("Commit blocked: {:?}", result.blocking_violations);

        // Emit refusal receipt
        let receipt = verifier.block_commit(&commit, &result).await?;
        println!("Refusal receipt: {}", receipt.receipt_id);
    }

    Ok(())
}
```

## Refusal Receipt Integration

When a commit is blocked, a `RefusalReceipt` is emitted with:

```rust
RefusalReceipt {
    receipt_id: "rcpt-<timestamp>",
    packet_id: commit.commit_id,
    reason: RefusalReason::InvariantViolation {
        invariant_ids: vec!["inv-balance-nonnegative"],
        message: "Commit blocked due to 1 invariant violation(s)",
    },
    timestamp: Utc::now(),
    signature: None,
    context: {
        "commit_id": "commit-overdraft",
        "pre_state_id": "snap-pre",
        "post_state_id": "snap-post",
    },
}
```

## Testing

### Domain Tests

Located in `src/domain/invariants.rs`:

- `test_invariant_predicate_serialization`: JSON round-trip
- `test_preservation_result_blocking`: Severity-based blocking logic
- `test_commit_verification_result`: Aggregation and blocking detection

### Adapter Tests

Located in `src/adapter/q_invariant_verifier.rs`:

- `test_register_and_get_invariant`: Registration and retrieval
- `test_check_invariant_satisfied`: Invariant holds
- `test_check_invariant_violated`: Invariant violated
- `test_verify_preservation`: Preserve(Q) across transition
- `test_verify_commit_blocked`: Commit blocking
- `test_block_commit_with_receipt`: Refusal receipt emission

### Example

Run the comprehensive demo:

```bash
cargo run --example q_invariant_demo
```

This demonstrates:
1. Valid commits (allowed)
2. Balance invariant violations (blocked)
3. Status invariant violations (blocked)
4. Multiple simultaneous violations (blocked)
5. Disabling/enabling invariants dynamically

## Error Handling

The verifier uses `InvariantVerificationError`:

```rust
pub enum InvariantVerificationError {
    InvariantNotFound(String),
    InvalidStateSnapshot(String),
    PredicateEvaluationFailed(String),
    MissingStateField(String),
    TypeMismatch { expected: String, actual: String },
    CustomExpressionError(String),
    CommitBlocked { violation_count: usize },
    InternalError(String),
}
```

## Extension Points

### Custom Predicates

The `InvariantPredicate::Custom` variant allows arbitrary expressions:

```rust
InvariantPredicate::Custom {
    expression: "user.age >= 18 && user.verified == true".to_string(),
    parameters: HashMap::new(),
}
```

To implement:

1. Add expression evaluator (e.g., `rhai`, `lua`)
2. Parse expression in `evaluate_predicate()`
3. Evaluate against state snapshot

### Regex Matching

The `ComparisonOperator::Matches` variant supports regex:

```rust
InvariantPredicate::StateComparison {
    field: "email".to_string(),
    operator: ComparisonOperator::Matches,
    value: serde_json::json!(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"),
}
```

To implement:

1. Add `regex` crate dependency
2. Compile regex in `compare_values()`
3. Match against string values

## Performance Considerations

1. **Read-heavy workload**: Uses `RwLock` for concurrent reads
2. **Predicate evaluation**: Recursive but short-circuiting (And/Or)
3. **State snapshots**: Use `serde_json::Value` for flexibility (can optimize with typed schemas)
4. **Disabled invariants**: Skip evaluation entirely

## Best Practices

1. **Use specific error messages**: Help operators understand why commits are blocked
2. **Set appropriate severity**: Critical for safety properties, Warning for soft constraints
3. **Test invariants in isolation**: Verify each invariant independently
4. **Monitor blocking rate**: High block rate may indicate misconfigured invariants
5. **Version invariants**: Track changes to invariant definitions over time

## Future Enhancements

1. **Invariant dependencies**: Express relationships between invariants
2. **Conditional invariants**: Enable based on runtime conditions
3. **Audit trail**: Log all verification attempts and results
4. **Performance metrics**: Track evaluation time per invariant
5. **Schema validation**: Full JSON Schema support for TypeInvariant
6. **Distributed verification**: Coordinate invariants across multiple nodes

## References

- Jidoka: https://en.wikipedia.org/wiki/Autonomation
- Invariant verification: https://en.wikipedia.org/wiki/Invariant_(computer_science)
- Hexagonal architecture: https://alistair.cockburn.us/hexagonal-architecture/
