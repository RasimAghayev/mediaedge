//! Configuration loader using Figment for layered configuration.
//!
//! Layers (highest priority last):
//! 1. Default values (from struct defaults)
//! 2. TOML config file (--config)
//! 3. Environment variables (MEDIAEDGE_*)
//! 4. CLI arguments

use std::path::Path;
use std::sync::Arc;

use arc_swap::ArcSwap;
use figment::value::{Dict, Map};
use figment::{Figment, providers::Env, providers::Format, providers::Serialized, providers::Toml};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{info, warn};
use validator::Validate;

use crate::types::ServiceConfig;

/// Configuration loader errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Figment error: {0}")]
    Figment(#[from] figment::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid config: {0}")]
    Invalid(String),

    #[error("Hot reload failed: {0}")]
    HotReload(String),
}

pub type ConfigResult<T> = Result<T, ConfigError>;

/// Configuration metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMeta {
    /// Config version (increments on each reload).
    pub version: u64,

    /// Source file path.
    pub source: Option<String>,

    /// Timestamp of load.
    pub loaded_at: chrono::DateTime<chrono::Utc>,

    /// Service name.
    pub service: String,
}

/// Loaded configuration with metadata.
#[derive(Debug, Clone)]
pub struct LoadedConfig<C> {
    pub config: C,
    pub meta: ConfigMeta,
}

/// Configuration loader with hot-reload support.
pub struct ConfigLoader<C> {
    /// Current config (atomic swap for hot reload).
    current: Arc<ArcSwap<LoadedConfig<C>>>,

    /// Figment builder for reloading.
    figment: Figment,

    /// Config file path (for reload).
    config_path: Option<String>,
}

