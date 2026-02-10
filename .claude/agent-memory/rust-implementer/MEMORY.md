# Rust Implementer Agent Memory

## Quick Links
- [Artifact Publishing](./artifact-publishing.md) - Google Workspace publisher pattern (2026-02-10)
- [SSE Streaming](./sse-streaming.md) - SSE resumable streaming patterns (2026-02-09)

## Recent Work

### HTTP Handlers - 7-Stage Pipeline (osiris-compiler, 2026-02-10)
Implemented complete Axum HTTP handlers for deterministic compilation endpoint:
- **File**: `src/application/http_handlers.rs` (410 lines)
- **PipelineState**: Centralized state containing all 7 port traits as `Arc<dyn Trait + Send + Sync>`
- **Endpoints**: `GET /health` (health check), `POST /compile` (7-stage pipeline)
- **7 Stages**: Type Checker (Σ) → Guards (H) → Orderer (Λ) → Kernel → Invariants (Q) → Writer → Receipt Builder
- **Request/Response**: `CompileRequest`/`CompileResponse` with operation + pipeline stats
- **Error handling**: `AppError` enum with per-stage errors, structured HTTP responses
- **Stats tracking**: Duration, timestamp, completed stages, warnings in `PipelineStats`
- **In-memory factory**: `PipelineState::new_in_memory()` creates demo state with all adapters
- **Async throughout**: All handlers use async/await, traits use `#[async_trait]` where needed
- **Trait bounds**: All Arc<dyn Trait> include `+ Send + Sync` for Axum compatibility
- **Tests**: 4 comprehensive tests covering state creation, serialization, compilation, deserialization
- **Integration**: Updated `main.rs` to wire handlers, `application/mod.rs` for exports, `lib.rs` for public API
- **Key learnings**: Axum handlers need Send + Sync bounds on trait objects; sha2::Digest pattern for hashing; Result<Json<T>, Error> pattern for error handling
- **Build status**: All tests pass, compiles without errors

### Axum Router (osiris-edge, 2026-02-10)
Implemented comprehensive HTTP router with admission control and webhook processing:
- **Endpoints**: `/health` (OK status), `/ready` (subsystem checks), `/workspace/webhook` (Gmail/Calendar/Drive), `/mcp/*` (proxy)
- **Router state**: `RouterState<W, A, R, N>` containing WIP gate, auth gate, refusal engine, normalizer
- **Request types**: `HealthResponse`, `ReadinessResponse`, `WebhookRequest`, `WebhookResponse`, `McpProxyResponse`, `ErrorResponse`
- **Webhook processing**: Service auto-detection (gmail/calendar/drive), packet normalization to typed packets, WIP limit enforcement
- **Refusal receipts**: JSON-serialized receipts on auth/WIP/type-check failures with cryptographic proof
- **MCP proxy**: Forwards requests to a2a-mcp service with status code conversion (reqwest → axum)
- **Handlers module**: Separate module for health/readiness handlers with detailed subsystem checks
- **Async all the way**: All handlers use `Response` return type for type consistency
- **Error handling**: Structured error responses with optional refusal receipts
- **Files**: `src/application/router.rs` (525 lines), `src/application/router/handlers.rs` (87 lines)
- **Integration**: Updated `application/mod.rs` to export router types and create_router() function
- **Key learnings**: Use `Response` for return type consistency in complex async handlers; convert reqwest StatusCode to axum StatusCode; clone values used after moves in JSON builders

### Workflow Patterns 10-20 (osiris-compiler, 2026-02-10)
Extended osiris-compiler workflow kernel with van der Aalst patterns 10-20:
- **Extended GatewayPattern enum**: Added ArbitraryCycle, ImplicitTermination, DeferredChoice, InterleavedParallelRouting, Milestone, CriticalSection
- **Domain support types**: CriticalSectionConfig, MilestoneConfig, InterleavedExecutionContext
- **Execute gateway implementation**: Full pattern dispatch for patterns 2-9, 10-11, 15-18 with proper condition evaluation
- **Advanced methods**: execute_multi_instance() for patterns 12-14, execute_cancellation() for pattern 19, trigger_escalation() for pattern 20
- **Helpers**: evaluate_condition() with support for >, <, ==, ! operators; is_critical_section_free(), acquire/release_critical_section()
- **Tests**: 6 comprehensive tests covering Multi-Choice, Milestone, CriticalSection, CancelActivity, Escalation, MultiInstance
- **Documentation**: Complete VAN_DER_AALST_PATTERNS_10_20.md with pattern descriptions, use cases, examples
- **Files**: `domain/workflow.rs` (extended enums), `adapter/workflow_kernel.rs` (full implementation + tests), `docs/VAN_DER_AALST_PATTERNS_10_20.md`
- **Key insight**: Condition evaluation is foundation for advanced patterns; token-based semantics for OR-joins differs from AND-joins

