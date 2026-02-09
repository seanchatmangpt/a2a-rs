---
paths:
  - "a2a-rs/src/adapter/**/*.rs"
---

# Adapter Layer Rules

Adapters implement ports. They can use external crates.

- Every adapter MUST implement a port trait from `crate::port`
- Feature-gate adapters behind Cargo feature flags
- Use `#[cfg(feature = "...")]` on modules and impls
- External crate imports are fine here (axum, reqwest, sqlx, etc.)
- No `unwrap()` or `expect()` - propagate errors with `?`
- Adapter errors should map to domain errors via `From` impls
- Transport adapters (HTTP, WS) go in `adapter::transport`
- Auth adapters go in `adapter::auth`
- Storage adapters go in `adapter::storage`
