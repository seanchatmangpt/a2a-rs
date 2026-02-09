---
name: trace-issue
description: Trace a bug or issue through the hexagonal layers from symptom to root cause
context: fork
agent: Explore
argument-hint: [error-message-or-symptom]
---

Trace the issue "$ARGUMENTS" through the codebase layers.

## Current state
- Recent changes: !`git log --oneline -10`
- Failing tests: !`cargo test --workspace 2>&1 | grep "FAILED\|error\[" | head -10`

## Investigation process

1. **Identify the symptom layer**: Is this a transport error (adapter), a protocol error (domain), a routing error (application), or a trait mismatch (port)?

2. **Trace the call chain**: Starting from the entry point (HTTP handler, WS handler, or client call), follow the code path through:
   - `adapter/transport/` - request parsing, response formatting
   - `application/handlers/` - JSON-RPC routing and dispatch
   - `port/` - trait method being called
   - `adapter/business/` or `adapter/storage/` - implementation
   - `domain/` - type validation, state transitions

3. **Check the spec**: Read the relevant `spec/*.json` to verify the expected behavior

4. **Identify root cause**: Pinpoint the exact file:line where behavior diverges from expected

5. **Report**: Provide the full trace with:
   - Entry point (file:line)
   - Each layer transition
   - Root cause (file:line)
   - Suggested fix with code snippet
