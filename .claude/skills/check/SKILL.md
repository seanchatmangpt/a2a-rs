---
name: check
description: Run the full CI pipeline locally (build, test, clippy, fmt, docs)
allowed-tools: Bash(cargo *)
---

Run the complete CI pipeline locally. This mirrors the GitHub Actions workflow.

## Steps (run sequentially, stop on first failure)

1. `cargo fmt --all -- --check`
2. `cargo clippy -- -D warnings`
3. `cargo build --verbose`
4. `cargo test --verbose`
5. `cargo doc --no-deps --all-features`

Report a clear pass/fail summary for each step.
