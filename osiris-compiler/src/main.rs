//! Osiris Compiler Service
//!
//! Internal service that provides deterministic compilation μ: O → A.
//! This is the CLM (Constraint Logic Markup) Compiler that produces
//! actions A from operations O.
//!
//! ## Pipeline
//!
//! The HTTP handlers implement a 7-stage compilation pipeline:
//! 1. **Type Checker** - Validates packet types against Σ (closed type system)
//! 2. **Guard Evaluator** - Evaluates H-guard temporal constraints
//! 3. **Orderer** - Establishes deterministic order via Λ laws
//! 4. **Workflow Kernel** - Executes van der Aalst's workflow patterns
//! 5. **Invariant Verifier** - Proves preserve(Q) for Q invariants
//! 6. **Writer** - Commits bounded RDF state mutations (8-unit limit)
//! 7. **Receipt Builder** - Generates cryptographic proofs with signatures
//!
//! ## Endpoints
//!
//! - `GET /health` - Health check
//! - `POST /compile` - Compile a single operation through the full pipeline

use axum::{
    Router,
    routing::{get, post},
};
use osiris_compiler::application::{PipelineState, compile, health_check};
use tracing::info;

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "osiris_compiler=info,tower_http=info".into()),
        )
        .with_target(false)
        .compact()
        .init();

    info!("Starting Osiris Compiler service");

    // Initialize pipeline state with in-memory adapters
    let pipeline_state = PipelineState::new_in_memory();
    info!("Pipeline state initialized with in-memory implementations");

    // Build router with all handlers
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/compile", post(compile))
        .with_state(pipeline_state);

    // Bind to address
    let addr =
        std::env::var("OSIRIS_COMPILER_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    info!("Listening on {}", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    info!("Server started successfully");
    axum::serve(listener, app).await.expect("Server error");
}
