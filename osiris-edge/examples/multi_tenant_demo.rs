//! Multi-tenant hierarchical authentication demo
//!
//! Demonstrates:
//! - Multi-tenant configuration with WIP limits
//! - Hierarchical authentication (tenant > organization > team > user)
//! - RBAC with role inheritance
//! - Dynamic policies with conditions
//! - Tenant-specific refusal rules
//! - HTTP API for tenant management

use osiris_edge::{
    Condition, Decision, DynamicPolicyEngine, Effect, EvaluationContext, HierarchicalIdentity,
    InMemoryTenantManager, OrganizationId, Permission, Policy, PolicyEngine, PolicyRule, Role,
    RoleBinding, Scope, TeamId, TenantConfig, TenantId, TenantManager, UserId, WipLimits,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Multi-Tenant Hierarchical Authentication Demo ===\n");

    // Setup: Create tenants with different configurations
    let tenant_manager = setup_tenants().await?;
    let policy_engine = Arc::new(DynamicPolicyEngine::new(tenant_manager.clone()));

    // Scenario 1: Basic RBAC with roles
    println!("--- Scenario 1: Basic RBAC with Roles ---");
    demo_basic_rbac(&tenant_manager, &policy_engine).await?;

    // Scenario 2: Role inheritance
    println!("\n--- Scenario 2: Role Inheritance ---");
    demo_role_inheritance(&tenant_manager, &policy_engine).await?;

    // Scenario 3: Dynamic policies with conditions
    println!("\n--- Scenario 3: Dynamic Policies with Conditions ---");
    demo_dynamic_policies(&tenant_manager, &policy_engine).await?;

    // Scenario 4: Hierarchical scopes (tenant/org/team)
    println!("\n--- Scenario 4: Hierarchical Scopes ---");
    demo_hierarchical_scopes(&tenant_manager, &policy_engine).await?;

    // Scenario 5: Tenant-specific WIP limits
    println!("\n--- Scenario 5: Tenant-Specific WIP Limits ---");
    demo_tenant_wip_limits(&tenant_manager).await?;

    // Scenario 6: Dynamic tenant updates
    println!("\n--- Scenario 6: Dynamic Tenant Updates ---");
    demo_dynamic_updates(&tenant_manager, &policy_engine).await?;

    println!("\n=== Demo Complete ===");
    Ok(())
}

async fn setup_tenants() -> Result<Arc<InMemoryTenantManager>, Box<dyn std::error::Error>> {
    let tenant_manager = Arc::new(InMemoryTenantManager::new());

    // Tenant 1: Enterprise with fine-grained WIP limits
    let enterprise_tenant = TenantConfig::new(TenantId::new("enterprise-corp"), "Enterprise Corp")
        .with_wip_limits(
            WipLimits::new(1000)
                .with_organization_limit(500)
                .with_team_limit(100)
                .with_user_limit(20),
        )
        .add_role(
            Role::new("admin")
                .with_permission(Permission::allow("*", "*"))
                .with_permission(Permission::deny("delete", "resource:critical")),
        )
        .add_role(
            Role::new("developer")
                .with_permission(Permission::allow("read", "*"))
                .with_permission(Permission::allow("write", "code:*")),
        )
        .add_role(Role::new("viewer").with_permission(Permission::allow("read", "*")));

    tenant_manager.create_tenant(enterprise_tenant).await?;

    // Tenant 2: Startup with relaxed limits
    let startup_tenant = TenantConfig::new(TenantId::new("startup-inc"), "Startup Inc")
        .with_wip_limits(WipLimits::new(100).with_user_limit(50))
        .add_role(Role::new("founder").with_permission(Permission::allow("*", "*")))
        .add_role(
            Role::new("engineer")
                .with_permission(Permission::allow("*", "resource:*"))
                .with_permission(Permission::deny("delete", "resource:production")),
        );

    tenant_manager.create_tenant(startup_tenant).await?;

    println!("Created 2 tenants:");
    println!("  - enterprise-corp (WIP: 1000 global, 20 per user)");
    println!("  - startup-inc (WIP: 100 global, 50 per user)");

    Ok(tenant_manager)
}

