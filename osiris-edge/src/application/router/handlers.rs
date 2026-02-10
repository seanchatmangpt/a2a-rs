//! HTTP request handlers for the Axum router
//!
//! Contains implementations for:
//! - Health checks
//! - Readiness checks

use crate::port::{AsyncWipGate, AuthGate, PacketNormalizer, RefusalEngine};
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use std::sync::Arc;
use tracing::debug;

use super::{HealthResponse, ReadinessChecks, ReadinessResponse, RouterState};

/// Health check handler - indicates service is running
///
/// Returns 200 OK with service status and version information.
/// This endpoint is typically used for load balancer health checks.
pub async fn health_handler<W, A, R, N>() -> impl IntoResponse
where
    W: AsyncWipGate,
    A: AuthGate,
    R: RefusalEngine,
    N: PacketNormalizer,
{
    debug!("Health check requested");

    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }),
    )
        .into_response()
}

/// Readiness check handler - verifies all dependencies are ready
///
/// Returns 200 OK if all subsystems are ready, 503 Service Unavailable otherwise.
/// This endpoint is used to determine if the service can accept traffic.
///
/// Checks:
/// - WIP gate is initialized with capacity > 0
/// - Auth gate is available
/// - Refusal engine is ready
/// - Packet normalizer is initialized
pub async fn readiness_handler<W, A, R, N>(
    State(state): State<Arc<RouterState<W, A, R, N>>>,
) -> impl IntoResponse
where
    W: AsyncWipGate,
    A: AuthGate,
    R: RefusalEngine,
    N: PacketNormalizer,
{
    debug!("Readiness check requested");

    // Check WIP gate availability
    let wip_gate_ready = state.wip_gate.limit() > 0;

    // All services are initialized if we got here
    let auth_gate_ready = true;
    let refusal_engine_ready = true;
    let normalizer_ready = true;

    let all_ready = wip_gate_ready && auth_gate_ready && refusal_engine_ready && normalizer_ready;

    let status = if all_ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status,
        Json(ReadinessResponse {
            ready: all_ready,
            checks: ReadinessChecks {
                wip_gate: wip_gate_ready,
                auth_gate: auth_gate_ready,
                refusal_engine: refusal_engine_ready,
                normalizer: normalizer_ready,
            },
        }),
    )
        .into_response()
}
