---
name: architect
description: Design architecture for new features following hexagonal architecture patterns
context: fork
agent: Plan
---

Design the architecture for $ARGUMENTS in this a2a-rs workspace.

## Constraints

- Follow hexagonal architecture: domain -> ports -> adapters -> application
- Domain types go in `a2a-rs/src/domain/`
- Port traits go in `a2a-rs/src/port/`
- Adapter implementations go in `a2a-rs/src/adapter/`
- Use feature flags for optional dependencies
- All public types must derive Serialize/Deserialize
- Use async-trait for async port definitions
- Use thiserror for error types
- Use bon for builder patterns

## Output

Provide a detailed implementation plan with:
1. New/modified files and their responsibilities
2. Trait definitions (ports)
3. Type definitions (domain)
4. Feature flag configuration
5. Test strategy