async fn demo_basic_rbac(
    tenant_manager: &Arc<InMemoryTenantManager>,
    policy_engine: &Arc<DynamicPolicyEngine<InMemoryTenantManager>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tenant_id = TenantId::new("enterprise-corp");
    let user_id = UserId::new("alice");
    let identity = HierarchicalIdentity::new(tenant_id.clone(), user_id.clone());

    // Assign developer role
    let binding = RoleBinding::new(user_id, "developer", Scope::tenant(tenant_id));
    tenant_manager.add_role_binding(binding).await?;

    let context = EvaluationContext::new();

    // Test permissions
    let can_read = policy_engine
        .is_allowed(&identity, "read", "document:readme", &context)
        .await?;
    let can_write_code = policy_engine
        .is_allowed(&identity, "write", "code:main.rs", &context)
        .await?;
    let can_delete = policy_engine
        .is_allowed(&identity, "delete", "resource:database", &context)
        .await?;

    println!("Alice (developer role):");
    println!("  - read document:readme: {}", can_read);
    println!("  - write code:main.rs: {}", can_write_code);
    println!("  - delete resource:database: {}", can_delete);

    Ok(())
}

async fn demo_role_inheritance(
    tenant_manager: &Arc<InMemoryTenantManager>,
    policy_engine: &Arc<DynamicPolicyEngine<InMemoryTenantManager>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tenant_id = TenantId::new("enterprise-corp");
    let user_id = UserId::new("bob");
    let identity = HierarchicalIdentity::new(tenant_id.clone(), user_id.clone());

    // Update tenant with inherited roles
    let mut config = tenant_manager.get_tenant(&tenant_id).await?.unwrap();

    let senior_dev = Role::new("senior_developer")
        .with_permission(Permission::allow("deploy", "service:*"))
        .inherits("developer"); // Inherits from developer

    config = config.add_role(senior_dev);
    tenant_manager.update_tenant(config).await?;

    // Assign senior_developer role
    let binding = RoleBinding::new(user_id, "senior_developer", Scope::tenant(tenant_id));
    tenant_manager.add_role_binding(binding).await?;

    let context = EvaluationContext::new();

    // Test inherited permissions
    let can_read = policy_engine
        .is_allowed(&identity, "read", "document:readme", &context)
        .await?;
    let can_deploy = policy_engine
        .is_allowed(&identity, "deploy", "service:api", &context)
        .await?;

    println!("Bob (senior_developer inherits developer):");
    println!("  - read document:readme (inherited): {}", can_read);
    println!("  - deploy service:api (own): {}", can_deploy);

    Ok(())
}

async fn demo_dynamic_policies(
    tenant_manager: &Arc<InMemoryTenantManager>,
    policy_engine: &Arc<DynamicPolicyEngine<InMemoryTenantManager>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tenant_id = TenantId::new("enterprise-corp");
    let user_id = UserId::new("charlie");
    let identity = HierarchicalIdentity::new(tenant_id.clone(), user_id.clone());

    // Assign admin role (wildcard permissions)
    let binding = RoleBinding::new(user_id, "admin", Scope::tenant(tenant_id.clone()));
    tenant_manager.add_role_binding(binding).await?;

    // Add time-based policy: Only allow deployments during business hours
    let business_hours_rule = PolicyRule::allow(
        vec!["deploy".to_string()],
        vec!["service:production".to_string()],
    )
    .with_condition(Condition::TimeWindowUtc {
        start_hour: 9,
        end_hour: 17,
    });

    let policy = Policy::new("business-hours-deploy", "Deploy only during business hours")
        .with_priority(1000)
        .add_rule(business_hours_rule);

    policy_engine.add_policy(&tenant_id, policy).await?;

    // Add environment-based policy
    let staging_rule = PolicyRule::allow(
        vec!["deploy".to_string()],
        vec!["service:staging".to_string()],
    )
    .with_condition(Condition::StringEquals {
        key: "environment".to_string(),
        value: "staging".to_string(),
    });

    let staging_policy = Policy::new("staging-deploy", "Staging deployments")
        .with_priority(500)
        .add_rule(staging_rule);

    policy_engine.add_policy(&tenant_id, staging_policy).await?;

    // Test with context
    let mut context = EvaluationContext::new();
    context.add_attribute(
        "environment",
        serde_json::Value::String("staging".to_string()),
    );

    let can_deploy_staging = policy_engine
        .is_allowed(&identity, "deploy", "service:staging", &context)
        .await?;

    println!("Charlie (admin with dynamic policies):");
    println!(
        "  - deploy service:staging (with environment=staging): {}",
        can_deploy_staging
    );
    println!("  - Time-based policy active for production deployments (9am-5pm UTC)");

    Ok(())
}

