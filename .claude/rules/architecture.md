# Architecture Rules

Hexagonal architecture - never violate layer boundaries:

- `domain/` - Core types, no external dependencies. Never imports from adapter/application.
- `port/` - Trait definitions only. Depends on domain, nothing else.
- `adapter/` - Implementations of ports. Can depend on domain + port + external crates.
- `application/` - Wiring layer. Connects adapters to ports, handles JSON-RPC routing.
- `services/` - High-level client/server wrappers.

New features must define a port trait before writing an adapter implementation.
