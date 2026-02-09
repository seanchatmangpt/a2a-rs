---
paths:
  - "a2a-rs/src/domain/**/*.rs"
---

# Domain Layer Rules

This is the innermost layer. Zero external dependencies.

- No imports from `crate::adapter`, `crate::application`, or `crate::services`
- No `async` traits here - domain types are pure data
- Every public type: `#[derive(Debug, Clone, Serialize, Deserialize)]`
- Use `bon::Builder` for types with 3+ fields
- Validation logic belongs here as methods on domain types
- Error types use `thiserror` and live in `domain::error`
- JSON-RPC protocol types go in `domain::protocols`
