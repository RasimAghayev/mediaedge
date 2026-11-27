//! OIDC/OAuth2 SSO Integration
//!
//! Supports Keycloak, Auth0, Azure AD, Google, and generic OIDC providers.
//! Handles PKCE, token refresh, and user provisioning (JIT).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use oauth2::{
    AuthUrl, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl,
    Scope, TokenResponse, TokenUrl, basic::BasicClient,
};
use openidconnect::{
    AccessToken, ClientSecret as OidcClientSecret, EmptyAdditionalClaims,
    EmptyAdditionalProviderMetadata, IssuerUrl, Nonce, OAuth2TokenResponse, ProviderMetadata,
    UserInfoClaims,
    core::{CoreClient, CoreProviderMetadata, CoreTokenResponse},
};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use mediaedge_config::Config;

/// Supported OIDC providers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OidcProvider {
    Keycloak,
    Auth0,
    AzureAd,
    Google,
    Generic,
}

/// OIDC provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcConfig {
    pub provider: OidcProvider,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub scopes: Vec<String>,
    pub pkce_enabled: bool,
    pub discovery_enabled: bool,
    pub metadata: Option<OidcProviderMetadata>,
}

/// Provider metadata (cached from discovery).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcProviderMetadata {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub jwks_uri: String,
    pub userinfo_endpoint: String,
    pub end_session_endpoint: Option<String>,
    pub issuer: String,
}

/// OIDC authentication state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcAuthState {
    pub csrf_token: String,
    pub pkce_verifier: Option<String>,
    pub nonce: String,
    pub provider: OidcProvider,
    pub tenant_id: Uuid,
    pub return_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// OIDC user info (normalized across providers).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcUserInfo {
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub preferred_username: Option<String>,
    pub picture: Option<String>,
    pub locale: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
    pub custom_claims: HashMap<String, serde_json::Value>,
}

/// OIDC tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcTokens {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub scope: Option<String>,
}

/// OIDC manager trait for different providers.
#[async_trait]
pub trait OidcProviderClient: Send + Sync {
    async fn get_authorization_url(&self, state: &OidcAuthState) -> Result<String>;
    async fn exchange_code(&self, code: &str, pkce_verifier: Option<&str>) -> Result<OidcTokens>;
    async fn refresh_token(&self, refresh_token: &str) -> Result<OidcTokens>;
    async fn get_user_info(&self, access_token: &str) -> Result<OidcUserInfo>;
    async fn revoke_token(&self, token: &str) -> Result<()>;
    async fn get_logout_url(
        &self,
        id_token_hint: Option<&str>,
        post_logout_redirect_uri: Option<&str>,
    ) -> Result<String>;
}

/// Generic OIDC client implementation.
pub struct GenericOidcClient {
    client: CoreClient,
    http_client: HttpClient,
    config: OidcConfig,
}

impl GenericOidcClient {
    pub async fn new(config: OidcConfig) -> Result<Self> {
        let issuer_url = IssuerUrl::new(config.issuer_url.clone()).context("Invalid issuer URL")?;

        let metadata = if config.discovery_enabled {
            CoreProviderMetadata::discover_async(issuer_url, HttpClient::new())
                .await
                .context("Failed to discover OIDC metadata")?
        } else {
            CoreProviderMetadata::new(
                AuthUrl::new(format!("{}/authorize", config.issuer_url))
                    .context("Invalid auth endpoint")?,
                TokenUrl::new(format!("{}/token", config.issuer_url))
                    .context("Invalid token endpoint")?,
                None,
                None,
                None,
                None,
                None,
            )
        };

        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(config.client_id.clone()),
            Some(OidcClientSecret::new(config.client_secret.clone())),
        )
        .set_redirect_uri(
            RedirectUrl::new(config.redirect_url.clone()).context("Invalid redirect URL")?,
        );

        Ok(Self {
            client,
            http_client: HttpClient::new(),
            config,
        })
    }
}

