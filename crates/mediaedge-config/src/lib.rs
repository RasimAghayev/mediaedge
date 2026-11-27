//! Configuration types, validation, and secrets management.
//!
//! Layered configuration (file, env, CLI) via figment. Called by all
//! services at startup (see `services/gateway/src/main.rs`).
//!
//! Features:
//! - Type-safe config structs for all 5 services
//! - Figment-based layered config (defaults → TOML → env → CLI)
//! - Validator-based validation with custom rules
//! - Age encryption + AWS KMS for secrets
//! - Atomic hot-reload via ArcSwap (single-digit-ns reads)
//! - Config subscription for reactive components
//! - JSON Schema / OpenAPI generation

#![warn(missing_docs)]

pub mod hot_reload;
pub mod loader;
pub mod secrets;
pub mod types;

// Re-export commonly used types
pub use types::{
    AdminConfig, AiConfig, AnomalyConfig, ArchiveConfig, AuthConfig, AuthMode, AzblobStorageConfig,
    BitratePreset, CacheConfig, CaptionConfig, EdgeCacheConfig, EdgeNodeConfig, ErasureConfig,
    EtcdConfig, EtcdTlsConfig, FecConfig, FederationConfig, FederationLinkConfig, FsStorageConfig,
    GatewayConfig, GatewayProtocolsConfig, GcsStorageConfig, HighlightConfig, HttpProtocolConfig,
    IceServerConfig, JaegerConfig, JwtConfig, MediaProcessorConfig, MemoryStorageConfig,
    MeshConfig, MeshReplicationConfig, MetricsConfig, ModerationConfig, NatsConfig,
    NatsReconnectConfig, NatsTlsConfig, OriginConfig, OtlpConfig, PlannerConfig, RateLimitConfig,
    RecordingConfig, RecordingSchedule, RedisConfig, RedisTlsConfig, ReplicationCoordinatorConfig,
    RistConfig, RtmpConfig, S3StorageConfig, S3TlsConfig, ServerConfig, ServiceConfig,
    SignedUrlConfig, SpriteConfig, SrtConfig, StaticTokenConfig, StorageConfig, StorageNodeConfig,
    StorageReplicationConfig, ThumbnailConfig, TierConfig, TieringConfig, TlsConfig, TracingConfig,
    TranscodingConfig, TurnServerConfig, UpstreamConfig, VodConfig, WebhookAuthConfig, WhipConfig,
};

pub use loader::{
    ConfigError, ConfigLoader, ConfigMeta, ConfigResult, ConfigSubscription, LoadedConfig,
    generate_json_schema, generate_openapi_schema, load_service_config, merge_config_files,
    validate_config_file,
};

pub use secrets::{
    AgeConfig, EncryptedSecret, KmsConfig, SecretSource, SecretValue, SecretsError, SecretsManager,
    SecretsManagerConfig, SecretsResult, cli::decrypt_value, cli::encrypt_value, decrypt_file_age,
    encrypt_file_age, generate_age_keypair,
};

pub use hot_reload::{
    HotReloadState, graceful_shutdown, hot_reload_routes, spawn_config_validator,
    spawn_file_watcher, spawn_sighup_handler,
};

/// Prelude for convenient imports.
pub mod prelude {
    pub use crate::{
        AuthConfig, ConfigError, ConfigLoader, ConfigMeta, ConfigResult, EncryptedSecret,
        LoadedConfig, MetricsConfig, SecretsError, SecretsManager, SecretsManagerConfig,
        SecretsResult, ServerConfig, ServiceConfig, TracingConfig,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prelude_imports() {
        // Just verify prelude compiles
        let _ = ConfigLoader::<ServiceConfig>::from_file("nonexistent.toml");
    }
}
