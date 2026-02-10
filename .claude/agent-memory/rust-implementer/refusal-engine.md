# Refusal Engine Implementation

## Overview

Implemented a complete refusal engine for osiris-edge following hexagonal architecture. The engine generates cryptographic inadmissible-before receipts when packets violate admission control policies.

## Architecture

### Domain Layer (`domain/refusal.rs`)
- **RefusalReceipt**: Core type with SHA-256 cryptographic proof hash
- **RefusalReason**: Enum with four refusal categories:
  - `WipCapExceeded`: Work-in-progress limit reached
  - `AuthenticationFailed`: Auth errors (token invalid, expired, etc.)
  - `GuardFailed`: H-guard violations (inadmissible-before constraints)
  - `TypeCheckFailed`: Type not in Σ or schema violations
- **AuthErrorCode**: Enum of auth error codes (SCREAMING_SNAKE_CASE)
- **TypeCheckErrorCode**: Enum of type check error codes (SCREAMING_SNAKE_CASE)

### Port Layer (`port/refusal_engine.rs`)
- **RefusalEngine**: Async trait with methods for each refusal type
- Includes convenience method `refuse_from_wip_error` for WipError conversion
- All methods return `RefusalReceipt` with cryptographic proof

### Adapter Layer (`adapter/refusal_engine.rs`)
- **CryptoRefusalEngine**: Implementation using SHA-256 for proof hashes
- Automatically adds retry-after hints:
  - WIP capacity: PT30S (30 seconds)
  - Guard failures: PT60S (60 seconds)
  - Auth/type failures: None (client must fix)

## Key Design Decisions

1. **Cryptographic Proof**: SHA-256 hash of (packet_id || timestamp || reason || issuer)
2. **Verify Proof**: `RefusalReceipt::verify_proof()` method for tamper detection
3. **No Negotiation**: All refusals are inadmissible-before (no discretionary bypass)
4. **Structured Reasons**: Enum with specific error codes for programmatic handling
5. **Retry Hints**: ISO 8601 duration strings (PT30S, PT60S) for transient errors
6. **Issuer Identity**: Each receipt includes gateway issuer for non-repudiation

## Dependencies Added

- `chrono = { version = "0.4", features = ["serde"] }` - Timestamps
- `sha2 = "0.10"` - Cryptographic hashing

## Usage Example

```rust
use osiris_edge::{
    adapter::CryptoRefusalEngine,
    port::RefusalEngine,
    domain::{AuthErrorCode, TypeCheckErrorCode},
};

#[tokio::main]
async fn main() {
    let engine = CryptoRefusalEngine::new("gateway-1");

    // Refuse due to WIP capacity
    let receipt = engine.refuse_wip_exceeded("pkt-123", 10, 10).await;
    println!("Refused: {}", receipt.summary());
    assert!(receipt.verify_proof());

    // Refuse due to auth failure
    let receipt = engine.refuse_auth_failed(
        "pkt-456",
        AuthErrorCode::InvalidSignature,
        "JWT signature verification failed"
    ).await;

    // Refuse due to guard violation
    let receipt = engine.refuse_guard_failed(
        "pkt-789",
        "precondition-auth",
        "RequiresPrior(AuthToken)",
        "Must authenticate before submitting data"
    ).await;

    // Refuse due to type check failure
    let receipt = engine.refuse_type_check_failed(
        "pkt-999",
        TypeCheckErrorCode::TypeNotInSigma,
        "UnknownPacket",
        "Packet type not in closed type system Σ"
    ).await;
}
```

## Integration with WIP Gate

```rust
use osiris_edge::{
    adapter::{KanbanWipGate, CryptoRefusalEngine},
    port::{AsyncWipGate, RefusalEngine},
};

async fn handle_packet(
    packet_id: &str,
    wip_gate: &KanbanWipGate,
    refusal_engine: &CryptoRefusalEngine,
) -> Result<(), RefusalReceipt> {
    match wip_gate.try_acquire().await {
        Ok(_permit) => {
            // Process packet while holding permit
            Ok(())
        }
        Err(wip_error) => {
            // Generate refusal receipt
            let receipt = refusal_engine
                .refuse_from_wip_error(packet_id, &wip_error)
                .await;

            // Log and return receipt to client
            tracing::warn!(
                packet_id = %packet_id,
                receipt_id = %receipt.receipt_id,
                "Packet refused: {}",
                receipt.summary()
            );

            Err(receipt)
        }
    }
}
```

## Testing

All components have comprehensive unit tests:
- Domain types: Serialization, proof verification, tamper detection
- Port trait: Default implementations
- Adapter: All refusal types, retry-after hints, proof generation

Run tests: `cargo test -p osiris-edge --lib`

## Hexagonal Architecture Compliance

✅ Domain depends on nothing (pure types)
✅ Port depends only on domain
✅ Adapter implements port, uses external crates (sha2, chrono)
✅ No layer violations
✅ All public types derive Debug, Clone, Serialize, Deserialize
✅ JSON compatibility with camelCase/SCREAMING_SNAKE_CASE
