//! Tenant management API
//!
//! HTTP API endpoints for multi-tenant management operations.

use crate::domain::{
    EdgeError, HierarchicalIdentity, Policy, Role, RoleBinding, TenantConfig, TenantId,
};
use crate::port::{EvaluationContext, PolicyEngine, TenantManager};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Application state for tenant API
#[derive(Clone)]
pub struct TenantApiState<T: TenantManager, P: PolicyEngine> {
    pub tenant_manager: Arc<T>,
    pub policy_engine: Arc<P>,
}

impl<T: TenantManager, P: PolicyEngine> TenantApiState<T, P> {
    pub fn new(tenant_manager: Arc<T>, policy_engine: Arc<P>) -> Self {
        Self {
            tenant_manager,
            policy_engine,
        }
    }
}

/// Request to create a tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTenantRequest {
    pub config: TenantConfig,
}

/// Request to update a tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTenantRequest {
    pub config: TenantConfig,
}

/// Request to add a role to a tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRoleRequest {
    pub role: Role,
}

/// Request to add a role binding
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRoleBindingRequest {
    pub binding: RoleBinding,
}

/// Request to add a policy
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddPolicyRequest {
    pub policy: Policy,
}

/// Request to evaluate a permission
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluatePermissionRequest {
    pub identity: HierarchicalIdentity,
    pub action: String,
    pub resource: String,
    pub context: Option<serde_json::Value>,
}

/// Response with permission decision
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluatePermissionResponse {
    pub allowed: bool,
    pub decision: String,
}

/// API error response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

