//! Service health check domain types
//!
//! This module defines types for liveness and readiness probes of the local service,
//! distinct from the agent health types in `discovery.rs` which are for service registry.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Overall health status of the service
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServiceHealthStatus {
    /// All checks passed - service is healthy
    Healthy,
    /// Some non-critical checks failed - service is degraded but operational
    Degraded,
    /// Critical checks failed - service is unhealthy
    Unhealthy,
}

impl ServiceHealthStatus {
    /// Returns true if the service can serve traffic
    pub fn is_ready(&self) -> bool {
        matches!(self, ServiceHealthStatus::Healthy | ServiceHealthStatus::Degraded)
    }

    /// Returns true if the service is alive (not necessarily ready)
    pub fn is_alive(&self) -> bool {
        // Even unhealthy services are alive if they can respond
        true
    }

    /// Returns HTTP status code for this health status
    pub fn http_status_code(&self) -> u16 {
        match self {
            ServiceHealthStatus::Healthy | ServiceHealthStatus::Degraded => 200,
            ServiceHealthStatus::Unhealthy => 503,
        }
    }
}

/// Result of a single health check
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckStatus {
    /// Name of the check
    pub name: String,
    /// Status of this check
    pub status: ServiceHealthStatus,
    /// Optional error message if the check failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Duration of the check in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// When the check was performed
    pub checked_at: DateTime<Utc>,
}

impl HealthCheckStatus {
    /// Create a healthy check result
    pub fn healthy(name: String) -> Self {
        Self {
            name,
            status: ServiceHealthStatus::Healthy,
            message: None,
            duration_ms: None,
            checked_at: Utc::now(),
        }
    }

    /// Create a degraded check result
    pub fn degraded(name: String, message: String) -> Self {
        Self {
            name,
            status: ServiceHealthStatus::Degraded,
            message: Some(message),
            duration_ms: None,
            checked_at: Utc::now(),
        }
    }

    /// Create an unhealthy check result
    pub fn unhealthy(name: String, message: String) -> Self {
        Self {
            name,
            status: ServiceHealthStatus::Unhealthy,
            message: Some(message),
            duration_ms: None,
            checked_at: Utc::now(),
        }
    }

    /// Set the duration of the check
    pub fn with_duration_ms(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }
}

/// Complete service health report
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceHealthReport {
    /// Overall service status
    pub status: ServiceHealthStatus,
    /// Individual check results
    pub checks: HashMap<String, HealthCheckStatus>,
    /// When the report was generated
    pub timestamp: DateTime<Utc>,
    /// Version of the service
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl ServiceHealthReport {
    /// Create a new health report from individual checks
    pub fn new(checks: HashMap<String, HealthCheckStatus>) -> Self {
        let status = Self::compute_overall_status(&checks);
        Self {
            status,
            checks,
            timestamp: Utc::now(),
            version: None,
        }
    }

    /// Add version information
    pub fn with_version(mut self, version: String) -> Self {
        self.version = Some(version);
        self
    }

    /// Compute overall status from individual checks
    fn compute_overall_status(checks: &HashMap<String, HealthCheckStatus>) -> ServiceHealthStatus {
        if checks.is_empty() {
            return ServiceHealthStatus::Healthy;
        }

        let has_unhealthy = checks.values().any(|c| c.status == ServiceHealthStatus::Unhealthy);
        let has_degraded = checks.values().any(|c| c.status == ServiceHealthStatus::Degraded);

        if has_unhealthy {
            ServiceHealthStatus::Unhealthy
        } else if has_degraded {
            ServiceHealthStatus::Degraded
        } else {
            ServiceHealthStatus::Healthy
        }
    }

    /// Returns true if the service is ready to serve traffic
    pub fn is_ready(&self) -> bool {
        self.status.is_ready()
    }

    /// Returns true if the service is alive
    pub fn is_alive(&self) -> bool {
        self.status.is_alive()
    }

    /// Returns HTTP status code for this health report
    pub fn http_status_code(&self) -> u16 {
        self.status.http_status_code()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_health_status_ready() {
        assert!(ServiceHealthStatus::Healthy.is_ready());
        assert!(ServiceHealthStatus::Degraded.is_ready());
        assert!(!ServiceHealthStatus::Unhealthy.is_ready());
    }

    #[test]
    fn test_service_health_status_http_codes() {
        assert_eq!(ServiceHealthStatus::Healthy.http_status_code(), 200);
        assert_eq!(ServiceHealthStatus::Degraded.http_status_code(), 200);
        assert_eq!(ServiceHealthStatus::Unhealthy.http_status_code(), 503);
    }

    #[test]
    fn test_health_check_status_constructors() {
        let healthy = HealthCheckStatus::healthy("db".to_string());
        assert_eq!(healthy.status, ServiceHealthStatus::Healthy);
        assert!(healthy.message.is_none());

        let degraded = HealthCheckStatus::degraded("cache".to_string(), "slow".to_string());
        assert_eq!(degraded.status, ServiceHealthStatus::Degraded);
        assert_eq!(degraded.message, Some("slow".to_string()));

        let unhealthy = HealthCheckStatus::unhealthy("api".to_string(), "timeout".to_string());
        assert_eq!(unhealthy.status, ServiceHealthStatus::Unhealthy);
        assert_eq!(unhealthy.message, Some("timeout".to_string()));
    }

    #[test]
    fn test_service_health_report_overall_status() {
        let mut checks = HashMap::new();
        checks.insert(
            "db".to_string(),
            HealthCheckStatus::healthy("db".to_string()),
        );
        checks.insert(
            "cache".to_string(),
            HealthCheckStatus::healthy("cache".to_string()),
        );

        let report = ServiceHealthReport::new(checks);
        assert_eq!(report.status, ServiceHealthStatus::Healthy);
        assert!(report.is_ready());
    }

    #[test]
    fn test_service_health_report_degraded() {
        let mut checks = HashMap::new();
        checks.insert(
            "db".to_string(),
            HealthCheckStatus::healthy("db".to_string()),
        );
        checks.insert(
            "cache".to_string(),
            HealthCheckStatus::degraded("cache".to_string(), "slow".to_string()),
        );

        let report = ServiceHealthReport::new(checks);
        assert_eq!(report.status, ServiceHealthStatus::Degraded);
        assert!(report.is_ready());
    }

    #[test]
    fn test_service_health_report_unhealthy() {
        let mut checks = HashMap::new();
        checks.insert(
            "db".to_string(),
            HealthCheckStatus::unhealthy("db".to_string(), "connection failed".to_string()),
        );

        let report = ServiceHealthReport::new(checks);
        assert_eq!(report.status, ServiceHealthStatus::Unhealthy);
        assert!(!report.is_ready());
        assert_eq!(report.http_status_code(), 503);
    }

    #[test]
    fn test_service_health_report_serialization() {
        let mut checks = HashMap::new();
        checks.insert(
            "liveness".to_string(),
            HealthCheckStatus::healthy("liveness".to_string()),
        );

        let report = ServiceHealthReport::new(checks).with_version("1.0.0".to_string());

        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["status"], "healthy");
        assert_eq!(json["version"], "1.0.0");
        assert!(json["checks"].is_object());
    }
}
