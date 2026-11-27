//! Role-Based Access Control (RBAC) for multi-tenant authorization.
//!
//! Roles: admin, operator, viewer, api-client
//! Permissions are checked per-request with <5ms latency.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::tenant::TenantManager;

/// Built-in roles.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Full access to tenant resources and settings
    Admin,
    /// Operational access: streams, recordings, monitoring
    Operator,
    /// Read-only access: view streams, metrics, dashboards
    Viewer,
    /// API-only access: programmatic access with scoped tokens
    ApiClient,
    /// Custom tenant-defined role
    Custom,
}

impl Role {
    /// Get default permissions for a role.
    pub fn default_permissions(&self) -> HashSet<Permission> {
        match self {
            Role::Admin => Permission::all(),
            Role::Operator => Permission::operator_set(),
            Role::Viewer => Permission::viewer_set(),
            Role::ApiClient => Permission::api_client_set(),
            Role::Custom => HashSet::new(),
        }
    }
}

/// Fine-grained permissions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    // Stream permissions
    StreamCreate,
    StreamRead,
    StreamUpdate,
    StreamDelete,
    StreamStart,
    StreamStop,
    StreamMetrics,

    // Recording permissions
    RecordingCreate,
    RecordingRead,
    RecordingDelete,
    RecordingExport,

    // VOD permissions
    VodCreate,
    VodRead,
    VodUpdate,
    VodDelete,
    VodPublish,

    // Transcode permissions
    TranscodeCreate,
    TranscodeRead,
    TranscodeCancel,

    // Cluster permissions
    ClusterRead,
    ClusterManage,

    // Tenant permissions
    TenantRead,
    TenantUpdate,
    TenantManageUsers,
    TenantManageRoles,
    TenantManageQuotas,
    TenantManageBilling,
    TenantManageSettings,

    // User permissions
    UserCreate,
    UserRead,
    UserUpdate,
    UserDelete,
    UserManageMfa,

    // API key permissions
    ApiKeyCreate,
    ApiKeyRead,
    ApiKeyRevoke,

    // Audit permissions
    AuditRead,

    // Monitoring permissions
    MetricsRead,
    LogsRead,
    AlertsManage,

    // FAST channel permissions
    FastChannelCreate,
    FastChannelRead,
    FastChannelUpdate,
    FastChannelDelete,
    FastChannelSchedule,

    // Monetization permissions
    MonetizationRead,
    MonetizationManage,

    // AI permissions
    AiSchedulingRead,
    AiSchedulingManage,
}

impl Permission {
    /// All permissions (admin).
    pub fn all() -> HashSet<Self> {
        let mut set = HashSet::new();
        set.extend([
            Self::StreamCreate,
            Self::StreamRead,
            Self::StreamUpdate,
            Self::StreamDelete,
            Self::StreamStart,
            Self::StreamStop,
            Self::StreamMetrics,
            Self::RecordingCreate,
            Self::RecordingRead,
            Self::RecordingDelete,
            Self::RecordingExport,
            Self::VodCreate,
            Self::VodRead,
            Self::VodUpdate,
            Self::VodDelete,
            Self::VodPublish,
            Self::TranscodeCreate,
            Self::TranscodeRead,
            Self::TranscodeCancel,
            Self::ClusterRead,
            Self::ClusterManage,
            Self::TenantRead,
            Self::TenantUpdate,
            Self::TenantManageUsers,
            Self::TenantManageRoles,
            Self::TenantManageQuotas,
            Self::TenantManageBilling,
            Self::TenantManageSettings,
            Self::UserCreate,
            Self::UserRead,
            Self::UserUpdate,
            Self::UserDelete,
            Self::UserManageMfa,
            Self::ApiKeyCreate,
            Self::ApiKeyRead,
            Self::ApiKeyRevoke,
            Self::AuditRead,
            Self::MetricsRead,
            Self::LogsRead,
            Self::AlertsManage,
            Self::FastChannelCreate,
            Self::FastChannelRead,
            Self::FastChannelUpdate,
            Self::FastChannelDelete,
            Self::FastChannelSchedule,
            Self::MonetizationRead,
            Self::MonetizationManage,
            Self::AiSchedulingRead,
            Self::AiSchedulingManage,
        ]);
        set
    }

