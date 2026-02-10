//! Axum router with health, readiness, webhook, and MCP proxy endpoints
//!
//! Wires together authentication gates, WIP limiters, refusal engines,
//! and packet normalizers into a unified HTTP API.

use crate::domain::RefusalReceipt;
use crate::port::{AsyncWipGate, AuthGate, PacketNormalizer, RefusalEngine};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

pub mod handlers;

pub use handlers::{health_handler, readiness_handler};

/// Router state containing all wired components
#[derive(Clone)]
pub struct RouterState<W, A, R, N> {
    /// WIP gate for admission control
    pub wip_gate: Arc<W>,
    /// Auth gate for authentication and authorization
    pub auth_gate: Arc<A>,
    /// Refusal engine for generating receipts
    pub refusal_engine: Arc<R>,
    /// Packet normalizer for webhook conversion
    pub normalizer: Arc<N>,
    /// Optional MCP proxy base URL
    pub mcp_proxy_url: Option<String>,
    /// Gateway issuer identity
    pub gateway_identity: String,
}

impl<W, A, R, N> RouterState<W, A, R, N> {
    /// Create a new router state
    pub fn new(
        wip_gate: Arc<W>,
        auth_gate: Arc<A>,
        refusal_engine: Arc<R>,
        normalizer: Arc<N>,
        gateway_identity: String,
    ) -> Self {
        Self {
            wip_gate,
            auth_gate,
            refusal_engine,
            normalizer,
            mcp_proxy_url: None,
            gateway_identity,
        }
    }

    /// Set the MCP proxy URL
    pub fn with_mcp_proxy(mut self, url: String) -> Self {
        self.mcp_proxy_url = Some(url);
        self
    }
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Readiness check response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessResponse {
    pub ready: bool,
    pub checks: ReadinessChecks,
}

/// Individual readiness checks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessChecks {
    pub wip_gate: bool,
    pub auth_gate: bool,
    pub refusal_engine: bool,
    pub normalizer: bool,
}

/// Webhook request body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookRequest {
    pub service: String, // "gmail", "calendar", "drive"
    pub payload: serde_json::Value,
}

/// Webhook response with refusal receipt on error
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookResponse {
    pub success: bool,
    pub packet_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<RefusalReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// MCP proxy response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpProxyResponse {
    pub proxied: bool,
    pub status: u16,
    pub body: serde_json::Value,
}

/// Error response with receipt
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<RefusalReceipt>,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let status = if self.refusal.is_some() {
            StatusCode::UNPROCESSABLE_ENTITY
        } else {
            StatusCode::BAD_REQUEST
        };

        (status, Json(self)).into_response()
    }
}

/// Create the Axum router with all endpoints
pub fn create_router<W, A, R, N>(state: RouterState<W, A, R, N>) -> Router
where
    W: AsyncWipGate + 'static,
    A: AuthGate + 'static,
    R: RefusalEngine + 'static,
    N: PacketNormalizer + 'static,
{
    Router::new()
        // Health and readiness endpoints
        .route("/health", get(health_handler::<W, A, R, N>))
        .route("/ready", get(readiness_handler::<W, A, R, N>))
        // Webhook endpoints for Google Workspace APIs
        .route("/workspace/webhook", post(webhook_handler::<W, A, R, N>))
        .route(
            "/workspace/webhook/:service",
            post(webhook_handler_with_service::<W, A, R, N>),
        )
        // MCP proxy endpoint
        .route(
            "/mcp/*path",
            get(mcp_proxy_handler::<W, A, R, N>).post(mcp_proxy_handler::<W, A, R, N>),
        )
        .with_state(Arc::new(state))
}

