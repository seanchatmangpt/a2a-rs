//! Hierarchical authentication adapter
//!
//! Implements hierarchical authentication with tenant isolation and identity resolution.

use crate::domain::{
    AuthErrorCode, AuthPrincipal, AuthRequest, EdgeError, HierarchicalIdentity, OrganizationId,
    RefusalReason, TeamId, TenantId, UserId,
};
use crate::port::{AuthGate, HierarchicalAuthGate, TenantManager};
use async_trait::async_trait;
use std::sync::Arc;

/// Hierarchical authentication adapter
///
/// Wraps a base AuthGate and adds hierarchical identity resolution
/// with tenant isolation validation.
pub struct HierarchicalAuthAdapter<A: AuthGate, T: TenantManager> {
    auth_gate: Arc<A>,
    tenant_manager: Arc<T>,
}

impl<A: AuthGate, T: TenantManager> HierarchicalAuthAdapter<A, T> {
    pub fn new(auth_gate: Arc<A>, tenant_manager: Arc<T>) -> Self {
        Self {
            auth_gate,
            tenant_manager,
        }
    }
}

#[async_trait]
impl<A: AuthGate, T: TenantManager> HierarchicalAuthGate for HierarchicalAuthAdapter<A, T> {
    async fn authenticate_hierarchical(
        &self,
        request: &AuthRequest,
        tenant_id: &TenantId,
    ) -> Result<(AuthPrincipal, HierarchicalIdentity), EdgeError> {
        // First, authenticate using the base auth gate
        let principal = self.auth_gate.authenticate(request).await?;

        // Verify tenant is enabled
        let enabled = self.tenant_manager.is_tenant_enabled(tenant_id).await?;
        if !enabled {
            return Err(EdgeError::Authentication {
                reason: RefusalReason::auth_failed(
                    AuthErrorCode::InsufficientPermissions,
                    format!("Tenant {} is disabled", tenant_id.as_str()),
                ),
            });
        }

        // Extract hierarchical identity from principal claims
        let identity =
            self.extract_identity(&principal)
                .await?
                .ok_or_else(|| EdgeError::Authentication {
                    reason: RefusalReason::auth_failed(
                        AuthErrorCode::MissingClaim,
                        "Missing tenant/organization/team claims in token",
                    ),
                })?;

        // Verify the identity belongs to the requested tenant
        if !identity.is_in_tenant(tenant_id) {
            return Err(EdgeError::Authentication {
                reason: RefusalReason::auth_failed(
                    AuthErrorCode::InsufficientPermissions,
                    format!(
                        "User belongs to tenant {}, not {}",
                        identity.tenant_id.as_str(),
                        tenant_id.as_str()
                    ),
                ),
            });
        }

        Ok((principal, identity))
    }

    async fn verify_tenant_membership(
        &self,
        principal: &AuthPrincipal,
        tenant_id: &TenantId,
    ) -> Result<bool, EdgeError> {
        if let Some(identity) = self.extract_identity(principal).await? {
            Ok(identity.is_in_tenant(tenant_id))
        } else {
            Ok(false)
        }
    }

    async fn verify_organization_membership(
        &self,
        identity: &HierarchicalIdentity,
        organization_id: &OrganizationId,
    ) -> Result<bool, EdgeError> {
        Ok(identity.is_in_organization(organization_id))
    }

    async fn verify_team_membership(
        &self,
        identity: &HierarchicalIdentity,
        team_id: &TeamId,
    ) -> Result<bool, EdgeError> {
        Ok(identity.is_in_team(team_id))
    }