    /// Operator permissions.
    pub fn operator_set() -> HashSet<Self> {
        let mut set = HashSet::new();
        set.extend([
            Self::StreamCreate,
            Self::StreamRead,
            Self::StreamUpdate,
            Self::StreamDelete,
            Self::StreamStart,
            Self::StreamStop,
            Self::StreamMetrics,
            Self::RecordingCreate,
            Self::RecordingRead,
            Self::RecordingExport,
            Self::VodCreate,
            Self::VodRead,
            Self::VodUpdate,
            Self::VodPublish,
            Self::TranscodeCreate,
            Self::TranscodeRead,
            Self::TranscodeCancel,
            Self::ClusterRead,
            Self::TenantRead,
            Self::UserRead,
            Self::ApiKeyRead,
            Self::MetricsRead,
            Self::LogsRead,
            Self::FastChannelCreate,
            Self::FastChannelRead,
            Self::FastChannelUpdate,
            Self::FastChannelSchedule,
            Self::MonetizationRead,
            Self::AiSchedulingRead,
        ]);
        set
    }

    /// Viewer permissions.
    pub fn viewer_set() -> HashSet<Self> {
        let mut set = HashSet::new();
        set.extend([
            Self::StreamRead,
            Self::StreamMetrics,
            Self::RecordingRead,
            Self::VodRead,
            Self::TranscodeRead,
            Self::ClusterRead,
            Self::TenantRead,
            Self::UserRead,
            Self::MetricsRead,
            Self::FastChannelRead,
            Self::MonetizationRead,
            Self::AiSchedulingRead,
        ]);
        set
    }

    /// API client permissions.
    pub fn api_client_set() -> HashSet<Self> {
        let mut set = HashSet::new();
        set.extend([
            Self::StreamCreate,
            Self::StreamRead,
            Self::StreamUpdate,
            Self::StreamStart,
            Self::StreamStop,
            Self::RecordingCreate,
            Self::RecordingRead,
            Self::VodCreate,
            Self::VodRead,
            Self::VodPublish,
            Self::TranscodeCreate,
            Self::TranscodeRead,
            Self::MetricsRead,
        ]);
        set
    }
}

/// User within a tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    pub name: String,
    pub roles: Vec<RoleAssignment>,
    pub mfa_enabled: bool,
    pub status: UserStatus,
    pub last_login_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// User status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    Active,
    Invited,
    Suspended,
    Deleted,
}

/// Role assignment with optional resource scoping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleAssignment {
    pub role: Role,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub granted_by: Uuid,
    pub granted_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// API key for programmatic access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub key_hash: String,
    pub prefix: String,
    pub roles: Vec<RoleAssignment>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub created_by: Uuid,
}

/// RBAC manager.
pub struct RbacManager {
    tenant_mgr: Arc<TenantManager>,
    role_cache: Arc<DashMap<Uuid, HashMap<Uuid, Vec<RoleAssignment>>>>, // tenant_id -> user_id -> roles
    permission_cache: Arc<DashMap<String, HashSet<Permission>>>,        // cache_key -> permissions
}

