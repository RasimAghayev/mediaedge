//! Token Management (JWT, API Keys)
//!
//! Handles JWT issuance, validation, refresh, and revocation.
//! Supports HS256, RS256, and JWKS for key rotation.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use mediaedge_config::Config;
use crate::tenant::TenantManager;

/// Token manager configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenManagerConfig {
    pub algorithm: TokenAlgorithm,
    pub hs256_secret: Option<String>,
    pub rs256_private_key: Option<String>,
    pub rs256_public_key: Option<String>,
    pub jwks_url: Option<String>,
    pub issuer: String,
    pub audience: String,
    pub access_token_ttl: Duration,
    pub refresh_token_ttl: Duration,
    pub api_key_prefix: String,
    pub api_key_ttl: Duration,
}

impl TokenManagerConfig {
    pub fn from_config(config: &Config) -> Result<Self> {
        Ok(Self {
            algorithm: config.auth.token.algorithm.clone().unwrap_or(TokenAlgorithm::Hs256),
            hs256_secret: config.auth.token.hs256_secret.clone(),
            rs256_private_key: config.auth.token.rs256_private_key.clone(),
            rs256_public_key: config.auth.token.rs256_public_key.clone(),
            jwks_url: config.auth.token.jwks_url.clone(),
            issuer: config.auth.token.issuer.clone().unwrap_or_else(|| "mediaedge".to_string()),
            audience: config.auth.token.audience.clone().unwrap_or_else(|| "mediaedge-api".to_string()),
            access_token_ttl: Duration::from_secs(config.auth.token.access_token_ttl_secs.unwrap_or(900)), // 15 min
            refresh_token_ttl: Duration::from_secs(config.auth.token.refresh_token_ttl_secs.unwrap_or(604800)), // 7 days
            api_key_prefix: config.auth.token.api_key_prefix.clone().unwrap_or_else(|| "me_".to_string()),
            api_key_ttl: Duration::from_secs(config.auth.token.api_key_ttl_secs.unwrap_or(31536000)), // 1 year
        })
    }
}

/// Supported token algorithms.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "uppercase")]
pub enum TokenAlgorithm {
    Hs256,
    Rs256,
    Es256,
}

impl TokenAlgorithm {
    pub fn to_jwt_algorithm(&self) -> Algorithm {
        match self {
            TokenAlgorithm::Hs256 => Algorithm::HS256,
            TokenAlgorithm::Rs256 => Algorithm::RS256,
            TokenAlgorithm::Es256 => Algorithm::ES256,
        }
    }
}

/// JWT claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,           // User ID or API key ID
    pub tenant_id: Uuid,
    pub roles: Vec<String>,    // Role names
    pub permissions: Vec<String>, // Permission strings
    pub iat: i64,              // Issued at
    pub exp: i64,              // Expiration
    pub iss: String,           // Issuer
    pub aud: String,           // Audience
    pub jti: String,           // JWT ID (for revocation)
    pub token_type: TokenType,
    pub scope: Option<String>,
}

/// Token type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    Access,
    Refresh,
    ApiKey,
}

/// Token pair (access + refresh).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub scope: Option<String>,
}

/// API key with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub key_hash: String,
    pub prefix: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub created_by: Uuid,
}

/// Revoked token entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokedToken {
    pub jti: String,
    pub revoked_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub reason: String,
}

/// Token manager.
pub struct TokenManager {
    config: TokenManagerConfig,
    tenant_mgr: Arc<TenantManager>,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
    revoked_tokens: Arc<DashMap<String, RevokedToken>>,
    api_keys: Arc<DashMap<Uuid, ApiKey>>, // key_id -> api_key
    jwks_cache: Arc<DashMap<String, Vec<jsonwebtoken::jwk::Jwk>>>,
}

