# Rust Implementer Agent Memory

## Quick Links
- [Audit Logging](./audit-logging.md) - Cloud Logging with trace context (2026-02-10)
- [WebSocket Transport](./websocket-transport.md) - Bidirectional streaming, heartbeat, reconnection (2026-02-10)
- [Rate Limiter](./rate-limiter.md) - Token bucket rate limiting, per-IP/tenant, Axum middleware (2026-02-10)
- [Artifact Publishing](./artifact-publishing.md) - Google Workspace publisher pattern (2026-02-10)
- [SSE Streaming](./sse-streaming.md) - SSE resumable streaming patterns (2026-02-09)
- [Prometheus Metrics](./metrics.md) - Metrics collection patterns (2026-02-10)
- [OAuth2 PKCE](./oauth2-pkce.md) - RFC 7636 PKCE authenticator (2026-02-10)
- [Redis Cache](./redis-cache.md) - Redis cache with TTL, invalidation, cache-aside (2026-02-10)
- [MCP Tasks](./mcp-tasks-pattern.md) - MCP task management patterns
- [Refusal Engine](./refusal-engine.md) - Cryptographic refusal receipts

## Recent Work (OSIRIS CLM Implementation)

### 20 Production Components (2026-02-10)
Implemented comprehensive infrastructure, observability, and security components across osiris-compiler and osiris-edge.

**osiris-compiler (10 components):**
- Workflow patterns 21-30 (van der Aalst complete basis)
- Workflow persistence (Firestore/Spanner checkpoint recovery)
- Circuit breaker (failure isolation with state machine)
- Audit logger (cryptographic event log with Cloud Logging)
- Backup/restore (state snapshots with versioning)
- Cloud Tasks queue (async job dispatch with retry)
- gRPC transport (high-performance RPC with streaming)
- Secret manager (credential storage with Cloud KMS)
- Spanner state store (globally distributed strong consistency)
- Cloud Workflows integration

**osiris-edge (9 components):**
- Pub/Sub event bus (async messaging with ordering)
- Redis cache (distributed caching with TTL)
- Rate limiter (token bucket, sliding window, per-tenant)
- OAuth2 PKCE (RFC 7636 secure auth flow)
- WebSocket transport (bidirectional streaming)
- OpenTelemetry tracing (W3C distributed tracing)
- Prometheus metrics (observability with Grafana)
- BigQuery telemetry sink (analytics warehouse)
- Rate limit middleware (Axum integration)

**Infrastructure:**
- Terraform configs (GCP deployment with all services)

### WebSocket Transport (osiris-edge, 2026-02-10)
Bidirectional WebSocket transport for TypedPacket streaming:
- **Port trait**: `Transport` with 8 async methods (connect, send, receive, reconnect, batch, status)
- **Adapter**: `WebSocketTransport` using tokio-tungstenite with TLS support
- **Heartbeat**: Ping/pong with configurable intervals and timeout detection
- **Reconnection**: Exponential backoff (100ms-30s), max retries, automatic recovery
- **Status tracking**: Disconnected, Connecting, Connected, Degraded, Failed transitions
- **Feature**: `ws = ["tokio-tungstenite"]`
- **Files**: `port/transport.rs`, `adapter/websocket.rs`, `examples/websocket_transport.rs`, `docs/WEBSOCKET_TRANSPORT.md`

### Workflow Persistence (osiris-compiler, 2026-02-10)
Complete checkpoint/recovery system for workflow instances:
- **Port trait**: `WorkflowStore` with 14 methods (checkpoint CRUD, query, recovery, export/import)
- **Adapter**: `FirestoreWorkflowStore` with SHA-256 deterministic IDs, in-memory cache, auto-pruning
- **Documents**: Lightweight metadata + full snapshots, flexible querying, batch operations
- **Recovery**: Event replay with summary results
- **Files**: `port/workflow_store.rs`, `adapter/workflow_persistence.rs`, `docs/WORKFLOW_PERSISTENCE.md`

### Circuit Breaker (osiris-compiler, 2026-02-10)
Production-ready failure isolation:
- **States**: Closed → Open (on threshold) → HalfOpen (timeout) → Closed (success)
- **Adapter**: `StandardCircuitBreaker` with `Arc<RwLock<InternalState>>` thread-safe state machine
- **Configuration**: failure_threshold (5), success_threshold (2), timeout (30s), half_open_max_calls (1)
- **Metrics**: failure/success/call counts, state transitions, timestamps
- **Files**: `port/circuit_breaker.rs`, `adapter/circuit_breaker.rs`, `docs/CIRCUIT_BREAKER.md`

### OpenTelemetry Integration (osiris-edge, 2026-02-10)
Comprehensive distributed tracing:
- **W3C Support**: Parse/format traceparent headers (00-traceId-spanId-flags)
- **Backends**: Cloud Trace (gcloud), Jaeger (otel-jaeger), OTLP (otel)
- **Context propagation**: Extract/inject headers for distributed traces
- **Features**: `otel`, `otel-gcloud`, `otel-jaeger`
- **Files**: `adapter/tracing.rs`, `examples/otel_tracing_demo.rs`, `docs/OTEL_INTEGRATION.md`

