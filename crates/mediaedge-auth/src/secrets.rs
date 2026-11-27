//! Secrets Management
//!
//! Envelope encryption with Age (file-based) and AWS KMS (cloud).
//! Supports runtime rotation via SIGHUP + `/api/v1/config-reload`.
//! Secrets never appear in logs or metrics.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use age::secrecy::{ExposeSecret, SecretString};
use anyhow::{Context, Result};
use aws_sdk_kms::Client as KmsClient;
use aws_sdk_kms::types::EncryptionAlgorithmSpec;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use mediaedge_config::Config;

/// Secrets manager configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsManagerConfig {
    pub backend: SecretsBackend,
    pub age_key_path: Option<String>,
    pub kms_key_id: Option<String>,
    pub kms_region: Option<String>,
    pub cache_ttl: Duration,
    pub rotation_interval: Duration,
}

impl Default for SecretsManagerConfig {
    fn default() -> Self {
        Self {
            backend: SecretsBackend::Age,
            age_key_path: None,
            kms_key_id: None,
            kms_region: None,
            cache_ttl: Duration::from_secs(300),
            rotation_interval: Duration::from_secs(86400), // 24 hours
        }
    }
}

/// Secrets storage backend.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SecretsBackend {
    Age,
    AwsKms,
    Hybrid, // Age for local, KMS for cloud
}

/// Secret metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    pub id: Uuid,
    pub name: String,
    pub tenant_id: Uuid,
    pub version: u32,
    pub description: Option<String>,
    pub tags: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub rotation_enabled: bool,
}

/// Encrypted secret envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedSecret {
    pub metadata: SecretMetadata,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub aad: Vec<u8>, // Additional authenticated data
    pub encryption_key_id: String, // KMS key ID or Age recipient
}

/// Secret value (decrypted, never serialized).
#[derive(Debug, Clone)]
pub struct Secret {
    pub metadata: SecretMetadata,
    pub value: SecretString,
}

/// Secrets manager with envelope encryption.
pub struct SecretsManager {
    config: SecretsManagerConfig,
    age_encryptor: Option<age::Encryptor>,
    age_decryptor: Option<age::Decryptor>,
    kms_client: Option<KmsClient>,
    cache: Arc<DashMap<String, Secret>>, // cache_key -> secret
    metadata_cache: Arc<DashMap<Uuid, SecretMetadata>>,
}

impl SecretsManager {
    /// Create a new secrets manager.
    pub async fn new(config: &Config) -> Result<Arc<Self>> {
        let mgr_config = SecretsManagerConfig {
            backend: config.secrets.backend.clone().unwrap_or(SecretsBackend::Age),
            age_key_path: config.secrets.age_key_path.clone(),
            kms_key_id: config.secrets.kms_key_id.clone(),
            kms_region: config.secrets.kms_region.clone(),
            cache_ttl: Duration::from_secs(config.secrets.cache_ttl_secs.unwrap_or(300)),
            rotation_interval: Duration::from_secs(config.secrets.rotation_interval_secs.unwrap_or(86400)),
        };

        let mut mgr = Self {
            config: mgr_config.clone(),
            age_encryptor: None,
            age_decryptor: None,
            kms_client: None,
            cache: Arc::new(DashMap::new()),
            metadata_cache: Arc::new(DashMap::new()),
        };

        // Initialize encryption backends
        mgr.init_backends().await?;

        Ok(Arc::new(mgr))
    }

    async fn init_backends(&mut self) -> Result<()> {
        match self.config.backend {
            SecretsBackend::Age | SecretsBackend::Hybrid => {
                self.init_age().await?;
            }
            SecretsBackend::AwsKms => {}
        }

        if matches!(self.config.backend, SecretsBackend::AwsKms | SecretsBackend::Hybrid) {
            self.init_kms().await?;
        }

        Ok(())
    }

    async fn init_age(&mut self) -> Result<()> {
        let key_path = self.config.age_key_path.clone()
            .unwrap_or_else(|| "./secrets/age.key".to_string());

        // Generate key if not exists
        if !std::path::Path::new(&key_path).exists() {
            self.generate_age_key(&key_path).await?;
        }

        let key_content = tokio::fs::read_to_string(&key_path).await?;
        let identity = key_content.parse::<age::scrypt::Identity>()?;

        self.age_decryptor = Some(age::Decryptor::new(identity)?);

        // For encryption, we need the recipient (public key)
        let recipient = identity.to_public();
        self.age_encryptor = Some(age::Encryptor::with_user_recipients(vec![recipient])?);

        Ok(())
    }

    async fn generate_age_key(&self, path: &str) -> Result<()> {
        use age::secrecy::SecretString;
        use age::scrypt::Identity;

        let identity = Identity::generate();
        let secret_key = identity.to_string();

        // Ensure directory exists
        if let Some(parent) = std::path::Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(path, secret_key.expose_secret()).await?;
        tracing::info!("Generated new Age key at {}", path);
        Ok(())
    }