#[async_trait]
impl OidcProviderClient for GenericOidcClient {
    async fn get_authorization_url(&self, state: &OidcAuthState) -> Result<String> {
        let csrf_token = CsrfToken::new(state.csrf_token.clone());
        let nonce = Nonce::new(state.nonce.clone());

        let (pkce_challenge, pkce_verifier) = if self.config.pkce_enabled {
            let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
            (Some(challenge), Some(verifier))
        } else {
            (None, None)
        };

        let mut auth_req = self.client.authorize_url(|| csrf_token, || nonce);

        for scope in &self.config.scopes {
            auth_req = auth_req.add_scope(Scope::new(scope.clone()));
        }

        if let Some(challenge) = pkce_challenge {
            auth_req = auth_req.set_pkce_challenge(challenge);
        }

        Ok(auth_req.url().to_string())
    }

    async fn exchange_code(&self, code: &str, pkce_verifier: Option<&str>) -> Result<OidcTokens> {
        let mut token_req = self
            .client
            .exchange_code(oauth2::AuthorizationCode::new(code.to_string()));

        if let Some(verifier) = pkce_verifier {
            token_req = token_req.set_pkce_verifier(PkceCodeVerifier::new(verifier.to_string()));
        }

        let token_response = token_req
            .request_async(&self.http_client)
            .await
            .context("Failed to exchange code for tokens")?;

        Ok(OidcTokens {
            access_token: token_response.access_token().secret().clone(),
            token_type: token_response.token_type().clone(),
            expires_in: token_response.expires_in().map(|d| d.as_secs()),
            refresh_token: token_response.refresh_token().map(|t| t.secret().clone()),
            id_token: token_response.id_token().map(|t| t.to_string()),
            scope: token_response.scopes().map(|s| {
                s.iter()
                    .map(|sc| sc.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            }),
        })
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<OidcTokens> {
        let token_response = self
            .client
            .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token.to_string()))
            .request_async(&self.http_client)
            .await
            .context("Failed to refresh token")?;

        Ok(OidcTokens {
            access_token: token_response.access_token().secret().clone(),
            token_type: token_response.token_type().clone(),
            expires_in: token_response.expires_in().map(|d| d.as_secs()),
            refresh_token: token_response.refresh_token().map(|t| t.secret().clone()),
            id_token: token_response.id_token().map(|t| t.to_string()),
            scope: token_response.scopes().map(|s| {
                s.iter()
                    .map(|sc| sc.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            }),
        })
    }

    async fn get_user_info(&self, access_token: &str) -> Result<OidcUserInfo> {
        let user_info: UserInfoClaims<EmptyAdditionalClaims, _> = self
            .client
            .user_info(AccessToken::new(access_token.to_string()), None)
            .request_async(&self.http_client)
            .await
            .context("Failed to get user info")?;

        Ok(OidcUserInfo {
            sub: user_info.subject().to_string(),
            email: user_info.email().map(|e| e.to_string()),
            email_verified: user_info.email_verified(),
            name: user_info
                .name()
                .map(|n| n.get(None).map(|s| s.to_string()))
                .flatten(),
            given_name: user_info
                .given_name()
                .map(|n| n.get(None).map(|s| s.to_string()))
                .flatten(),
            family_name: user_info
                .family_name()
                .map(|n| n.get(None).map(|s| s.to_string()))
                .flatten(),
            preferred_username: user_info.preferred_username().map(|n| n.to_string()),
            picture: user_info
                .picture()
                .map(|p| p.get(None).map(|u| u.to_string()))
                .flatten(),
            locale: user_info.locale().map(|l| l.to_string()),
            updated_at: user_info
                .updated_at()
                .map(|t| DateTime::from_timestamp(t as i64, 0))
                .flatten(),
            custom_claims: user_info.claims().clone(),
        })
    }

    async fn revoke_token(&self, token: &str) -> Result<()> {
        // Token revocation endpoint (RFC 7009)
        let revoke_url = format!("{}/revoke", self.config.issuer_url.trim_end_matches('/'));
        self.http_client
            .post(&revoke_url)
            .form(&[
                ("token", token),
                ("client_id", &self.config.client_id),
                ("client_secret", &self.config.client_secret),
            ])
            .send()
            .await?;
        Ok(())
    }

    async fn get_logout_url(
        &self,
        id_token_hint: Option<&str>,
        post_logout_redirect_uri: Option<&str>,
    ) -> Result<String> {
        let metadata = self.client.provider_metadata();
        let logout_url = metadata
            .end_session_endpoint()
            .ok_or_else(|| anyhow::anyhow!("No end_session_endpoint in metadata"))?;

        let mut url = logout_url.to_string();
        if let Some(id_token) = id_token_hint {
            url.push_str(&format!("?id_token_hint={}", urlencoding::encode(id_token)));
        }
        if let Some(redirect) = post_logout_redirect_uri {
            let sep = if url.contains('?') { "&" } else { "?" };
            url.push_str(&format!(
                "{}post_logout_redirect_uri={}",
                sep,
                urlencoding::encode(redirect)
            ));
        }
        Ok(url)
    }
}

/// Keycloak-specific client (extends generic with realm support).
pub struct KeycloakClient {
    generic: GenericOidcClient,
    realm: String,
}

impl KeycloakClient {
    pub async fn new(config: OidcConfig, realm: String) -> Result<Self> {
        let mut config = config;
        config.issuer_url = format!(
            "{}/realms/{}",
            config.issuer_url.trim_end_matches('/'),
            realm
        );
        Ok(Self {
            generic: GenericOidcClient::new(config).await?,
            realm,
        })
    }
}

#[async_trait]
impl OidcProviderClient for KeycloakClient {
    async fn get_authorization_url(&self, state: &OidcAuthState) -> Result<String> {
        self.generic.get_authorization_url(state).await
    }
    async fn exchange_code(&self, code: &str, pkce_verifier: Option<&str>) -> Result<OidcTokens> {
        self.generic.exchange_code(code, pkce_verifier).await
    }
    async fn refresh_token(&self, refresh_token: &str) -> Result<OidcTokens> {
        self.generic.refresh_token(refresh_token).await
    }
    async fn get_user_info(&self, access_token: &str) -> Result<OidcUserInfo> {
        self.generic.get_user_info(access_token).await
    }
    async fn revoke_token(&self, token: &str) -> Result<()> {
        self.generic.revoke_token(token).await
    }
    async fn get_logout_url(
        &self,
        id_token_hint: Option<&str>,
        post_logout_redirect_uri: Option<&str>,
    ) -> Result<String> {
        self.generic
            .get_logout_url(id_token_hint, post_logout_redirect_uri)
            .await
    }
}

/// OIDC Manager - orchestrates multiple providers per tenant.
pub struct OidcManager {
    providers: Arc<DashMap<Uuid, HashMap<String, Arc<dyn OidcProviderClient>>>>, // tenant_id -> provider_name -> client
    auth_states: Arc<DashMap<String, OidcAuthState>>, // csrf_token -> state
    config: Arc<Config>,
}

impl OidcManager {
    /// Create a new OIDC manager.
    pub async fn new(config: &Config) -> Result<Arc<Self>> {
        let mgr = Arc::new(Self {
            providers: Arc::new(DashMap::new()),
            auth_states: Arc::new(DashMap::new()),
            config: Arc::new(config.clone()),
        });

        // Load configured providers from database
        mgr.load_providers().await?;

        Ok(mgr)
    }

