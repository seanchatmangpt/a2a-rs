---
name: ci
description: Run the full CI pipeline locally with live diagnostics
disable-model-invocation: true
allowed-tools: Bash(cargo *)
---

Run the exact CI pipeline from `.github/workflows/rust.yml`. Stop on first failure.

## Current project state
- Dirty files: !`git status --porcelain | wc -l | tr -d ' '`
- Last commit: !`git log --oneline -1`

## Pipeline (run sequentially, stop on first failure)

1. `cargo fmt --all -- --check`
   - If this fails, run `cargo fmt --all` to fix, then report what changed
2. `cargo clippy -- -D warnings`
   - Report each warning with file:line and the clippy lint name
3. `cargo build --verbose`
4. `cargo test --verbose`
5. `cargo doc --no-deps --all-features`

After all steps, output a summary table:

| Step | Result | Duration |
|------|--------|----------|

If any step failed, suggest the specific fix.
