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

pub mod types;
pub mod loader;
pub mod secrets;
pub mod hot_reload;

// Re-export commonly used types
pub use types::{
    ServiceConfig, ServerConfig, TlsConfig, AuthConfig, AuthMode, JwtConfig,
    StaticTokenConfig, WebhookAuthConfig, MetricsConfig, OtlpConfig, TracingConfig,
    JaegerConfig, RateLimitConfig, RedisConfig, RedisTlsConfig, EtcdConfig,
    EtcdTlsConfig, NatsConfig, NatsTlsConfig, NatsReconnectConfig, StorageConfig,
    FsStorageConfig, S3StorageConfig, S3TlsConfig, AzblobStorageConfig,
    GcsStorageConfig, MemoryStorageConfig, GatewayConfig, UpstreamConfig,
    GatewayProtocolsConfig, RtmpConfig, SrtConfig, RistConfig, WhipConfig,
    TurnServerConfig, HttpProtocolConfig, AdminConfig, StorageNodeConfig,
    StorageReplicationConfig, ErasureConfig, ArchiveConfig, TieringConfig,
    TierConfig, CacheConfig, EdgeNodeConfig, OriginConfig, EdgeCacheConfig,
    MeshConfig, IceServerConfig, SignedUrlConfig, ReplicationCoordinatorConfig,
    PlannerConfig, FecConfig, MeshReplicationConfig, MediaProcessorConfig,
    TranscodingConfig, BitratePreset, RecordingConfig, RecordingSchedule,
    VodConfig, ThumbnailConfig, SpriteConfig, CaptionConfig, AiConfig,
    ModerationConfig, HighlightConfig, AnomalyConfig,
    FederationConfig, FederationLinkConfig,
};

pub use loader::{
    ConfigLoader, LoadedConfig, ConfigMeta, ConfigSubscription, ConfigError,
    ConfigResult, load_service_config, generate_json_schema, generate_openapi_schema,
    validate_config_file, merge_config_files,
};

pub use secrets::{
    SecretsManager, SecretsManagerConfig, EncryptedSecret, SecretValue, SecretSource,
    AgeConfig, KmsConfig, SecretsError, SecretsResult,
    generate_age_keypair, encrypt_file_age, decrypt_file_age,
    cli::encrypt_value, cli::decrypt_value,
};

pub use hot_reload::{
    HotReloadState, hot_reload_routes, spawn_sighup_handler, spawn_config_validator,
    graceful_shutdown, spawn_file_watcher,
};

/// Prelude for convenient imports.
pub mod prelude {
    pub use crate::{
        ConfigLoader, ConfigError, ConfigResult, LoadedConfig, ConfigMeta,
        ServiceConfig, ServerConfig, AuthConfig, MetricsConfig, TracingConfig,
        SecretsManager, SecretsManagerConfig, EncryptedSecret, SecretsError, SecretsResult,
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