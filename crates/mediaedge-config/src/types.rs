//! Configuration types for all MediaEdge services.
//!
//! Uses figment for layered configuration (file → env → CLI).
//! All structs implement `Deserialize` and `Validate`.

use serde::{Deserialize, Serialize};
use validator::Validate;

/// Top-level configuration for a specific service.
/// The `service` field determines which nested config is used.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
#[serde(tag = "service")]
pub enum ServiceConfig {
    #[serde(rename = "gateway")]
    Gateway(GatewayConfig),
    #[serde(rename = "storage-node")]
    StorageNode(StorageNodeConfig),
    #[serde(rename = "edge-node")]
    EdgeNode(EdgeNodeConfig),
    #[serde(rename = "replication-coordinator")]
    ReplicationCoordinator(ReplicationCoordinatorConfig),
    #[serde(rename = "media-processor")]
    MediaProcessor(MediaProcessorConfig),
}

impl ServiceConfig {
    /// Returns the service name as a string.
    pub fn service_name(&self) -> &'static str {
        match self {
            ServiceConfig::Gateway(_) => "gateway",
            ServiceConfig::StorageNode(_) => "storage-node",
            ServiceConfig::EdgeNode(_) => "edge-node",
            ServiceConfig::ReplicationCoordinator(_) => "replication-coordinator",
            ServiceConfig::MediaProcessor(_) => "media-processor",
        }
    }
}

/// Common server configuration shared across services.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct ServerConfig {
    /// Host to bind to.
    #[validate(length(min = 1))]
    pub host: String,

    /// Port to listen on.
    #[validate(range(min = 1, max = 65535))]
    pub port: u16,

    /// TLS configuration (optional).
    pub tls: Option<TlsConfig>,

    /// Request timeout in seconds.
    #[validate(range(min = 1))]
    pub request_timeout_secs: u64,

    /// Maximum request body size in bytes.
    #[validate(range(min = 1024))]
    pub max_body_size: usize,

    /// Enable HTTP/2.
    pub http2: bool,

    /// Cluster port for gossip.
    #[validate(range(min = 1, max = 65535))]
    pub cluster_port: u16,

    /// Bind address for cluster communication.
    pub bind_address: String,

    /// Node ID (auto-generated if not provided).
    pub node_id: Option<String>,

    /// Mesh configuration (WebRTC peer-to-peer).
    pub mesh: MeshConfig,

    /// Federation configuration for cross-cluster replication.
    pub federation: Option<FederationConfig>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            tls: None,
            request_timeout_secs: 30,
            max_body_size: 10 * 1024 * 1024, // 10 MB
            http2: true,
            cluster_port: 7946,
            bind_address: "0.0.0.0".to_string(),
            node_id: None,
            mesh: MeshConfig::default(),
            federation: None,
        }
    }
}

/// TLS configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct TlsConfig {
    /// Path to certificate file (PEM).
    #[validate(length(min = 1))]
    pub cert_file: String,

    /// Path to private key file (PEM).
    #[validate(length(min = 1))]
    pub key_file: String,

    /// Path to CA file for client cert verification (optional).
    pub ca_file: Option<String>,

    /// Require client certificates.
    pub require_client_cert: bool,

    /// ALPN protocols.
    pub alpn_protocols: Vec<String>,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert_file: "".to_string(),
            key_file: "".to_string(),
            ca_file: None,
            require_client_cert: false,
            alpn_protocols: vec!["h2".to_string(), "http/1.1".to_string()],
        }
    }
}

/// Authentication configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct AuthConfig {
    /// JWT configuration.
    pub jwt: Option<JwtConfig>,

    /// Static token configuration (for simple deployments).
    pub static_tokens: Option<StaticTokenConfig>,

    /// Webhook authentication (for external auth services).
    pub webhook: Option<WebhookAuthConfig>,

    /// Default auth mode if multiple configured.
    #[serde(default = "default_auth_mode")]
    pub default_mode: AuthMode,
}

fn default_auth_mode() -> AuthMode {
    AuthMode::Jwt
}

/// Authentication mode.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    Jwt,
    Static,
    Webhook,
    None,
}

impl Default for AuthMode {
    fn default() -> Self {
        AuthMode::None
    }
}

/// JWT configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct JwtConfig {
    /// Algorithm: HS256, RS256, ES256.
    #[serde(default = "default_jwt_algorithm")]
    pub algorithm: String,

    /// HS256 secret (for symmetric keys).
    pub secret: Option<String>,

    /// JWKS endpoint URL (for asymmetric keys).
    pub jwks_url: Option<String>,

    /// JWKS cache TTL in seconds.
    #[serde(default = "default_jwks_cache_ttl")]
    pub jwks_cache_ttl_secs: u64,

    /// Issuer validation.
    pub issuer: Option<String>,

    /// Audience validation.
    pub audience: Option<String>,

    /// Clock skew tolerance in seconds.
    #[serde(default = "default_clock_skew")]
    pub clock_skew_secs: u64,
}

fn default_jwt_algorithm() -> String {
    "HS256".to_string()
}

fn default_jwks_cache_ttl() -> u64 {
    300 // 5 minutes
}

fn default_clock_skew() -> u64 {
    60 // 1 minute
}

/// Static token configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct StaticTokenConfig {
    /// Map of token -> permissions (space-separated scopes).
    pub tokens: std::collections::HashMap<String, String>,
}

/// Webhook authentication configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct WebhookAuthConfig {
    /// Webhook URL for token validation.
    #[validate(url)]
    pub url: String,

    /// Timeout for webhook calls in seconds.
    #[serde(default = "default_webhook_timeout")]
    pub timeout_secs: u64,

    /// Headers to include in webhook request.
    pub headers: std::collections::HashMap<String, String>,

    /// Cache TTL for successful validations in seconds.
    #[serde(default = "default_webhook_cache_ttl")]
    pub cache_ttl_secs: u64,
}

fn default_webhook_timeout() -> u64 {
    5
}

fn default_webhook_cache_ttl() -> u64 {
    60
}

/// Metrics configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct MetricsConfig {
    /// Enable Prometheus metrics endpoint.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Path for metrics endpoint.
    #[serde(default = "default_metrics_path")]
    pub path: String,

    /// Port for metrics (if different from main server).
    pub port: Option<u16>,

    /// Enable OTLP export.
    #[serde(default)]
    pub otlp: OtlpConfig,
}

