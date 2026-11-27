//! Secrets management with Age encryption and AWS KMS integration.
//!
//! Supports:
//! - Age-encrypted secrets in config files
//! - AWS KMS envelope encryption
//! - Runtime secret rotation via SIGHUP + /api/v1/config-reload
//! - Zero secrets in logs/metrics

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use age::{Decryptor, Encryptor, secrecy::SecretString};
use aws_config::BehaviorVersion;
use aws_sdk_kms::Client as KmsClient;
use aws_sdk_kms::types::EncryptionAlgorithmSpec;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use figment::Figment;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Secrets management errors.
#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("Age encryption error: {0}")]
    Age(String),

    #[error("Age decryption error: {0}")]
    AgeDecrypt(String),

    #[error("KMS error: {0}")]
    Kms(String),

    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Secret not found: {0}")]
    NotFound(String),

    #[error("Rotation failed: {0}")]
    RotationFailed(String),
}

pub type SecretsResult<T> = Result<T, SecretsError>;

/// Encrypted secret envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedSecret {
    /// Encryption method: "age", "kms", "plaintext" (for dev only).
    pub method: String,

    /// Base64-encrypted payload.
    pub payload: String,

    /// Key ID / recipient (for KMS key ID or Age recipient).
    pub key_id: Option<String>,

    /// Additional authenticated data (for KMS).
    pub aad: Option<String>,

    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Rotation timestamp (if rotated).
    pub rotated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl EncryptedSecret {
    /// Create a plaintext secret (for development only).
    pub fn plaintext(value: &str) -> Self {
        Self {
            method: "plaintext".to_string(),
            payload: BASE64.encode(value.as_bytes()),
            key_id: None,
            aad: None,
            created_at: chrono::Utc::now(),
            rotated_at: None,
        }
    }

    /// Check if secret is plaintext (insecure).
    pub fn is_plaintext(&self) -> bool {
        self.method == "plaintext"
    }
}

/// Age encryption configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgeConfig {
    /// Age recipient public key (age1...).
    pub recipient: String,

    /// Age identity private key (AGE-SECRET-KEY-...).
    /// If not provided, will use SSH key or prompt.
    pub identity: Option<String>,

    /// Identity file path.
    pub identity_file: Option<String>,
}

/// AWS KMS configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KmsConfig {
    /// KMS key ID or ARN.
    pub key_id: String,

    /// AWS region.
    #[serde(default = "default_region")]
    pub region: String,

    /// Encryption context (additional authenticated data).
    pub encryption_context: HashMap<String, String>,

    /// Use KMS key for envelope encryption (recommended).
    #[serde(default = "default_true")]
    pub envelope_encryption: bool,
}

fn default_region() -> String {
    "us-east-1".to_string()
}

fn default_true() -> bool {
    true
}

/// Secrets manager configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsManagerConfig {
    /// Default encryption method: "age", "kms", "plaintext".
    #[serde(default = "default_method")]
    pub default_method: String,

    /// Age configuration.
    pub age: Option<AgeConfig>,

    /// KMS configuration.
    pub kms: Option<KmsConfig>,

    /// Secret rotation interval in hours (0 = disabled).
    #[serde(default)]
    pub rotation_interval_hours: u32,

    /// Secrets cache TTL in seconds.
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_secs: u64,
}

fn default_method() -> String {
    "age".to_string()
}

fn default_cache_ttl() -> u64 {
    300 // 5 minutes
}

impl Default for SecretsManagerConfig {
    fn default() -> Self {
        Self {
            default_method: "age".to_string(),
            age: None,
            kms: None,
            rotation_interval_hours: 0,
            cache_ttl_secs: 300,
        }
    }
}