async fn demo_hierarchical_scopes(
    tenant_manager: &Arc<InMemoryTenantManager>,
    policy_engine: &Arc<DynamicPolicyEngine<InMemoryTenantManager>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tenant_id = TenantId::new("enterprise-corp");
    let org_id = OrganizationId::new("engineering");
    let team_id = TeamId::new("backend");
    let user_id = UserId::new("david");

    let identity = HierarchicalIdentity::new(tenant_id.clone(), user_id.clone())
        .with_organization(org_id.clone())
        .with_team(team_id.clone());

    // Role at tenant level (broad permissions)
    let tenant_binding =
        RoleBinding::new(user_id.clone(), "viewer", Scope::tenant(tenant_id.clone()));
    tenant_manager.add_role_binding(tenant_binding).await?;

    // Role at team level (narrow permissions)
    let team_binding = RoleBinding::new(
        user_id,
        "developer",
        Scope::team(tenant_id.clone(), org_id, team_id),
    );
    tenant_manager.add_role_binding(team_binding).await?;

    let context = EvaluationContext::new();

    // Test combined permissions
    let can_read = policy_engine
        .is_allowed(&identity, "read", "document:all", &context)
        .await?;
    let can_write = policy_engine
        .is_allowed(&identity, "write", "code:backend.rs", &context)
        .await?;

    println!("David (viewer at tenant + developer at team):");
    println!("  - read document:all (tenant scope): {}", can_read);
    println!("  - write code:backend.rs (team scope): {}", can_write);

    Ok(())
}

async fn demo_tenant_wip_limits(
    tenant_manager: &Arc<InMemoryTenantManager>,
) -> Result<(), Box<dyn std::error::Error>> {
    let enterprise = tenant_manager
        .get_tenant(&TenantId::new("enterprise-corp"))
        .await?
        .unwrap();
    let startup = tenant_manager
        .get_tenant(&TenantId::new("startup-inc"))
        .await?
        .unwrap();

    println!("WIP Limits per tenant:");
    println!("\nEnterprise Corp:");
    println!("  - Global: {}", enterprise.wip_limits.global);
    println!("  - Per Org: {:?}", enterprise.wip_limits.per_organization);
    println!("  - Per Team: {:?}", enterprise.wip_limits.per_team);
    println!("  - Per User: {:?}", enterprise.wip_limits.per_user);

    println!("\nStartup Inc:");
    println!("  - Global: {}", startup.wip_limits.global);
    println!("  - Per User: {:?}", startup.wip_limits.per_user);

    Ok(())
}

async fn demo_dynamic_updates(
    tenant_manager: &Arc<InMemoryTenantManager>,
    policy_engine: &Arc<DynamicPolicyEngine<InMemoryTenantManager>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tenant_id = TenantId::new("startup-inc");
    let user_id = UserId::new("eve");
    let identity = HierarchicalIdentity::new(tenant_id.clone(), user_id.clone());

    // Initial role assignment
    let binding = RoleBinding::new(user_id, "engineer", Scope::tenant(tenant_id.clone()));
    tenant_manager.add_role_binding(binding.clone()).await?;

    let context = EvaluationContext::new();

    let can_delete_prod_before = policy_engine
        .is_allowed(&identity, "delete", "resource:production", &context)
        .await?;

    println!("Eve (engineer at startup-inc):");
    println!(
        "  - delete resource:production (before policy): {}",
        can_delete_prod_before
    );

    // Add runtime policy to deny production deletes
    let deny_rule = PolicyRule::deny(
        vec!["delete".to_string()],
        vec!["resource:production".to_string()],
    );

    let policy = Policy::new("protect-production", "Protect production resources")
        .with_priority(2000)
        .add_rule(deny_rule);

    policy_engine.add_policy(&tenant_id, policy).await?;

    let can_delete_prod_after = policy_engine
        .is_allowed(&identity, "delete", "resource:production", &context)
        .await?;

    println!(
        "  - delete resource:production (after policy): {}",
        can_delete_prod_after
    );
    println!("  - Policy added at runtime without restart!");

    Ok(())
}