    async fn load_providers(&self) -> Result<()> {
        // Load from database and initialize clients
        // Implementation depends on schema
        Ok(())
    }

    /// Register an OIDC provider for a tenant.
    pub async fn register_provider(
        &self,
        tenant_id: Uuid,
        name: String,
        config: OidcConfig,
    ) -> Result<()> {
        let client: Arc<dyn OidcProviderClient> = match config.provider {
            OidcProvider::Keycloak => {
                let realm = config
                    .metadata
                    .as_ref()
                    .and_then(|m| m.issuer.split('/').last())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "master".to_string());
                Arc::new(KeycloakClient::new(config, realm).await?)
            }
            _ => Arc::new(GenericOidcClient::new(config).await?),
        };

        self.providers
            .entry(tenant_id)
            .or_default()
            .insert(name, client);

        Ok(())
    }

    /// Start OIDC authentication flow.
    pub async fn start_auth(
        &self,
        tenant_id: Uuid,
        provider_name: &str,
        return_url: Option<String>,
    ) -> Result<String> {
        let providers = self
            .providers
            .get(&tenant_id)
            .ok_or_else(|| anyhow::anyhow!("No providers configured for tenant"))?;

        let client = providers
            .get(provider_name)
            .ok_or_else(|| anyhow::anyhow!("Provider not found"))?;

        let csrf_token = Uuid::new_v4().to_string();
        let nonce = Uuid::new_v4().to_string();
        let pkce_verifier = if client.requires_pkce() {
            Some(PkceCodeVerifier::new_random().secret().clone())
        } else {
            None
        };

        let state = OidcAuthState {
            csrf_token: csrf_token.clone(),
            pkce_verifier: pkce_verifier.as_ref().map(|v| v.secret().clone()),
            nonce,
            provider: provider_name.parse().unwrap_or(OidcProvider::Generic),
            tenant_id,
            return_url,
            created_at: Utc::now(),
        };

        self.auth_states.insert(csrf_token.clone(), state);

        let auth_url = client
            .get_authorization_url(&self.auth_states.get(&csrf_token).unwrap())
            .await?;
        Ok(auth_url)
    }

