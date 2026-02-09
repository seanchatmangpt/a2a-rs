# Rust Conventions

- Edition 2024, MSRV 1.85
- `cargo clippy -- -D warnings` must pass (warnings are errors)
- `cargo fmt --all -- --check` must pass
- No `unwrap()` or `expect()` in library code; use `?` operator
- All public types: `#[derive(Debug, Clone, Serialize, Deserialize)]`
- Use `thiserror` for error enums, `bon` for builders
- Feature-gate optional dependencies in Cargo.toml
- Async traits use `#[async_trait]`