    async fn extract_identity(
        &self,
        principal: &AuthPrincipal,
    ) -> Result<Option<HierarchicalIdentity>, EdgeError> {
        // Extract tenant_id from claims
        let tenant_id = principal
            .claims
            .get("tenant_id")
            .and_then(|v| v.as_str())
            .map(TenantId::new);

        if tenant_id.is_none() {
            return Ok(None);
        }

        // Extract user_id (from subject or explicit claim)
        let user_id = principal
            .claims
            .get("user_id")
            .and_then(|v| v.as_str())
            .map(UserId::new)
            .unwrap_or_else(|| UserId::new(principal.subject.clone()));

        let mut identity = HierarchicalIdentity::new(tenant_id.unwrap(), user_id);

        // Extract optional organization_id
        if let Some(org_id) = principal
            .claims
            .get("organization_id")
            .and_then(|v| v.as_str())
        {
            identity = identity.with_organization(OrganizationId::new(org_id));
        }

        // Extract optional team_id
        if let Some(team_id) = principal.claims.get("team_id").and_then(|v| v.as_str()) {
            identity = identity.with_team(TeamId::new(team_id));
        }

        Ok(Some(identity))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{PrincipalType, TenantConfig, TokenValidationConfig};
    use crate::port::TenantManager;
    use async_trait::async_trait;
    use std::collections::HashMap;

    // Mock auth gate for testing
    struct MockAuthGate;

    #[async_trait]
    impl AuthGate for MockAuthGate {
        async fn authenticate(&self, _request: &AuthRequest) -> Result<AuthPrincipal, EdgeError> {
            let mut claims = HashMap::new();
            claims.insert(
                "tenant_id".to_string(),
                serde_json::Value::String("tenant-1".to_string()),
            );
            claims.insert(
                "organization_id".to_string(),
                serde_json::Value::String("org-1".to_string()),
            );
            claims.insert(
                "team_id".to_string(),
                serde_json::Value::String("team-1".to_string()),
            );

            Ok(AuthPrincipal {
                subject: "user-1".to_string(),
                email: Some("user@example.com".to_string()),
                issuer: Some("test-issuer".to_string()),
                audience: Some("test-audience".to_string()),
                principal_type: PrincipalType::User,
                claims,
                expires_at: None,
            })
        }

        async fn validate_token(&self, _token: &str) -> Result<bool, EdgeError> {
            Ok(true)
        }

        async fn authorize(
            &self,
            _principal: &AuthPrincipal,
            _resource: &str,
            _action: &str,
        ) -> Result<bool, EdgeError> {
            Ok(true)
        }

        fn validation_config(&self) -> &TokenValidationConfig {
            &TokenValidationConfig::default()
        }
    }

    // Mock tenant manager for testing
    struct MockTenantManager {
        enabled: bool,
    }

    #[async_trait]
    impl TenantManager for MockTenantManager {
        async fn get_tenant(
            &self,
            _tenant_id: &TenantId,
        ) -> Result<Option<TenantConfig>, EdgeError> {
            Ok(None)
        }

        async fn create_tenant(&self, _config: TenantConfig) -> Result<(), EdgeError> {
            Ok(())
        }

        async fn update_tenant(&self, _config: TenantConfig) -> Result<(), EdgeError> {
            Ok(())
        }

        async fn delete_tenant(&self, _tenant_id: &TenantId) -> Result<(), EdgeError> {
            Ok(())
        }

        async fn list_tenants(&self) -> Result<Vec<TenantId>, EdgeError> {
            Ok(vec![])
        }

        async fn reload(&self) -> Result<(), EdgeError> {
            Ok(())
        }

        async fn add_role_binding(
            &self,
            _binding: crate::domain::RoleBinding,
        ) -> Result<(), EdgeError> {
            Ok(())
        }

        async fn remove_role_binding(&self, _binding_id: &str) -> Result<(), EdgeError> {
            Ok(())
        }

        async fn get_role_bindings(
            &self,
            _identity: &HierarchicalIdentity,
        ) -> Result<Vec<crate::domain::RoleBinding>, EdgeError> {
            Ok(vec![])
        }

        async fn is_tenant_enabled(&self, _tenant_id: &TenantId) -> Result<bool, EdgeError> {
            Ok(self.enabled)
        }
    }

    #[tokio::test]
    async fn test_authenticate_hierarchical_success() {
        let auth_gate = Arc::new(MockAuthGate);
        let tenant_manager = Arc::new(MockTenantManager { enabled: true });
        let adapter = HierarchicalAuthAdapter::new(auth_gate, tenant_manager);

        let request = AuthRequest::new("test-token".to_string());
        let tenant_id = TenantId::new("tenant-1");

        let result = adapter
            .authenticate_hierarchical(&request, &tenant_id)
            .await;

        assert!(result.is_ok());
        let (principal, identity) = result.unwrap();
        assert_eq!(principal.subject, "user-1");
        assert_eq!(identity.tenant_id, tenant_id);
        assert_eq!(identity.organization_id, Some(OrganizationId::new("org-1")));
        assert_eq!(identity.team_id, Some(TeamId::new("team-1")));
    }

    #[tokio::test]
    async fn test_authenticate_hierarchical_disabled_tenant() {
        let auth_gate = Arc::new(MockAuthGate);
        let tenant_manager = Arc::new(MockTenantManager { enabled: false });
        let adapter = HierarchicalAuthAdapter::new(auth_gate, tenant_manager);

        let request = AuthRequest::new("test-token".to_string());
        let tenant_id = TenantId::new("tenant-1");

        let result = adapter
            .authenticate_hierarchical(&request, &tenant_id)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_verify_tenant_membership() {
        let auth_gate = Arc::new(MockAuthGate);
        let tenant_manager = Arc::new(MockTenantManager { enabled: true });
        let adapter = HierarchicalAuthAdapter::new(auth_gate, tenant_manager);

        let request = AuthRequest::new("test-token".to_string());
        let principal = MockAuthGate.authenticate(&request).await.unwrap();

        let result = adapter
            .verify_tenant_membership(&principal, &TenantId::new("tenant-1"))
            .await
            .unwrap();

        assert!(result);

        let result = adapter
            .verify_tenant_membership(&principal, &TenantId::new("wrong-tenant"))
            .await
            .unwrap();

        assert!(!result);
    }

    #[tokio::test]
    async fn test_extract_identity() {
        let auth_gate = Arc::new(MockAuthGate);
        let tenant_manager = Arc::new(MockTenantManager { enabled: true });
        let adapter = HierarchicalAuthAdapter::new(auth_gate, tenant_manager);

        let request = AuthRequest::new("test-token".to_string());
        let principal = MockAuthGate.authenticate(&request).await.unwrap();

        let identity = adapter.extract_identity(&principal).await.unwrap();

        assert!(identity.is_some());
        let identity = identity.unwrap();
        assert_eq!(identity.tenant_id, TenantId::new("tenant-1"));
        assert_eq!(identity.user_id, UserId::new("user-1"));
    }
}