fn default_true() -> bool {
    true
}

fn default_metrics_path() -> String {
    "/metrics".to_string()
}

/// OTLP export configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct OtlpConfig {
    /// Enable OTLP export.
    #[serde(default)]
    pub enabled: bool,

    /// OTLP endpoint (gRPC).
    #[validate(url)]
    pub endpoint: Option<String>,

    /// Service name for traces/metrics.
    #[serde(default = "default_service_name")]
    pub service_name: String,

    /// Trace sample ratio (0.0 - 1.0).
    #[serde(default = "default_trace_sample_ratio")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub trace_sample_ratio: f64,

    /// Metric export interval in seconds.
    #[serde(default = "default_metric_interval")]
    pub metric_interval_secs: u64,

    /// Headers for OTLP endpoint.
    pub headers: std::collections::HashMap<String, String>,
}

fn default_service_name() -> String {
    "mediaedge".to_string()
}

fn default_trace_sample_ratio() -> f64 {
    0.1
}

fn default_metric_interval() -> u64 {
    60
}

/// Tracing configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct TracingConfig {
    /// Enable tracing.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Log level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Output format: json, pretty, compact.
    #[serde(default = "default_log_format")]
    pub format: String,

    /// Jaeger configuration.
    pub jaeger: Option<JaegerConfig>,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "json".to_string()
}

/// Jaeger configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct JaegerConfig {
    /// Enable Jaeger export.
    #[serde(default)]
    pub enabled: bool,

    /// Jaeger agent host.
    #[serde(default = "default_jaeger_host")]
    pub agent_host: String,

    /// Jaeger agent port.
    #[serde(default = "default_jaeger_port")]
    pub agent_port: u16,

    /// Sampler type: const, probabilistic, rate_limiting, remote.
    #[serde(default = "default_sampler_type")]
    pub sampler_type: String,

    /// Sampler param (depends on type).
    #[serde(default = "default_sampler_param")]
    pub sampler_param: f64,
}

fn default_jaeger_host() -> String {
    "localhost".to_string()
}

fn default_jaeger_port() -> u16 {
    6831
}

fn default_sampler_type() -> String {
    "probabilistic".to_string()
}

fn default_sampler_param() -> f64 {
    0.1
}

/// Rate limiting configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct RateLimitConfig {
    /// Enable rate limiting.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Requests per window.
    #[validate(range(min = 1))]
    pub requests_per_window: u32,

    /// Window size in seconds.
    #[validate(range(min = 1))]
    pub window_secs: u64,

    /// Redis configuration for distributed rate limiting.
    pub redis: Option<RedisConfig>,

    /// Key prefix for Redis.
    #[serde(default = "default_rl_prefix")]
    pub key_prefix: String,
}

fn default_rl_prefix() -> String {
    "rl:".to_string()
}

/// Redis configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct RedisConfig {
    /// Redis URL (redis:// or rediss://).
    #[validate(url)]
    pub url: String,

    /// Connection pool size.
    #[serde(default = "default_pool_size")]
    #[validate(range(min = 1))]
    pub pool_size: u32,

    /// Connection timeout in seconds.
    #[serde(default = "default_redis_timeout")]
    pub connect_timeout_secs: u64,

    /// Command timeout in seconds.
    #[serde(default = "default_redis_timeout")]
    pub command_timeout_secs: u64,

    /// TLS configuration.
    pub tls: Option<RedisTlsConfig>,
}

fn default_pool_size() -> u32 {
    10
}

fn default_redis_timeout() -> u64 {
    5
}

/// Redis TLS configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct RedisTlsConfig {
    /// CA certificate file.
    pub ca_file: Option<String>,

    /// Client certificate file.
    pub cert_file: Option<String>,

    /// Client key file.
    pub key_file: Option<String>,

    /// Skip certificate verification (insecure).
    #[serde(default)]
    pub insecure: bool,
}

/// Etcd configuration for cluster coordination.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct EtcdConfig {
    /// Etcd endpoints.
    #[validate(length(min = 1))]
    pub endpoints: Vec<String>,

    /// TLS configuration.
    pub tls: Option<EtcdTlsConfig>,

    /// Request timeout in seconds.
    #[serde(default = "default_etcd_timeout")]
    #[validate(range(min = 1))]
    pub request_timeout_secs: u64,

    /// Dial timeout in seconds.
    #[serde(default = "default_etcd_timeout")]
    pub dial_timeout_secs: u64,

    /// Username for authentication.
    pub username: Option<String>,

    /// Password for authentication.
    pub password: Option<String>,
}

fn default_etcd_timeout() -> u64 {
    10
}

/// Etcd TLS configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct EtcdTlsConfig {
    /// CA certificate file.
    pub ca_file: String,

    /// Client certificate file.
    pub cert_file: String,

    /// Client key file.
    pub key_file: String,
}

/// NATS configuration for event streaming.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct NatsConfig {
    /// NATS server URLs.
    #[validate(length(min = 1))]
    pub urls: Vec<String>,

    /// JetStream domain.
    pub jetstream_domain: Option<String>,

    /// Connection name.
    #[serde(default = "default_nats_name")]
    pub name: String,

    /// TLS configuration.
    pub tls: Option<NatsTlsConfig>,

    /// Reconnect options.
    pub reconnect: Option<NatsReconnectConfig>,
}

fn default_nats_name() -> String {
    "mediaedge".to_string()
}

/// NATS TLS configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct NatsTlsConfig {
    /// CA certificate file.
    pub ca_file: Option<String>,

    /// Client certificate file.
    pub cert_file: Option<String>,

    /// Client key file.
    pub key_file: Option<String>,

    /// Skip certificate verification (insecure).
    #[serde(default)]
    pub insecure: bool,
}

/// NATS reconnect configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct NatsReconnectConfig {
    /// Maximum reconnect attempts (0 = infinite).
    #[serde(default = "default_max_reconnect")]
    pub max_attempts: usize,

    /// Reconnect delay in milliseconds.
    #[serde(default = "default_reconnect_delay")]
    pub delay_ms: u64,

    /// Reconnect jitter in milliseconds.
    #[serde(default = "default_reconnect_jitter")]
    pub jitter_ms: u64,
}