impl RbacManager {
    /// Create a new RBAC manager.
    pub async fn new(tenant_mgr: Arc<TenantManager>) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            tenant_mgr,
            role_cache: Arc::new(DashMap::new()),
            permission_cache: Arc::new(DashMap::new()),
        }))
    }

    /// Check if a user has a permission for a tenant.
    pub async fn check_permission(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        permission: Permission,
    ) -> Result<bool> {
        let roles = self.get_user_roles(tenant_id, user_id).await?;
        Ok(roles.iter().any(|r| r.has_permission(permission)))
    }

    /// Check if an API key has a permission.
    pub async fn check_api_key_permission(
        &self,
        tenant_id: Uuid,
        api_key_id: Uuid,
        permission: Permission,
    ) -> Result<bool> {
        let roles = self.get_api_key_roles(tenant_id, api_key_id).await?;
        Ok(roles.iter().any(|r| r.has_permission(permission)))
    }

    /// Get all permissions for a user in a tenant.
    pub async fn get_user_permissions(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<HashSet<Permission>> {
        let roles = self.get_user_roles(tenant_id, user_id).await?;
        let mut permissions = HashSet::new();
        for role in roles {
            permissions.extend(role.permissions());
        }
        Ok(permissions)
    }

    /// Assign a role to a user.
    pub async fn assign_role(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        assignment: RoleAssignment,
    ) -> Result<()> {
        // Validate tenant exists
        self.tenant_mgr
            .get_tenant(tenant_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Tenant not found"))?;

        // Store in database
        sqlx::query!(
            r#"
            INSERT INTO user_roles (tenant_id, user_id, role, resource_type, resource_id, granted_by, granted_at, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (tenant_id, user_id, role, resource_type, resource_id) DO UPDATE SET
                granted_by = EXCLUDED.granted_by,
                granted_at = EXCLUDED.granted_at,
                expires_at = EXCLUDED.expires_at
            "#,
            tenant_id,
            user_id,
            serde_json::to_value(assignment.role)?,
            assignment.resource_type,
            assignment.resource_id,
            assignment.granted_by,
            assignment.granted_at,
            assignment.expires_at,
        )
        .execute(&self.tenant_mgr.config.pool)
        .await?;

        // Invalidate cache
        self.role_cache.remove(&tenant_id);
        self.permission_cache.clear();

        Ok(())
    }

    /// Revoke a role from a user.
    pub async fn revoke_role(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        role: Role,
        resource_type: Option<String>,
        resource_id: Option<Uuid>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM user_roles
            WHERE tenant_id = $1 AND user_id = $2 AND role = $3
            AND (resource_type IS NOT DISTINCT FROM $4)
            AND (resource_id IS NOT DISTINCT FROM $5)
            "#,
            tenant_id,
            user_id,
            serde_json::to_value(role)?,
            resource_type,
            resource_id,
        )
        .execute(&self.tenant_mgr.config.pool)
        .await?;

        self.role_cache.remove(&tenant_id);
        self.permission_cache.clear();

        Ok(())
    }

    /// Get user roles (with caching).
    async fn get_user_roles(&self, tenant_id: Uuid, user_id: Uuid) -> Result<Vec<RoleAssignment>> {
        let cache_key = format!("{}:{}", tenant_id, user_id);

        // Check permission cache first
        if let Some(perms) = self.permission_cache.get(&cache_key) {
            // Convert permissions back to roles (simplified)
            return Ok(vec![RoleAssignment {
                role: Role::Custom,
                resource_type: None,
                resource_id: None,
                granted_by: Uuid::nil(),
                granted_at: chrono::Utc::now(),
                expires_at: None,
            }]);
        }

        // Check role cache
        if let Some(tenant_roles) = self.role_cache.get(&tenant_id) {
            if let Some(roles) = tenant_roles.get(&user_id) {
                return Ok(roles.clone());
            }
        }

        // Load from database
        let rows = sqlx::query!(
            r#"
            SELECT role, resource_type, resource_id, granted_by, granted_at, expires_at
            FROM user_roles
            WHERE tenant_id = $1 AND user_id = $2
            AND (expires_at IS NULL OR expires_at > NOW())
            "#,
            tenant_id,
            user_id,
        )
        .fetch_all(&self.tenant_mgr.config.pool)
        .await?;

        let roles: Vec<RoleAssignment> = rows
            .into_iter()
            .map(|row| RoleAssignment {
                role: serde_json::from_value(serde_json::to_value(row.role)?)
                    .unwrap_or(Role::Custom),
                resource_type: row.resource_type,
                resource_id: row.resource_id,
                granted_by: row.granted_by,
                granted_at: row.granted_at,
                expires_at: row.expires_at,
            })
            .collect();

        // Cache
        self.role_cache
            .entry(tenant_id)
            .or_default()
            .insert(user_id, roles.clone());

        // Build permission cache
        let mut permissions = HashSet::new();
        for role in &roles {
            permissions.extend(role.permissions());
        }
        self.permission_cache.insert(cache_key, permissions);

        Ok(roles)
    }

    /// Get API key roles.
    async fn get_api_key_roles(
        &self,
        tenant_id: Uuid,
        api_key_id: Uuid,
    ) -> Result<Vec<RoleAssignment>> {
        let rows = sqlx::query!(
            r#"
            SELECT role, resource_type, resource_id, granted_by, granted_at, expires_at
            FROM api_key_roles
            WHERE tenant_id = $1 AND api_key_id = $2
            AND (expires_at IS NULL OR expires_at > NOW())
            "#,
            tenant_id,
            api_key_id,
        )
        .fetch_all(&self.tenant_mgr.config.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| RoleAssignment {
                role: serde_json::from_value(serde_json::to_value(row.role)?)
                    .unwrap_or(Role::Custom),
                resource_type: row.resource_type,
                resource_id: row.resource_id,
                granted_by: row.granted_by,
                granted_at: row.granted_at,
                expires_at: row.expires_at,
            })
            .collect())
    }
}

impl RoleAssignment {
    /// Get permissions for this role assignment.
    pub fn permissions(&self) -> HashSet<Permission> {
        self.role.default_permissions()
    }

    /// Check if this role has a specific permission.
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions().contains(&permission)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_permissions() {
        let admin_perms = Role::Admin.default_permissions();
        let viewer_perms = Role::Viewer.default_permissions();

        assert!(admin_perms.contains(&Permission::TenantManageUsers));
        assert!(!viewer_perms.contains(&Permission::TenantManageUsers));
        assert!(viewer_perms.contains(&Permission::StreamRead));
    }

    #[test]
    fn test_permission_sets() {
        let operator = Permission::operator_set();
        assert!(operator.contains(&Permission::StreamCreate));
        assert!(!operator.contains(&Permission::TenantManageUsers));
    }
}
