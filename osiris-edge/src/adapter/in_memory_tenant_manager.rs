//! In-memory tenant manager adapter
//!
//! Thread-safe in-memory implementation of TenantManager with async reload support.

use crate::domain::{EdgeError, HierarchicalIdentity, RoleBinding, TenantConfig, TenantId};
use crate::port::TenantManager;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// In-memory tenant manager with async reload capability
#[derive(Clone)]
pub struct InMemoryTenantManager {
    /// Tenant configurations indexed by tenant ID
    tenants: Arc<RwLock<HashMap<TenantId, TenantConfig>>>,
    /// Role bindings indexed by binding ID
    role_bindings: Arc<RwLock<HashMap<String, RoleBinding>>>,
    /// Optional reload function for external data source
    reload_fn: Option<Arc<dyn Fn() -> Result<Vec<TenantConfig>, EdgeError> + Send + Sync>>,
}

impl InMemoryTenantManager {
    /// Create a new empty tenant manager
    pub fn new() -> Self {
        Self {
            tenants: Arc::new(RwLock::new(HashMap::new())),
            role_bindings: Arc::new(RwLock::new(HashMap::new())),
            reload_fn: None,
        }
    }

    /// Create a tenant manager with initial configurations
    pub fn with_tenants(configs: Vec<TenantConfig>) -> Self {
        let tenants = configs
            .into_iter()
            .map(|c| (c.tenant_id.clone(), c))
            .collect();

        Self {
            tenants: Arc::new(RwLock::new(tenants)),
            role_bindings: Arc::new(RwLock::new(HashMap::new())),
            reload_fn: None,
        }
    }

    /// Set a reload function that will be called when reload() is invoked
    pub fn with_reload_fn<F>(mut self, reload_fn: F) -> Self
    where
        F: Fn() -> Result<Vec<TenantConfig>, EdgeError> + Send + Sync + 'static,
    {
        self.reload_fn = Some(Arc::new(reload_fn));
        self
    }

    /// Get current tenant count (for testing/monitoring)
    pub async fn tenant_count(&self) -> usize {
        self.tenants.read().await.len()
    }

    /// Get current role binding count (for testing/monitoring)
    pub async fn binding_count(&self) -> usize {
        self.role_bindings.read().await.len()
    }
}

impl Default for InMemoryTenantManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TenantManager for InMemoryTenantManager {
    async fn get_tenant(&self, tenant_id: &TenantId) -> Result<Option<TenantConfig>, EdgeError> {
        let tenants = self.tenants.read().await;
        Ok(tenants.get(tenant_id).cloned())
    }

    async fn create_tenant(&self, config: TenantConfig) -> Result<(), EdgeError> {
        let mut tenants = self.tenants.write().await;

        if tenants.contains_key(&config.tenant_id) {
            return Err(EdgeError::Configuration(format!(
                "Tenant {} already exists",
                config.tenant_id.as_str()
            )));
        }

        tenants.insert(config.tenant_id.clone(), config);
        Ok(())
    }

    async fn update_tenant(&self, config: TenantConfig) -> Result<(), EdgeError> {
        let mut tenants = self.tenants.write().await;

        if !tenants.contains_key(&config.tenant_id) {
            return Err(EdgeError::Configuration(format!(
                "Tenant {} not found",
                config.tenant_id.as_str()
            )));
        }

        tenants.insert(config.tenant_id.clone(), config);
        Ok(())
    }

    async fn delete_tenant(&self, tenant_id: &TenantId) -> Result<(), EdgeError> {
        let mut tenants = self.tenants.write().await;

        if tenants.remove(tenant_id).is_none() {
            return Err(EdgeError::Configuration(format!(
                "Tenant {} not found",
                tenant_id.as_str()
            )));
        }

        Ok(())
    }

    async fn list_tenants(&self) -> Result<Vec<TenantId>, EdgeError> {
        let tenants = self.tenants.read().await;
        Ok(tenants.keys().cloned().collect())
    }

    async fn reload(&self) -> Result<(), EdgeError> {
        if let Some(reload_fn) = &self.reload_fn {
            let configs = reload_fn()?;
            let mut tenants = self.tenants.write().await;
            tenants.clear();
            for config in configs {
                tenants.insert(config.tenant_id.clone(), config);
            }
            Ok(())
        } else {
            Err(EdgeError::Configuration(
                "No reload function configured".to_string(),
            ))
        }
    }

    async fn add_role_binding(&self, binding: RoleBinding) -> Result<(), EdgeError> {
        let mut bindings = self.role_bindings.write().await;
        bindings.insert(binding.binding_id.clone(), binding);
        Ok(())
    }

    async fn remove_role_binding(&self, binding_id: &str) -> Result<(), EdgeError> {
        let mut bindings = self.role_bindings.write().await;

        if bindings.remove(binding_id).is_none() {
            return Err(EdgeError::Configuration(format!(
                "Role binding {} not found",
                binding_id
            )));
        }

        Ok(())
    }

