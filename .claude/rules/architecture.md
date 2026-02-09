# Architecture Rules

Hexagonal architecture - dependency direction is inward only:

```
domain/ <-- port/ <-- adapter/ <-- application/ <-- services/
```

- `domain/` - Pure types. Zero crate dependencies. Never imports adapter/application/services.
- `port/` - Async trait definitions. Depends on domain only.
- `adapter/` - Implements ports. Feature-gated. Can use external crates.
- `application/` - JSON-RPC routing. Wires adapters to ports.
- `services/` - High-level client/server wrappers.

Every new feature: port trait first, then adapter implementation.
Layer violations are blocked by the `enforce-layers.sh` PreToolUse hook.
