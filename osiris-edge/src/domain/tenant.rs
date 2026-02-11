//! Multi-tenant domain types
//!
//! Core types for multi-tenant hierarchical authentication without external dependencies.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Tenant identifier (top-level isolation boundary)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct TenantId(pub String);

impl TenantId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for TenantId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TenantId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Organization identifier (within tenant)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct OrganizationId(pub String);

impl OrganizationId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Team identifier (within organization)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct TeamId(pub String);

impl TeamId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// User identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct UserId(pub String);

impl UserId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Hierarchical identity (tenant > organization > team > user)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HierarchicalIdentity {
    pub tenant_id: TenantId,
    pub organization_id: Option<OrganizationId>,
    pub team_id: Option<TeamId>,
    pub user_id: UserId,
}

impl HierarchicalIdentity {
    pub fn new(tenant_id: TenantId, user_id: UserId) -> Self {
        Self {
            tenant_id,
            organization_id: None,
            team_id: None,
            user_id,
        }
    }

    pub fn with_organization(mut self, org_id: OrganizationId) -> Self {
        self.organization_id = Some(org_id);
        self
    }

    pub fn with_team(mut self, team_id: TeamId) -> Self {
        self.team_id = Some(team_id);
        self
    }

    /// Check if this identity is within a given tenant
    pub fn is_in_tenant(&self, tenant_id: &TenantId) -> bool {
        &self.tenant_id == tenant_id
    }

    /// Check if this identity is within a given organization
    pub fn is_in_organization(&self, org_id: &OrganizationId) -> bool {
        self.organization_id.as_ref() == Some(org_id)
    }

    /// Check if this identity is within a given team
    pub fn is_in_team(&self, team_id: &TeamId) -> bool {
        self.team_id.as_ref() == Some(team_id)
    }
}

/// Role in RBAC system
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    pub name: String,
    pub permissions: HashSet<Permission>,
    pub inherits_from: Vec<String>,
}

impl Role {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            permissions: HashSet::new(),
            inherits_from: Vec::new(),
        }
    }

    pub fn with_permission(mut self, permission: Permission) -> Self {
        self.permissions.insert(permission);
        self
    }

    pub fn with_permissions(mut self, permissions: impl IntoIterator<Item = Permission>) -> Self {
        self.permissions.extend(permissions);
        self
    }

    pub fn inherits(mut self, parent_role: impl Into<String>) -> Self {
        self.inherits_from.push(parent_role.into());
        self
    }

    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }
}

/// Permission (action + resource pattern)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    pub action: String,
    pub resource: String,
    pub effect: Effect,
}

impl Permission {
    pub fn allow(action: impl Into<String>, resource: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            resource: resource.into(),
            effect: Effect::Allow,
        }
    }

    pub fn deny(action: impl Into<String>, resource: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            resource: resource.into(),
            effect: Effect::Deny,
        }
    }

    /// Check if this permission matches a given action and resource
    pub fn matches(&self, action: &str, resource: &str) -> bool {
        self.action_matches(action) && self.resource_matches(resource)
    }

    fn action_matches(&self, action: &str) -> bool {
        self.action == "*" || self.action == action
    }

    fn resource_matches(&self, resource: &str) -> bool {
        if self.resource == "*" {
            return true;
        }
        if self.resource.ends_with("*") {
            let prefix = &self.resource[..self.resource.len() - 1];
            return resource.starts_with(prefix);
        }
        self.resource == resource
    }
}

/// Permission effect
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    Allow,
    Deny,
}

/// Role binding (assigns role to identity at specific scope)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleBinding {
    pub binding_id: String,
    pub user_id: UserId,
    pub role_name: String,
    pub scope: Scope,
}

impl RoleBinding {
    pub fn new(user_id: UserId, role_name: impl Into<String>, scope: Scope) -> Self {
        Self {
            binding_id: Uuid::new_v4().to_string(),
            user_id,
            role_name: role_name.into(),
            scope,
        }
    }
}