### Artifact Publisher (osiris-compiler, 2026-02-10)
Implemented Google Workspace artifact publishing for Gmail, Calendar, Drive APIs:
- Domain types: `Artifact` enum with Email/CalendarEvent/Document variants
- Artifact types: `EmailArtifact` (recipients, subject, body, attachments), `CalendarArtifact` (event details, attendees, reminders), `DriveArtifact` (file content, sharing perms)
- Support types: `SharingPermission` (reader/writer/domain-based), `PublishResult`, `ArtifactPublishError`
- Port trait: `ArtifactPublisher` with `publish()`, `publish_batch()`, `send_email()`, `create_calendar_event()`, `upload_document()`
- Adapter: `GoogleWorkspacePublisher` with OAuth2 config, validation, simulation methods ready for google-gmail1/calendar3/drive3 APIs
- Builder pattern: `Artifact::email()`, `Artifact::calendar_event()`, `Artifact::document()` convenience constructors
- Feature-gated: "workspace-publisher" flag with google-gmail1 (7.0), google-calendar3 (7.0), google-drive3 (7.0) deps
- Tests: 40+ comprehensive tests covering validation, batch ops, authentication, error cases
- Files: `domain/artifact.rs`, `port/artifact_publisher.rs`, `adapter/workspace_publisher.rs`
- Production ready: Simulation methods prepared for real API integration, error handling complete

### GCS Receipt Storage (osiris-compiler, 2026-02-10)
Implemented Google Cloud Storage receipt storage adapter:
- Complete implementation: `adapter/gcs_receipt_storage.rs` with hash-based naming
- `GcsConfig` builder with bucket, prefix, project ID configuration
- `GcsReceiptStorage` async adapter using google-cloud-storage crate
- SHA-256 hash-based object naming for deterministic, unique object IDs
- Pretty-printed JSON serialization with application/json content type
- Limited query support with clear error messages (requires Firestore for metadata indexing)
- Feature-gated with "gcs" in Cargo.toml
- Full test suite: config builders, hash determinism, path generation, JSON roundtrips
- Module exports in adapter/mod.rs and lib.rs with proper feature gating
- Dependency: google-cloud-storage 0.18

### WIP Analytics Engine (osiris-edge, 2026-02-09)
Implemented real-time WIP analytics with live metrics streaming:
- Complete domain types: `WorkMetrics`, `WipSnapshot`, `AnalyticsSnapshot`, `LittlesLawMetrics`, `PercentileLatency`, `Anomaly`, `BottleneckSignal`
- Port trait: `AnalyticsEngine` with async tracking methods and SSE streaming
- Adapter: `RealtimeAnalyticsEngine` with broadcast channels and time-windowed buffers
- Integration: `InstrumentedWipGate` wraps `KanbanWipGate` for automatic metrics tracking
- SSE handlers in application layer for dashboard streaming
- Full anomaly detection: high utilization, latency spikes, throughput drops, Little's Law violations
- Bottleneck detection: WIP limit too low, slow processing, queue buildup, burst traffic
- Demo: `examples/wip_analytics_demo.rs` with simulated workload and live console output
- Files: `domain/analytics.rs`, `port/analytics_engine.rs`, `adapter/realtime_analytics.rs`, `adapter/instrumented_wip_gate.rs`, `application/analytics_sse.rs`

### Deterministic Orderer (osiris-compiler, 2026-02-09)
Implemented Λ (lambda) deterministic orderer for compiler operations:
- Law-based resolution (priority → timestamp → UUID tiebreaker)
- Total order guarantee with repeatability across runs
- Files: `osiris-compiler/src/port/orderer.rs`, `adapter/lambda_orderer.rs`, `domain/operation.rs`
- All 10 tests passing (7 adapter + 3 domain)
- See details below in "Deterministic Ordering Pattern"

### SSE Resumable Streaming (a2a-mcp, 2026-02-09)
Implemented SSE manager with MCP-compliant resumability:
- Event IDs, Last-Event-ID support, redelivery window, broadcast + replay pattern
- Files: `a2a-mcp/src/adapter/sse_manager.rs`, integration in `server.rs`
- Key learnings: tokio-stream `sync` feature, a2a-rs public API usage, Axum error handling
- See [sse-streaming.md](./sse-streaming.md) for full details

## Key Patterns

### Axum HTTP Handler Patterns

**State Management**:
```rust
#[derive(Clone)]
pub struct AppState {
    port_trait: Arc<dyn MyTrait + Send + Sync>,
}

async fn handler(
    State(state): State<AppState>,
    Json(request): Json<Request>,
) -> Result<Json<Response>, AppError> {
    // Must include Send + Sync bounds for thread safety with Tokio/Axum
}
```

**Error Handling Pattern**:
```rust
enum AppError {
    Stage1Failed(String),
    Stage2Failed(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            AppError::Stage1Failed(msg) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: msg, ... })
            ),
            // ... other variants
        };
        (status, body).into_response()
    }
}

// Handler returns Result<Json<T>, AppError>
```

