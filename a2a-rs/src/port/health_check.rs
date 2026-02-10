//! Health check port definitions
//!
//! Defines the contract for checking the health and readiness of application dependencies.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Health check status for a single dependency
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Dependency is healthy and ready
    Healthy,
    /// Dependency is degraded but functional
    Degraded,
    /// Dependency is unhealthy
    Unhealthy,
}

/// Result of a health check for a single dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResult {
    /// Name of the dependency being checked
    pub name: String,
    /// Health status
    pub status: HealthStatus,
    /// Optional message providing additional context
    pub message: Option<String>,
    /// Duration taken to perform the check
    pub check_duration: Duration,
    /// Optional details about the check (e.g., metrics, debug info)
    pub details: Option<serde_json::Value>,
}

impl HealthCheckResult {
    /// Create a healthy result
    pub fn healthy(name: impl Into<String>, duration: Duration) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Healthy,
            message: None,
            check_duration: duration,
            details: None,
        }
    }

    /// Create a degraded result
    pub fn degraded(name: impl Into<String>, message: impl Into<String>, duration: Duration) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Degraded,
            message: Some(message.into()),
            check_duration: duration,
            details: None,
        }
    }

    /// Create an unhealthy result
    pub fn unhealthy(name: impl Into<String>, message: impl Into<String>, duration: Duration) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Unhealthy,
            message: Some(message.into()),
            check_duration: duration,
            details: None,
        }
    }

    /// Add optional details to the result
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Check if the result is healthy
    pub fn is_healthy(&self) -> bool {
        self.status == HealthStatus::Healthy
    }
}

/// Aggregated health check results for multiple dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedHealthResult {
    /// Overall status (AND logic: healthy only if all are healthy)
    pub status: HealthStatus,
    /// Individual check results
    pub checks: Vec<HealthCheckResult>,
    /// Total duration of all checks
    pub total_duration: Duration,
}

impl AggregatedHealthResult {
    /// Create a new aggregated result from individual checks
    pub fn new(checks: Vec<HealthCheckResult>) -> Self {
        let total_duration = checks.iter().map(|c| c.check_duration).sum();

        // AND logic: all must be healthy for overall healthy status
        let status = if checks.iter().all(|c| c.status == HealthStatus::Healthy) {
            HealthStatus::Healthy
        } else if checks.iter().any(|c| c.status == HealthStatus::Unhealthy) {
            HealthStatus::Unhealthy
        } else {
            HealthStatus::Degraded
        };

        Self {
            status,
            checks,
            total_duration,
        }
    }

    /// Check if all dependencies are healthy
    pub fn is_healthy(&self) -> bool {
        self.status == HealthStatus::Healthy
    }

    /// Get all unhealthy checks
    pub fn unhealthy_checks(&self) -> Vec<&HealthCheckResult> {
        self.checks
            .iter()
            .filter(|c| c.status == HealthStatus::Unhealthy)
            .collect()
    }
}

/// Port for health checking individual dependencies
///
/// Implementations should be fast (< 100ms) and non-blocking.
#[async_trait]
pub trait HealthCheck: Send + Sync {
    /// Perform a health check
    ///
    /// Should complete quickly (< 100ms recommended) to avoid blocking
    /// health check endpoints.
    async fn check(&self) -> HealthCheckResult;

    /// Get the name of this health check
    fn name(&self) -> &str;
}

/// Port for readiness checking with multiple dependencies
///
/// Aggregates multiple health checks with AND logic: the system is ready
/// only if all dependencies are healthy.
#[async_trait]
pub trait ReadinessCheck: Send + Sync {
    /// Perform readiness check on all dependencies
    ///
    /// Executes all checks in parallel and aggregates results.
    /// Returns detailed status for each dependency.
    async fn check_readiness(&self) -> AggregatedHealthResult;

    /// Get the list of dependency names being checked
    fn dependencies(&self) -> Vec<String>;
}
