//! Health check endpoints for the A2A protocol server
//!
//! Provides comprehensive health monitoring including:
//! - Basic liveness checks
//! - Readiness checks (dependency verification)
//! - Detailed health metrics
//! - Component status tracking

#[cfg(feature = "http-server")]
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

/// Health check status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// The service is healthy
    Healthy,
    /// The service is degraded but operational
    Degraded,
    /// The service is unhealthy
    Unhealthy,
}

impl HealthStatus {
    /// Returns true if the status is healthy or degraded
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }
}

/// Component health status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentHealth {
    /// Component name
    pub name: String,

    /// Component status
    pub status: HealthStatus,

    /// Optional message describing the status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Optional metrics specific to this component
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<serde_json::Value>,
}

impl ComponentHealth {
    /// Create a new component health
    pub fn new(name: String, status: HealthStatus) -> Self {
        Self {
            name,
            status,
            message: None,
            metrics: None,
        }
    }

    /// Add a message
    pub fn with_message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }

    /// Add metrics
    pub fn with_metrics(mut self, metrics: serde_json::Value) -> Self {
        self.metrics = Some(metrics);
        self
    }
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResponse {
    /// Overall health status
    pub status: HealthStatus,

    /// Timestamp of the health check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,

    /// Service version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Individual component health
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<ComponentHealth>>,

    /// Overall uptime in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_seconds: Option<f64>,

    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

impl HealthCheckResponse {
    /// Create a new health check response
    pub fn new(status: HealthStatus) -> Self {
        Self {
            status,
            timestamp: Some(chrono::Utc::now()),
            version: None,
            components: None,
            uptime_seconds: None,
            metadata: None,
        }
    }

    /// Add version
    pub fn with_version(mut self, version: String) -> Self {
        self.version = Some(version);
        self
    }

    /// Add components
    pub fn with_components(mut self, components: Vec<ComponentHealth>) -> Self {
        self.components = Some(components);
        self
    }

    /// Add uptime
    pub fn with_uptime(mut self, uptime: Duration) -> Self {
        self.uptime_seconds = Some(uptime.as_secs_f64());
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Readiness check response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessCheckResponse {
    /// Whether the service is ready
    pub ready: bool,

    /// Optional message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Dependency status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<HashMap<String, bool>>,
}

impl ReadinessCheckResponse {
    /// Create a new readiness response
    pub fn new(ready: bool) -> Self {
        Self {
            ready,
            message: None,
            dependencies: None,
        }
    }

    /// Add a message
    pub fn with_message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }

    /// Add dependencies
    pub fn with_dependencies(mut self, dependencies: HashMap<String, bool>) -> Self {
        self.dependencies = Some(dependencies);
        self
    }
}

/// Liveness check response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LivenessCheckResponse {
    /// Whether the service is alive
    pub alive: bool,

    /// Timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

impl LivenessCheckResponse {
    /// Create a new liveness response
    pub fn new(alive: bool) -> Self {
        Self {
            alive,
            timestamp: Some(chrono::Utc::now()),
        }
    }
}

/// Health checker for tracking component health
#[cfg(feature = "http-server")]
#[derive(Clone)]
pub struct HealthChecker {
    /// Service start time
    start_time: Instant,

    /// Service version
    version: Option<String>,

    /// Component status trackers
    components: Arc<tokio::sync::RwLock<HashMap<String, ComponentHealth>>>,
}

#[cfg(feature = "http-server")]
impl HealthChecker {
    /// Create a new health checker
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            version: None,
            components: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Set the service version
    pub fn with_version(mut self, version: String) -> Self {
        self.version = Some(version);
        self
    }

    /// Register a component
    pub async fn register_component(&self, name: String, status: HealthStatus) {
        let mut components = self.components.write().await;
        components.insert(name.clone(), ComponentHealth::new(name, status));
    }

    /// Update component status
    pub async fn update_component(&self, name: &str, status: HealthStatus) {
        let mut components = self.components.write().await;
        if let Some(component) = components.get_mut(name) {
            component.status = status;
        }
    }

    /// Update component with message
    pub async fn update_component_with_message(
        &self,
        name: &str,
        status: HealthStatus,
        message: String,
    ) {
        let mut components = self.components.write().await;
        if let Some(component) = components.get_mut(name) {
            component.status = status;
            component.message = Some(message);
        }
    }

    /// Update component with metrics
    pub async fn update_component_with_metrics(
        &self,
        name: &str,
        status: HealthStatus,
        metrics: serde_json::Value,
    ) {
        let mut components = self.components.write().await;
        if let Some(component) = components.get_mut(name) {
            component.status = status;
            component.metrics = Some(metrics);
        }
    }

    /// Get overall health status
    pub async fn get_health_status(&self) -> HealthStatus {
        let components = self.components.read().await;

        if components.is_empty() {
            return HealthStatus::Healthy;
        }

        let has_unhealthy = components.values().any(|c| c.status == HealthStatus::Unhealthy);
        let has_degraded = components.values().any(|c| c.status == HealthStatus::Degraded);

        if has_unhealthy {
            HealthStatus::Unhealthy
        } else if has_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    /// Build health check response
    pub async fn build_response(&self) -> HealthCheckResponse {
        let status = self.get_health_status().await;
        let components = self.components.read().await;
        let component_vec: Vec<ComponentHealth> = components.values().cloned().collect();

        let mut response = HealthCheckResponse::new(status);

        if let Some(ref version) = self.version {
            response = response.with_version(version.clone());
        }

        if !component_vec.is_empty() {
            response = response.with_components(component_vec);
        }

        response = response.with_uptime(self.start_time.elapsed());
        response
    }

    /// Check if all critical components are healthy
    pub async fn is_ready(&self, critical_components: &[String]) -> bool {
        let components = self.components.read().await;

        for name in critical_components {
            if let Some(component) = components.get(name) {
                if !component.status.is_operational() {
                    return false;
                }
            } else {
                // Critical component not registered
                return false;
            }
        }

        true
    }
}

#[cfg(feature = "http-server")]
impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Health check handlers
#[cfg(feature = "http-server")]
pub mod handlers {
    use super::*;
    use axum::{extract::State, Json};

    /// Handle liveness check
    pub async fn liveness() -> Json<LivenessCheckResponse> {
        Json(LivenessCheckResponse::new(true))
    }

    /// Handle readiness check
    pub async fn readiness(
        State(checker): State<HealthChecker>,
    ) -> Json<ReadinessCheckResponse> {
        // In production, you might want to specify critical components
        let critical_components = vec![];

        let ready = checker.is_ready(&critical_components).await;

        let mut response = ReadinessCheckResponse::new(ready);

        if ready {
            response = response.with_message("Service is ready".to_string());
        } else {
            response = response.with_message("Service is not ready".to_string());
        }

        Json(response)
    }

    /// Handle detailed health check
    pub async fn health(State(checker): State<HealthChecker>) -> Json<HealthCheckResponse> {
        Json(checker.build_response().await)
    }
}

#[cfg(all(test, feature = "http-server"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_checker() {
        let checker = HealthChecker::new().with_version("1.0.0".to_string());

        // Register components
        checker.register_component("database".to_string(), HealthStatus::Healthy).await;
        checker.register_component("cache".to_string(), HealthStatus::Healthy).await;

        // Check status
        let status = checker.get_health_status().await;
        assert_eq!(status, HealthStatus::Healthy);

        // Update component to degraded
        checker.update_component("cache", HealthStatus::Degraded).await;
        let status = checker.get_health_status().await;
        assert_eq!(status, HealthStatus::Degraded);

        // Update component to unhealthy
        checker.update_component("database", HealthStatus::Unhealthy).await;
        let status = checker.get_health_status().await;
        assert_eq!(status, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_readiness_check() {
        let checker = HealthChecker::new();

        let critical = vec!["database".to_string()];

        // Not ready when component not registered
        assert!(!checker.is_ready(&critical).await);

        // Register and still not ready (unhealthy)
        checker
            .register_component("database".to_string(), HealthStatus::Unhealthy)
            .await;
        assert!(!checker.is_ready(&critical).await);

        // Ready when healthy
        checker
            .update_component("database", HealthStatus::Healthy)
            .await;
        assert!(checker.is_ready(&critical).await);
    }
}