fn default_max_reconnect() -> usize {
    0
}

fn default_reconnect_delay() -> u64 {
    1000
}

fn default_reconnect_jitter() -> u64 {
    500
}

/// Object storage configuration (using OpenDAL).
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct StorageConfig {
    /// Storage backend type: fs, s3, azblob, gcs, memory.
    #[serde(default = "default_storage_backend")]
    pub backend: String,

    /// Filesystem configuration.
    pub fs: Option<FsStorageConfig>,

    /// S3 configuration.
    pub s3: Option<S3StorageConfig>,

    /// Azure Blob configuration.
    pub azblob: Option<AzblobStorageConfig>,

    /// GCS configuration.
    pub gcs: Option<GcsStorageConfig>,

    /// Memory configuration (for testing).
    pub memory: Option<MemoryStorageConfig>,

    /// Default bucket/container name.
    #[validate(length(min = 1))]
    pub default_bucket: String,

    /// Root path within bucket.
    #[serde(default)]
    pub root: String,
}

fn default_storage_backend() -> String {
    "fs".to_string()
}

/// Filesystem storage configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct FsStorageConfig {
    /// Root directory.
    #[validate(length(min = 1))]
    pub root: String,

    /// Create root if not exists.
    #[serde(default = "default_true")]
    pub create_if_missing: bool,
}

/// S3 storage configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct S3StorageConfig {
    /// Bucket name.
    #[validate(length(min = 1))]
    pub bucket: String,

    /// Region.
    #[validate(length(min = 1))]
    pub region: String,

    /// Endpoint URL (for S3-compatible).
    pub endpoint: Option<String>,

    /// Access key ID.
    pub access_key_id: Option<String>,

    /// Secret access key.
    pub secret_access_key: Option<String>,

    /// Session token (for temporary credentials).
    pub session_token: Option<String>,

    /// Use path-style addressing.
    #[serde(default)]
    pub path_style: bool,

    /// TLS configuration.
    pub tls: Option<S3TlsConfig>,
}

/// S3 TLS configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct S3TlsConfig {
    /// CA certificate file.
    pub ca_file: Option<String>,

    /// Skip certificate verification (insecure).
    #[serde(default)]
    pub insecure: bool,
}

/// Azure Blob storage configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct AzblobStorageConfig {
    /// Container name.
    #[validate(length(min = 1))]
    pub container: String,

    /// Account name.
    #[validate(length(min = 1))]
    pub account: String,

    /// Account key.
    pub account_key: Option<String>,

    /// SAS token.
    pub sas_token: Option<String>,

    /// Endpoint suffix.
    #[serde(default = "default_azblob_endpoint")]
    pub endpoint_suffix: String,
}

fn default_azblob_endpoint() -> String {
    "blob.core.windows.net".to_string()
}

/// GCS storage configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct GcsStorageConfig {
    /// Bucket name.
    #[validate(length(min = 1))]
    pub bucket: String,

    /// Service account key file.
    pub credentials_file: Option<String>,

    /// Project ID.
    pub project_id: Option<String>,
}

/// In-memory storage configuration (for testing).
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct MemoryStorageConfig {
    /// Maximum size in bytes.
    #[serde(default = "default_memory_size")]
    pub max_size: usize,
}

fn default_memory_size() -> usize {
    100 * 1024 * 1024 // 100 MB
}

/// Gateway service configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct GatewayConfig {
    /// Server configuration.
    #[validate]
    pub server: ServerConfig,

    /// Authentication configuration.
    #[validate]
    pub auth: AuthConfig,

    /// Metrics configuration.
    #[validate]
    pub metrics: MetricsConfig,

    /// Tracing configuration.
    #[validate]
    pub tracing: TracingConfig,

    /// Rate limiting configuration.
    #[validate]
    pub rate_limit: RateLimitConfig,

    /// Redis configuration for session/rate limit.
    pub redis: Option<RedisConfig>,

    /// NATS configuration for event publishing.
    pub nats: Option<NatsConfig>,

    /// Etcd configuration for cluster coordination.
    pub etcd: Option<EtcdConfig>,

    /// Upstream services (for proxying).
    pub upstreams: Vec<UpstreamConfig>,

    /// Protocol-specific configuration.
    pub protocols: GatewayProtocolsConfig,

    /// Admin API configuration.
    pub admin: AdminConfig,

    /// Federation configuration for cross-cluster replication.
    pub federation: Option<FederationConfig>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            auth: AuthConfig::default(),
            metrics: MetricsConfig::default(),
            tracing: TracingConfig::default(),
            rate_limit: RateLimitConfig::default(),
            redis: None,
            nats: None,
            etcd: None,
            upstreams: Vec::new(),
            protocols: GatewayProtocolsConfig::default(),
            admin: AdminConfig::default(),
            federation: None,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt: None,
            static_tokens: None,
            webhook: None,
            default_mode: AuthMode::None,
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: "/metrics".to_string(),
            port: None,
            otlp: OtlpConfig::default(),
        }
    }
}

impl Default for OtlpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            service_name: "mediaedge-gateway".to_string(),
            trace_sample_ratio: 0.1,
            metric_interval_secs: 60,
            headers: std::collections::HashMap::new(),
        }
    }
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: "info".to_string(),
            format: "json".to_string(),
            jaeger: None,
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_window: 1000,
            window_secs: 60,
            redis: None,
            key_prefix: "rl:".to_string(),
        }
    }
}

/// Upstream service configuration for gateway proxying.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct UpstreamConfig {
    /// Service name.
    #[validate(length(min = 1))]
    pub name: String,

    /// Upstream URL.
    #[validate(url)]
    pub url: String,

    /// Health check path.
    #[serde(default = "default_health_path")]
    pub health_path: String,

    /// Health check interval in seconds.
    #[serde(default = "default_health_interval")]
    pub health_interval_secs: u64,

    /// Timeout in seconds.
    #[serde(default = "default_upstream_timeout")]
    pub timeout_secs: u64,

    /// Weight for load balancing.
    #[serde(default = "default_weight")]
    #[validate(range(min = 1))]
    pub weight: u32,
}

fn default_health_path() -> String {
    "/health".to_string()
}

fn default_health_interval() -> u64 {
    10
}

fn default_upstream_timeout() -> u64 {
    30
}

