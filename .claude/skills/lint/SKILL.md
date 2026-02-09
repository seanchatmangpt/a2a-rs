---
name: lint
description: Run clippy and format checks on the workspace
allowed-tools: Bash(cargo clippy *), Bash(cargo fmt *)
---

Run linting and formatting checks.

## Steps

1. Run `cargo fmt --all -- --check` to verify formatting
2. Run `cargo clippy -- -D warnings` to check for lint issues
3. If there are formatting issues, run `cargo fmt --all` to fix them
4. If there are clippy warnings, report them with file locations and suggest fixes