### Prometheus Metrics (osiris-edge, 2026-02-10)
Comprehensive metrics collection:
- **Port trait**: `MetricsCollector` with record_request, record_error, gauges, counters, histograms
- **Adapter**: `PrometheusCollector` with lazy metric creation, TextEncoder for /metrics
- **Built-in metrics**: http_requests_total, http_request_duration_seconds, errors_total, active_connections
- **Middleware**: Auto-tracks request duration and status codes
- **Files**: `port/metrics.rs`, `adapter/metrics.rs`, `application/metrics_handler.rs`, `docs/METRICS.md`

### Redis Cache (osiris-edge, 2026-02-10)
Production-ready distributed caching:
- **Port trait**: `Cache` generic over `Serialize + Deserialize` types
- **Operations**: get, set, delete, exists, ttl, invalidate_pattern, get_or_load (cache-aside)
- **Adapter**: `RedisCache` with JSON serialization, SCAN-based patterns, connection pooling
- **Configuration**: URL, key prefix, TTL bounds, pattern limits
- **Feature**: `redis = ["dep:redis"]`
- **Files**: `port/cache.rs`, `adapter/cache.rs`, `examples/redis_cache_demo.rs`, `docs/REDIS_CACHE_GUIDE.md`

### OAuth2 PKCE (osiris-edge, 2026-02-10)
RFC 7636 PKCE flow for public clients:
- **Domain types**: `CodeVerifier`, `CodeChallenge` (SHA256+base64url), authorization/token request/response
- **Port trait**: `Oauth2Authenticator` with 13 async methods
- **Adapter**: `PkceAuthenticator` with reqwest HTTP client, session storage, UUID+SHA256 crypto
- **Security**: CSRF via state, code interception prevention, no client secrets
- **Files**: `domain/oauth2.rs`, `port/oauth2_authenticator.rs`, `adapter/oauth_pkce.rs`, `examples/oauth2_pkce_flow.rs`, `docs/OAUTH2_PKCE.md`

### Rate Limiter (osiris-edge, 2026-02-10)
Token bucket and sliding window rate limiting:
- **Port trait**: `RateLimiter` with check_limit, reset, remaining, get_limits
- **Adapters**: `TokenBucketLimiter` (refill rate), `SlidingWindowLimiter` (time windows)
- **Configuration**: Per-IP, per-tenant, per-user, global limits
- **Middleware**: Axum middleware with automatic rejection
- **Files**: `port/rate_limiter.rs`, `adapter/rate_limiter.rs`, `application/rate_limit_middleware.rs`, `examples/rate_limiter_demo.rs`, `docs/RATE_LIMITER.md`

### HTTP Handlers - 7-Stage Pipeline (osiris-compiler, 2026-02-10)
Complete compilation endpoint:
- **Endpoints**: GET /health, POST /compile
- **Pipeline**: Type Checker (Σ) → Guards (H) → Orderer (Λ) → Kernel → Invariants (Q) → Writer → Receipt Builder
- **State**: `PipelineState` with `Arc<dyn Trait + Send + Sync>` for all ports
- **Error handling**: `AppError` enum with per-stage HTTP status codes
- **Files**: `application/http_handlers.rs`, `main.rs`