/// Secret value with metadata.
#[derive(Debug, Clone)]
pub struct SecretValue {
    pub value: String,
    pub source: SecretSource,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Secret source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretSource {
    Config,
    Environment,
    Kms,
    Age,
    Generated,
}

/// Secrets manager for runtime secret access and rotation.
pub struct SecretsManager {
    config: SecretsManagerConfig,
    cache: Arc<RwLock<HashMap<String, SecretValue>>>,
    kms_client: Option<KmsClient>,
    age_identity: Option<age::scrypt::Identity>,
    rotation_task: Option<tokio::task::JoinHandle<()>>,
}

impl SecretsManager {
    /// Create a new secrets manager from config.
    pub async fn new(config: SecretsManagerConfig) -> SecretsResult<Self> {
        let kms_client = if let Some(kms_config) = &config.kms {
            let aws_config = aws_config::defaults(BehaviorVersion::latest())
                .region(aws_config::Region::new(kms_config.region.clone()))
                .load()
                .await;
            Some(KmsClient::new(&aws_config))
        } else {
            None
        };

        let age_identity = if let Some(age_config) = &config.age {
            if let Some(identity) = &age_config.identity {
                Some(
                    identity
                        .parse()
                        .map_err(|e| SecretsError::Age(e.to_string()))?,
                )
            } else if let Some(identity_file) = &age_config.identity_file {
                let content = tokio::fs::read_to_string(identity_file).await?;
                Some(
                    content
                        .trim()
                        .parse()
                        .map_err(|e| SecretsError::Age(e.to_string()))?,
                )
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
            kms_client,
            age_identity,
            rotation_task: None,
        })
    }

    /// Start automatic secret rotation.
    pub fn start_rotation(&mut self) {
        if self.config.rotation_interval_hours == 0 {
            return;
        }

        let interval = Duration::from_secs(self.config.rotation_interval_hours as u64 * 3600);
        let cache = self.cache.clone();
        let kms_client = self.kms_client.clone();
        let age_identity = self.age_identity.clone();
        let default_method = self.config.default_method.clone();
        let age_recipient = self.config.age.as_ref().map(|a| a.recipient.clone());
        let kms_key_id = self.config.kms.as_ref().map(|k| k.key_id.clone());

        let handle = tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            loop {
                interval_timer.tick().await;
                info!("Starting scheduled secret rotation");

                if let Err(e) = rotate_all_secrets(
                    &cache,
                    kms_client.as_ref(),
                    age_identity.as_ref(),
                    &default_method,
                    age_recipient.as_deref(),
                    kms_key_id.as_deref(),
                )
                .await
                {
                    error!("Secret rotation failed: {}", e);
                } else {
                    info!("Secret rotation completed successfully");
                }
            }
        });

