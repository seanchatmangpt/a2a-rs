# Rust Conventions

- Edition 2024, MSRV 1.85
- `cargo clippy -- -D warnings` must pass
- `cargo fmt --all -- --check` must pass
- No `unwrap()` or `expect()` in library code; use `?`
- All public types: `#[derive(Debug, Clone, Serialize, Deserialize)]`
- JSON compatibility: `#[serde(rename_all = "camelCase")]`
- Error types: `thiserror` enums in each layer
- Builders: `bon::Builder` for types with 3+ fields
- Async traits: `#[async_trait]`
- Feature-gate optional deps: `#[cfg(feature = "...")]`
- Tests: `proptest` for properties, `jsonschema` for spec compliance
