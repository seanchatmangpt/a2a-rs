---
name: test
description: Run tests for the a2a-rs workspace. Use when the user wants to run tests or verify code changes.
allowed-tools: Bash(cargo test *)
---

Run the test suite for this Rust workspace.

## Steps

1. Run `cargo test --workspace` to test all workspace members
2. If a specific crate is mentioned in $ARGUMENTS, run `cargo test -p <crate>`
3. If a specific test name is in $ARGUMENTS, run `cargo test -- $ARGUMENTS`
4. Report results clearly: pass count, fail count, any failures with context
5. If tests fail, read the failing test and relevant source to suggest fixes