/// Webhook handler with service extracted from request body
async fn webhook_handler<W, A, R, N>(
    State(state): State<Arc<RouterState<W, A, R, N>>>,
    _headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response
where
    W: AsyncWipGate,
    A: AuthGate,
    R: RefusalEngine,
    N: PacketNormalizer,
{
    // Try to acquire WIP permit
    let permit = match state.wip_gate.try_acquire().await {
        Ok(p) => p,
        Err(wip_err) => {
            let packet_id = uuid::Uuid::new_v4().to_string();
            let receipt = state
                .refusal_engine
                .refuse_from_wip_error(&packet_id, &wip_err)
                .await;

            warn!(
                "Webhook rejected due to WIP limit: {} ({})",
                receipt.packet_id, wip_err
            );

            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "wip_limit_exceeded".to_string(),
                    message: wip_err.to_string(),
                    refusal: Some(receipt),
                }),
            )
                .into_response();
        }
    };

    // Parse webhook request
    let webhook_req: WebhookRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            let packet_id = uuid::Uuid::new_v4().to_string();
            let receipt = state
                .refusal_engine
                .refuse_type_check_failed(
                    &packet_id,
                    crate::domain::TypeCheckErrorCode::MalformedPayload,
                    "WebhookRequest",
                    format!("Invalid JSON: {}", e).as_str(),
                )
                .await;

            error!("Failed to parse webhook: {} ({})", packet_id, e);

            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "invalid_webhook_format".to_string(),
                    message: format!("Failed to parse webhook: {}", e),
                    refusal: Some(receipt),
                }),
            )
                .into_response();
        }
    };

    debug!(
        "Received webhook for service: {} (size: {} bytes)",
        webhook_req.service,
        body.len()
    );

    webhook_handler_impl(state, webhook_req, permit)
        .await
        .into_response()
}

/// Webhook handler with service path parameter
async fn webhook_handler_with_service<W, A, R, N>(
    State(state): State<Arc<RouterState<W, A, R, N>>>,
    Path(service): Path<String>,
    _headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response
where
    W: AsyncWipGate,
    A: AuthGate,
    R: RefusalEngine,
    N: PacketNormalizer,
{
    // Try to acquire WIP permit
    let permit = match state.wip_gate.try_acquire().await {
        Ok(p) => p,
        Err(wip_err) => {
            let packet_id = uuid::Uuid::new_v4().to_string();
            let receipt = state
                .refusal_engine
                .refuse_from_wip_error(&packet_id, &wip_err)
                .await;

            warn!(
                "Webhook rejected due to WIP limit: {} ({})",
                receipt.packet_id, wip_err
            );

            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "wip_limit_exceeded".to_string(),
                    message: wip_err.to_string(),
                    refusal: Some(receipt),
                }),
            )
                .into_response();
        }
    };

    // Parse JSON payload
    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            let packet_id = uuid::Uuid::new_v4().to_string();
            let receipt = state
                .refusal_engine
                .refuse_type_check_failed(
                    &packet_id,
                    crate::domain::TypeCheckErrorCode::MalformedPayload,
                    &service,
                    format!("Invalid JSON: {}", e).as_str(),
                )
                .await;

            error!(
                "Failed to parse webhook payload for service {}: {} ({})",
                service, packet_id, e
            );

            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "invalid_webhook_format".to_string(),
                    message: format!("Failed to parse webhook payload: {}", e),
                    refusal: Some(receipt),
                }),
            )
                .into_response();
        }
    };

    let webhook_req = WebhookRequest { service, payload };

    debug!(
        "Received webhook for service: {} (size: {} bytes)",
        webhook_req.service,
        body.len()
    );

    webhook_handler_impl(state, webhook_req, permit)
        .await
        .into_response()
}

