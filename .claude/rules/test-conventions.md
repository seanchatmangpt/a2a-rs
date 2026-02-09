---
paths:
  - "a2a-rs/tests/**/*.rs"
  - "**/*_test.rs"
  - "**/tests/*.rs"
---

# Test Conventions

- Use `proptest` for property-based tests on domain types
- Use `jsonschema` to validate against spec files in `spec/*.json`
- Integration tests go in `a2a-rs/tests/`
- Unit tests use `#[cfg(test)] mod tests` inside the source file
- Name tests descriptively: `test_message_serialization_roundtrip`
- Test error cases, not just happy paths
- Mock ports, not adapters - test through the trait interface
