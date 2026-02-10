//! Tenant manager port - defines multi-tenant management interface
//!
//! Port trait for CRUD operations on tenant configurations with async reload support.

use crate::domain::{EdgeError, HierarchicalIdentity, RoleBinding, TenantConfig, TenantId};
use async_trait::async_trait;

/// Port interface for tenant configuration management
///
/// Provides CRUD operations for tenant configs and role bindings.
/// Implementations typically use in-memory storage with async reload capabilities.
#[async_trait]
pub trait TenantManager: Send + Sync {
    /// Get tenant configuration by ID
    ///
    /// # Arguments
    ///
    /// * `tenant_id` - The tenant identifier
    ///
    /// # Returns
    ///
    /// * `Ok(Some(config))` - Tenant configuration found
    /// * `Ok(None)` - Tenant not found
    /// * `Err(EdgeError)` - Retrieval error
    async fn get_tenant(&self, tenant_id: &TenantId) -> Result<Option<TenantConfig>, EdgeError>;

    /// Create a new tenant configuration
    ///
    /// # Arguments
    ///
    /// * `config` - The tenant configuration to create
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Tenant created successfully
    /// * `Err(EdgeError)` - Creation failed (e.g., tenant already exists)
    async fn create_tenant(&self, config: TenantConfig) -> Result<(), EdgeError>;

    /// Update an existing tenant configuration
    ///
    /// # Arguments
    ///
    /// * `config` - The updated tenant configuration
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Tenant updated successfully
    /// * `Err(EdgeError)` - Update failed (e.g., tenant not found)
    async fn update_tenant(&self, config: TenantConfig) -> Result<(), EdgeError>;

    /// Delete a tenant configuration
    ///
    /// # Arguments
    ///
    /// * `tenant_id` - The tenant identifier
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Tenant deleted successfully
    /// * `Err(EdgeError)` - Deletion failed
    async fn delete_tenant(&self, tenant_id: &TenantId) -> Result<(), EdgeError>;

    /// List all tenant IDs
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<TenantId>)` - List of all tenant IDs
    /// * `Err(EdgeError)` - Retrieval error
    async fn list_tenants(&self) -> Result<Vec<TenantId>, EdgeError>;

    /// Reload tenant configurations from persistent storage
    ///
    /// This method asynchronously reloads all tenant configurations,
    /// typically from a backing store like a database or configuration file.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Reload successful
    /// * `Err(EdgeError)` - Reload failed
    async fn reload(&self) -> Result<(), EdgeError>;

    /// Add a role binding for a user
    ///
    /// # Arguments
    ///
    /// * `binding` - The role binding to add
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Role binding added successfully
    /// * `Err(EdgeError)` - Addition failed
    async fn add_role_binding(&self, binding: RoleBinding) -> Result<(), EdgeError>;

    /// Remove a role binding
    ///
    /// # Arguments
    ///
    /// * `binding_id` - The role binding identifier
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Role binding removed successfully
    /// * `Err(EdgeError)` - Removal failed
    async fn remove_role_binding(&self, binding_id: &str) -> Result<(), EdgeError>;

    /// Get all role bindings for a user across all scopes
    ///
    /// # Arguments
    ///
    /// * `identity` - The hierarchical identity
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<RoleBinding>)` - List of role bindings
    /// * `Err(EdgeError)` - Retrieval error
    async fn get_role_bindings(
        &self,
        identity: &HierarchicalIdentity,
    ) -> Result<Vec<RoleBinding>, EdgeError>;

    /// Check if a tenant is enabled
    ///
    /// # Arguments
    ///
    /// * `tenant_id` - The tenant identifier
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Tenant is enabled
    /// * `Ok(false)` - Tenant is disabled or not found
    /// * `Err(EdgeError)` - Check error
    async fn is_tenant_enabled(&self, tenant_id: &TenantId) -> Result<bool, EdgeError>;
}