### Axum Router (osiris-edge, 2026-02-10)
HTTP gateway with admission control:
- **Endpoints**: /health, /ready, /workspace/webhook, /mcp/*
- **Middleware**: WIP gate, auth gate, refusal engine, normalizer
- **Webhook processing**: Service detection (gmail/calendar/drive), typed packets
- **Refusal receipts**: JSON receipts on auth/WIP/validation failures
- **Files**: `application/router.rs`, `application/router/handlers.rs`

## Earlier Work (a2a-rs Core)

### Workflow Pattern Implementation (2026-02-09)
All 43 workflow patterns from Workflow Patterns Initiative in `a2a-rs/src/domain/workflow/patterns.rs`:
- **Patterns**: BasicControlFlow, AdvancedBranchingAndSynchronization, MultipleInstance, StateBased, CancellationAndCompletion, Iteration, Termination, Trigger, Special (43 total)
- **Graph**: petgraph DiGraph for topology analysis
- **Detection**: Dfs for reachability, unreachable states, dead-ends, export states
- **Testing**: Property-based tests proving incompleteness theorem
- **Files**: `domain/workflow/patterns.rs`, `examples/workflow_pattern_checker.rs`

### SPARQL CONSTRUCT Optimizer (2026-02-09)
Production-grade optimizer for ggen in `ggen-optimizer/`:
- **Parser**: nom-based recursive descent for PREFIX, CONSTRUCT, WHERE clauses
- **Analysis**: Dependency graph, connected components, join graph, selectivity estimation
- **Cost Model**: Base operation costs, cardinality estimation, Amdahl's law for parallelism
- **Optimization Passes**: Predicate pushdown, join elimination, subquery flattening, redundant elimination, parallel decomposition
- **Files**: `lib.rs`, `parser.rs`, `analyzer.rs`, `cost.rs`, `rewriter.rs`

### TPS Coordinator (2026-02-09)
Autonomous agent coordinator with Toyota Production System in `a2a-rs/src/services/coordinator.rs`:
- **TPS Concepts**: Kanban board, pull scheduling, Andon system, Jidoka, Heijunka, Takt time
- **Architecture**: `Arc<RwLock<CoordinatorState>>`, background tasks with tokio::spawn
- **Metrics**: Cycle time, throughput, WIP, defect rate
- **Files**: `services/coordinator.rs`, `examples/tps_coordinator.rs`

### Cryptographic Receipt Validation (2026-02-09)
Production-ready receipt validation in `a2a-rs/src/services/receipt.rs`:
- **Components**: Receipt (ed25519 signature), ReceiptChain (hash pointers), MerkleTree (batch proofs), ReplayValidator
- **Ed25519**: dalek v2.1 with SigningKey::from_bytes
- **Merkle Proofs**: Bottom-up collection, (hash, is_right_sibling) tuples
- **Feature**: `crypto = ["sha2", "ed25519-dalek", "hex"]`
- **Files**: `services/receipt.rs`, `examples/receipt_demo.rs`, `examples/receipt_debug.rs`

## Key Patterns

### Hexagonal Architecture
1. **Domain first**: Pure types with validation in `domain/`
2. **Port traits**: Async traits with `#[async_trait]` in `port/`
3. **Adapter implementations**: Concrete implementations in `adapter/`
4. **Module exports**: Update `mod.rs` files to export types/traits
5. **Lib.rs integration**: Add public re-exports

### Deterministic Ordering (Λ-Laws)
```rust
A < B ⟺ priority(A) > priority(B)  ∨
        (priority(A) = priority(B) ∧ timestamp(A) < timestamp(B))  ∨
        (priority(A) = priority(B) ∧ timestamp(A) = timestamp(B) ∧ id(A) < id(B))
```
**Guarantees**: Determinism, Totality, Transitivity, Repeatability

### CONSTRUCT8 Bounded Writer
- Domain validation before backend execution
- MAX_MUTATION_UNITS constant (8) enforced
- Pluggable backend via StorageBackend + Transaction traits
- Atomic commits with rollback
- Delete before insert (SPARQL CONSTRUCT semantics)

### Axum HTTP Handlers
```rust
#[derive(Clone)]
pub struct AppState {
    port_trait: Arc<dyn MyTrait + Send + Sync>, // Must be Send + Sync
}

async fn handler(
    State(state): State<AppState>,
    Json(request): Json<Request>,
) -> Result<Json<Response>, AppError> {
    // ...
}
```

### Testing Strategy
- Domain validation tests in domain module
- Adapter tests with mock backends
- Property-based tests for theorems (proptest)
- Run specific tests: `cargo test -- module::path`
- Use `#[tokio::test]` for async tests

## Common Issues

### Cargo.toml Features
- Some dependencies define their own feature flags causing `unexpected_cfgs` warnings (safe to ignore)
- Always verify workspace member is in workspace Cargo.toml

### Borrow Checker
- Collect data before mutating to avoid multiple mutable borrows:
  ```rust
  let data: Vec<_> = state.map.iter().map(|(k, v)| (k.clone(), *v)).collect();
  for (key, val) in data {
      state.other_map.get_mut(&key); // OK - previous borrow done
  }
  ```

### Parser Debugging (nom)
```rust
println!("Query length: {}", query.len());
println!("Character at position {}: {:?}", pos, query.chars().nth(pos));
println!("Context: {:?}", &query[pos-5..pos+5]);
```

### Ed25519 (dalek v2.1)
- No `SigningKey::generate()` - use `SigningKey::from_bytes(&seed)` with 32 random bytes
- `Signature::from_bytes()` returns `Signature`, not `Result`

### Bon Builder
- Don't use `#[builder(default)]` on `Option<T>` - Option implies `None`

### Clippy
- Use `while let` instead of `loop { if let }`
- Prefix unused parameters with `_`
- Don't explicitly `drop()` references

## Next Steps

1. Complete Firestore backend integration with real API
2. Add property-based tests for optimization passes
3. Implement WebSocket/HTTP endpoints for TPS coordinator monitoring
4. Add receipt validation middleware for A2A protocol
5. Implement receipt storage adapter (SQLx persistent chain)
6. Complete Spanner backend using same trait design
7. Add BPMN 2.0 import/export adapter
8. Visualization adapter (GraphViz export)
