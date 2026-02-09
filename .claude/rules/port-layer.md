---
paths:
  - "a2a-rs/src/port/**/*.rs"
---

# Port Layer Rules

Ports define contracts. They depend only on domain types.

- Every port is an `#[async_trait]` trait
- No concrete implementations - only trait definitions
- Parameters and return types must be domain types
- Use `Result<T, DomainError>` for fallible operations
- No imports from `crate::adapter`, `crate::application`, or `crate::services`
- New features MUST define a port trait before any adapter implementation
- Keep traits focused: one responsibility per trait