fn default_weight() -> u32 {
    1
}

/// Gateway protocol-specific configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct GatewayProtocolsConfig {
    /// RTMP configuration.
    pub rtmp: Option<RtmpConfig>,

    /// SRT configuration.
    pub srt: Option<SrtConfig>,

    /// RIST configuration.
    pub rist: Option<RistConfig>,

    /// WHIP configuration.
    pub whip: Option<WhipConfig>,

    /// HTTP/HLS/DASH configuration.
    pub http: Option<HttpProtocolConfig>,
}

/// RTMP configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct RtmpConfig {
    /// Enable RTMP ingest.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Port for RTMP.
    #[validate(range(min = 1, max = 65535))]
    pub port: u16,

    /// Port for RTMPS (TLS).
    pub port_tls: Option<u16>,

    /// Chunk size.
    #[serde(default = "default_rtmp_chunk_size")]
    #[validate(range(min = 128))]
    pub chunk_size: u32,

    /// Ping interval in seconds.
    #[serde(default = "default_rtmp_ping")]
    pub ping_interval_secs: u64,

    /// Max stream key length.
    #[serde(default = "default_max_stream_key")]
    #[validate(range(min = 1, max = 256))]
    pub max_stream_key_len: usize,
}

fn default_rtmp_chunk_size() -> u32 {
    4096
}

fn default_rtmp_ping() -> u64 {
    30
}

fn default_max_stream_key() -> usize {
    128
}

/// SRT configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct SrtConfig {
    /// Enable SRT ingest.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Port for SRT.
    #[validate(range(min = 1, max = 65535))]
    pub port: u16,

    /// Default latency in milliseconds.
    #[serde(default = "default_srt_latency")]
    #[validate(range(min = 1))]
    pub latency_ms: u32,

    /// Enable encryption.
    #[serde(default)]
    pub encryption: bool,

    /// Passphrase for encryption (if enabled).
    pub passphrase: Option<String>,

    /// Stream ID prefix for access control.
    #[serde(default)]
    pub stream_id_prefix: String,
}

fn default_srt_latency() -> u32 {
    120
}

/// RIST configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct RistConfig {
    /// Enable RIST ingest.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Port for RIST.
    #[validate(range(min = 1, max = 65535))]
    pub port: u16,

    /// Profile: simple, main, advanced.
    #[serde(default = "default_rist_profile")]
    pub profile: String,
}

fn default_rist_profile() -> String {
    "simple".to_string()
}

/// WHIP configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct WhipConfig {
    /// Enable WHIP ingest.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Path for WHIP endpoint.
    #[serde(default = "default_whip_path")]
    pub path: String,

    /// STUN servers.
    pub stun_servers: Vec<String>,

    /// TURN servers.
    pub turn_servers: Vec<TurnServerConfig>,
}

fn default_whip_path() -> String {
    "/whip".to_string()
}

/// TURN server configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct TurnServerConfig {
    /// URLs.
    #[validate(length(min = 1))]
    pub urls: Vec<String>,

    /// Username.
    pub username: String,

    /// Credential.
    pub credential: String,
}

/// HTTP protocol configuration (HLS, DASH, etc.).
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct HttpProtocolConfig {
    /// Enable HLS egress.
    #[serde(default = "default_true")]
    pub hls_enabled: bool,

    /// Enable DASH egress.
    #[serde(default = "default_true")]
    pub dash_enabled: bool,

    /// Segment duration in seconds.
    #[serde(default = "default_segment_duration")]
    #[validate(range(min = 1))]
    pub segment_duration_secs: u32,

    /// Playlist window size (number of segments).
    #[serde(default = "default_playlist_window")]
    #[validate(range(min = 1))]
    pub playlist_window: usize,
}

fn default_segment_duration() -> u32 {
    6
}

fn default_playlist_window() -> usize {
    10
}

/// Admin API configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct AdminConfig {
    /// Enable admin API.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Admin API path prefix.
    #[serde(default = "default_admin_path")]
    pub path: String,

    /// Admin token (if not using auth service).
    pub token: Option<String>,

    /// Allowed IPs (CIDR notation).
    pub allowed_ips: Vec<String>,
}

fn default_admin_path() -> String {
    "/api/v1/admin".to_string()
}

/// Storage Node configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct StorageNodeConfig {
    /// Server configuration.
    #[validate]
    pub server: ServerConfig,

    /// Authentication configuration.
    #[validate]
    pub auth: AuthConfig,

    /// Metrics configuration.
    #[validate]
    pub metrics: MetricsConfig,

    /// Tracing configuration.
    #[validate]
    pub tracing: TracingConfig,

    /// Storage configuration.
    #[validate]
    pub storage: StorageConfig,

    /// Etcd configuration for cluster coordination.
    pub etcd: Option<EtcdConfig>,

    /// NATS configuration for events.
    pub nats: Option<NatsConfig>,

    /// Replication configuration.
    pub replication: StorageReplicationConfig,

    /// Archive configuration.
    pub archive: ArchiveConfig,

    /// Cache configuration.
    pub cache: CacheConfig,
}

impl Default for StorageNodeConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig { port: 8081, ..Default::default() },
            auth: AuthConfig::default(),
            metrics: MetricsConfig::default(),
            tracing: TracingConfig::default(),
            storage: StorageConfig::default(),
            etcd: None,
            nats: None,
            replication: StorageReplicationConfig::default(),
            archive: ArchiveConfig::default(),
            cache: CacheConfig::default(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: "fs".to_string(),
            fs: Some(FsStorageConfig { root: "/data/storage".to_string(), create_if_missing: true }),
            s3: None,
            azblob: None,
            gcs: None,
            memory: None,
            default_bucket: "mediaedge".to_string(),
            root: "".to_string(),
        }
    }
}

/// Storage replication configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct StorageReplicationConfig {
    /// Enable replication.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Replication factor (number of copies).
    #[serde(default = "default_replication_factor")]
    #[validate(range(min = 1))]
    pub factor: usize,

    /// Erasure coding: k data shards, m parity shards.
    pub erasure: Option<ErasureConfig>,

    /// Replication interval in seconds.
    #[serde(default = "default_replication_interval")]
    pub interval_secs: u64,

    /// Max concurrent replication tasks.
    #[serde(default = "default_max_replication_tasks")]
    #[validate(range(min = 1))]
    pub max_concurrent: usize,
}

