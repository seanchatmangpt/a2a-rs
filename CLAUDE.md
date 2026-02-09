# CLAUDE.md - Development Guide for a2a-rs

## Project Overview

Rust workspace implementing the Agent-to-Agent (A2A) Protocol v0.3.0. Hexagonal architecture with modular feature flags. Published on crates.io as `a2a-rs`.

**Workspace members:** `a2a-rs` (core library), `a2a-agents` (examples), `a2a-client` (web UI), `a2a-ap2` (payments extension)

**Rust edition:** 2024 | **MSRV:** 1.85

## Build Commands

```bash
# Build entire workspace
cargo build --workspace

# Build with all features (needed for full compilation)
cargo build --all-features

# Build specific member
cargo build -p a2a-rs
cargo build -p a2a-agents
```

## Test Commands

```bash
# Run all workspace tests
cargo test --workspace

# Run all tests with all features enabled
cargo test --all-features

# Run tests for core library only
cargo test -p a2a-rs

# Run a specific test
cargo test -p a2a-rs -- test_name

# Run tests verbose (as CI does)
cargo test --verbose
```

## Linting and Formatting

```bash
# Check formatting (CI runs this)
cargo fmt --all -- --check

# Auto-format code
cargo fmt --all

# Run clippy (CI treats warnings as errors)
cargo clippy -- -D warnings

# Check documentation builds
cargo doc --no-deps --all-features
```

## CI Pipeline

GitHub Actions runs on push/PR to `master` with 4 jobs:
1. **Build and Test** - `cargo build --verbose && cargo test --verbose`
2. **Clippy** - `cargo clippy -- -D warnings` (warnings are errors)
3. **Format** - `cargo fmt --all -- --check`
4. **Doc Check** - `cargo doc --no-deps --all-features`

## Feature Flags (a2a-rs)

| Feature | Description |
|---------|-------------|
| `default` | `server` + `tracing` |
| `client` | Base client (tokio, async-trait, futures) |
| `http-client` | HTTP client via reqwest |
| `ws-client` | WebSocket client via tungstenite |
| `server` | Base server (tokio, async-trait, futures) |
| `http-server` | HTTP server via axum |
| `ws-server` | WebSocket server via tungstenite |
| `tracing` | Structured logging |
| `auth` | JWT, OAuth2, OIDC authentication |
| `sqlx-storage` | SQLx base storage |
| `sqlite` / `postgres` / `mysql` | Database backends |
| `full` | Everything except mysql |

## Architecture

Hexagonal (Ports & Adapters) pattern:

- **`domain/`** - Core types: `Message`, `Task`, `AgentCard`, JSON-RPC protocol types, validation
- **`port/`** - Trait definitions: `MessageHandler`, `TaskManager`, `StreamingHandler`, `NotificationManager`, `Authenticator`
- **`adapter/`** - Implementations: HTTP/WS transports, auth providers, SQLx storage, business logic handlers
- **`application/`** - JSON-RPC routing, request handlers for agent/message/task/notification
- **`services/`** - High-level client/server service wrappers
- **`observability/`** - Tracing setup

## Code Conventions

- Async-first design using `tokio` runtime and `async-trait`
- Builder pattern via `bon` crate for complex structs
- Error types via `thiserror` with domain-specific error enums
- All public types derive `Serialize`/`Deserialize` via serde
- Tests use `proptest` for property-based testing and `jsonschema` for spec compliance
- Protocol spec JSON schemas live in `spec/` directory

## Key Files

- `a2a-rs/src/lib.rs` - Library root, re-exports public API
- `a2a-rs/src/domain/core/` - Core protocol types (agent, message, task)
- `a2a-rs/src/port/` - Trait definitions (the "ports" in hexagonal architecture)
- `a2a-rs/src/adapter/transport/` - HTTP and WebSocket transport implementations
- `a2a-agents/src/reimbursement/` - Primary example agent implementation
- `spec/*.json` - A2A Protocol v0.3.0 specification schemas

## Running Examples

```bash
# HTTP client/server example
cargo run -p a2a-rs --example http_client_server --features "http-server,http-client"

# WebSocket example
cargo run -p a2a-rs --example websocket_client_server --features "ws-server,ws-client"

# Reimbursement agent demo (includes web UI at localhost:3000)
cargo run -p a2a-agents --bin reimbursement_demo
```

## Code Generation (ggen + CONSTRUCT)

The project uses ggen for ontology-driven code generation. Domain types are defined as RDF ontology in `ggen/ontology/*.ttl` and generated into Rust via SPARQL CONSTRUCT queries + Tera templates.

### Workflow
1. Define/update types in RDF ontology (`ggen/ontology/`)
2. Write SPARQL CONSTRUCT queries in `ggen/ggen.toml`
3. Templates in `ggen/templates/` transform CONSTRUCT results to Rust
4. Generated code goes to `a2a-rs/src/generated/`
5. Hand-written code imports from generated modules

### Key Principle
- Domain types: CONSTRUCT from ontology (single source of truth)
- Port traits: CONSTRUCT from ontology
- Adapter implementations: hand-written (not ontology-derivable)
- Spec compliance: three-way validation (spec JSON <-> ontology <-> generated Rust)

### Commands
```bash
# Generate code from ontology
ggen generate --config ggen/ggen.toml

# Validate ontology against spec
/ontology agent

# Run CONSTRUCT-based generation
/construct
```
