//! Dynamic policy engine adapter
//!
//! Implements RBAC policy evaluation with role inheritance and dynamic conditions.

use crate::domain::{
    Condition, EdgeError, Effect, HierarchicalIdentity, Permission, Policy, Role, TenantId,
};
use crate::port::{Decision, EvaluationContext, PolicyEngine, TenantManager};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Dynamic policy engine with RBAC support
///
/// Evaluates permissions based on:
/// - Direct role permissions
/// - Inherited role permissions (role hierarchy)
/// - Dynamic policies with conditions
/// - Deny-by-default with explicit allow
pub struct DynamicPolicyEngine<T: TenantManager> {
    tenant_manager: Arc<T>,
}

impl<T: TenantManager> DynamicPolicyEngine<T> {
    pub fn new(tenant_manager: Arc<T>) -> Self {
        Self { tenant_manager }
    }

    /// Resolve all roles for an identity (including inherited roles)
    async fn resolve_roles(&self, identity: &HierarchicalIdentity) -> Result<Vec<Role>, EdgeError> {
        let tenant_config = self
            .tenant_manager
            .get_tenant(&identity.tenant_id)
            .await?
            .ok_or_else(|| {
                EdgeError::ConfigError(format!("Tenant {} not found", identity.tenant_id.as_str()))
            })?;

        let role_bindings = self.tenant_manager.get_role_bindings(identity).await?;

        let mut resolved_roles = Vec::new();
        let mut visited_roles = HashSet::new();

        for binding in role_bindings {
            self.resolve_role_recursive(
                &binding.role_name,
                &tenant_config.roles,
                &mut resolved_roles,
                &mut visited_roles,
            )?;
        }

        Ok(resolved_roles)
    }

    /// Recursively resolve role inheritance
    fn resolve_role_recursive(
        &self,
        role_name: &str,
        all_roles: &HashMap<String, Role>,
        resolved: &mut Vec<Role>,
        visited: &mut HashSet<String>,
    ) -> Result<(), EdgeError> {
        if visited.contains(role_name) {
            return Ok(()); // Prevent infinite loops
        }

        visited.insert(role_name.to_string());

        if let Some(role) = all_roles.get(role_name) {
            // First resolve inherited roles
            for parent_role_name in &role.inherits_from {
                self.resolve_role_recursive(parent_role_name, all_roles, resolved, visited)?;
            }

            // Then add this role
            resolved.push(role.clone());
        }

        Ok(())
    }

    /// Get all permissions from resolved roles
    fn get_permissions_from_roles(&self, roles: &[Role]) -> Vec<Permission> {
        let mut permissions = Vec::new();
        for role in roles {
            permissions.extend(role.permissions.iter().cloned());
        }
        permissions
    }