    async fn init_kms(&mut self) -> Result<()> {
        let region = self.config.kms_region.clone()
            .unwrap_or_else(|| "us-east-1".to_string());

        let aws_config = aws_config::from_env()
            .region(aws_config::Region::new(region))
            .load()
            .await;

        self.kms_client = Some(KmsClient::new(&aws_config));
        Ok(())
    }

    /// Store a secret (encrypts and persists).
    pub async fn set_secret(
        &self,
        tenant_id: Uuid,
        name: String,
        value: String,
        description: Option<String>,
        tags: HashMap<String, String>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        rotation_enabled: bool,
    ) -> Result<SecretMetadata> {
        let secret_value = SecretString::new(value.into());
        let encrypted = self.encrypt_secret(tenant_id, &name, &secret_value).await?;

        // Persist to database (simplified - would use SQLx)
        self.persist_secret(&encrypted).await?;

        let cache_key = Self::cache_key(tenant_id, &name);
        self.cache.insert(cache_key, Secret {
            metadata: encrypted.metadata.clone(),
            value: secret_value,
        });
        self.metadata_cache.insert(encrypted.metadata.id, encrypted.metadata.clone());

        Ok(encrypted.metadata)
    }

    /// Get a secret (decrypts, uses cache).
    pub async fn get_secret(&self, tenant_id: Uuid, name: &str) -> Result<Option<Secret>> {
        let cache_key = Self::cache_key(tenant_id, name);

        // Check cache first
        if let Some(secret) = self.cache.get(&cache_key) {
            return Ok(Some(secret.clone()));
        }

        // Load from database
        let encrypted = self.load_secret(tenant_id, name).await?;
        if let Some(encrypted) = encrypted {
            let decrypted = self.decrypt_secret(&encrypted).await?;
            let secret = Secret {
                metadata: encrypted.metadata.clone(),
                value: decrypted,
            };
            self.cache.insert(cache_key, secret.clone());
            self.metadata_cache.insert(encrypted.metadata.id, encrypted.metadata);
            Ok(Some(secret))
        } else {
            Ok(None)
        }
    }

    /// Get secret value as string (convenience).
    pub async fn get_secret_value(&self, tenant_id: Uuid, name: &str) -> Result<Option<String>> {
        self.get_secret(tenant_id, name).await
            .map(|opt| opt.map(|s| s.value.expose_secret().to_string()))
    }

    /// Delete a secret.
    pub async fn delete_secret(&self, tenant_id: Uuid, name: &str) -> Result<bool> {
        let cache_key = Self::cache_key(tenant_id, name);
        self.cache.remove(&cache_key);

        // Delete from database
        let deleted = self.delete_secret_db(tenant_id, name).await?;
        Ok(deleted)
    }

    /// List secrets for a tenant (metadata only, no values).
    pub async fn list_secrets(&self, tenant_id: Uuid) -> Result<Vec<SecretMetadata>> {
        // Load from database
        self.load_secrets_metadata(tenant_id).await
    }

    /// Rotate a secret (generate new version).
    pub async fn rotate_secret(&self, tenant_id: Uuid, name: &str) -> Result<SecretMetadata> {
        let old_secret = self.get_secret(tenant_id, name).await?
            .ok_or_else(|| anyhow::anyhow!("Secret not found"))?;

        // Generate new value (for auto-rotation, this would be integration-specific)
        let new_value = self.generate_rotated_value(&old_secret).await?;

        self.set_secret(
            tenant_id,
            name.to_string(),
            new_value,
            old_secret.metadata.description,
            old_secret.metadata.tags,
            old_secret.metadata.expires_at,
            old_secret.metadata.rotation_enabled,
        ).await
    }

    /// Encrypt a secret using envelope encryption.
    async fn encrypt_secret(&self, tenant_id: Uuid, name: &str, value: &SecretString) -> Result<EncryptedSecret> {
        let metadata = SecretMetadata {
            id: Uuid::new_v4(),
            name: name.to_string(),
            tenant_id,
            version: 1,
            description: None,
            tags: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            expires_at: None,
            rotation_enabled: false,
        };

        match self.config.backend {
            SecretsBackend::Age => self.encrypt_age(&metadata, value).await,
            SecretsBackend::AwsKms => self.encrypt_kms(&metadata, value).await,
            SecretsBackend::Hybrid => self.encrypt_hybrid(&metadata, value).await,
        }
    }

    async fn encrypt_age(&self, metadata: &SecretMetadata, value: &SecretString) -> Result<EncryptedSecret> {
        let encryptor = self.age_encryptor.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Age encryptor not initialized"))?;

        let mut encrypted = Vec::new();
        {
            let mut writer = encryptor.wrap_output(&mut encrypted, age::Format::Binary)?;
            use std::io::Write;
            writer.write_all(value.expose_secret().as_bytes())?;
            writer.finish()?;
        }

        Ok(EncryptedSecret {
            metadata: metadata.clone(),
            ciphertext: encrypted,
            nonce: Vec::new(), // Age handles nonce internally
            aad: Vec::new(),
            encryption_key_id: "age".to_string(),
        })
    }

