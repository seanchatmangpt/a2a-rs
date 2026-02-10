# HTTP Handlers Implementation - Osiris Compiler

## Overview

Created `osiris-compiler/src/application/http_handlers.rs` implementing a complete 7-stage HTTP compilation pipeline for the Osiris compiler using Axum framework.

## Architecture

### PipelineState
Centralized application state containing all pipeline components:
- `type_checker: Arc<dyn TypeChecker + Send + Sync>` - Σ (closed type system) validation
- `guard_evaluator: Arc<dyn GuardEvaluator + Send + Sync>` - H-guard temporal constraints
- `orderer: Arc<dyn DeterministicOrderer + Send + Sync>` - Λ deterministic ordering
- `workflow_kernel: Arc<dyn WorkflowKernel + Send + Sync>` - Workflow pattern execution
- `invariant_verifier: Arc<dyn InvariantVerifier + Send + Sync>` - Q invariant verification
- `writer: Arc<dyn BoundedWriter + Send + Sync>` - Bounded RDF state mutations
- `receipt_builder: Arc<dyn ReceiptBuilder + Send + Sync>` - Cryptographic proof generation

All traits are Send + Sync to ensure thread-safety with Tokio/Axum.

### HTTP Endpoints

#### POST /compile
Compiles a single operation through the full 7-stage pipeline.

**Request (CompileRequest):**
```json
{
  "operation": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "timestamp": "2024-01-01T00:00:00Z",
    "priority": 1,
    "kind": {
      "type": "parse",
      "input": "main.rs"
    },
    "source": null
  },
  "replayPointers": [],
  "metadata": {}
}
```

**Response (CompileResponse):**
```json
{
  "receipt": {
    "id": "...",
    "timestamp": "...",
    "operationId": "...",
    "operationHash": "...",
    "attestationHash": "...",
    "signature": "...",
    "replayPointers": [],
    "result": {
      "status": "success",
      "outputHash": "..."
    },
    "refusal": null,
    "metadata": {}
  },
  "stats": {
    "durationMs": 42,
    "timestamp": "2024-01-01T00:00:00Z",
    "completedStages": [
      "type_checker",
      "guard_evaluator",
      "orderer",
      "kernel",
      "invariant_verifier",
      "writer",
      "receipt_builder"
    ],
    "warnings": []
  }
}
```

**Error Response (ErrorResponse):**
```json
{
  "error": "Type check failed",
  "category": "type_check_error",
  "details": "Packet type not in Σ",
  "stage": "type_checker",
  "receipt": null
}
```

#### GET /health
Health check endpoint returns service status.

**Response:**
```json
{
  "status": "healthy",
  "service": "osiris-compiler",
  "version": "0.1.0",
  "pipeline": "ready"
}
```

## Pipeline Stages

The `compile` handler runs operations through 7 sequential stages:

1. **Type Checker (Σ)** - Validates packet types are in closed type system
   - Stage: `type_checker`
   - Error: `TypeCheckFailed`
   - Status Code: 400 Bad Request

2. **Guard Evaluator (H)** - Evaluates temporal constraint guards
   - Stage: `guard_evaluator`
   - Error: `GuardViolation`
   - Status Code: 403 Forbidden

3. **Deterministic Orderer (Λ)** - Establishes deterministic operation order
   - Stage: `orderer`
   - Error: `OrderingFailed`
   - Status Code: 400 Bad Request
   - Uses LambdaOrderer with law-based resolution (priority → timestamp → UUID)

4. **Workflow Kernel** - Executes van der Aalst's 43 workflow patterns
   - Stage: `kernel`
   - Error: `KernelExecutionFailed`
   - Status Code: 500 Internal Server Error

5. **Invariant Verifier (Q)** - Proves preserve(Q) invariants hold
   - Stage: `invariant_verifier`
   - Error: `InvariantViolation`
   - Status Code: 422 Unprocessable Entity

6. **Bounded Writer** - Commits RDF state mutations (max 8 units)
   - Stage: `writer`
   - Error: `WriterFailed`
   - Status Code: 500 Internal Server Error

7. **Receipt Builder** - Generates cryptographic proofs
   - Stage: `receipt_builder`
   - Error: `ReceiptBuildingFailed`
   - Status Code: 500 Internal Server Error
   - Verifies receipt invariant: hash(A) = hash(μ(O))

## Error Handling