impl<C> ConfigLoader<C>
where
    C: DeserializeOwned + Validate + Clone + Send + Sync + 'static,
{
    /// Create a new config loader from a Figment.
    pub fn new(figment: Figment) -> ConfigResult<Self> {
        let config: C = figment.clone().extract()?;
        config
            .validate()
            .map_err(|e| ConfigError::Validation(e.to_string()))?;

        let meta = ConfigMeta {
            version: 1,
            source: None,
            loaded_at: chrono::Utc::now(),
            service: std::any::type_name::<C>().to_string(),
        };

        let loaded = LoadedConfig { config, meta };
        let current = Arc::new(ArcSwap::from_pointee(loaded));

        Ok(Self {
            current,
            figment,
            config_path: None,
        })
    }

    /// Create a new config loader from a TOML file path.
    pub fn from_file<P: AsRef<Path>>(path: P) -> ConfigResult<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(ConfigError::FileNotFound(path.display().to_string()));
        }

        let figment = Figment::new()
            .merge(Serialized::defaults(C::default()))
            .merge(Toml::file(path))
            .merge(Env::prefixed("MEDIAEDGE_").split("_"))
            .select("config");

        let mut loader = Self::new(figment)?;
        loader.config_path = Some(path.display().to_string());
        loader.current.rcu(|loaded| LoadedConfig {
            config: loaded.config.clone(),
            meta: ConfigMeta {
                version: loaded.meta.version,
                source: loader.config_path.clone(),
                loaded_at: chrono::Utc::now(),
                service: loaded.meta.service.clone(),
            },
        });

        Ok(loader)
    }

    /// Create a new config loader with CLI overrides.
    pub fn with_cli<P: AsRef<Path>>(path: P, cli_overrides: Dict) -> ConfigResult<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(ConfigError::FileNotFound(path.display().to_string()));
        }

        let figment = Figment::new()
            .merge(Serialized::defaults(C::default()))
            .merge(Toml::file(path))
            .merge(Env::prefixed("MEDIAEDGE_").split("_"))
            .merge(Serialized::default(cli_overrides))
            .select("config");

        let mut loader = Self::new(figment)?;
        loader.config_path = Some(path.display().to_string());
        Ok(loader)
    }

    /// Get the current configuration (read-only, lock-free).
    pub fn get(&self) -> Arc<LoadedConfig<C>> {
        self.current.load_full()
    }

    /// Get just the config (convenience).
    pub fn config(&self) -> C {
        self.get().config.clone()
    }

    /// Get config metadata.
    pub fn meta(&self) -> ConfigMeta {
        self.get().meta.clone()
    }

    /// Hot reload configuration from source.
    /// Returns true if config changed.
    pub fn reload(&self) -> ConfigResult<bool> {
        let path = self
            .config_path
            .as_ref()
            .ok_or_else(|| ConfigError::HotReload("No config file path for reload".to_string()))?;

        info!("Reloading configuration from {}", path);

        let new_figment = Figment::new()
            .merge(Serialized::defaults(C::default()))
            .merge(Toml::file(path))
            .merge(Env::prefixed("MEDIAEDGE_").split("_"))
            .select("config");

        let new_config: C = new_figment.extract()?;
        new_config
            .validate()
            .map_err(|e| ConfigError::Validation(e.to_string()))?;

        let old_version = self.current.load().meta.version;
        let new_version = old_version + 1;

        let new_loaded = LoadedConfig {
            config: new_config,
            meta: ConfigMeta {
                version: new_version,
                source: self.config_path.clone(),
                loaded_at: chrono::Utc::now(),
                service: self.current.load().meta.service.clone(),
            },
        };

        self.current.store(Arc::new(new_loaded));

        info!(
            "Configuration reloaded successfully (version {})",
            new_version
        );
        Ok(true)
    }

    /// Reload with new CLI overrides (for runtime config changes).
    pub fn reload_with_cli(&self, cli_overrides: Dict) -> ConfigResult<bool> {
        let path = self
            .config_path
            .as_ref()
            .ok_or_else(|| ConfigError::HotReload("No config file path for reload".to_string()))?;

        info!("Reloading configuration with CLI overrides");

        let new_figment = Figment::new()
            .merge(Serialized::defaults(C::default()))
            .merge(Toml::file(path))
            .merge(Env::prefixed("MEDIAEDGE_").split("_"))
            .merge(Serialized::default(cli_overrides))
            .select("config");

        let new_config: C = new_figment.extract()?;
        new_config
            .validate()
            .map_err(|e| ConfigError::Validation(e.to_string()))?;

        let old_version = self.current.load().meta.version;
        let new_version = old_version + 1;

        let new_loaded = LoadedConfig {
            config: new_config,
            meta: ConfigMeta {
                version: new_version,
                source: self.config_path.clone(),
                loaded_at: chrono::Utc::now(),
                service: self.current.load().meta.service.clone(),
            },
        };

        self.current.store(Arc::new(new_loaded));

        info!(
            "Configuration reloaded with CLI overrides (version {})",
            new_version
        );
        Ok(true)
    }

    /// Subscribe to config changes (for components that need to react).
    pub fn subscribe(&self) -> ConfigSubscription<C> {
        ConfigSubscription {
            current: self.current.clone(),
            last_version: self.current.load().meta.version,
        }
    }
}

/// Subscription to config changes.
pub struct ConfigSubscription<C> {
    current: Arc<ArcSwap<LoadedConfig<C>>>,
    last_version: u64,
}

impl<C> ConfigSubscription<C>
where
    C: Clone + Send + Sync + 'static,
{
    /// Check if config has changed and return new config if so.
    pub fn check(&mut self) -> Option<C> {
        let current = self.current.load();
        if current.meta.version != self.last_version {
            self.last_version = current.meta.version;
            Some(current.config.clone())
        } else {
            None
        }
    }

    /// Get current config regardless of changes.
    pub fn current(&self) -> C {
        self.current.load().config.clone()
    }
}

/// Load service configuration from file.
/// This is the main entry point for services.
pub fn load_service_config<P: AsRef<Path>>(path: P) -> ConfigResult<ConfigLoader<ServiceConfig>> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(ConfigError::FileNotFound(path.display().to_string()));
    }

    let figment = Figment::new()
        .merge(Serialized::defaults(ServiceConfig::Gateway(
            crate::types::GatewayConfig::default(),
        )))
        .merge(Toml::file(path))
        .merge(Env::prefixed("MEDIAEDGE_").split("_"))
        .select("config");

    ConfigLoader::new(figment)
}

