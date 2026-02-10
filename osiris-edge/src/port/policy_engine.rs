//! Policy engine port - defines RBAC policy evaluation interface
//!
//! Port trait for dynamic policy evaluation with role-based access control (RBAC).

use crate::domain::{EdgeError, HierarchicalIdentity, Policy, TenantId};
use async_trait::async_trait;
use std::collections::HashMap;

/// Context for policy evaluation
#[derive(Debug, Clone)]
pub struct EvaluationContext {
    /// Request attributes for condition evaluation
    pub attributes: HashMap<String, serde_json::Value>,
}

impl EvaluationContext {
    pub fn new() -> Self {
        Self {
            attributes: HashMap::new(),
        }
    }

    pub fn with_attribute(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.attributes.insert(key.into(), value);
        self
    }

    pub fn add_attribute(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.attributes.insert(key.into(), value);
    }
}

impl Default for EvaluationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of policy evaluation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Access explicitly allowed
    Allow,
    /// Access explicitly denied
    Deny,
    /// No matching policy (default deny)
    Undecided,
}

impl Decision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Decision::Allow)
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, Decision::Deny)
    }
}

/// Port interface for policy engine
///
/// Evaluates RBAC policies and dynamic conditions to determine access decisions.
/// Supports role inheritance and hierarchical permissions.
#[async_trait]
pub trait PolicyEngine: Send + Sync {
    /// Evaluate whether an identity has permission for an action on a resource
    ///
    /// # Arguments
    ///
    /// * `identity` - The hierarchical identity
    /// * `action` - The action being performed (e.g., "read", "write")
    /// * `resource` - The resource identifier (e.g., "bucket:my-bucket")
    /// * `context` - Additional context for condition evaluation
    ///
    /// # Returns
    ///
    /// * `Ok(Decision)` - The access decision
    /// * `Err(EdgeError)` - Evaluation error
    async fn evaluate(
        &self,
        identity: &HierarchicalIdentity,
        action: &str,
        resource: &str,
        context: &EvaluationContext,
    ) -> Result<Decision, EdgeError>;

    /// Get effective permissions for an identity (all resolved permissions)
    ///
    /// # Arguments
    ///
    /// * `identity` - The hierarchical identity
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<(action, resource)>)` - List of allowed action-resource pairs
    /// * `Err(EdgeError)` - Retrieval error
    async fn get_effective_permissions(
        &self,
        identity: &HierarchicalIdentity,
    ) -> Result<Vec<(String, String)>, EdgeError>;

    /// Add a dynamic policy for a tenant
    ///
    /// # Arguments
    ///
    /// * `tenant_id` - The tenant identifier
    /// * `policy` - The policy to add
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Policy added successfully
    /// * `Err(EdgeError)` - Addition failed
    async fn add_policy(&self, tenant_id: &TenantId, policy: Policy) -> Result<(), EdgeError>;

    /// Remove a policy
    ///
    /// # Arguments
    ///
    /// * `tenant_id` - The tenant identifier
    /// * `policy_id` - The policy identifier
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Policy removed successfully
    /// * `Err(EdgeError)` - Removal failed
    async fn remove_policy(&self, tenant_id: &TenantId, policy_id: &str) -> Result<(), EdgeError>;

    /// List all policies for a tenant
    ///
    /// # Arguments
    ///
    /// * `tenant_id` - The tenant identifier
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<Policy>)` - List of policies
    /// * `Err(EdgeError)` - Retrieval error
    async fn list_policies(&self, tenant_id: &TenantId) -> Result<Vec<Policy>, EdgeError>;

    /// Check if a specific permission is granted
    ///
    /// Convenience method that returns a boolean instead of Decision.
    ///
    /// # Arguments
    ///
    /// * `identity` - The hierarchical identity
    /// * `action` - The action being performed
    /// * `resource` - The resource identifier
    /// * `context` - Additional context for condition evaluation
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Permission granted
    /// * `Ok(false)` - Permission denied or undecided
    /// * `Err(EdgeError)` - Evaluation error
    async fn is_allowed(
        &self,
        identity: &HierarchicalIdentity,
        action: &str,
        resource: &str,
        context: &EvaluationContext,
    ) -> Result<bool, EdgeError> {
        let decision = self.evaluate(identity, action, resource, context).await?;
        Ok(decision.is_allowed())
    }
}