fn default_replication_factor() -> usize {
    3
}

fn default_replication_interval() -> u64 {
    60
}

fn default_max_replication_tasks() -> usize {
    10
}

/// Erasure coding configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct ErasureConfig {
    /// Data shards (k).
    #[validate(range(min = 1, max = 255))]
    pub data_shards: u8,

    /// Parity shards (m).
    #[validate(range(min = 1, max = 255))]
    pub parity_shards: u8,
}

impl Default for ErasureConfig {
    fn default() -> Self {
        Self {
            data_shards: 10,
            parity_shards: 4,
        }
    }
}

/// Archive configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct ArchiveConfig {
    /// Enable archive/DVR.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Archive segment duration in seconds.
    #[serde(default = "default_archive_segment")]
    #[validate(range(min = 1))]
    pub segment_duration_secs: u32,

    /// Retention period in days (0 = forever).
    #[serde(default)]
    pub retention_days: u32,

    /// Tiering configuration.
    pub tiering: Option<TieringConfig>,

    /// Orphaned recording cleanup interval in hours.
    #[serde(default = "default_orphan_cleanup")]
    pub orphan_cleanup_interval_hours: u32,
}

fn default_archive_segment() -> u32 {
    300 // 5 minutes
}

fn default_orphan_cleanup() -> u32 {
    24
}

/// Storage tiering configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct TieringConfig {
    /// Hot tier: NVMe/SSD, recent data.
    pub hot: TierConfig,

    /// Warm tier: SSD/HDD, older data.
    pub warm: Option<TierConfig>,

    /// Cold tier: S3/Glacier, archival data.
    pub cold: Option<TierConfig>,
}

/// Tier configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct TierConfig {
    /// Storage backend for this tier.
    #[validate(length(min = 1))]
    pub backend: String,

    /// Age threshold to move to this tier (days).
    #[validate(range(min = 1))]
    pub age_days: u32,

    /// Storage config (inline).
    pub config: StorageConfig,
}

/// Cache configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct CacheConfig {
    /// Enable cache.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Max cache size in bytes.
    #[serde(default = "default_cache_size")]
    #[validate(range(min = 1))]
    pub max_size_bytes: usize,

    /// Max cache entries.
    #[serde(default = "default_cache_entries")]
    #[validate(range(min = 1))]
    pub max_entries: usize,

    /// TTL in seconds.
    #[serde(default = "default_cache_ttl")]
    pub ttl_secs: u64,

    /// Eviction policy: lru, lfu, fifo.
    #[serde(default = "default_eviction_policy")]
    pub eviction_policy: String,
}

fn default_cache_size() -> usize {
    10 * 1024 * 1024 * 1024 // 10 GB
}

fn default_cache_entries() -> usize {
    100000
}

fn default_cache_ttl() -> u64 {
    3600 // 1 hour
}

fn default_eviction_policy() -> String {
    "lru".to_string()
}

/// Edge Node configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct EdgeNodeConfig {
    /// Server configuration.
    #[validate]
    pub server: ServerConfig,

    /// Authentication configuration.
    #[validate]
    pub auth: AuthConfig,

    /// Metrics configuration.
    #[validate]
    pub metrics: MetricsConfig,

    /// Tracing configuration.
    #[validate]
    pub tracing: TracingConfig,

    /// Storage configuration (for local cache).
    #[validate]
    pub storage: StorageConfig,

    /// NATS configuration for events.
    pub nats: Option<NatsConfig>,

    /// Etcd configuration for cluster coordination.
    pub etcd: Option<EtcdConfig>,

    /// Origin configuration (upstream storage/gateway).
    pub origin: OriginConfig,

    /// Cache configuration.
    pub cache: EdgeCacheConfig,

    /// Mesh configuration (WebRTC peer-to-peer).
    pub mesh: MeshConfig,

    /// Signed URL configuration.
    pub signed_urls: SignedUrlConfig,
}

impl Default for EdgeNodeConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig { port: 8082, ..Default::default() },
            auth: AuthConfig::default(),
            metrics: MetricsConfig::default(),
            tracing: TracingConfig::default(),
            storage: StorageConfig::default(),
            nats: None,
            etcd: None,
            origin: OriginConfig::default(),
            cache: EdgeCacheConfig::default(),
            mesh: MeshConfig::default(),
            signed_urls: SignedUrlConfig::default(),
        }
    }
}

/// Origin configuration for edge nodes.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct OriginConfig {
    /// Origin URLs (load balanced).
    #[validate(length(min = 1))]
    pub urls: Vec<String>,

    /// Health check path.
    #[serde(default = "default_health_path")]
    pub health_path: String,

    /// Timeout in seconds.
    #[serde(default = "default_upstream_timeout")]
    pub timeout_secs: u64,

    /// TLS verification.
    #[serde(default = "default_true")]
    pub verify_tls: bool,
}

/// Edge cache configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct EdgeCacheConfig {
    /// Enable cache.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Max cache size in bytes.
    #[serde(default = "default_edge_cache_size")]
    #[validate(range(min = 1))]
    pub max_size_bytes: usize,

    /// Max cache entries.
    #[serde(default = "default_cache_entries")]
    #[validate(range(min = 1))]
    pub max_entries: usize,

    /// TTL for live segments in seconds.
    #[serde(default = "default_live_ttl")]
    pub live_ttl_secs: u64,

    /// TTL for VOD segments in seconds.
    #[serde(default = "default_vod_ttl")]
    pub vod_ttl_secs: u64,

    /// Stale-while-revalidate in seconds.
    #[serde(default = "default_stale_while_revalidate")]
    pub stale_while_revalidate_secs: u64,

    /// Cache warming enabled.
    #[serde(default = "default_true")]
    pub warming_enabled: bool,

    /// Cache invalidation on stream end.
    #[serde(default = "default_true")]
    pub invalidate_on_stream_end: bool,
}

fn default_edge_cache_size() -> usize {
    50 * 1024 * 1024 * 1024 // 50 GB
}

fn default_live_ttl() -> u64 {
    300 // 5 minutes
}

fn default_vod_ttl() -> u64 {
    86400 // 24 hours
}

fn default_stale_while_revalidate() -> u64 {
    60
}