/// Scope at which a role is bound
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "level", rename_all = "camelCase")]
pub enum Scope {
    #[serde(rename_all = "camelCase")]
    Tenant { tenant_id: TenantId },
    #[serde(rename_all = "camelCase")]
    Organization {
        tenant_id: TenantId,
        organization_id: OrganizationId,
    },
    #[serde(rename_all = "camelCase")]
    Team {
        tenant_id: TenantId,
        organization_id: OrganizationId,
        team_id: TeamId,
    },
}

impl Scope {
    pub fn tenant(tenant_id: TenantId) -> Self {
        Self::Tenant { tenant_id }
    }

    pub fn organization(tenant_id: TenantId, organization_id: OrganizationId) -> Self {
        Self::Organization {
            tenant_id,
            organization_id,
        }
    }

    pub fn team(tenant_id: TenantId, organization_id: OrganizationId, team_id: TeamId) -> Self {
        Self::Team {
            tenant_id,
            organization_id,
            team_id,
        }
    }

    /// Check if this scope applies to the given identity
    pub fn applies_to(&self, identity: &HierarchicalIdentity) -> bool {
        match self {
            Scope::Tenant { tenant_id } => &identity.tenant_id == tenant_id,
            Scope::Organization {
                tenant_id,
                organization_id,
            } => {
                &identity.tenant_id == tenant_id
                    && identity.organization_id.as_ref() == Some(organization_id)
            }
            Scope::Team {
                tenant_id,
                organization_id,
                team_id,
            } => {
                &identity.tenant_id == tenant_id
                    && identity.organization_id.as_ref() == Some(organization_id)
                    && identity.team_id.as_ref() == Some(team_id)
            }
        }
    }

    pub fn tenant_id(&self) -> &TenantId {
        match self {
            Scope::Tenant { tenant_id } => tenant_id,
            Scope::Organization { tenant_id, .. } => tenant_id,
            Scope::Team { tenant_id, .. } => tenant_id,
        }
    }
}

/// Dynamic policy expression for runtime evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub policy_id: String,
    pub name: String,
    pub description: Option<String>,
    pub rules: Vec<PolicyRule>,
    pub priority: i32,
}

impl Policy {
    pub fn new(policy_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            policy_id: policy_id.into(),
            name: name.into(),
            description: None,
            rules: Vec::new(),
            priority: 0,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn add_rule(mut self, rule: PolicyRule) -> Self {
        self.rules.push(rule);
        self
    }
}

/// Policy rule with conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    pub effect: Effect,
    pub actions: Vec<String>,
    pub resources: Vec<String>,
    pub conditions: Vec<Condition>,
}

impl PolicyRule {
    pub fn allow(actions: Vec<String>, resources: Vec<String>) -> Self {
        Self {
            effect: Effect::Allow,
            actions,
            resources,
            conditions: Vec::new(),
        }
    }

    pub fn deny(actions: Vec<String>, resources: Vec<String>) -> Self {
        Self {
            effect: Effect::Deny,
            actions,
            resources,
            conditions: Vec::new(),
        }
    }

    pub fn with_condition(mut self, condition: Condition) -> Self {
        self.conditions.push(condition);
        self
    }

    pub fn matches(&self, action: &str, resource: &str) -> bool {
        let action_match = self.actions.iter().any(|a| {
            a == "*" || a == action || (a.ends_with("*") && action.starts_with(&a[..a.len() - 1]))
        });
        let resource_match = self.resources.iter().any(|r| {
            r == "*"
                || r == resource
                || (r.ends_with("*") && resource.starts_with(&r[..r.len() - 1]))
        });

        action_match && resource_match
    }
}