impl IntoResponse for EdgeError {
    fn into_response(self) -> Response {
        let (status, error_type) = match &self {
            EdgeError::Configuration(_) => (StatusCode::BAD_REQUEST, "config_error"),
            EdgeError::Authentication(_) => (StatusCode::UNAUTHORIZED, "authentication_error"),
            EdgeError::Authorization(_) => (StatusCode::FORBIDDEN, "authorization_error"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };

        let body = Json(ErrorResponse {
            error: error_type.to_string(),
            message: self.to_string(),
        });

        (status, body).into_response()
    }
}

/// Create a new tenant
pub async fn create_tenant<T: TenantManager, P: PolicyEngine>(
    State(state): State<TenantApiState<T, P>>,
    Json(request): Json<CreateTenantRequest>,
) -> Result<StatusCode, EdgeError> {
    state.tenant_manager.create_tenant(request.config).await?;
    Ok(StatusCode::CREATED)
}

/// Get a tenant by ID
pub async fn get_tenant<T: TenantManager, P: PolicyEngine>(
    State(state): State<TenantApiState<T, P>>,
    Path(tenant_id): Path<String>,
) -> Result<Json<TenantConfig>, EdgeError> {
    let tenant_id = TenantId::new(tenant_id);
    let config = state
        .tenant_manager
        .get_tenant(&tenant_id)
        .await?
        .ok_or_else(|| {
            EdgeError::Configuration(format!("Tenant {} not found", tenant_id.as_str()))
        })?;

    Ok(Json(config))
}

/// Update a tenant
pub async fn update_tenant<T: TenantManager, P: PolicyEngine>(
    State(state): State<TenantApiState<T, P>>,
    Path(tenant_id): Path<String>,
    Json(request): Json<UpdateTenantRequest>,
) -> Result<StatusCode, EdgeError> {
    let tenant_id = TenantId::new(tenant_id);

    if request.config.tenant_id != tenant_id {
        return Err(EdgeError::Configuration(
            "Tenant ID in path does not match request body".to_string(),
        ));
    }

    state.tenant_manager.update_tenant(request.config).await?;
    Ok(StatusCode::OK)
}

/// Delete a tenant
pub async fn delete_tenant<T: TenantManager, P: PolicyEngine>(
    State(state): State<TenantApiState<T, P>>,
    Path(tenant_id): Path<String>,
) -> Result<StatusCode, EdgeError> {
    let tenant_id = TenantId::new(tenant_id);
    state.tenant_manager.delete_tenant(&tenant_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// List all tenants
pub async fn list_tenants<T: TenantManager, P: PolicyEngine>(
    State(state): State<TenantApiState<T, P>>,
) -> Result<Json<Vec<TenantId>>, EdgeError> {
    let tenants = state.tenant_manager.list_tenants().await?;
    Ok(Json(tenants))
}

/// Reload tenant configurations
pub async fn reload_tenants<T: TenantManager, P: PolicyEngine>(
    State(state): State<TenantApiState<T, P>>,
) -> Result<StatusCode, EdgeError> {
    state.tenant_manager.reload().await?;
    Ok(StatusCode::OK)
}

/// Add a role binding
pub async fn add_role_binding<T: TenantManager, P: PolicyEngine>(
    State(state): State<TenantApiState<T, P>>,
    Json(request): Json<AddRoleBindingRequest>,
) -> Result<StatusCode, EdgeError> {
    state
        .tenant_manager
        .add_role_binding(request.binding)
        .await?;
    Ok(StatusCode::CREATED)
}

/// Remove a role binding
pub async fn remove_role_binding<T: TenantManager, P: PolicyEngine>(
    State(state): State<TenantApiState<T, P>>,
    Path(binding_id): Path<String>,
) -> Result<StatusCode, EdgeError> {
    state
        .tenant_manager
        .remove_role_binding(&binding_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Add a policy
pub async fn add_policy<T: TenantManager, P: PolicyEngine>(
    State(state): State<TenantApiState<T, P>>,
    Path(tenant_id): Path<String>,
    Json(request): Json<AddPolicyRequest>,
) -> Result<StatusCode, EdgeError> {
    let tenant_id = TenantId::new(tenant_id);
    state
        .policy_engine
        .add_policy(&tenant_id, request.policy)
        .await?;
    Ok(StatusCode::CREATED)
}

/// Remove a policy
pub async fn remove_policy<T: TenantManager, P: PolicyEngine>(
    State(state): State<TenantApiState<T, P>>,
    Path((tenant_id, policy_id)): Path<(String, String)>,
) -> Result<StatusCode, EdgeError> {
    let tenant_id = TenantId::new(tenant_id);
    state
        .policy_engine
        .remove_policy(&tenant_id, &policy_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// List policies for a tenant
pub async fn list_policies<T: TenantManager, P: PolicyEngine>(
    State(state): State<TenantApiState<T, P>>,
    Path(tenant_id): Path<String>,
) -> Result<Json<Vec<Policy>>, EdgeError> {
    let tenant_id = TenantId::new(tenant_id);
    let policies = state.policy_engine.list_policies(&tenant_id).await?;
    Ok(Json(policies))
}

/// Evaluate a permission
pub async fn evaluate_permission<T: TenantManager, P: PolicyEngine>(
    State(state): State<TenantApiState<T, P>>,
    Json(request): Json<EvaluatePermissionRequest>,
) -> Result<Json<EvaluatePermissionResponse>, EdgeError> {
    let mut context = EvaluationContext::new();

    if let Some(attrs) = request.context {
        if let Some(obj) = attrs.as_object() {
            for (key, value) in obj {
                context.add_attribute(key.clone(), value.clone());
            }
        }
    }

    let decision = state
        .policy_engine
        .evaluate(
            &request.identity,
            &request.action,
            &request.resource,
            &context,
        )
        .await?;

    let allowed = decision.is_allowed();
    let decision_str = match decision {
        crate::port::Decision::Allow => "allow",
        crate::port::Decision::Deny => "deny",
        crate::port::Decision::Undecided => "undecided",
    };

    Ok(Json(EvaluatePermissionResponse {
        allowed,
        decision: decision_str.to_string(),
    }))
}

/// Get effective permissions for an identity
pub async fn get_effective_permissions<T: TenantManager, P: PolicyEngine>(
    State(state): State<TenantApiState<T, P>>,
    Json(identity): Json<HierarchicalIdentity>,
) -> Result<Json<Vec<(String, String)>>, EdgeError> {
    let permissions = state
        .policy_engine
        .get_effective_permissions(&identity)
        .await?;
    Ok(Json(permissions))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{DynamicPolicyEngine, InMemoryTenantManager};
    use crate::domain::{UserId, WipLimits};

    #[tokio::test]
    async fn test_tenant_api_state_creation() {
        let tenant_manager = Arc::new(InMemoryTenantManager::new());
        let policy_engine = Arc::new(DynamicPolicyEngine::new(tenant_manager.clone()));
        let _state = TenantApiState::new(tenant_manager, policy_engine);
    }

    #[tokio::test]
    async fn test_create_and_get_tenant_flow() {
        let tenant_manager = Arc::new(InMemoryTenantManager::new());
        let policy_engine = Arc::new(DynamicPolicyEngine::new(tenant_manager.clone()));

        let tenant_id = TenantId::new("test-tenant");
        let config = TenantConfig::new(tenant_id.clone(), "Test Tenant")
            .with_wip_limits(WipLimits::new(100));

        tenant_manager.create_tenant(config).await.unwrap();

        let retrieved = tenant_manager.get_tenant(&tenant_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Tenant");
    }
}
