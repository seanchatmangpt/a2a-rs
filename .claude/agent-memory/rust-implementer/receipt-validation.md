# Cryptographic Receipt Validation System

## Implementation Notes

### Merkle Tree Proof Generation
- **Critical lesson**: Proof collection order matters! Proofs must be collected bottom-up (leaf to root)
- When recursing through the tree, add sibling hashes AFTER recursing into target subtree
- This ensures proof elements are ordered from leaf level upward to root
- Each proof element is a tuple `(hash, is_right_sibling)` to track positioning

### Ed25519 Key Generation
- `ed25519-dalek` v2.1 doesn't have `SigningKey::generate()`
- Use `SigningKey::from_bytes(&seed)` where seed is 32 random bytes from RNG
- `Signature::from_bytes()` returns `Signature` directly, not `Result`

### Bon Builder Conventions
- Don't use `#[builder(default)]` on `Option<T>` fields - `Option` implies default of `None`

### Module Organization
- Cryptographic features are feature-gated with `crypto` feature
- Dependencies: `sha2`, `ed25519-dalek`, `hex`
- All types implement `Debug, Clone, Serialize, Deserialize`
- Error handling via `thiserror`-based `ReceiptError` enum

### Architecture
- Receipt: Single cryptographic receipt with signature
- ReceiptChain: Linked receipts with hash pointers (blockchain-like)
- MerkleTree: Batch verification with O(log n) proofs
- ReplayValidator: Deterministic build verification

## Testing
- Tests require `crypto` feature and `rand` dev dependency
- Example programs demonstrate all functionality
- Use `cargo run -p a2a-rs --example receipt_demo --features crypto`