/// Mesh configuration (WebRTC peer-to-peer).
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct MeshConfig {
    /// Enable mesh.
    #[serde(default)]
    pub enabled: bool,

    /// Max peers per node.
    #[serde(default = "default_max_peers")]
    #[validate(range(min = 1))]
    pub max_peers: usize,

    /// Root peer count.
    #[serde(default = "default_root_peers")]
    #[validate(range(min = 1))]
    pub root_peer_count: usize,

    /// ICE servers.
    pub ice_servers: Vec<IceServerConfig>,

    /// Signaling server URL.
    pub signaling_url: Option<String>,
}

fn default_max_peers() -> usize {
    50
}

fn default_root_peers() -> usize {
    3
}

/// ICE server configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct IceServerConfig {
    /// URLs.
    #[validate(length(min = 1))]
    pub urls: Vec<String>,

    /// Username.
    pub username: Option<String>,

    /// Credential.
    pub credential: Option<String>,
}

/// Signed URL configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct SignedUrlConfig {
    /// Enable signed URLs.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Signing algorithm: hmac-sha256, ed25519.
    #[serde(default = "default_signing_algo")]
    pub algorithm: String,

    /// HMAC secret (for hmac-sha256).
    pub hmac_secret: Option<String>,

    /// Ed25519 private key file (for ed25519).
    pub ed25519_key_file: Option<String>,

    /// Default TTL in seconds.
    #[serde(default = "default_signed_url_ttl")]
    #[validate(range(min = 1))]
    pub default_ttl_secs: u64,

    /// Allowed query parameters for signing.
    pub allowed_params: Vec<String>,
}

fn default_signing_algo() -> String {
    "hmac-sha256".to_string()
}

fn default_signed_url_ttl() -> u64 {
    3600 // 1 hour
}

/// Replication Coordinator configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct ReplicationCoordinatorConfig {
    /// Server configuration.
    #[validate]
    pub server: ServerConfig,

    /// Authentication configuration.
    #[validate]
    pub auth: AuthConfig,

    /// Metrics configuration.
    #[validate]
    pub metrics: MetricsConfig,

    /// Tracing configuration.
    #[validate]
    pub tracing: TracingConfig,

    /// Etcd configuration (required for Raft).
    #[validate]
    pub etcd: EtcdConfig,

    /// NATS configuration for events.
    pub nats: Option<NatsConfig>,

    /// Replication planner configuration.
    pub planner: PlannerConfig,

    /// FEC configuration for mesh replication.
    pub fec: FecConfig,

    /// Mesh replication configuration.
    pub mesh: MeshReplicationConfig,
}

impl Default for ReplicationCoordinatorConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig { port: 8083, ..Default::default() },
            auth: AuthConfig::default(),
            metrics: MetricsConfig::default(),
            tracing: TracingConfig::default(),
            etcd: EtcdConfig {
                endpoints: vec!["http://localhost:2379".to_string()],
                tls: None,
                request_timeout_secs: 10,
                dial_timeout_secs: 10,
                username: None,
                password: None,
            },
            nats: None,
            planner: PlannerConfig::default(),
            fec: FecConfig::default(),
            mesh: MeshReplicationConfig::default(),
        }
    }
}

/// Replication planner configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct PlannerConfig {
    /// Enable planner.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Planning interval in seconds.
    #[serde(default = "default_planning_interval")]
    #[validate(range(min = 1))]
    pub interval_secs: u64,

    /// Continent/region awareness.
    #[serde(default = "default_true")]
    pub region_aware: bool,

    /// Demand-triggered replicas.
    #[serde(default = "default_true")]
    pub demand_triggered: bool,

    /// Storage capacity awareness.
    #[serde(default = "default_true")]
    pub capacity_aware: bool,

    /// Anti-affinity for nearby nodes.
    #[serde(default = "default_true")]
    pub anti_affinity: bool,

    /// Minimum replicas per stream.
    #[serde(default = "default_min_replicas")]
    #[validate(range(min = 1))]
    pub min_replicas: usize,

    /// Maximum replicas per stream.
    #[serde(default = "default_max_replicas")]
    #[validate(range(min = 1))]
    pub max_replicas: usize,
}

fn default_planning_interval() -> u64 {
    30
}

fn default_min_replicas() -> usize {
    3
}

fn default_max_replicas() -> usize {
    10
}

/// FEC configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct FecConfig {
    /// Enable FEC.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// FEC algorithm: raptorq, reedsolomon.
    #[serde(default = "default_fec_algo")]
    pub algorithm: String,

    /// Symbol size in bytes.
    #[serde(default = "default_symbol_size")]
    #[validate(range(min = 1))]
    pub symbol_size: usize,

    /// Source symbols per block.
    #[serde(default = "default_source_symbols")]
    #[validate(range(min = 1))]
    pub source_symbols: usize,

    /// Repair symbols per block.
    #[serde(default = "default_repair_symbols")]
    #[validate(range(min = 1))]
    pub repair_symbols: usize,
}

fn default_fec_algo() -> String {
    "raptorq".to_string()
}

fn default_symbol_size() -> usize {
    1400
}

fn default_source_symbols() -> usize {
    100
}

fn default_repair_symbols() -> usize {
    30
}

/// Mesh replication configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct MeshReplicationConfig {
    /// Enable mesh replication.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Gossip interval in milliseconds.
    #[serde(default = "default_gossip_interval")]
    pub gossip_interval_ms: u64,

    /// Max gossip fanout.
    #[serde(default = "default_gossip_fanout")]
    #[validate(range(min = 1))]
    pub gossip_fanout: usize,

    /// Replica request timeout in seconds.
    #[serde(default = "default_replica_timeout")]
    pub replica_timeout_secs: u64,
}

fn default_gossip_interval() -> u64 {
    5000
}

fn default_gossip_fanout() -> usize {
    3
}

fn default_replica_timeout() -> u64 {
    30
}

/// Media Processor configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct MediaProcessorConfig {
    /// Server configuration.
    #[validate]
    pub server: ServerConfig,

    /// Authentication configuration.
    #[validate]
    pub auth: AuthConfig,

    /// Metrics configuration.
    #[validate]
    pub metrics: MetricsConfig,

    /// Tracing configuration.
    #[validate]
    pub tracing: TracingConfig,

    /// NATS configuration for job queue.
    pub nats: Option<NatsConfig>,

    /// Storage configuration for output.
    #[validate]
    pub storage: StorageConfig,

    /// Transcoding configuration.
    pub transcoding: TranscodingConfig,

    /// Recording configuration.
    pub recording: RecordingConfig,

    /// VOD processing configuration.
    pub vod: VodConfig,

    /// AI processing configuration.
    pub ai: AiConfig,
}