    async fn get_role_bindings(
        &self,
        identity: &HierarchicalIdentity,
    ) -> Result<Vec<RoleBinding>, EdgeError> {
        let bindings = self.role_bindings.read().await;

        let matching_bindings: Vec<RoleBinding> = bindings
            .values()
            .filter(|binding| {
                // Filter bindings that match this user and apply to their scope
                binding.user_id == identity.user_id && binding.scope.applies_to(identity)
            })
            .cloned()
            .collect();

        Ok(matching_bindings)
    }

    async fn is_tenant_enabled(&self, tenant_id: &TenantId) -> Result<bool, EdgeError> {
        let tenants = self.tenants.read().await;
        Ok(tenants
            .get(tenant_id)
            .map(|config| config.enabled)
            .unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Role, Scope, UserId, WipLimits};

    #[tokio::test]
    async fn test_create_and_get_tenant() {
        let manager = InMemoryTenantManager::new();
        let tenant_id = TenantId::new("test-tenant");
        let config = TenantConfig::new(tenant_id.clone(), "Test Tenant");

        manager.create_tenant(config.clone()).await.unwrap();

        let retrieved = manager.get_tenant(&tenant_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Tenant");
    }

    #[tokio::test]
    async fn test_duplicate_tenant_error() {
        let manager = InMemoryTenantManager::new();
        let tenant_id = TenantId::new("test-tenant");
        let config = TenantConfig::new(tenant_id.clone(), "Test Tenant");

        manager.create_tenant(config.clone()).await.unwrap();
        let result = manager.create_tenant(config).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_tenant() {
        let manager = InMemoryTenantManager::new();
        let tenant_id = TenantId::new("test-tenant");
        let mut config = TenantConfig::new(tenant_id.clone(), "Test Tenant");

        manager.create_tenant(config.clone()).await.unwrap();

        config.name = "Updated Tenant".to_string();
        manager.update_tenant(config).await.unwrap();

        let retrieved = manager.get_tenant(&tenant_id).await.unwrap().unwrap();
        assert_eq!(retrieved.name, "Updated Tenant");
    }

    #[tokio::test]
    async fn test_delete_tenant() {
        let manager = InMemoryTenantManager::new();
        let tenant_id = TenantId::new("test-tenant");
        let config = TenantConfig::new(tenant_id.clone(), "Test Tenant");

        manager.create_tenant(config).await.unwrap();
        manager.delete_tenant(&tenant_id).await.unwrap();

        let retrieved = manager.get_tenant(&tenant_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_list_tenants() {
        let manager = InMemoryTenantManager::new();

        for i in 0..3 {
            let tenant_id = TenantId::new(format!("tenant-{}", i));
            let config = TenantConfig::new(tenant_id, format!("Tenant {}", i));
            manager.create_tenant(config).await.unwrap();
        }

        let tenants = manager.list_tenants().await.unwrap();
        assert_eq!(tenants.len(), 3);
    }

    #[tokio::test]
    async fn test_role_bindings() {
        let manager = InMemoryTenantManager::new();
        let tenant_id = TenantId::new("test-tenant");
        let user_id = UserId::new("user-1");
        let identity = HierarchicalIdentity::new(tenant_id.clone(), user_id.clone());

        let binding = RoleBinding::new(user_id, "admin", Scope::tenant(tenant_id.clone()));

        manager.add_role_binding(binding.clone()).await.unwrap();

        let bindings = manager.get_role_bindings(&identity).await.unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].role_name, "admin");

        manager
            .remove_role_binding(&binding.binding_id)
            .await
            .unwrap();

        let bindings = manager.get_role_bindings(&identity).await.unwrap();
        assert_eq!(bindings.len(), 0);
    }

    #[tokio::test]
    async fn test_tenant_enabled() {
        let manager = InMemoryTenantManager::new();
        let tenant_id = TenantId::new("test-tenant");
        let config = TenantConfig::new(tenant_id.clone(), "Test Tenant");

        manager.create_tenant(config).await.unwrap();

        let enabled = manager.is_tenant_enabled(&tenant_id).await.unwrap();
        assert!(enabled);

        let mut config = manager.get_tenant(&tenant_id).await.unwrap().unwrap();
        config = config.disable();
        manager.update_tenant(config).await.unwrap();

        let enabled = manager.is_tenant_enabled(&tenant_id).await.unwrap();
        assert!(!enabled);
    }

    #[tokio::test]
    async fn test_reload_with_function() {
        let manager = InMemoryTenantManager::new().with_reload_fn(|| {
            let tenant1 = TenantConfig::new(TenantId::new("tenant-1"), "Tenant 1");
            let tenant2 = TenantConfig::new(TenantId::new("tenant-2"), "Tenant 2");
            Ok(vec![tenant1, tenant2])
        });

        manager.reload().await.unwrap();

        let tenants = manager.list_tenants().await.unwrap();
        assert_eq!(tenants.len(), 2);
    }

    #[tokio::test]
    async fn test_with_tenants_constructor() {
        let configs = vec![
            TenantConfig::new(TenantId::new("tenant-1"), "Tenant 1"),
            TenantConfig::new(TenantId::new("tenant-2"), "Tenant 2"),
        ];

        let manager = InMemoryTenantManager::with_tenants(configs);

        let count = manager.tenant_count().await;
        assert_eq!(count, 2);
    }
}