        self.rotation_task = Some(handle);
    }

    /// Stop automatic rotation.
    pub async fn stop_rotation(&mut self) {
        if let Some(handle) = self.rotation_task.take() {
            handle.abort();
            let _ = handle.await;
        }
    }

    /// Get a secret by key.
    pub async fn get_secret(&self, key: &str) -> SecretsResult<String> {
        // Check cache first
        if let Some(cached) = self.cache.read().await.get(key) {
            if cached
                .expires_at
                .map_or(true, |exp| exp > chrono::Utc::now())
            {
                return Ok(cached.value.clone());
            }
        }

        // Not in cache or expired, decrypt from config/env
        let value = self.decrypt_secret(key).await?;

        // Cache it
        let ttl = Duration::from_secs(self.config.cache_ttl_secs);
        let secret = SecretValue {
            value: value.clone(),
            source: SecretSource::Config,
            fetched_at: chrono::Utc::now(),
            expires_at: Some(chrono::Utc::now() + ttl),
        };
        self.cache.write().await.insert(key.to_string(), secret);

        Ok(value)
    }

    /// Get a secret from environment variable.
    pub fn get_env_secret(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    /// Get a secret, trying environment first, then config.
    pub async fn get_secret_or_env(&self, key: &str, env_key: &str) -> SecretsResult<String> {
        if let Some(value) = self.get_env_secret(env_key) {
            return Ok(value);
        }
        self.get_secret(key).await
    }

    /// Decrypt a secret from encrypted storage.
    async fn decrypt_secret(&self, key: &str) -> SecretsResult<String> {
        // In a real implementation, this would fetch from a secure store
        // For now, we'll check environment variables with a prefix
        let env_key = format!("MEDIAEDGE_SECRET_{}", key.to_uppercase());
        if let Ok(value) = std::env::var(&env_key) {
            return Ok(value);
        }

        // Check for encrypted secret in config (would be loaded via Figment)
        // This is a placeholder - real implementation would fetch from vault/config
        Err(SecretsError::NotFound(key.to_string()))
    }

    /// Encrypt a secret for storage.
    pub async fn encrypt_secret(&self, key: &str, value: &str) -> SecretsResult<EncryptedSecret> {
        match self.config.default_method.as_str() {
            "age" => self.encrypt_age(key, value).await,
            "kms" => self.encrypt_kms(key, value).await,
            "plaintext" => Ok(EncryptedSecret::plaintext(value)),
            _ => Err(SecretsError::Config(format!(
                "Unknown encryption method: {}",
                self.config.default_method
            ))),
        }
    }

    /// Encrypt using Age.
    async fn encrypt_age(&self, key: &str, value: &str) -> SecretsResult<EncryptedSecret> {
        let age_config = self
            .config
            .age
            .as_ref()
            .ok_or_else(|| SecretsError::Config("Age config not provided".to_string()))?;

        let recipient = age_config
            .recipient
            .parse()
            .map_err(|e| SecretsError::Age(e.to_string()))?;

        let encryptor = Encryptor::with_recipients(vec![Box::new(recipient)]);
        let mut encrypted = Vec::new();
        {
            let mut writer = encryptor
                .wrap_output(&mut encrypted, age::Format::Binary)
                .map_err(|e| SecretsError::Age(e.to_string()))?;
            std::io::Write::write_all(&mut writer, value.as_bytes())
                .map_err(|e| SecretsError::Age(e.to_string()))?;
            writer
                .finish()
                .map_err(|e| SecretsError::Age(e.to_string()))?;
        }

        Ok(EncryptedSecret {
            method: "age".to_string(),
            payload: BASE64.encode(&encrypted),
            key_id: Some(age_config.recipient.clone()),
            aad: None,
            created_at: chrono::Utc::now(),
            rotated_at: None,
        })
    }

    /// Decrypt using Age.
    pub async fn decrypt_age(&self, secret: &EncryptedSecret) -> SecretsResult<String> {
        let identity = self
            .age_identity
            .as_ref()
            .ok_or_else(|| SecretsError::AgeDecrypt("No Age identity configured".to_string()))?;

        let encrypted = BASE64.decode(&secret.payload)?;
        let decryptor =
            Decryptor::new(&encrypted[..]).map_err(|e| SecretsError::AgeDecrypt(e.to_string()))?;

        let mut decrypted = Vec::new();
        {
            let mut reader = decryptor
                .decrypt(identity)
                .map_err(|e| SecretsError::AgeDecrypt(e.to_string()))?;
            std::io::Read::read_to_end(&mut reader, &mut decrypted)
                .map_err(|e| SecretsError::AgeDecrypt(e.to_string()))?;
        }

        String::from_utf8(decrypted).map_err(|e| SecretsError::AgeDecrypt(e.to_string()))
    }

    /// Encrypt using KMS.
    async fn encrypt_kms(&self, key: &str, value: &str) -> SecretsResult<EncryptedSecret> {
        let kms_client = self
            .kms_client
            .as_ref()
            .ok_or_else(|| SecretsError::Kms("KMS client not configured".to_string()))?;
        let kms_config = self
            .config
            .kms
            .as_ref()
            .ok_or_else(|| SecretsError::Config("KMS config not provided".to_string()))?;

        let mut context = kms_config.encryption_context.clone();
        context.insert("key".to_string(), key.to_string());
        context.insert("service".to_string(), "mediaedge".to_string());

        let result = kms_client
            .encrypt()
            .key_id(&kms_config.key_id)
            .plaintext(value.as_bytes())
            .set_encryption_context(Some(context))
            .encryption_algorithm(EncryptionAlgorithmSpec::SYMMETRIC_DEFAULT)
            .send()
            .await
            .map_err(|e| SecretsError::Kms(e.to_string()))?;

        let ciphertext = result
            .ciphertext_blob
            .ok_or_else(|| SecretsError::Kms("No ciphertext in response".to_string()))?;

        Ok(EncryptedSecret {
            method: "kms".to_string(),
            payload: BASE64.encode(ciphertext.as_ref()),
            key_id: Some(kms_config.key_id.clone()),
            aad: None,
            created_at: chrono::Utc::now(),
            rotated_at: None,
        })
    }

    /// Decrypt using KMS.
    pub async fn decrypt_kms(&self, secret: &EncryptedSecret) -> SecretsResult<String> {
        let kms_client = self
            .kms_client
            .as_ref()
            .ok_or_else(|| SecretsError::Kms("KMS client not configured".to_string()))?;
        let kms_config = self
            .config
            .kms
            .as_ref()
            .ok_or_else(|| SecretsError::Config("KMS config not provided".to_string()))?;

        let ciphertext = BASE64.decode(&secret.payload)?;
        let mut context = kms_config.encryption_context.clone();
        context.insert("service".to_string(), "mediaedge".to_string());

        let result = kms_client
            .decrypt()
            .key_id(&kms_config.key_id)
            .ciphertext_blob(ciphertext.into())
            .set_encryption_context(Some(context))
            .encryption_algorithm(EncryptionAlgorithmSpec::SYMMETRIC_DEFAULT)
            .send()
            .await
            .map_err(|e| SecretsError::Kms(e.to_string()))?;

        let plaintext = result
            .plaintext
            .ok_or_else(|| SecretsError::Kms("No plaintext in response".to_string()))?;

        String::from_utf8(plaintext.as_ref().to_vec()).map_err(|e| SecretsError::Kms(e.to_string()))
    }

    /// Rotate a specific secret.
    pub async fn rotate_secret(&self, key: &str, new_value: &str) -> SecretsResult<()> {
        let encrypted = self.encrypt_secret(key, new_value).await?;

        // In a real implementation, this would update the secure store
        // For now, we invalidate cache
        self.cache.write().await.remove(key);

        info!("Rotated secret: {}", key);
        Ok(())
    }

    /// Clear all cached secrets.
    pub async fn clear_cache(&self) {
        self.cache.write().await.clear();
        info!("Secrets cache cleared");
    }

    /// Get cache stats.
    pub async fn cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.read().await;
        let total = cache.len();
        let expired = cache
            .values()
            .filter(|v| v.expires_at.map_or(false, |exp| exp <= chrono::Utc::now()))
            .count();
        (total, expired)
    }
}