    /// Evaluate dynamic policies
    async fn evaluate_policies(
        &self,
        identity: &HierarchicalIdentity,
        action: &str,
        resource: &str,
        context: &EvaluationContext,
    ) -> Result<Option<Effect>, EdgeError> {
        let tenant_config = self
            .tenant_manager
            .get_tenant(&identity.tenant_id)
            .await?
            .ok_or_else(|| {
                EdgeError::ConfigError(format!("Tenant {} not found", identity.tenant_id.as_str()))
            })?;

        let mut policies = tenant_config.policies.clone();
        // Sort by priority (higher priority first)
        policies.sort_by(|a, b| b.priority.cmp(&a.priority));

        for policy in policies {
            for rule in policy.rules {
                if rule.matches(action, resource) {
                    // Check conditions
                    let conditions_met = self.evaluate_conditions(&rule.conditions, context)?;
                    if conditions_met {
                        return Ok(Some(rule.effect));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Evaluate policy conditions
    fn evaluate_conditions(
        &self,
        conditions: &[Condition],
        context: &EvaluationContext,
    ) -> Result<bool, EdgeError> {
        for condition in conditions {
            if !self.evaluate_condition(condition, context)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Evaluate a single condition
    fn evaluate_condition(
        &self,
        condition: &Condition,
        context: &EvaluationContext,
    ) -> Result<bool, EdgeError> {
        match condition {
            Condition::StringEquals { key, value } => {
                if let Some(attr_value) = context.attributes.get(key) {
                    Ok(attr_value.as_str() == Some(value.as_str()))
                } else {
                    Ok(false)
                }
            }
            Condition::StringLike { key, pattern } => {
                if let Some(attr_value) = context.attributes.get(key) {
                    if let Some(s) = attr_value.as_str() {
                        // Simple wildcard matching (* at start or end)
                        if pattern.starts_with('*') && pattern.ends_with('*') {
                            let inner = &pattern[1..pattern.len() - 1];
                            Ok(s.contains(inner))
                        } else if pattern.starts_with('*') {
                            Ok(s.ends_with(&pattern[1..]))
                        } else if pattern.ends_with('*') {
                            Ok(s.starts_with(&pattern[..pattern.len() - 1]))
                        } else {
                            Ok(s == pattern)
                        }
                    } else {
                        Ok(false)
                    }
                } else {
                    Ok(false)
                }
            }
            Condition::NumericLessThan { key, value } => {
                if let Some(attr_value) = context.attributes.get(key) {
                    if let Some(n) = attr_value.as_i64() {
                        Ok(n < *value)
                    } else {
                        Ok(false)
                    }
                } else {
                    Ok(false)
                }
            }
            Condition::NumericGreaterThan { key, value } => {
                if let Some(attr_value) = context.attributes.get(key) {
                    if let Some(n) = attr_value.as_i64() {
                        Ok(n > *value)
                    } else {
                        Ok(false)
                    }
                } else {
                    Ok(false)
                }
            }
            Condition::IpAddress { key, cidr: _ } => {
                // Simplified IP check - just verify key exists
                // Full CIDR matching would require additional dependencies
                Ok(context.attributes.contains_key(key))
            }
            Condition::TimeWindowUtc {
                start_hour,
                end_hour,
            } => {
                let now = Utc::now();
                let current_hour = now.hour() as u8;
                Ok(current_hour >= *start_hour && current_hour < *end_hour)
            }
        }
    }
}

#[async_trait]
impl<T: TenantManager> PolicyEngine for DynamicPolicyEngine<T> {
    async fn evaluate(
        &self,
        identity: &HierarchicalIdentity,
        action: &str,
        resource: &str,
        context: &EvaluationContext,
    ) -> Result<Decision, EdgeError> {
        // 1. Check if tenant is enabled
        if !self
            .tenant_manager
            .is_tenant_enabled(&identity.tenant_id)
            .await?
        {
            return Ok(Decision::Deny);
        }

        // 2. Evaluate dynamic policies (highest priority)
        if let Some(effect) = self
            .evaluate_policies(identity, action, resource, context)
            .await?
        {
            return Ok(match effect {
                Effect::Allow => Decision::Allow,
                Effect::Deny => Decision::Deny,
            });
        }

        // 3. Evaluate role-based permissions
        let roles = self.resolve_roles(identity).await?;
        let permissions = self.get_permissions_from_roles(&roles);

        for permission in permissions {
            if permission.matches(action, resource) {
                return Ok(match permission.effect {
                    Effect::Allow => Decision::Allow,
                    Effect::Deny => Decision::Deny,
                });
            }
        }

        // 4. Default deny
        Ok(Decision::Undecided)
    }

    async fn get_effective_permissions(
        &self,
        identity: &HierarchicalIdentity,
    ) -> Result<Vec<(String, String)>, EdgeError> {
        let roles = self.resolve_roles(identity).await?;
        let permissions = self.get_permissions_from_roles(&roles);

        let effective_permissions: Vec<(String, String)> = permissions
            .iter()
            .filter(|p| p.effect == Effect::Allow)
            .map(|p| (p.action.clone(), p.resource.clone()))
            .collect();

        Ok(effective_permissions)
    }

    async fn add_policy(&self, tenant_id: &TenantId, policy: Policy) -> Result<(), EdgeError> {
        let mut config = self
            .tenant_manager
            .get_tenant(tenant_id)
            .await?
            .ok_or_else(|| {
                EdgeError::ConfigError(format!("Tenant {} not found", tenant_id.as_str()))
            })?;

        config.policies.push(policy);
        self.tenant_manager.update_tenant(config).await
    }

    async fn remove_policy(&self, tenant_id: &TenantId, policy_id: &str) -> Result<(), EdgeError> {
        let mut config = self
            .tenant_manager
            .get_tenant(tenant_id)
            .await?
            .ok_or_else(|| {
                EdgeError::ConfigError(format!("Tenant {} not found", tenant_id.as_str()))
            })?;

        config.policies.retain(|p| p.policy_id != policy_id);
        self.tenant_manager.update_tenant(config).await
    }

    async fn list_policies(&self, tenant_id: &TenantId) -> Result<Vec<Policy>, EdgeError> {
        let config = self
            .tenant_manager
            .get_tenant(tenant_id)
            .await?
            .ok_or_else(|| {
                EdgeError::ConfigError(format!("Tenant {} not found", tenant_id.as_str()))
            })?;

        Ok(config.policies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::InMemoryTenantManager;
    use crate::domain::{PolicyRule, RoleBinding, Scope, TenantConfig, UserId};

    #[tokio::test]
    async fn test_evaluate_role_based_permission() {
        let tenant_id = TenantId::new("test-tenant");
        let user_id = UserId::new("user-1");
        let identity = HierarchicalIdentity::new(tenant_id.clone(), user_id.clone());

        let admin_role = Role::new("admin").with_permission(Permission::allow("*", "*"));

        let config = TenantConfig::new(tenant_id.clone(), "Test Tenant").add_role(admin_role);

        let tenant_manager = Arc::new(InMemoryTenantManager::with_tenants(vec![config]));

        let binding = RoleBinding::new(user_id, "admin", Scope::tenant(tenant_id.clone()));
        tenant_manager.add_role_binding(binding).await.unwrap();

        let engine = DynamicPolicyEngine::new(tenant_manager);
        let context = EvaluationContext::new();

        let decision = engine
            .evaluate(&identity, "read", "resource:foo", &context)
            .await
            .unwrap();

        assert_eq!(decision, Decision::Allow);
    }

    #[tokio::test]
    async fn test_evaluate_role_inheritance() {
        let tenant_id = TenantId::new("test-tenant");
        let user_id = UserId::new("user-1");
        let identity = HierarchicalIdentity::new(tenant_id.clone(), user_id.clone());

        let user_role = Role::new("user").with_permission(Permission::allow("read", "*"));
        let admin_role = Role::new("admin")
            .with_permission(Permission::allow("write", "*"))
            .inherits("user");

        let config = TenantConfig::new(tenant_id.clone(), "Test Tenant")
            .add_role(user_role)
            .add_role(admin_role);

        let tenant_manager = Arc::new(InMemoryTenantManager::with_tenants(vec![config]));

        let binding = RoleBinding::new(user_id, "admin", Scope::tenant(tenant_id.clone()));
        tenant_manager.add_role_binding(binding).await.unwrap();

        let engine = DynamicPolicyEngine::new(tenant_manager);
        let context = EvaluationContext::new();

        // Admin should have read permission from inherited user role
        let decision = engine
            .evaluate(&identity, "read", "resource:foo", &context)
            .await
            .unwrap();
        assert_eq!(decision, Decision::Allow);

        // Admin should have write permission from own role
        let decision = engine
            .evaluate(&identity, "write", "resource:foo", &context)
            .await
            .unwrap();
        assert_eq!(decision, Decision::Allow);
    }

    #[tokio::test]
    async fn test_evaluate_dynamic_policy_with_conditions() {
        let tenant_id = TenantId::new("test-tenant");
        let user_id = UserId::new("user-1");
        let identity = HierarchicalIdentity::new(tenant_id.clone(), user_id.clone());

        let rule = PolicyRule::allow(vec!["read".to_string()], vec!["bucket:*".to_string()])
            .with_condition(Condition::StringEquals {
                key: "environment".to_string(),
                value: "production".to_string(),
            });

        let policy = Policy::new("prod-read-policy", "Production Read Access")
            .with_priority(100)
            .add_rule(rule);

        let config = TenantConfig::new(tenant_id.clone(), "Test Tenant").add_policy(policy);

        let tenant_manager = Arc::new(InMemoryTenantManager::with_tenants(vec![config]));

        let binding = RoleBinding::new(user_id, "user", Scope::tenant(tenant_id.clone()));
        tenant_manager.add_role_binding(binding).await.unwrap();

        let engine = DynamicPolicyEngine::new(tenant_manager);

        // Context with matching condition
        let context = EvaluationContext::new().with_attribute(
            "environment",
            serde_json::Value::String("production".to_string()),
        );

        let decision = engine
            .evaluate(&identity, "read", "bucket:my-bucket", &context)
            .await
            .unwrap();
        assert_eq!(decision, Decision::Allow);

        // Context without matching condition
        let context = EvaluationContext::new()
            .with_attribute("environment", serde_json::Value::String("dev".to_string()));

        let decision = engine
            .evaluate(&identity, "read", "bucket:my-bucket", &context)
            .await
            .unwrap();
        assert_eq!(decision, Decision::Undecided);
    }

    #[tokio::test]
    async fn test_deny_overrides_allow() {
        let tenant_id = TenantId::new("test-tenant");
        let user_id = UserId::new("user-1");
        let identity = HierarchicalIdentity::new(tenant_id.clone(), user_id.clone());

        let deny_rule = PolicyRule::deny(
            vec!["delete".to_string()],
            vec!["bucket:sensitive".to_string()],
        );

        let policy = Policy::new("deny-sensitive-delete", "Deny Sensitive Deletes")
            .with_priority(1000) // High priority
            .add_rule(deny_rule);

        let admin_role = Role::new("admin").with_permission(Permission::allow("*", "*"));

        let config = TenantConfig::new(tenant_id.clone(), "Test Tenant")
            .add_role(admin_role)
            .add_policy(policy);

        let tenant_manager = Arc::new(InMemoryTenantManager::with_tenants(vec![config]));

        let binding = RoleBinding::new(user_id, "admin", Scope::tenant(tenant_id.clone()));
        tenant_manager.add_role_binding(binding).await.unwrap();

        let engine = DynamicPolicyEngine::new(tenant_manager);
        let context = EvaluationContext::new();

        // Deny policy should override admin's wildcard permission
        let decision = engine
            .evaluate(&identity, "delete", "bucket:sensitive", &context)
            .await
            .unwrap();
        assert_eq!(decision, Decision::Deny);

        // But admin can still delete other buckets
        let decision = engine
            .evaluate(&identity, "delete", "bucket:normal", &context)
            .await
            .unwrap();
        assert_eq!(decision, Decision::Allow);
    }

    #[tokio::test]
    async fn test_get_effective_permissions() {
        let tenant_id = TenantId::new("test-tenant");
        let user_id = UserId::new("user-1");
        let identity = HierarchicalIdentity::new(tenant_id.clone(), user_id.clone());

        let role = Role::new("reader")
            .with_permission(Permission::allow("read", "*"))
            .with_permission(Permission::allow("list", "*"));

        let config = TenantConfig::new(tenant_id.clone(), "Test Tenant").add_role(role);

        let tenant_manager = Arc::new(InMemoryTenantManager::with_tenants(vec![config]));

        let binding = RoleBinding::new(user_id, "reader", Scope::tenant(tenant_id.clone()));
        tenant_manager.add_role_binding(binding).await.unwrap();

        let engine = DynamicPolicyEngine::new(tenant_manager);

        let permissions = engine.get_effective_permissions(&identity).await.unwrap();

        assert_eq!(permissions.len(), 2);
        assert!(permissions.contains(&("read".to_string(), "*".to_string())));
        assert!(permissions.contains(&("list".to_string(), "*".to_string())));
    }

    #[tokio::test]
    async fn test_disabled_tenant() {
        let tenant_id = TenantId::new("test-tenant");
        let user_id = UserId::new("user-1");
        let identity = HierarchicalIdentity::new(tenant_id.clone(), user_id.clone());

        let admin_role = Role::new("admin").with_permission(Permission::allow("*", "*"));

        let config = TenantConfig::new(tenant_id.clone(), "Test Tenant")
            .add_role(admin_role)
            .disable();

        let tenant_manager = Arc::new(InMemoryTenantManager::with_tenants(vec![config]));

        let binding = RoleBinding::new(user_id, "admin", Scope::tenant(tenant_id.clone()));
        tenant_manager.add_role_binding(binding).await.unwrap();

        let engine = DynamicPolicyEngine::new(tenant_manager);
        let context = EvaluationContext::new();

        let decision = engine
            .evaluate(&identity, "read", "resource:foo", &context)
            .await
            .unwrap();

        assert_eq!(decision, Decision::Deny);
    }
}