impl Default for MediaProcessorConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig { port: 8084, ..Default::default() },
            auth: AuthConfig::default(),
            metrics: MetricsConfig::default(),
            tracing: TracingConfig::default(),
            nats: None,
            storage: StorageConfig::default(),
            transcoding: TranscodingConfig::default(),
            recording: RecordingConfig::default(),
            vod: VodConfig::default(),
            ai: AiConfig::default(),
        }
    }
}

/// Transcoding configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct TranscodingConfig {
    /// Enable transcoding.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// FFmpeg path.
    #[serde(default = "default_ffmpeg_path")]
    pub ffmpeg_path: String,

    /// Hardware acceleration: none, nvenc, vaapi, videotoolbox.
    #[serde(default = "default_hwaccel")]
    pub hwaccel: String,

    /// GPU device ID (for nvenc/vaapi).
    pub gpu_device: Option<u32>,

    /// Default video codec.
    #[serde(default = "default_video_codec")]
    pub default_video_codec: String,

    /// Default audio codec.
    #[serde(default = "default_audio_codec")]
    pub default_audio_codec: String,

    /// Bitrate ladder presets.
    pub presets: Vec<BitratePreset>,

    /// Max concurrent jobs.
    #[serde(default = "default_max_transcode_jobs")]
    #[validate(range(min = 1))]
    pub max_concurrent_jobs: usize,

    /// Job queue size.
    #[serde(default = "default_queue_size")]
    #[validate(range(min = 1))]
    pub queue_size: usize,

    /// Priority levels: live > vod > archive.
    #[serde(default = "default_priority_levels")]
    pub priority_levels: Vec<String>,
}

fn default_ffmpeg_path() -> String {
    "ffmpeg".to_string()
}

fn default_hwaccel() -> String {
    "none".to_string()
}

fn default_video_codec() -> String {
    "libx264".to_string()
}

fn default_audio_codec() -> String {
    "aac".to_string()
}

fn default_max_transcode_jobs() -> usize {
    10
}

fn default_queue_size() -> usize {
    100
}

fn default_priority_levels() -> Vec<String> {
    vec!["live".to_string(), "vod".to_string(), "archive".to_string()]
}

/// Bitrate preset.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct BitratePreset {
    /// Preset name.
    #[validate(length(min = 1))]
    pub name: String,

    /// Video bitrate in kbps.
    #[validate(range(min = 1))]
    pub video_bitrate_kbps: u32,

    /// Audio bitrate in kbps.
    #[validate(range(min = 1))]
    pub audio_bitrate_kbps: u32,

    /// Resolution: WxH (e.g., 1920x1080).
    #[validate(length(min = 1))]
    pub resolution: String,

    /// Frame rate.
    #[serde(default = "default_framerate")]
    #[validate(range(min = 1, max = 120))]
    pub framerate: u32,

    /// Video codec.
    #[serde(default = "default_video_codec")]
    pub video_codec: String,

    /// Audio codec.
    #[serde(default = "default_audio_codec")]
    pub audio_codec: String,

    /// HW acceleration for this preset.
    pub hwaccel: Option<String>,

    /// Profile (e.g., high, main, baseline).
    pub profile: Option<String>,

    /// Level (e.g., 4.1, 5.1).
    pub level: Option<String>,
}

fn default_framerate() -> u32 {
    30
}

/// Recording configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct RecordingConfig {
    /// Enable recording.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Segment duration in seconds.
    #[serde(default = "default_recording_segment")]
    #[validate(range(min = 1))]
    pub segment_duration_secs: u32,

    /// Output format: mp4, mkv, ts.
    #[serde(default = "default_recording_format")]
    pub format: String,

    /// Crash recovery enabled.
    #[serde(default = "default_true")]
    pub crash_recovery: bool,

    /// Max recording duration in hours (0 = unlimited).
    #[serde(default)]
    pub max_duration_hours: u32,

    /// Scheduled recordings (cron expressions).
    pub schedules: Vec<RecordingSchedule>,
}

fn default_recording_segment() -> u32 {
    300
}

fn default_recording_format() -> String {
    "mp4".to_string()
}

/// Recording schedule.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct RecordingSchedule {
    /// Schedule name.
    #[validate(length(min = 1))]
    pub name: String,

    /// Cron expression.
    #[validate(length(min = 1))]
    pub cron: String,

    /// Stream name/pattern.
    #[validate(length(min = 1))]
    pub stream: String,

    /// Output path template.
    #[validate(length(min = 1))]
    pub output_template: String,

    /// Retention days.
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

fn default_retention_days() -> u32 {
    30
}

/// VOD processing configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct VodConfig {
    /// Enable VOD processing.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Transcode ladder for VOD.
    pub transcode_ladder: Vec<BitratePreset>,

    /// Thumbnail generation.
    pub thumbnails: ThumbnailConfig,

    /// Sprite generation.
    pub sprites: SpriteConfig,

    /// Caption/translation.
    pub captions: CaptionConfig,

    /// Packaging: hls, dash, both.
    #[serde(default = "default_packaging")]
    pub packaging: String,
}

fn default_packaging() -> String {
    "both".to_string()
}

/// Thumbnail configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct ThumbnailConfig {
    /// Enable thumbnails.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Interval in seconds.
    #[serde(default = "default_thumb_interval")]
    #[validate(range(min = 1))]
    pub interval_secs: u32,

    /// Resolution.
    #[serde(default = "default_thumb_resolution")]
    pub resolution: String,

    /// Format: jpeg, png, webp.
    #[serde(default = "default_thumb_format")]
    pub format: String,
}

fn default_thumb_interval() -> u32 {
    10
}

fn default_thumb_resolution() -> String {
    "320x180".to_string()
}

fn default_thumb_format() -> String {
    "jpeg".to_string()
}