impl TokenManager {
    /// Create a new token manager.
    pub async fn new(config: &Config, tenant_mgr: Arc<TenantManager>) -> Result<Arc<Self>> {
        let mgr_config = TokenManagerConfig::from_config(config)?;

        let (encoding_key, decoding_key) = Self::load_keys(&mgr_config)?;

        let mut validation = Validation::new(mgr_config.algorithm.to_jwt_algorithm());
        validation.set_issuer(&[mgr_config.issuer.clone()]);
        validation.set_audience(&[mgr_config.audience.clone()]);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.leeway = 60; // 1 minute clock skew

        let mgr = Arc::new(Self {
            config: mgr_config,
            tenant_mgr,
            encoding_key,
            decoding_key,
            validation,
            revoked_tokens: Arc::new(DashMap::new()),
            api_keys: Arc::new(DashMap::new()),
            jwks_cache: Arc::new(DashMap::new()),
        });

        // Load revoked tokens from database
        mgr.load_revoked_tokens().await?;

        // Load API keys
        mgr.load_api_keys().await?;

        // Start JWKS refresh if configured
        if mgr.config.jwks_url.is_some() {
            mgr.start_jwks_refresh();
        }

        Ok(mgr)
    }

    fn load_keys(config: &TokenManagerConfig) -> Result<(EncodingKey, DecodingKey)> {
        match config.algorithm {
            TokenAlgorithm::Hs256 => {
                let secret = config.hs256_secret.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("HS256 secret not configured"))?;
                Ok((
                    EncodingKey::from_secret(secret.as_bytes()),
                    DecodingKey::from_secret(secret.as_bytes()),
                ))
            }
            TokenAlgorithm::Rs256 => {
                let private_key = config.rs256_private_key.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("RS256 private key not configured"))?;
                let public_key = config.rs256_public_key.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("RS256 public key not configured"))?;