/// Condition for policy evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Condition {
    #[serde(rename_all = "camelCase")]
    StringEquals { key: String, value: String },
    #[serde(rename_all = "camelCase")]
    StringLike { key: String, pattern: String },
    #[serde(rename_all = "camelCase")]
    NumericLessThan { key: String, value: i64 },
    #[serde(rename_all = "camelCase")]
    NumericGreaterThan { key: String, value: i64 },
    #[serde(rename_all = "camelCase")]
    IpAddress { key: String, cidr: String },
    #[serde(rename_all = "camelCase")]
    TimeWindowUtc { start_hour: u8, end_hour: u8 },
}

/// Tenant-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TenantConfig {
    pub tenant_id: TenantId,
    pub name: String,
    pub enabled: bool,
    pub wip_limits: WipLimits,
    pub refusal_rules: RefusalRules,
    pub roles: HashMap<String, Role>,
    pub policies: Vec<Policy>,
    pub metadata: HashMap<String, String>,
}

impl TenantConfig {
    pub fn new(tenant_id: TenantId, name: impl Into<String>) -> Self {
        Self {
            tenant_id,
            name: name.into(),
            enabled: true,
            wip_limits: WipLimits::default(),
            refusal_rules: RefusalRules::default(),
            roles: HashMap::new(),
            policies: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_wip_limits(mut self, limits: WipLimits) -> Self {
        self.wip_limits = limits;
        self
    }

    pub fn with_refusal_rules(mut self, rules: RefusalRules) -> Self {
        self.refusal_rules = rules;
        self
    }

    pub fn add_role(mut self, role: Role) -> Self {
        self.roles.insert(role.name.clone(), role);
        self
    }

    pub fn add_policy(mut self, policy: Policy) -> Self {
        self.policies.push(policy);
        self
    }

    pub fn disable(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Tenant-specific WIP limits
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WipLimits {
    pub global: usize,
    pub per_organization: Option<usize>,
    pub per_team: Option<usize>,
    pub per_user: Option<usize>,
}

impl Default for WipLimits {
    fn default() -> Self {
        Self {
            global: 100,
            per_organization: Some(50),
            per_team: Some(25),
            per_user: Some(10),
        }
    }
}

impl WipLimits {
    pub fn new(global: usize) -> Self {
        Self {
            global,
            per_organization: None,
            per_team: None,
            per_user: None,
        }
    }

    pub fn with_organization_limit(mut self, limit: usize) -> Self {
        self.per_organization = Some(limit);
        self
    }

    pub fn with_team_limit(mut self, limit: usize) -> Self {
        self.per_team = Some(limit);
        self
    }

    pub fn with_user_limit(mut self, limit: usize) -> Self {
        self.per_user = Some(limit);
        self
    }
}

/// Tenant-specific refusal rules
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefusalRules {
    pub auto_reject_patterns: Vec<String>,
    pub require_auth_for_types: Vec<String>,
    pub max_payload_size_bytes: Option<usize>,
    pub rate_limit_per_minute: Option<usize>,
}

impl Default for RefusalRules {
    fn default() -> Self {
        Self {
            auto_reject_patterns: Vec::new(),
            require_auth_for_types: Vec::new(),
            max_payload_size_bytes: Some(1024 * 1024), // 1MB default
            rate_limit_per_minute: Some(1000),
        }
    }
}

impl RefusalRules {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_rejection_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.auto_reject_patterns.push(pattern.into());
        self
    }

    pub fn require_auth_for(mut self, type_name: impl Into<String>) -> Self {
        self.require_auth_for_types.push(type_name.into());
        self
    }

    pub fn with_max_payload_size(mut self, bytes: usize) -> Self {
        self.max_payload_size_bytes = Some(bytes);
        self
    }

    pub fn with_rate_limit(mut self, per_minute: usize) -> Self {
        self.rate_limit_per_minute = Some(per_minute);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hierarchical_identity() {
        let tenant = TenantId::new("tenant-1");
        let org = OrganizationId::new("org-1");
        let team = TeamId::new("team-1");
        let user = UserId::new("user-1");

        let identity = HierarchicalIdentity::new(tenant.clone(), user)
            .with_organization(org.clone())
            .with_team(team.clone());

        assert!(identity.is_in_tenant(&tenant));
        assert!(identity.is_in_organization(&org));
        assert!(identity.is_in_team(&team));
    }

    #[test]
    fn test_permission_matching() {
        let perm = Permission::allow("read", "resource:*");
        assert!(perm.matches("read", "resource:foo"));
        assert!(perm.matches("read", "resource:bar"));
        assert!(!perm.matches("write", "resource:foo"));
        assert!(!perm.matches("read", "other:foo"));

        let wildcard = Permission::allow("*", "*");
        assert!(wildcard.matches("read", "anything"));
        assert!(wildcard.matches("write", "anything"));
    }

    #[test]
    fn test_scope_applies_to() {
        let tenant_id = TenantId::new("tenant-1");
        let org_id = OrganizationId::new("org-1");
        let team_id = TeamId::new("team-1");
        let user_id = UserId::new("user-1");

        let identity = HierarchicalIdentity::new(tenant_id.clone(), user_id)
            .with_organization(org_id.clone())
            .with_team(team_id.clone());

        let tenant_scope = Scope::tenant(tenant_id.clone());
        assert!(tenant_scope.applies_to(&identity));

        let org_scope = Scope::organization(tenant_id.clone(), org_id.clone());
        assert!(org_scope.applies_to(&identity));

        let team_scope = Scope::team(tenant_id.clone(), org_id.clone(), team_id.clone());
        assert!(team_scope.applies_to(&identity));

        let other_tenant = Scope::tenant(TenantId::new("other"));
        assert!(!other_tenant.applies_to(&identity));
    }

    #[test]
    fn test_role_with_permissions() {
        let role = Role::new("admin")
            .with_permission(Permission::allow("*", "*"))
            .inherits("user");

        assert_eq!(role.name, "admin");
        assert_eq!(role.permissions.len(), 1);
        assert_eq!(role.inherits_from, vec!["user"]);
    }

    #[test]
    fn test_policy_rule_matching() {
        let rule = PolicyRule::allow(
            vec!["read".to_string(), "list".to_string()],
            vec!["bucket:*".to_string()],
        );

        assert!(rule.matches("read", "bucket:my-bucket"));
        assert!(rule.matches("list", "bucket:other"));
        assert!(!rule.matches("write", "bucket:my-bucket"));
        assert!(!rule.matches("read", "table:my-table"));
    }

    #[test]
    fn test_tenant_config_builder() {
        let tenant_id = TenantId::new("test-tenant");
        let config = TenantConfig::new(tenant_id.clone(), "Test Tenant")
            .with_wip_limits(WipLimits::new(50).with_user_limit(5))
            .add_role(Role::new("admin").with_permission(Permission::allow("*", "*")));

        assert_eq!(config.tenant_id, tenant_id);
        assert_eq!(config.wip_limits.global, 50);
        assert_eq!(config.wip_limits.per_user, Some(5));
        assert!(config.roles.contains_key("admin"));
    }

    #[test]
    fn test_wip_limits() {
        let limits = WipLimits::new(100)
            .with_organization_limit(50)
            .with_team_limit(25)
            .with_user_limit(10);

        assert_eq!(limits.global, 100);
        assert_eq!(limits.per_organization, Some(50));
        assert_eq!(limits.per_team, Some(25));
        assert_eq!(limits.per_user, Some(10));
    }

    #[test]
    fn test_refusal_rules() {
        let rules = RefusalRules::new()
            .add_rejection_pattern("spam:*")
            .require_auth_for("sensitive")
            .with_max_payload_size(512 * 1024)
            .with_rate_limit(500);

        assert_eq!(rules.auto_reject_patterns, vec!["spam:*"]);
        assert_eq!(rules.require_auth_for_types, vec!["sensitive"]);
        assert_eq!(rules.max_payload_size_bytes, Some(512 * 1024));
        assert_eq!(rules.rate_limit_per_minute, Some(500));
    }
}
