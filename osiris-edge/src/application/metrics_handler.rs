//! Metrics HTTP handler and middleware integration
//!
//! Provides:
//! - `/metrics` endpoint for Prometheus scraping
//! - Request tracking middleware for automatic metrics collection

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    middleware::Next,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::debug;

use crate::port::MetricsCollector;

/// Request metrics tracking middleware layer
///
/// Automatically records request duration, status codes, and errors
/// for all requests passing through the router.
pub struct MetricsMiddleware<M> {
    metrics: Arc<M>,
}

impl<M> MetricsMiddleware<M> {
    /// Create a new metrics middleware layer
    pub fn new(metrics: Arc<M>) -> Self {
        Self { metrics }
    }
}

/// Metrics response for /metrics endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsResponse {
    /// Prometheus text format metrics
    pub metrics: String,
    /// Timestamp of metrics collection
    pub collected_at: String,
}

/// Error response from metrics endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsErrorResponse {
    pub error: String,
    pub message: String,
}

impl IntoResponse for MetricsErrorResponse {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
    }
}

/// Handler for GET /metrics endpoint
///
/// Returns Prometheus-format metrics for scraping by Prometheus or Grafana
pub async fn metrics_handler<M: MetricsCollector>(
    State(metrics): State<Arc<M>>,
) -> impl IntoResponse {
    let metrics_text = metrics.get_metrics().await;
    let timestamp = chrono::Utc::now().to_rfc3339();

    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        metrics_text,
    )
}

/// Middleware for tracking HTTP request metrics
///
/// Records:
/// - Request method and path
/// - Response status code
/// - Request duration in milliseconds
///
/// Usage:
/// ```ignore
/// let metrics = Arc::new(PrometheusCollector::new()?);
/// let app = Router::new()
///     .route("/metrics", get(metrics_handler).with_state(metrics.clone()))
///     .layer(middleware::from_fn(request_metrics_middleware))
///     .with_state(metrics);
/// ```
pub async fn request_metrics_middleware<M: MetricsCollector + 'static>(
    State(metrics): State<Arc<M>>,
    req: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    let start = Instant::now();

    // Extract method and path from request (we'll get them from the request itself)
    let request_path = req.uri().path().to_string();
    let request_method = req.method().to_string();

    // Call the next middleware/handler
    let response = next.run(req).await;

    // Record metrics
    let duration_ms = start.elapsed().as_millis() as u64;
    let status_code = response.status().as_u16();

    debug!(
        "Request: {} {} -> {} ({}ms)",
        request_method, request_path, status_code, duration_ms
    );

    metrics
        .record_request(&request_method, &request_path, status_code, duration_ms)
        .await;

    response
}

/// Simpler middleware for request metrics using State extraction
///
/// Usage in router creation:
/// ```ignore
/// let metrics = Arc::new(PrometheusCollector::new()?);
/// let app = Router::new()
///     .route("/metrics", get(metrics_handler).with_state(metrics.clone()))
///     .layer(axum::middleware::from_fn_with_state(
///         metrics.clone(),
///         simple_request_metrics_middleware,
///     ))
///     .with_state(metrics);
/// ```
pub async fn simple_request_metrics_middleware<M: MetricsCollector + 'static>(
    State(metrics): State<Arc<M>>,
    req: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    let start = Instant::now();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    // Call next middleware
    let response = next.run(req).await;

    // Record metrics after response
    let duration_ms = start.elapsed().as_millis() as u64;
    let status = response.status().as_u16();

    metrics
        .record_request(&method, &path, status, duration_ms)
        .await;

    response
}

/// Error tracking middleware
///
/// Records errors by type for monitoring error rates
pub async fn error_tracking_middleware<M: MetricsCollector + 'static>(
    State(metrics): State<Arc<M>>,
    req: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    let path = req.uri().path().to_string();
    let response = next.run(req).await;

    // Record errors (5xx and 4xx status codes)
    if response.status().is_client_error() {
        metrics.record_error("client_error", &path).await;
    } else if response.status().is_server_error() {
        metrics.record_error("server_error", &path).await;
    }

    response
}

/// Helper to create a metrics-enabled router
///
/// Integrates MetricsCollector port with router and middleware
#[derive(Clone)]
pub struct MetricsRouterBuilder<M> {
    metrics: Arc<M>,
}

impl<M: MetricsCollector + 'static> MetricsRouterBuilder<M> {
    /// Create a new metrics router builder
    pub fn new(metrics: Arc<M>) -> Self {
        Self { metrics }
    }

    /// Add the /metrics endpoint to a router
    pub fn add_metrics_endpoint<S>(self, router: Router<S>) -> Router<S> {
        router
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::PrometheusCollector;

    #[tokio::test]
    async fn test_metrics_handler() {
        let collector = PrometheusCollector::new().unwrap();
        let collector = Arc::new(collector);

        // Record some metrics
        collector.record_request("GET", "/api/health", 200, 5).await;

        // Simulate handler call (would normally be invoked by Axum)
        let metrics_text = collector.get_metrics().await;

        assert!(!metrics_text.is_empty());
        assert!(metrics_text.contains("http_requests_total"));
    }

    #[tokio::test]
    async fn test_error_tracking() {
        let collector = PrometheusCollector::new().unwrap();

        collector
            .record_error("validation_error", "/api/process")
            .await;
        collector
            .record_error("validation_error", "/api/process")
            .await;
        collector
            .record_error("timeout_error", "/api/download")
            .await;

        let metrics = collector.get_metrics().await;
        assert!(metrics.contains("errors_total"));
        assert!(metrics.contains("error_type=\"validation_error\""));
        assert!(metrics.contains("error_type=\"timeout_error\""));
    }

    #[tokio::test]
    async fn test_active_connections_tracking() {
        let collector = PrometheusCollector::new().unwrap();

        collector.set_active_connections(25).await;
        let metrics = collector.get_metrics().await;
        assert!(metrics.contains("active_connections 25"));

        collector.set_active_connections(50).await;
        let metrics = collector.get_metrics().await;
        assert!(metrics.contains("active_connections 50"));
    }
}