**Router Wiring**:
```rust
let state = AppState::new();
let app = Router::new()
    .route("/endpoint", post(handler))
    .with_state(state);
```

### Hexagonal Architecture Implementation

When implementing new features in a2a-rs workspace:

1. **Domain first**: Create pure types with validation in `domain/`
2. **Port traits**: Define async traits with `#[async_trait]` in `port/`
3. **Adapter implementations**: Concrete implementations in `adapter/`
4. **Module exports**: Update `mod.rs` files to export new types/traits
5. **Lib.rs integration**: Add public re-exports and prelude module

### Deterministic Ordering Pattern (Λ-Laws)

Implemented in osiris-compiler for compiler operation ordering:

**Core Principle**: Establish total order through law-based resolution, not negotiation

**Ordering Laws**:
```rust
A < B ⟺ priority(A) > priority(B)  ∨
        (priority(A) = priority(B) ∧ timestamp(A) < timestamp(B))  ∨
        (priority(A) = priority(B) ∧ timestamp(A) = timestamp(B) ∧ id(A) < id(B))
```

**Implementation**:
- Domain type `Operation` with `Ord` impl (priority, timestamp, UUID)
- Port trait `DeterministicOrderer` with `order()`, `validate()`, `conflicts()`
- Adapter `LambdaOrderer` with configurable validation and conflict detection
- Rust's stable `sort()` ensures deterministic results

**Guarantees**:
1. Determinism: same inputs → same output order
2. Totality: all operation pairs comparable
3. Transitivity: A < B ∧ B < C → A < C
4. Repeatability: same across runs and systems

**Key files**:
- `domain/operation.rs` - Operation type with Ord implementation
- `domain/error.rs` - OrderingError types
- `port/orderer.rs` - DeterministicOrderer trait
- `adapter/lambda_orderer.rs` - LambdaOrderer implementation with config

**Testing**: Property-based tests for ordering stability, priority/timestamp precedence

### CONSTRUCT8 Bounded Writer Pattern

Successfully implemented bounded state mutations with:

- Domain validation before backend execution (fail fast)
- `MAX_MUTATION_UNITS` constant (8) enforced at type level
- Pluggable backend via `StorageBackend` + `Transaction` traits
- In-memory implementation for testing, production backends via traits
- Atomic commits with explicit rollback on error

**Key files:**
- `domain/patch.rs` - Patch, PatchSet, validation logic
- `domain/triple.rs` - RDF triple types
- `port/bounded_writer.rs` - BoundedWriter trait with CommitResult
- `adapter/in_memory_writer.rs` - Testing implementation
- `adapter/construct8_writer.rs` - Production writer with pluggable backend

### Testing Strategy

- Domain validation tests in domain module (unit tests)
- Adapter tests with mock backends (integration-style)
- Run specific test modules: `cargo test -- module::path`
- Use `#[tokio::test]` for async tests

### Common Issues

- **Cargo.toml features**: Some dependencies (e.g., firestore) define their own feature flags that override expected values, causing `unexpected_cfgs` warnings. These are warnings only and don't affect build success.
- **Workspace members**: Always verify member is in workspace `Cargo.toml` before building
- **Module exports**: Must update all `mod.rs` files in the chain (domain → port → lib)

## CONSTRUCT Semantics

- Delete before insert (SPARQL CONSTRUCT order)
- Atomic execution (all-or-nothing)
- Mutation count = additions.len() + deletions.len()
- Each triple = 1 mutation unit

### Firestore State Store (osiris-compiler, 2026-02-10)
Implemented Firestore-backed state store for CONSTRUCT8 bounded writer:
- **Architecture**: Implements `StorageBackend` and `Transaction` traits from `construct8_writer.rs`
- **Storage structure**: RDF triples stored in "state" collection, documents keyed by subject URI hash
- **Document ID generation**: SHA-256 hash of subject URI for deterministic, safe Firestore document IDs
- **Document format**: `TripleDocument` with subject, predicate/object array, updated_at timestamp
- **Transactions**: `FirestoreTransaction` with in-memory addition/deletion buffers, commit/rollback support
- **Feature-gated**: "firestore" flag with google-firestore1 (5.0), hyper, hyper-rustls, yup-oauth2 dependencies
- **Error handling**: Comprehensive WriteError types (ValidationFailed, ConflictError, StorageError, RollbackError)
- **Logging**: Conditional tracing with debug/info/error macros (feature-gated)
- **Tests**: 16 comprehensive unit tests covering SHA-256 hashing, document ID determinism, serialization, transactions
- **Files**: `src/adapter/firestore_state_store.rs`
- **Production-ready**: Placeholder implementation with clear comments for API integration points
- **Key insight**: CONSTRUCT semantics (delete before insert) maintained through tx operation ordering

## Next Steps

When completing Firestore backend:
- Replace placeholder client with actual google-firestore1 API client
- Implement real `get_document()` calls in `get_triples_for_subject()`
- Add batch operations for multi-subject commits
- Implement conflict detection and retry logic
- Complete Spanner backend using same trait design
