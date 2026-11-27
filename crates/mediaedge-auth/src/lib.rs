//! MediaEdge Authentication & Authorization (AuthN/AuthZ)
//!
//! Implements multi-tenant IAM with:
//! - Tenant CRUD, resource quotas, isolation
//! - RBAC: admin, operator, viewer, api-client roles
//! - OIDC SSO (Keycloak, Auth0, Azure AD, Google)
//! - MFA: TOTP, WebAuthn
//! - Encrypted secrets at rest (Age + AWS KMS envelope encryption)
//! - Immutable audit log (tamper-evident)
//!
//! All services depend on this crate for identity and access control.

#![warn(missing_docs)]

pub mod audit;
pub mod mfa;
pub mod oidc;
pub mod rbac;
pub mod secrets;
pub mod tenant;
pub mod token;

use std::sync::Arc;

use arc_swap::ArcSwap;
use color_eyre::eyre::Result;
use mediaedge_config::Config;
use mediaedge_observability::tracing as obs_tracing;

use self::audit::AuditLogger;
use self::mfa::MfaManager;
use self::oidc::OidcManager;
use self::rbac::RbacManager;
use self::secrets::SecretsManager;
use self::tenant::{TenantManager, TenantManagerConfig};
use self::token::TokenManager;

/// Core authentication and authorization service.
///
/// Wraps all auth subsystems behind a single facade.
/// Config hot-reload is supported via ArcSwap.
#[derive(Clone)]
pub struct AuthService {
    config: Arc<ArcSwap<Config>>,
    tenant_mgr: Arc<TenantManager>,
    rbac_mgr: Arc<RbacManager>,
    oidc_mgr: Arc<OidcManager>,
    mfa_mgr: Arc<MfaManager>,
    secrets_mgr: Arc<SecretsManager>,
    audit_log: Arc<AuditLogger>,
    token_mgr: Arc<TokenManager>,
}

impl AuthService {
    /// Create a new AuthService from configuration.
    pub async fn new(config: Config) -> Result<Self> {
        let config_arc = Arc::new(ArcSwap::from_pointee(config.clone()));

        let tenant_mgr =
            Arc::new(TenantManager::new(TenantManagerConfig::from_config(&config)?).await?);
        let rbac_mgr = Arc::new(RbacManager::new(tenant_mgr.clone()).await?);
        let oidc_mgr = Arc::new(OidcManager::new(&config).await?);
        let mfa_mgr = Arc::new(MfaManager::new(&config).await?);
        let secrets_mgr = Arc::new(SecretsManager::new(&config).await?);
        let audit_log = Arc::new(AuditLogger::new(&config).await?);
        let token_mgr = Arc::new(TokenManager::new(&config, tenant_mgr.clone()).await?);

        Ok(Self {
            config: config_arc,
            tenant_mgr,
            rbac_mgr,
            oidc_mgr,
            mfa_mgr,
            secrets_mgr,
            audit_log,
            token_mgr,
        })
    }

    /// Reload configuration atomically (called on SIGHUP or POST /api/v1/config-reload).
    pub fn reload_config(&self, new_config: Config) {
        self.config.store(Arc::new(new_config));
        tracing::info!("Auth service config reloaded");
    }

    /// Get current configuration snapshot.
    pub fn config(&self) -> Arc<Config> {
        self.config.load_full()
    }

    // Subsystem accessors
    pub fn tenant_manager(&self) -> &Arc<TenantManager> {
        &self.tenant_mgr
    }
    pub fn rbac_manager(&self) -> &Arc<RbacManager> {
        &self.rbac_mgr
    }
    pub fn oidc_manager(&self) -> &Arc<OidcManager> {
        &self.oidc_mgr
    }
    pub fn mfa_manager(&self) -> &Arc<MfaManager> {
        &self.mfa_mgr
    }
    pub fn secrets_manager(&self) -> &Arc<SecretsManager> {
        &self.secrets_mgr
    }
    pub fn audit_logger(&self) -> &Arc<AuditLogger> {
        &self.audit_log
    }
    pub fn token_manager(&self) -> &Arc<TokenManager> {
        &self.token_mgr
    }
}

/// Initialize tracing for auth crate.
pub fn init_tracing() -> obs_tracing::TracingGuard {
    obs_tracing::init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_auth_service_creation() {
        let config = Config::default();
        let service = AuthService::new(config).await;
        assert!(service.is_ok());
    }
}
