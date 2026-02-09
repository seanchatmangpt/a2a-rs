---
name: review
description: Review code changes for quality, correctness, and A2A protocol compliance
context: fork
agent: Explore
---

Review the current changes in this repository.

## Steps

1. Run `git diff` to see all unstaged changes
2. Run `git diff --cached` to see staged changes
3. For each changed file, evaluate:
   - Correctness: logic errors, edge cases, error handling
   - Rust idioms: proper use of Result/Option, ownership, lifetimes
   - A2A protocol compliance: message types, task states, JSON-RPC conformance
   - Security: no panics in library code, proper input validation
   - Tests: are changes covered by tests?
4. Provide a structured review with file-by-file findings
