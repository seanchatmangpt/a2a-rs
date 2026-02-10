//! Hierarchical authentication port - extends auth gate with tenant hierarchy
//!
//! Port trait for hierarchical authentication with tenant > organization > team > user hierarchy.

use crate::domain::{
    AuthPrincipal, AuthRequest, EdgeError, HierarchicalIdentity, OrganizationId, TeamId, TenantId,
};
use async_trait::async_trait;

/// Port interface for hierarchical authentication
///
/// Extends basic authentication with multi-tenant hierarchical identity resolution.
/// Validates that principals belong to correct tenant/organization/team hierarchy.
#[async_trait]
pub trait HierarchicalAuthGate: Send + Sync {
    /// Authenticate and resolve hierarchical identity
    ///
    /// # Arguments
    ///
    /// * `request` - The authentication request
    /// * `tenant_id` - The tenant context for this request
    ///
    /// # Returns
    ///
    /// * `Ok((AuthPrincipal, HierarchicalIdentity))` - Authenticated principal with resolved hierarchy
    /// * `Err(EdgeError)` - Authentication failed or identity not in tenant
    async fn authenticate_hierarchical(
        &self,
        request: &AuthRequest,
        tenant_id: &TenantId,
    ) -> Result<(AuthPrincipal, HierarchicalIdentity), EdgeError>;

    /// Verify principal belongs to specified tenant
    ///
    /// # Arguments
    ///
    /// * `principal` - The authenticated principal
    /// * `tenant_id` - The tenant to verify against
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Principal belongs to tenant
    /// * `Ok(false)` - Principal does not belong to tenant
    /// * `Err(EdgeError)` - Verification error
    async fn verify_tenant_membership(
        &self,
        principal: &AuthPrincipal,
        tenant_id: &TenantId,
    ) -> Result<bool, EdgeError>;

    /// Verify principal belongs to specified organization within tenant
    ///
    /// # Arguments
    ///
    /// * `identity` - The hierarchical identity
    /// * `organization_id` - The organization to verify against
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Identity belongs to organization
    /// * `Ok(false)` - Identity does not belong to organization
    /// * `Err(EdgeError)` - Verification error
    async fn verify_organization_membership(
        &self,
        identity: &HierarchicalIdentity,
        organization_id: &OrganizationId,
    ) -> Result<bool, EdgeError>;

    /// Verify principal belongs to specified team within organization
    ///
    /// # Arguments
    ///
    /// * `identity` - The hierarchical identity
    /// * `team_id` - The team to verify against
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Identity belongs to team
    /// * `Ok(false)` - Identity does not belong to team
    /// * `Err(EdgeError)` - Verification error
    async fn verify_team_membership(
        &self,
        identity: &HierarchicalIdentity,
        team_id: &TeamId,
    ) -> Result<bool, EdgeError>;

    /// Extract hierarchical identity from principal claims
    ///
    /// # Arguments
    ///
    /// * `principal` - The authenticated principal
    ///
    /// # Returns
    ///
    /// * `Ok(Some(identity))` - Hierarchical identity extracted
    /// * `Ok(None)` - No hierarchy information in claims
    /// * `Err(EdgeError)` - Extraction error
    async fn extract_identity(
        &self,
        principal: &AuthPrincipal,
    ) -> Result<Option<HierarchicalIdentity>, EdgeError>;
}