                Ok((
                    EncodingKey::from_rsa_pem(private_key.as_bytes())?,
                    DecodingKey::from_rsa_pem(public_key.as_bytes())?,
                ))
            }
            TokenAlgorithm::Es256 => {
                let private_key = config.rs256_private_key.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("ES256 private key not configured"))?;
                let public_key = config.rs256_public_key.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("ES256 public key not configured"))?;

                Ok((
                    EncodingKey::from_ec_pem(private_key.as_bytes())?,
                    DecodingKey::from_ec_pem(public_key.as_bytes())?,
                ))
            }
        }
    }

    /// Issue access and refresh tokens for a user.
    pub async fn issue_user_tokens(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        roles: Vec<String>,
        permissions: Vec<String>,
        scope: Option<String>,
    ) -> Result<TokenPair> {
        let access_jti = Uuid::new_v4().to_string();
        let refresh_jti = Uuid::new_v4().to_string();

        let now = Utc::now();
        let access_exp = now + chrono::Duration::from_std(self.config.access_token_ttl)?;
        let refresh_exp = now + chrono::Duration::from_std(self.config.refresh_token_ttl)?;

        let access_claims = Claims {
            sub: user_id.to_string(),
            tenant_id,
            roles: roles.clone(),
            permissions: permissions.clone(),
            iat: now.timestamp(),
            exp: access_exp.timestamp(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            jti: access_jti,
            token_type: TokenType::Access,
            scope: scope.clone(),
        };

        let refresh_claims = Claims {
            sub: user_id.to_string(),
            tenant_id,
            roles,
            permissions,
            iat: now.timestamp(),
            exp: refresh_exp.timestamp(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            jti: refresh_jti,
            token_type: TokenType::Refresh,
            scope,
        };

        let access_token = encode(&Header::default(), &access_claims, &self.encoding_key)?;
        let refresh_token = encode(&Header::default(), &refresh_claims, &self.encoding_key)?;

        Ok(TokenPair {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.access_token_ttl.as_secs(),
            scope: access_claims.scope,
        })
    }

    /// Issue API key token.
    pub async fn issue_api_key(
        &self,
        tenant_id: Uuid,
        name: String,
        roles: Vec<String>,
        permissions: Vec<String>,
        expires_at: Option<DateTime<Utc>>,
        created_by: Uuid,
    ) -> Result<(ApiKey, String)> {
        let key_id = Uuid::new_v4();
        let raw_key = format!("{}{}", self.config.api_key_prefix, Uuid::new_v4().simple());
        let key_hash = self.hash_api_key(&raw_key);

        let now = Utc::now();
        let exp = expires_at.unwrap_or_else(|| now + chrono::Duration::from_std(self.config.api_key_ttl)?);

        let claims = Claims {
            sub: key_id.to_string(),
            tenant_id,
            roles: roles.clone(),
            permissions: permissions.clone(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            jti: key_id.to_string(),
            token_type: TokenType::ApiKey,
            scope: None,
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)?;

        let api_key = ApiKey {
            id: key_id,
            tenant_id,
            name,
            key_hash,
            prefix: self.config.api_key_prefix.clone(),
            roles,
            permissions,
            expires_at: Some(exp),
            last_used_at: None,
            created_at: now,
            created_by,
        };

        self.api_keys.insert(key_id, api_key.clone());

        // Persist to database
        self.persist_api_key(&api_key).await?;

        Ok((api_key, raw_key))
    }

    /// Validate an access token.
    pub async fn validate_access_token(&self, token: &str) -> Result<TokenData<Claims>> {
        self.validate_token(token, TokenType::Access).await
    }

    /// Validate a refresh token.
    pub async fn validate_refresh_token(&self, token: &str) -> Result<TokenData<Claims>> {
        self.validate_token(token, TokenType::Refresh).await
    }

    /// Validate an API key token.
    pub async fn validate_api_key(&self, token: &str) -> Result<TokenData<Claims>> {
        self.validate_token(token, TokenType::ApiKey).await
    }

    async fn validate_token(&self, token: &str, expected_type: TokenType) -> Result<TokenData<Claims>> {
        // Check revocation
        if let Some(header) = jsonwebtoken::decode_header(token).ok() {
            if let Some(jti) = header.jti {
                if self.revoked_tokens.contains_key(&jti) {
                    return Err(anyhow::anyhow!("Token revoked"));
                }
            }
        }

        let mut validation = self.validation.clone();

        let token_data = decode::<Claims>(token, &self.decoding_key, &validation)
            .context("Token validation failed")?;

        // Verify token type
        if token_data.claims.token_type != expected_type {
            return Err(anyhow::anyhow!("Invalid token type"));
        }

        // Verify tenant exists and is active
        let tenant = self.tenant_mgr.get_tenant(token_data.claims.tenant_id).await?
            .ok_or_else(|| anyhow::anyhow!("Tenant not found"))?;

        if tenant.status != crate::tenant::TenantStatus::Active {
            return Err(anyhow::anyhow!("Tenant not active"));
        }

        Ok(token_data)
    }

    /// Refresh access token using refresh token.
    pub async fn refresh_access_token(&self, refresh_token: &str) -> Result<TokenPair> {
        let token_data = self.validate_refresh_token(refresh_token).await?;

        // Revoke old refresh token
        self.revoke_token(&token_data.claims.jti, "refresh".to_string()).await?;

        // Issue new token pair
        self.issue_user_tokens(
            token_data.claims.tenant_id,
            Uuid::parse_str(&token_data.claims.sub)?,
            token_data.claims.roles,
            token_data.claims.permissions,
            token_data.claims.scope,
        ).await
    }

    /// Revoke a token (add to revocation list).
    pub async fn revoke_token(&self, jti: &str, reason: String) -> Result<()> {
        let revoked = RevokedToken {
            jti: jti.to_string(),
            revoked_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::days(30), // Keep for 30 days
            reason,
        };

        self.revoked_tokens.insert(jti.to_string(), revoked.clone());

        // Persist to database
        self.persist_revoked_token(&revoked).await?;

        Ok(())
    }

    /// Revoke all tokens for a user.
    pub async fn revoke_user_tokens(&self, user_id: Uuid, reason: String) -> Result<()> {
        // In practice, would query database for all active JTIs for this user
        // For now, this is a placeholder
        tracing::info!("Revoking all tokens for user {}", user_id);
        Ok(())
    }

    /// Revoke an API key.
    pub async fn revoke_api_key(&self, key_id: Uuid) -> Result<()> {
        self.api_keys.remove(&key_id);

        // Update database
        sqlx::query!(
            "UPDATE api_keys SET revoked_at = NOW() WHERE id = $1",
            key_id
        )
        .execute(&self.tenant_mgr.config.pool)
        .await?;

        Ok(())
    }

    /// List API keys for a tenant.
    pub async fn list_api_keys(&self, tenant_id: Uuid) -> Result<Vec<ApiKey>> {
        Ok(self.api_keys.iter()
            .filter(|k| k.tenant_id == tenant_id)
            .map(|k| k.clone())
            .collect())
    }

    /// Get JWKS for public key distribution.
    pub async fn get_jwks(&self) -> Result<JsonWebKeySet> {
        let keys = match self.config.algorithm {
            TokenAlgorithm::Hs256 => {
                // HS256 doesn't have public JWKS
                vec![]
            }
            TokenAlgorithm::Rs256 | TokenAlgorithm::Es256 => {
                // Generate JWK from public key
                let public_key_pem = self.config.rs256_public_key.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Public key not configured"))?;

                let jwk = jsonwebtoken::jwk::Jwk::from_rsa_pem(public_key_pem.as_bytes())?;
                vec![jwk]
            }
        };

        Ok(JsonWebKeySet { keys })
    }

    /// Hash an API key for storage.
    fn hash_api_key(&self, key: &str) -> String {
        use ring::digest::{digest, SHA256};
        let hash = digest(&SHA256, key.as_bytes());
        hex::encode(hash.as_ref())
    }

    /// Verify an API key against hash.
    pub fn verify_api_key(&self, key: &str, hash: &str) -> bool {
        self.hash_api_key(key) == hash
    }

    // Database operations
    async fn load_revoked_tokens(&self) -> Result<()> {
        // SELECT jti, revoked_at, expires_at, reason FROM revoked_tokens WHERE expires_at > NOW()
        Ok(())
    }

    async fn persist_revoked_token(&self, revoked: &RevokedToken) -> Result<()> {
        // INSERT INTO revoked_tokens (jti, revoked_at, expires_at, reason) VALUES (...)
        Ok(())
    }

    async fn load_api_keys(&self) -> Result<()> {
        // SELECT * FROM api_keys WHERE revoked_at IS NULL AND (expires_at IS NULL OR expires_at > NOW())
        Ok(())
    }

    async fn persist_api_key(&self, api_key: &ApiKey) -> Result<()> {
        // INSERT INTO api_keys (...)
        Ok(())
    }

    fn start_jwks_refresh(&self) {
        if let Some(jwks_url) = self.config.jwks_url.clone() {
            let cache = self.jwks_cache.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(3600)); // 1 hour
                loop {
                    interval.tick().await;
                    if let Ok(response) = reqwest::get(&jwks_url).await {
                        if let Ok(jwks) = response.json::<JsonWebKeySet>().await {
                            cache.insert("default".to_string(), jwks.keys);
                        }
                    }
                }
            });
        }
    }
}

/// JWKS response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonWebKeySet {
    pub keys: Vec<jsonwebtoken::jwk::Jwk>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_algorithm_conversion() {
        assert_eq!(TokenAlgorithm::Hs256.to_jwt_algorithm(), Algorithm::HS256);
        assert_eq!(TokenAlgorithm::Rs256.to_jwt_algorithm(), Algorithm::RS256);
    }

    #[test]
    fn test_claims_serialization() {
        let claims = Claims {
            sub: "user-123".to_string(),
            tenant_id: Uuid::new_v4(),
            roles: vec!["admin".to_string()],
            permissions: vec!["stream.create".to_string()],
            iat: chrono::Utc::now().timestamp(),
            exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp(),
            iss: "mediaedge".to_string(),
            aud: "mediaedge-api".to_string(),
            jti: Uuid::new_v4().to_string(),
            token_type: TokenType::Access,
            scope: Some("read write".to_string()),
        };

        let json = serde_json::to_string(&claims).unwrap();
        assert!(json.contains("user-123"));
        assert!(json.contains("access"));
    }
}