### AppError Enum
Comprehensive error types for each pipeline stage:
- `TypeCheckFailed` - Type validation failure
- `GuardViolation` - H-guard constraint violation
- `OrderingFailed` - Operation ordering failure
- `KernelExecutionFailed` - Workflow kernel failure
- `InvariantViolation` - Q invariant violation
- `WriterFailed` - State mutation failure
- `ReceiptBuildingFailed` - Receipt generation failure
- `InternalError` - Generic server error

Each error includes:
- HTTP status code (400/403/422/500)
- JSON error response with category, details, and stage
- Structured tracing/logging

## Statistics Tracking

PipelineStats provides visibility into execution:
- `duration_ms` - Total execution time in milliseconds
- `timestamp` - RFC 3339 timestamp of execution
- `completed_stages` - List of successfully completed stages
- `warnings` - Any non-fatal warnings encountered

## In-Memory Implementations

`PipelineState::new_in_memory()` creates test/demo state:
- `SigmaTypeChecker::new()` - Validates packet types
- `HGuardEvaluatorAdapter::new()` - Evaluates temporal guards
- `LambdaOrderer::default()` - Deterministic ordering
- `InMemoryWorkflowKernel::new()` - Workflow execution
- `QInvariantVerifier::new()` - Invariant checking
- `InMemoryWriter::new()` - In-memory state storage
- `StandardReceiptBuilder::new(LocalSigner::new())` - Local signing

## Integration

### main.rs
Server initialization:
```rust
let pipeline_state = PipelineState::new_in_memory();
let app = Router::new()
    .route("/health", get(health_check))
    .route("/compile", post(compile))
    .with_state(pipeline_state);
```

### application/mod.rs
Module exports:
```rust
pub mod http_handlers;
pub use http_handlers::{
    compile, health_check, AppError, CompileRequest, CompileResponse,
    ErrorResponse, PipelineState, PipelineStats,
};
```

### lib.rs
Public API exports for library consumers:
```rust
pub use application::{
    compile, health_check, AppError, Compiler, CompilerConfig,
    CompileRequest, CompileResponse, ErrorResponse, PipelineState,
    PipelineStats,
};
```

## Testing

Four unit tests verify core functionality:
1. `test_pipeline_state_creation` - Verify state initialization
2. `test_error_response_serialization` - JSON error handling
3. `test_compile_operation_success` - End-to-end pipeline
4. `test_compile_request_deserialization` - JSON deserialization

All tests pass:
```
test result: ok. 4 passed; 0 failed
```

## Build Status

✅ Compiles without errors: `cargo check -p osiris-compiler`
✅ Tests pass: `cargo test -p osiris-compiler --lib application::http_handlers`
✅ Binary builds: `cargo build -p osiris-compiler --bin osiris-compiler`

## Usage Example

Starting the server:
```bash
cargo run -p osiris-compiler --bin osiris-compiler
# Listening on 127.0.0.1:8080
```

Compiling an operation:
```bash
curl -X POST http://localhost:8080/compile \
  -H "Content-Type: application/json" \
  -d '{
    "operation": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "timestamp": "2024-01-01T00:00:00Z",
      "priority": 1,
      "kind": {"type": "parse", "input": "main.rs"},
      "source": null
    },
    "replayPointers": [],
    "metadata": {}
  }'
```

## Production Considerations

For production deployment, replace in-memory adapters:
- **Type Checker**: Connect to ontology database
- **Guard Evaluator**: Integrate with policy engine
- **Workflow Kernel**: Use GCP Cloud Workflows
- **Invariant Verifier**: Connect to constraints database
- **Writer**: Use Firestore/Spanner with transactions
- **Receipt Builder**: Use Cloud KMS for signing

Modify `PipelineState::new_in_memory()` or create `PipelineState::new_production()`.

## Files Modified

1. **Created**: `/home/user/a2a-rs/osiris-compiler/src/application/http_handlers.rs` (410 lines)
2. **Updated**: `/home/user/a2a-rs/osiris-compiler/src/application/mod.rs` - Added exports
3. **Updated**: `/home/user/a2a-rs/osiris-compiler/src/main.rs` - Wired handlers
4. **Updated**: `/home/user/a2a-rs/osiris-compiler/src/lib.rs` - Public API exports

## Lines of Code

- http_handlers.rs: 410 lines
- Tests: 55 lines
- Documentation: Comprehensive inline + this guide