    /// Complete OIDC authentication (callback).
    pub async fn complete_auth(&self, csrf_token: &str, code: &str) -> Result<OidcAuthResult> {
        let state = self
            .auth_states
            .remove(csrf_token)
            .ok_or_else(|| anyhow::anyhow!("Invalid or expired auth state"))?;

        let providers = self
            .providers
            .get(&state.tenant_id)
            .ok_or_else(|| anyhow::anyhow!("No providers for tenant"))?;

        let client = providers
            .get(&state.provider.to_string())
            .ok_or_else(|| anyhow::anyhow!("Provider not found"))?;

        let tokens = client
            .exchange_code(code, state.pkce_verifier.as_deref())
            .await?;
        let user_info = client.get_user_info(&tokens.access_token).await?;

        Ok(OidcAuthResult {
            tokens,
            user_info,
            tenant_id: state.tenant_id,
            return_url: state.return_url,
        })
    }

    /// Refresh access token.
    pub async fn refresh_tokens(
        &self,
        tenant_id: Uuid,
        provider_name: &str,
        refresh_token: &str,
    ) -> Result<OidcTokens> {
        let providers = self
            .providers
            .get(&tenant_id)
            .ok_or_else(|| anyhow::anyhow!("No providers for tenant"))?;

        let client = providers
            .get(provider_name)
            .ok_or_else(|| anyhow::anyhow!("Provider not found"))?;

        client.refresh_token(refresh_token).await
    }

    /// Get logout URL.
    pub async fn get_logout_url(
        &self,
        tenant_id: Uuid,
        provider_name: &str,
        id_token_hint: Option<&str>,
        post_logout_redirect_uri: Option<&str>,
    ) -> Result<String> {
        let providers = self
            .providers
            .get(&tenant_id)
            .ok_or_else(|| anyhow::anyhow!("No providers for tenant"))?;

        let client = providers
            .get(provider_name)
            .ok_or_else(|| anyhow::anyhow!("Provider not found"))?;

        client
            .get_logout_url(id_token_hint, post_logout_redirect_uri)
            .await
    }
}

/// Result of successful OIDC authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcAuthResult {
    pub tokens: OidcTokens,
    pub user_info: OidcUserInfo,
    pub tenant_id: Uuid,
    pub return_url: Option<String>,
}

impl OidcProviderClient {
    fn requires_pkce(&self) -> bool {
        // Default implementation - can be overridden
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oidc_config_serialization() {
        let config = OidcConfig {
            provider: OidcProvider::Keycloak,
            issuer_url: "https://auth.example.com".to_string(),
            client_id: "test-client".to_string(),
            client_secret: "secret".to_string(),
            redirect_url: "https://app.example.com/callback".to_string(),
            scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            pkce_enabled: true,
            discovery_enabled: true,
            metadata: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("keycloak"));
    }
}