/// Sprite configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct SpriteConfig {
    /// Enable sprites.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Thumbnails per sprite.
    #[serde(default = "default_sprite_columns")]
    #[validate(range(min = 1))]
    pub columns: u32,

    /// Rows per sprite.
    #[serde(default = "default_sprite_rows")]
    #[validate(range(min = 1))]
    pub rows: u32,

    /// Interval in seconds.
    #[serde(default = "default_sprite_interval")]
    #[validate(range(min = 1))]
    pub interval_secs: u32,
}

fn default_sprite_columns() -> u32 {
    10
}

fn default_sprite_rows() -> u32 {
    10
}

fn default_sprite_interval() -> u32 {
    5
}

/// Caption configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct CaptionConfig {
    /// Enable captions.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Languages to generate.
    pub languages: Vec<String>,

    /// ASR provider: whisper, cloud.
    #[serde(default = "default_asr_provider")]
    pub provider: String,

    /// Whisper model size: tiny, base, small, medium, large.
    #[serde(default = "default_whisper_model")]
    pub whisper_model: String,
}

fn default_asr_provider() -> String {
    "whisper".to_string()
}

fn default_whisper_model() -> String {
    "base".to_string()
}

/// AI processing configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct AiConfig {
    /// Enable AI processing.
    #[serde(default)]
    pub enabled: bool,

    /// Content moderation.
    pub moderation: ModerationConfig,

    /// Highlight detection.
    pub highlights: HighlightConfig,

    /// Anomaly detection.
    pub anomaly: AnomalyConfig,
}

/// Content moderation configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct ModerationConfig {
    /// Enable moderation.
    #[serde(default)]
    pub enabled: bool,

    /// Provider: builtin, cloud.
    #[serde(default = "default_moderation_provider")]
    pub provider: String,

    /// Confidence threshold.
    #[serde(default = "default_moderation_threshold")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub threshold: f64,

    /// Categories to detect.
    pub categories: Vec<String>,
}

fn default_moderation_provider() -> String {
    "builtin".to_string()
}

fn default_moderation_threshold() -> f64 {
    0.8
}

/// Highlight detection configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct HighlightConfig {
    /// Enable highlights.
    #[serde(default)]
    pub enabled: bool,

    /// Scene change threshold.
    #[serde(default = "default_scene_threshold")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub scene_change_threshold: f64,

    /// Audio energy threshold.
    #[serde(default = "default_audio_threshold")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub audio_energy_threshold: f64,

    /// Speech detection enabled.
    #[serde(default = "default_true")]
    pub speech_detection: bool,

    /// Min highlight duration in seconds.
    #[serde(default = "default_min_highlight")]
    #[validate(range(min = 1))]
    pub min_duration_secs: u32,
}

fn default_scene_threshold() -> f64 {
    0.3
}

fn default_audio_threshold() -> f64 {
    0.5
}

fn default_min_highlight() -> u32 {
    5
}

/// Anomaly detection configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct AnomalyConfig {
    /// Enable anomaly detection.
    #[serde(default)]
    pub enabled: bool,

    /// Bitrate drop threshold (%).
    #[serde(default = "default_bitrate_drop")]
    #[validate(range(min = 0.0, max = 100.0))]
    pub bitrate_drop_threshold_pct: f64,

    /// Error spike threshold (errors/min).
    #[serde(default = "default_error_spike")]
    #[validate(range(min = 1))]
    pub error_spike_threshold: u32,

    /// Check interval in seconds.
    #[serde(default = "default_anomaly_interval")]
    #[validate(range(min = 1))]
    pub check_interval_secs: u64,
}

fn default_bitrate_drop() -> f64 {
    50.0
}

fn default_error_spike() -> u32 {
    100
}

fn default_anomaly_interval() -> u64 {
    60
}

/// Federation configuration for cross-cluster replication.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct FederationConfig {
    /// Enable federation.
    #[serde(default)]
    pub enabled: bool,

    /// Federation links.
    pub links: Vec<FederationLinkConfig>,
}

/// Federation link configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct FederationLinkConfig {
    /// Unique link identifier.
    #[validate(length(min = 1))]
    pub link_id: String,

    /// Remote cluster identifier.
    #[validate(length(min = 1))]
    pub remote_cluster_id: String,

    /// Remote endpoint (MoQ URL).
    #[validate(length(min = 1))]
    pub remote_endpoint: String,

    /// Stream patterns to mirror.
    pub streams: Vec<String>,

    /// Enable mTLS.
    #[serde(default = "default_true")]
    pub mtls_enabled: bool,

    /// Certificate file path (PEM).
    pub cert_file: Option<String>,

    /// Private key file path (PEM).
    pub key_file: Option<String>,

    /// CA file path for verification (PEM).
    pub ca_file: Option<String>,

    /// Reconnect base delay in seconds.
    #[serde(default = "default_federation_reconnect_base")]
    #[validate(range(min = 1))]
    pub reconnect_base_delay_secs: u64,

    /// Reconnect max delay in seconds.
    #[serde(default = "default_federation_reconnect_max")]
    #[validate(range(min = 1))]
    pub reconnect_max_delay_secs: u64,

    /// Reconnect jitter percentage.
    #[serde(default = "default_federation_reconnect_jitter")]
    #[validate(range(min = 0, max = 100))]
    pub reconnect_jitter_percent: u8,

    /// Health check interval in seconds.
    #[serde(default = "default_federation_health_check")]
    #[validate(range(min = 1))]
    pub health_check_interval_secs: u64,

    /// Offset compensation in milliseconds.
    #[serde(default)]
    pub offset_compensation_ms: i64,
}

fn default_federation_reconnect_base() -> u64 {
    1
}

fn default_federation_reconnect_max() -> u64 {
    60
}

fn default_federation_reconnect_jitter() -> u8 {
    10
}

fn default_federation_health_check() -> u64 {
    30
}

impl Default for FederationLinkConfig {
    fn default() -> Self {
        Self {
            link_id: String::new(),
            remote_cluster_id: String::new(),
            remote_endpoint: String::new(),
            streams: Vec::new(),
            mtls_enabled: true,
            cert_file: None,
            key_file: None,
            ca_file: None,
            reconnect_base_delay_secs: 1,
            reconnect_max_delay_secs: 60,
            reconnect_jitter_percent: 10,
            health_check_interval_secs: 30,
            offset_compensation_ms: 0,
        }
    }
}