/// Common webhook handler implementation
async fn webhook_handler_impl<W, A, R, N>(
    state: Arc<RouterState<W, A, R, N>>,
    webhook_req: WebhookRequest,
    _permit: W::Permit,
) -> impl IntoResponse
where
    W: AsyncWipGate,
    A: AuthGate,
    R: RefusalEngine,
    N: PacketNormalizer,
{
    let service = webhook_req.service.to_lowercase();
    let packet_id = uuid::Uuid::new_v4().to_string();

    // Normalize webhook to typed packet
    let packet_result = match service.as_str() {
        "gmail" => state.normalizer.normalize_gmail(webhook_req.payload).await,
        "calendar" => {
            state
                .normalizer
                .normalize_calendar(webhook_req.payload)
                .await
        }
        "drive" => state.normalizer.normalize_drive(webhook_req.payload).await,
        _ => state.normalizer.normalize_auto(webhook_req.payload).await,
    };

    match packet_result {
        Ok(_packet) => {
            info!(
                "Webhook normalized successfully: {} (service: {})",
                packet_id, service
            );

            (
                StatusCode::ACCEPTED,
                Json(WebhookResponse {
                    success: true,
                    packet_id: Some(packet_id.clone()),
                    refusal: None,
                    message: Some(format!("Packet {} queued for processing", packet_id)),
                }),
            )
                .into_response()
        }
        Err(norm_err) => {
            let receipt = state
                .refusal_engine
                .refuse_type_check_failed(
                    &packet_id,
                    crate::domain::TypeCheckErrorCode::SchemaViolation,
                    &service,
                    format!("Normalization failed: {}", norm_err).as_str(),
                )
                .await;

            warn!(
                "Webhook normalization failed: {} (service: {}, error: {})",
                packet_id, service, norm_err
            );

            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ErrorResponse {
                    error: "normalization_failed".to_string(),
                    message: format!("Failed to normalize webhook: {}", norm_err),
                    refusal: Some(receipt),
                }),
            )
                .into_response()
        }
    }
}

/// MCP proxy handler - forwards requests to a2a-mcp service
async fn mcp_proxy_handler<W, A, R, N>(
    State(state): State<Arc<RouterState<W, A, R, N>>>,
    Path(path): Path<String>,
    _headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response
where
    W: AsyncWipGate,
    A: AuthGate,
    R: RefusalEngine,
    N: PacketNormalizer,
{
    let mcp_url = match &state.mcp_proxy_url {
        Some(url) => url.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "mcp_not_configured".to_string(),
                    message: "MCP proxy is not configured".to_string(),
                    refusal: None,
                }),
            )
                .into_response();
        }
    };

    // Try to acquire WIP permit for MCP request
    let _permit = match state.wip_gate.try_acquire().await {
        Ok(p) => p,
        Err(wip_err) => {
            let packet_id = uuid::Uuid::new_v4().to_string();
            let receipt = state
                .refusal_engine
                .refuse_from_wip_error(&packet_id, &wip_err)
                .await;

            warn!(
                "MCP request rejected due to WIP limit: {} ({})",
                receipt.packet_id, wip_err
            );

            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "wip_limit_exceeded".to_string(),
                    message: wip_err.to_string(),
                    refusal: Some(receipt),
                }),
            )
                .into_response();
        }
    };

    debug!("Proxying MCP request to path: {}", path);

    // Forward to MCP service
    let resp = match reqwest::Client::new()
        .post(format!("{}/{}", mcp_url, path))
        .body(body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("MCP proxy request failed: {}", e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: "mcp_proxy_failed".to_string(),
                    message: format!("Failed to proxy to MCP: {}", e),
                    refusal: None,
                }),
            )
                .into_response();
        }
    };

    let resp_status = resp.status();
    let axum_status =
        StatusCode::from_u16(resp_status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let response_bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            error!("MCP proxy response read failed: {}", e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: "mcp_response_read_failed".to_string(),
                    message: format!("Failed to read MCP response: {}", e),
                    refusal: None,
                }),
            )
                .into_response();
        }
    };

    let response_body = match serde_json::from_slice::<serde_json::Value>(&response_bytes) {
        Ok(json) => json,
        Err(_) => {
            // Try to return as string if not JSON
            serde_json::Value::String(String::from_utf8_lossy(&response_bytes).to_string())
        }
    };

    info!("MCP proxy response: {} (status: {})", path, resp_status);

    (
        axum_status,
        Json(McpProxyResponse {
            proxied: true,
            status: resp_status.as_u16(),
            body: response_body,
        }),
    )
        .into_response()
}