/// Generate JSON schema for a config type.
pub fn generate_json_schema<C: Serialize + schemars::JsonSchema>() -> ConfigResult<String> {
    let schema = schemars::schema_for!(C);
    serde_json::to_string_pretty(&schema).map_err(|e| ConfigError::Invalid(e.to_string()))
}

/// Generate OpenAPI schema component for a config type.
pub fn generate_openapi_schema<C: Serialize + schemars::JsonSchema>()
-> ConfigResult<utoipa::openapi::Schema> {
    let schema = schemars::schema_for!(C);
    utoipa::openapi::schema::try_from(schema).map_err(|e| ConfigError::Invalid(e.to_string()))
}

/// Validate a TOML config file without loading.
pub fn validate_config_file<P: AsRef<Path>>(path: P) -> ConfigResult<()> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(ConfigError::FileNotFound(path.display().to_string()));
    }

    let figment = Figment::new()
        .merge(Toml::file(path))
        .merge(Env::prefixed("MEDIAEDGE_").split("_"))
        .select("config");

    let config: ServiceConfig = figment.extract()?;
    config
        .validate()
        .map_err(|e| ConfigError::Validation(e.to_string()))?;

    Ok(())
}

/// Merge multiple config files (later files override earlier).
pub fn merge_config_files<P: AsRef<Path>>(paths: &[P]) -> ConfigResult<Figment> {
    if paths.is_empty() {
        return Err(ConfigError::Invalid("No config files provided".to_string()));
    }

    let mut figment = Figment::new();
    for path in paths {
        let path = path.as_ref();
        if !path.exists() {
            warn!("Config file not found: {}", path.display());
            continue;
        }
        figment = figment.merge(Toml::file(path));
    }

    figment = figment
        .merge(Env::prefixed("MEDIAEDGE_").split("_"))
        .select("config");

    Ok(figment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_config() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        let config = r#"
service = "gateway"

[server]
host = "0.0.0.0"
port = 8080
request_timeout_secs = 30
max_body_size = 10485760
http2 = true

[auth]
default_mode = "jwt"

[auth.jwt]
algorithm = "HS256"
secret = "test-secret"
issuer = "mediaedge"
audience = "api"
clock_skew_secs = 60

[metrics]
enabled = true
path = "/metrics"

[metrics.otlp]
enabled = false

[tracing]
enabled = true
level = "info"
format = "json"

[rate_limit]
enabled = true
requests_per_window = 1000
window_secs = 60
key_prefix = "rl:"

[protocols.rtmp]
enabled = true
port = 1935
chunk_size = 4096
ping_interval_secs = 30
max_stream_key_len = 128

[admin]
enabled = true
path = "/api/v1/admin"
"#;
        file.write_all(config.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_load_gateway_config() {
        let file = create_test_config();
        let loader = ConfigLoader::<GatewayConfig>::from_file(file.path()).unwrap();
        let config = loader.config();

        assert_eq!(config.server.port, 8080);
        assert_eq!(config.auth.default_mode, AuthMode::Jwt);
        assert!(config.auth.jwt.is_some());
        assert_eq!(config.auth.jwt.as_ref().unwrap().algorithm, "HS256");
        assert!(config.protocols.rtmp.is_some());
        assert_eq!(config.protocols.rtmp.as_ref().unwrap().port, 1935);
    }

    #[test]
    fn test_hot_reload() {
        let mut file = create_test_config();
        let loader = ConfigLoader::<GatewayConfig>::from_file(file.path()).unwrap();
        let initial_version = loader.meta().version;
        assert_eq!(initial_version, 1);

        // Modify config file
        let new_config = r#"
service = "gateway"

[server]
host = "0.0.0.0"
port = 9090
request_timeout_secs = 30
max_body_size = 10485760
http2 = true

[auth]
default_mode = "jwt"

[auth.jwt]
algorithm = "HS256"
secret = "new-secret"
issuer = "mediaedge"
audience = "api"
clock_skew_secs = 60

[metrics]
enabled = true
path = "/metrics"

[metrics.otlp]
enabled = false

[tracing]
enabled = true
level = "info"
format = "json"

[rate_limit]
enabled = true
requests_per_window = 1000
window_secs = 60
key_prefix = "rl:"

[protocols.rtmp]
enabled = true
port = 1935
chunk_size = 4096
ping_interval_secs = 30
max_stream_key_len = 128

[admin]
enabled = true
path = "/api/v1/admin"
"#;
        std::fs::write(file.path(), new_config).unwrap();

        // Reload
        let changed = loader.reload().unwrap();
        assert!(changed);
        assert_eq!(loader.meta().version, 2);
        assert_eq!(loader.config().server.port, 9090);
        assert_eq!(
            loader.config().auth.jwt.as_ref().unwrap().secret,
            "new-secret"
        );
    }

    #[test]
    fn test_subscription() {
        let file = create_test_config();
        let loader = ConfigLoader::<GatewayConfig>::from_file(file.path()).unwrap();
        let mut sub = loader.subscribe();

        // No change initially
        assert!(sub.check().is_none());

        // Modify and reload
        let new_config = r#"
service = "gateway"

[server]
host = "0.0.0.0"
port = 9090
request_timeout_secs = 30
max_body_size = 10485760
http2 = true

[auth]
default_mode = "jwt"

[auth.jwt]
algorithm = "HS256"
secret = "new-secret"
issuer = "mediaedge"
audience = "api"
clock_skew_secs = 60

[metrics]
enabled = true
path = "/metrics"

[metrics.otlp]
enabled = false

[tracing]
enabled = true
level = "info"
format = "json"

[rate_limit]
enabled = true
requests_per_window = 1000
window_secs = 60
key_prefix = "rl:"

[protocols.rtmp]
enabled = true
port = 1935
chunk_size = 4096
ping_interval_secs = 30
max_stream_key_len = 128

[admin]
enabled = true
path = "/api/v1/admin"
"#;
        std::fs::write(file.path(), new_config).unwrap();
        loader.reload().unwrap();

        // Subscription should detect change
        let new_config = sub.check().unwrap();
        assert_eq!(new_config.server.port, 9090);
    }

    #[test]
    fn test_validation_error() {
        let mut file = NamedTempFile::new().unwrap();
        let config = r#"
service = "gateway"

[server]
host = "0.0.0.0"
port = 99999  # Invalid port > 65535
"#;
        file.write_all(config.as_bytes()).unwrap();
        file.flush().unwrap();

        let result = ConfigLoader::<GatewayConfig>::from_file(file.path());
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::Validation(msg) => assert!(msg.contains("port")),
            _ => panic!("Expected validation error"),
        }
    }

    #[test]
    fn test_env_override() {
        let mut file = create_test_config();

        // Set env var
        std::env::set_var("MEDIAEDGE_SERVER__PORT", "7070");

        let loader = ConfigLoader::<GatewayConfig>::from_file(file.path()).unwrap();
        assert_eq!(loader.config().server.port, 7070);

        std::env::remove_var("MEDIAEDGE_SERVER__PORT");
    }

    #[test]
    fn test_service_config_discriminant() {
        let gateway = ServiceConfig::Gateway(GatewayConfig::default());
        assert_eq!(gateway.service_name(), "gateway");

        let storage = ServiceConfig::StorageNode(StorageNodeConfig::default());
        assert_eq!(storage.service_name(), "storage-node");

        let edge = ServiceConfig::EdgeNode(EdgeNodeConfig::default());
        assert_eq!(edge.service_name(), "edge-node");

        let replication =
            ServiceConfig::ReplicationCoordinator(ReplicationCoordinatorConfig::default());
        assert_eq!(replication.service_name(), "replication-coordinator");

        let processor = ServiceConfig::MediaProcessor(MediaProcessorConfig::default());
        assert_eq!(processor.service_name(), "media-processor");
    }
}