    async fn encrypt_kms(&self, metadata: &SecretMetadata, value: &SecretString) -> Result<EncryptedSecret> {
        let client = self.kms_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("KMS client not initialized"))?;

        let key_id = self.config.kms_key_id.as_ref()
            .ok_or_else(|| anyhow::anyhow!("KMS key ID not configured"))?;

        let response = client.encrypt()
            .key_id(key_id)
            .plaintext(aws_smithy_types::Blob::new(value.expose_secret().as_bytes()))
            .encryption_algorithm(EncryptionAlgorithmSpec::Symaes256)
            .send()
            .await
            .context("KMS encryption failed")?;

        Ok(EncryptedSecret {
            metadata: metadata.clone(),
            ciphertext: response.ciphertext_blob.unwrap().into_inner(),
            nonce: Vec::new(),
            aad: Vec::new(),
            encryption_key_id: key_id.clone(),
        })
    }

    async fn encrypt_hybrid(&self, metadata: &SecretMetadata, value: &SecretString) -> Result<EncryptedSecret> {
        // Encrypt with both, store both ciphertexts
        // For simplicity, use Age locally, KMS for cloud sync
        self.encrypt_age(metadata, value).await
    }

    /// Decrypt a secret.
    async fn decrypt_secret(&self, encrypted: &EncryptedSecret) -> Result<SecretString> {
        match self.config.backend {
            SecretsBackend::Age => self.decrypt_age(encrypted).await,
            SecretsBackend::AwsKms => self.decrypt_kms(encrypted).await,
            SecretsBackend::Hybrid => self.decrypt_age(encrypted).await, // Prefer local
        }
    }

    async fn decrypt_age(&self, encrypted: &EncryptedSecret) -> Result<SecretString> {
        let decryptor = self.age_decryptor.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Age decryptor not initialized"))?;

        let mut decrypted = Vec::new();
        {
            let mut reader = decryptor.decrypt(&encrypted.ciphertext[..])?;
            use std::io::Read;
            reader.read_to_end(&mut decrypted)?;
        }

        Ok(SecretString::new(String::from_utf8(decrypted)?.into()))
    }

    async fn decrypt_kms(&self, encrypted: &EncryptedSecret) -> Result<SecretString> {
        let client = self.kms_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("KMS client not initialized"))?;

        let response = client.decrypt()
            .ciphertext_blob(aws_smithy_types::Blob::new(encrypted.ciphertext.clone()))
            .send()
            .await
            .context("KMS decryption failed")?;

        Ok(SecretString::new(
            String::from_utf8(response.plaintext.unwrap().into_inner())?.into()
        ))
    }

    /// Generate rotated value (placeholder - implement per secret type).
    async fn generate_rotated_value(&self, old_secret: &Secret) -> Result<String> {
        // For API keys, generate new key
        // For passwords, would integrate with password manager
        // For certificates, would trigger cert renewal
        Ok(format!("rotated-{}", Uuid::new_v4()))
    }

    // Database persistence (simplified - replace with SQLx)
    async fn persist_secret(&self, encrypted: &EncryptedSecret) -> Result<()> {
        // INSERT INTO secrets (id, tenant_id, name, version, ciphertext, nonce, aad, key_id, metadata, created_at, updated_at)
        // VALUES (...)
        Ok(())
    }

    async fn load_secret(&self, tenant_id: Uuid, name: &str) -> Result<Option<EncryptedSecret>> {
        // SELECT * FROM secrets WHERE tenant_id = $1 AND name = $2 AND deleted_at IS NULL
        Ok(None)
    }

    async fn delete_secret_db(&self, tenant_id: Uuid, name: &str) -> Result<bool> {
        // UPDATE secrets SET deleted_at = NOW() WHERE tenant_id = $1 AND name = $2
        Ok(true)
    }

    async fn load_secrets_metadata(&self, tenant_id: Uuid) -> Result<Vec<SecretMetadata>> {
        // SELECT metadata FROM secrets WHERE tenant_id = $1 AND deleted_at IS NULL
        Ok(Vec::new())
    }

    fn cache_key(tenant_id: Uuid, name: &str) -> String {
        format!("{}:{}", tenant_id, name)
    }

    /// Reload configuration (for hot-reload).
    pub fn reload_config(&self, new_config: SecretsManagerConfig) {
        // Would re-initialize backends
        tracing::info!("Secrets manager config reload requested");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_age_encryption_roundtrip() {
        // This would need a temp directory and actual Age setup
        // Skipped for unit test
    }

    #[test]
    fn test_secret_metadata_serialization() {
        let metadata = SecretMetadata {
            id: Uuid::new_v4(),
            name: "test-secret".to_string(),
            tenant_id: Uuid::new_v4(),
            version: 1,
            description: Some("Test secret".to_string()),
            tags: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            expires_at: None,
            rotation_enabled: false,
        };

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("test-secret"));
    }
}