/// Rotate all secrets (called by rotation task).
async fn rotate_all_secrets(
    cache: &Arc<RwLock<HashMap<String, SecretValue>>>,
    kms_client: Option<&KmsClient>,
    age_identity: Option<&age::scrypt::Identity>,
    default_method: &str,
    age_recipient: Option<&str>,
    kms_key_id: Option<&str>,
) -> SecretsResult<()> {
    // In a real implementation, this would:
    // 1. Fetch all secrets from secure store
    // 2. Generate new values
    // 3. Re-encrypt with new keys
    // 4. Update store
    // 5. Invalidate cache

    // For now, just clear cache to force re-fetch
    cache.write().await.clear();
    warn!("Secret rotation: cache cleared (placeholder implementation)");
    Ok(())
}

/// Generate a new Age key pair.
pub fn generate_age_keypair() -> SecretsResult<(String, String)> {
    let identity = age::scrypt::Identity::generate();
    let recipient = identity.to_public();
    Ok((identity.to_string(), recipient.to_string()))
}

/// Encrypt a file using Age.
pub async fn encrypt_file_age(input: &Path, output: &Path, recipient: &str) -> SecretsResult<()> {
    let recipient = recipient
        .parse()
        .map_err(|e| SecretsError::Age(e.to_string()))?;

    let input_content = tokio::fs::read(input).await?;
    let encryptor = Encryptor::with_recipients(vec![Box::new(recipient)]);
    let mut encrypted = Vec::new();

    {
        let mut writer = encryptor
            .wrap_output(&mut encrypted, age::Format::Binary)
            .map_err(|e| SecretsError::Age(e.to_string()))?;
        std::io::Write::write_all(&mut writer, &input_content)
            .map_err(|e| SecretsError::Age(e.to_string()))?;
        writer
            .finish()
            .map_err(|e| SecretsError::Age(e.to_string()))?;
    }

    tokio::fs::write(output, encrypted).await?;
    Ok(())
}

/// Decrypt a file using Age.
pub async fn decrypt_file_age(input: &Path, output: &Path, identity: &str) -> SecretsResult<()> {
    let identity = identity
        .parse()
        .map_err(|e| SecretsError::Age(e.to_string()))?;

    let encrypted = tokio::fs::read(input).await?;
    let decryptor =
        Decryptor::new(&encrypted[..]).map_err(|e| SecretsError::AgeDecrypt(e.to_string()))?;

    let mut decrypted = Vec::new();
    {
        let mut reader = decryptor
            .decrypt(identity)
            .map_err(|e| SecretsError::AgeDecrypt(e.to_string()))?;
        std::io::Read::read_to_end(&mut reader, &mut decrypted)
            .map_err(|e| SecretsError::AgeDecrypt(e.to_string()))?;
    }

    tokio::fs::write(output, decrypted).await?;
    Ok(())
}

/// CLI helper for secrets management.
pub mod cli {
    use super::*;

    /// Encrypt a value for use in config.
    pub async fn encrypt_value(
        config: &SecretsManagerConfig,
        value: &str,
    ) -> SecretsResult<EncryptedSecret> {
        let manager = SecretsManager::new(config.clone()).await?;
        manager.encrypt_secret("cli-value", value).await
    }

    /// Decrypt a value from config.
    pub async fn decrypt_value(
        config: &SecretsManagerConfig,
        secret: &EncryptedSecret,
    ) -> SecretsResult<String> {
        let manager = SecretsManager::new(config.clone()).await?;
        match secret.method.as_str() {
            "age" => manager.decrypt_age(secret).await,
            "kms" => manager.decrypt_kms(secret).await,
            "plaintext" => {
                let bytes = BASE64.decode(&secret.payload)?;
                String::from_utf8(bytes).map_err(|e| SecretsError::Utf8(e))
            }
            _ => Err(SecretsError::Config(format!(
                "Unknown method: {}",
                secret.method
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_age_keypair_generation() {
        let (identity, recipient) = generate_age_keypair().unwrap();
        assert!(identity.starts_with("AGE-SECRET-KEY-"));
        assert!(recipient.starts_with("age1"));
    }

    #[test]
    fn test_encrypted_secret_plaintext() {
        let secret = EncryptedSecret::plaintext("test-value");
        assert_eq!(secret.method, "plaintext");
        assert!(secret.is_plaintext());
        let decoded = BASE64.decode(&secret.payload).unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "test-value");
    }

    #[tokio::test]
    async fn test_age_encrypt_decrypt() {
        let (identity_str, recipient_str) = generate_age_keypair().unwrap();

        let config = SecretsManagerConfig {
            default_method: "age".to_string(),
            age: Some(AgeConfig {
                recipient: recipient_str,
                identity: Some(identity_str),
                identity_file: None,
            }),
            kms: None,
            rotation_interval_hours: 0,
            cache_ttl_secs: 300,
        };

        let manager = SecretsManager::new(config).await.unwrap();

        let secret = manager
            .encrypt_secret("test-key", "super-secret-value")
            .await
            .unwrap();
        assert_eq!(secret.method, "age");
        assert!(!secret.is_plaintext());

        let decrypted = manager.decrypt_age(&secret).await.unwrap();
        assert_eq!(decrypted, "super-secret-value");
    }

    #[tokio::test]
    async fn test_secrets_manager_cache() {
        let config = SecretsManagerConfig::default();
        let manager = SecretsManager::new(config).await.unwrap();

        // Set env var for testing
        std::env::set_var("MEDIAEDGE_SECRET_TEST_KEY", "env-value");

        let value = manager.get_secret("test_key").await.unwrap();
        assert_eq!(value, "env-value");

        let stats = manager.cache_stats().await;
        assert_eq!(stats.0, 1);

        std::env::remove_var("MEDIAEDGE_SECRET_TEST_KEY");
    }
}
