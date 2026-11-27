//! OpenAPI 3.1 specification for MediaEdge REST API.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use utoipa::{
    OpenApi, ToSchema,
    openapi::{
        ComponentsBuilder, InfoBuilder, OpenApiBuilder, ServerBuilder,
        security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    },
};
use utoipa_rapidoc::RapiDoc;
use utoipa_redoc::Redoc;
use utoipa_swagger_ui::SwaggerUi;

use crate::validation::ValidatedJson;

/// OpenAPI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiConfig {
    /// Service name.
    pub service_name: String,

    /// Service version.
    pub version: String,

    /// Service description.
    pub description: String,

    /// Server URLs.
    pub servers: Vec<ServerConfig>,

    /// Enable Swagger UI.
    #[serde(default = "default_true")]
    pub swagger_ui: bool,

    /// Enable ReDoc.
    #[serde(default = "default_true")]
    pub redoc: bool,

    /// Enable RapiDoc.
    #[serde(default = "default_true")]
    pub rapidoc: bool,

    /// Swagger UI path.
    #[serde(default = "default_swagger_path")]
    pub swagger_path: String,

    /// ReDoc path.
    #[serde(default = "default_redoc_path")]
    pub redoc_path: String,

    /// RapiDoc path.
    #[serde(default = "default_rapidoc_path")]
    pub rapidoc_path: String,
}

fn default_true() -> bool {
    true
}

fn default_swagger_path() -> String {
    "/api/docs/swagger".to_string()
}

fn default_redoc_path() -> String {
    "/api/docs/redoc".to_string()
}

fn default_rapidoc_path() -> String {
    "/api/docs/rapidoc".to_string()
}

/// Server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub url: String,
    pub description: String,
}

impl Default for OpenApiConfig {
    fn default() -> Self {
        Self {
            service_name: "MediaEdge API".to_string(),
            version: "1.0.0".to_string(),
            description: "MediaEdge Media Infrastructure Platform API".to_string(),
            servers: vec![
                ServerConfig {
                    url: "http://localhost:8080".to_string(),
                    description: "Development server".to_string(),
                },
                ServerConfig {
                    url: "https://api.mediaedge.example.com".to_string(),
                    description: "Production server".to_string(),
                },
            ],
            swagger_ui: true,
            redoc: true,
            rapidoc: true,
            swagger_path: "/api/docs/swagger".to_string(),
            redoc_path: "/api/docs/redoc".to_string(),
            rapidoc_path: "/api/docs/rapidoc".to_string(),
        }
    }
}

/// Create the OpenAPI documentation.
pub fn api_doc() -> utoipa::openapi::OpenApi {
    let mut doc = OpenApiBuilder::new()
        .info(
            InfoBuilder::new()
                .title("MediaEdge API")
                .version("1.0.0")
                .description(Some(
                    "MediaEdge Media Infrastructure Platform API\n\n\
                    Provides REST and gRPC APIs for managing media streams, \
                    storage, edge nodes, replication, and media processing.",
                ))
                .build(),
        )
        .servers(
            vec![
                ServerBuilder::new()
                    .url("http://localhost:8080")
                    .description(Some("Development server"))
                    .build(),
                ServerBuilder::new()
                    .url("https://api.mediaedge.example.com")
                    .description(Some("Production server"))
                    .build(),
            ]
            .into_iter()
            .collect(),
        )
        .components(
            ComponentsBuilder::new()
                .security_scheme(
                    "bearer_auth",
                    SecurityScheme::Http(
                        HttpBuilder::new()
                            .scheme(HttpAuthScheme::Bearer)
                            .bearer_format("JWT")
                            .description(Some("JWT token from authentication service"))
                            .build(),
                    ),
                )
                .security_scheme(
                    "api_key",
                    SecurityScheme::ApiKey(utoipa::openapi::security::ApiKey::Header(
                        utoipa::openapi::security::ApiKeyValue::new("X-API-Key"),
                    )),
                )
                .build(),
        )
        .security(vec![
            [("bearer_auth", vec![])].into_iter().collect(),
            [("api_key", vec![])].into_iter().collect(),
        ])
        .build();

    // Add tags
    doc.tags = Some(vec![
        utoipa::openapi::Tag::new("streams", "Stream management"),
        utoipa::openapi::Tag::new("ingest", "Ingest protocols (RTMP, SRT, RIST, WHIP)"),
        utoipa::openapi::Tag::new("egress", "Egress protocols (HLS, DASH, WHEP, MoQ)"),
        utoipa::openapi::Tag::new("storage", "Storage and archive management"),
        utoipa::openapi::Tag::new("edge", "Edge node and CDN management"),
        utoipa::openapi::Tag::new("replication", "Replication and cluster coordination"),
        utoipa::openapi::Tag::new("transcoding", "Transcoding and media processing"),
        utoipa::openapi::Tag::new("cluster", "Cluster management and gossip"),
        utoipa::openapi::Tag::new("config", "Configuration and hot-reload"),
        utoipa::openapi::Tag::new("metrics", "Metrics and observability"),
        utoipa::openapi::Tag::new("admin", "Administrative operations"),
    ]);

    doc
}

/// Create OpenAPI router with Swagger UI, ReDoc, and RapiDoc.
pub fn create_openapi_router(config: &OpenApiConfig) -> axum::Router {
    let mut router = axum::Router::new();

    // OpenAPI JSON endpoint
    router = router.route(
        "/api/openapi.json",
        axum::routing::get(|| async { axum::Json(api_doc()) }),
    );

    // Swagger UI
    if config.swagger_ui {
        router =
            router.merge(SwaggerUi::new(&config.swagger_path).url("/api/openapi.json", api_doc()));
    }

    // ReDoc
    if config.redoc {
        router = router.merge(Redoc::with_url(&config.redoc_path, api_doc()));
    }

    // RapiDoc
    if config.rapidoc {
        router = router.merge(RapiDoc::new(&config.rapidoc_path).url("/api/openapi.json"));
    }

    router
}

/// API version enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ApiVersion {
    V1,
    V2,
}

impl ApiVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiVersion::V1 => "v1",
            ApiVersion::V2 => "v2",
        }
    }

    pub fn from_header(header: &str) -> Option<Self> {
        match header.to_lowercase().as_str() {
            "v1" | "1" => Some(ApiVersion::V1),
            "v2" | "2" => Some(ApiVersion::V2),
            _ => None,
        }
    }
}

impl Default for ApiVersion {
    fn default() -> Self {
        ApiVersion::V1
    }
}

// ============================================================================
// Common Types
// ============================================================================

/// Standard API error response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub request_id: Option<String>,
}

/// Pagination parameters.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginationParams {
    /// Page number (1-indexed).
    #[serde(default = "default_page")]
    #[schema(minimum = 1)]
    pub page: u32,

    /// Page size.
    #[serde(default = "default_page_size")]
    #[schema(minimum = 1, maximum = 1000)]
    pub per_page: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    50
}

/// Paginated response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub pagination: PaginationInfo,
}

/// Pagination info.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginationInfo {
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
    pub total_pages: u32,
}

/// Stream key for authentication.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StreamKey {
    pub key: String,
    pub name: String,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub permissions: Vec<String>,
}

// ============================================================================
// Stream Management
// ============================================================================

/// Stream status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum StreamStatus {
    Offline,
    Starting,
    Live,
    Ending,
    Error,
}

/// Stream health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum StreamHealth {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Stream information.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Stream {
    pub id: String,
    pub name: String,
    pub status: StreamStatus,
    pub health: StreamHealth,
    pub ingest_protocol: Option<IngestProtocol>,
    pub egress_protocols: Vec<EgressProtocol>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
    pub viewer_count: u64,
    pub bitrate_bps: u64,
    pub resolution: Option<String>,
    pub codec: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Ingest protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum IngestProtocol {
    Rtmp,
    Srt,
    Rist,
    Whip,
    Rtsp,
}

/// Egress protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum EgressProtocol {
    Hls,
    Dash,
    Whep,
    Moq,
    Ws,
    Rtmp,
    Srt,
    Rtsp,
}

/// Create stream request.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateStreamRequest {
    pub name: String,
    pub ingest_protocol: IngestProtocol,
    pub egress_protocols: Vec<EgressProtocol>,
    pub stream_key: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
    pub recording: Option<RecordingConfig>,
    pub transcoding: Option<TranscodingConfig>,
}

/// Update stream request.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateStreamRequest {
    pub name: Option<String>,
    pub egress_protocols: Option<Vec<EgressProtocol>>,
    pub metadata: Option<HashMap<String, String>>,
    pub recording: Option<RecordingConfig>,
    pub transcoding: Option<TranscodingConfig>,
}

/// Recording configuration.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RecordingConfig {
    pub enabled: bool,
    pub format: String,
    pub segment_duration_secs: u32,
    pub retention_days: u32,
    pub storage_path: Option<String>,
}

/// Transcoding configuration.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TranscodingConfig {
    pub enabled: bool,
    pub presets: Vec<String>,
    pub hwaccel: Option<String>,
}

// ============================================================================
// Ingest API
// ============================================================================

/// RTMP ingest info.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RtmpIngestInfo {
    pub server_url: String,
    pub stream_key: String,
    pub app_name: String,
}

/// SRT ingest info.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SrtIngestInfo {
    pub host: String,
    pub port: u16,
    pub stream_id: String,
    pub latency_ms: u32,
    pub passphrase: Option<String>,
}

/// RIST ingest info.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RistIngestInfo {
    pub host: String,
    pub port: u16,
    pub stream_id: String,
}

/// WHIP ingest info.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WhipIngestInfo {
    pub endpoint: String,
    pub stun_servers: Vec<String>,
    pub turn_servers: Vec<TurnServer>,
}

/// TURN server info.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TurnServer {
    pub urls: Vec<String>,
    pub username: String,
    pub credential: String,
}

/// Ingest info response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "protocol")]
pub enum IngestInfo {
    #[serde(rename = "rtmp")]
    Rtmp(RtmpIngestInfo),
    #[serde(rename = "srt")]
    Srt(SrtIngestInfo),
    #[serde(rename = "rist")]
    Rist(RistIngestInfo),
    #[serde(rename = "whip")]
    Whip(WhipIngestInfo),
}

// ============================================================================
// Egress API
// ============================================================================

/// HLS playback info.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HlsPlaybackInfo {
    pub master_playlist_url: String,
    pub variant_playlists: Vec<HlsVariant>,
}

/// HLS variant.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HlsVariant {
    pub resolution: String,
    pub bitrate_kbps: u32,
    pub codec: String,
    pub playlist_url: String,
}

/// DASH playback info.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DashPlaybackInfo {
    pub mpd_url: String,
    pub representations: Vec<DashRepresentation>,
}

/// DASH representation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DashRepresentation {
    pub id: String,
    pub resolution: String,
    pub bitrate_kbps: u32,
    pub codec: String,
    pub mime_type: String,
}

/// WHEP playback info.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WhepPlaybackInfo {
    pub endpoint: String,
    pub stun_servers: Vec<String>,
    pub turn_servers: Vec<TurnServer>,
}

/// MoQ playback info.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MoqPlaybackInfo {
    pub endpoint: String,
    pub track_namespace: String,
}

/// Egress info response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "protocol")]
pub enum EgressInfo {
    #[serde(rename = "hls")]
    Hls(HlsPlaybackInfo),
    #[serde(rename = "dash")]
    Dash(DashPlaybackInfo),
    #[serde(rename = "whep")]
    Whep(WhepPlaybackInfo),
    #[serde(rename = "moq")]
    Moq(MoqPlaybackInfo),
    #[serde(rename = "ws")]
    Ws { url: String },
}

// ============================================================================
// Storage API
// ============================================================================

/// Storage object.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StorageObject {
    pub key: String,
    pub bucket: String,
    pub size: u64,
    pub content_type: String,
    pub etag: String,
    pub last_modified: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, String>,
    pub tier: StorageTier,
}

/// Storage tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum StorageTier {
    Hot,
    Warm,
    Cold,
}

/// Archive segment.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ArchiveSegment {
    pub id: String,
    pub stream_id: String,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: chrono::DateTime<chrono::Utc>,
    pub duration_secs: u32,
    pub size_bytes: u64,
    pub storage_path: String,
    pub tier: StorageTier,
}

/// Storage usage.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StorageUsage {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub by_tier: HashMap<StorageTier, TierUsage>,
}

/// Tier usage.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TierUsage {
    pub used_bytes: u64,
    pub object_count: u64,
}

// ============================================================================
// Edge API
// ============================================================================

/// Edge node info.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EdgeNode {
    pub id: String,
    pub name: String,
    pub region: String,
    pub status: NodeStatus,
    pub capacity: EdgeCapacity,
    pub cache_stats: CacheStats,
    pub last_seen: chrono::DateTime<chrono::Utc>,
}

/// Node status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    Online,
    Offline,
    Degraded,
    Maintenance,
}

/// Edge capacity.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EdgeCapacity {
    pub max_viewers: u64,
    pub max_bandwidth_mbps: u64,
    pub current_viewers: u64,
    pub current_bandwidth_mbps: u64,
}

/// Cache statistics.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CacheStats {
    pub hit_ratio: f64,
    pub size_bytes: u64,
    pub entry_count: u64,
    pub eviction_count: u64,
}

/// Signed URL request.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SignedUrlRequest {
    pub stream_id: String,
    pub protocol: EgressProtocol,
    pub ttl_secs: u64,
    pub ip_whitelist: Option<Vec<String>>,
    pub geo_whitelist: Option<Vec<String>>,
}

/// Signed URL response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SignedUrlResponse {
    pub url: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub signature: String,
}

// ============================================================================
// Replication API
// ============================================================================

/// Replication task.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReplicationTask {
    pub id: String,
    pub stream_id: String,
    pub source_node: String,
    pub target_nodes: Vec<String>,
    pub status: ReplicationStatus,
    pub progress_pct: f64,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
}

/// Replication status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReplicationStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Replication plan.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReplicationPlan {
    pub stream_id: String,
    pub min_replicas: usize,
    pub max_replicas: usize,
    pub current_replicas: usize,
    pub target_nodes: Vec<String>,
    pub region_preferences: Vec<String>,
    pub anti_affinity: bool,
}

// ============================================================================
// Transcoding API
// ============================================================================

/// Transcode job.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TranscodeJob {
    pub id: String,
    pub stream_id: String,
    pub input_path: String,
    pub output_paths: Vec<String>,
    pub preset: String,
    pub status: TranscodeStatus,
    pub progress_pct: f64,
    pub priority: JobPriority,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
}

/// Transcode status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TranscodeStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Job priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum JobPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Bitrate preset.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BitratePreset {
    pub name: String,
    pub resolution: String,
    pub video_bitrate_kbps: u32,
    pub audio_bitrate_kbps: u32,
    pub framerate: u32,
    pub video_codec: String,
    pub audio_codec: String,
    pub hwaccel: Option<String>,
}

// ============================================================================
// Cluster API
// ============================================================================

/// Cluster node.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClusterNode {
    pub id: String,
    pub name: String,
    pub role: NodeRole,
    pub status: NodeStatus,
    pub address: String,
    pub capacity: ClusterCapacity,
    pub metadata: HashMap<String, String>,
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
}

/// Node role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    Gateway,
    Storage,
    Edge,
    ReplicationCoordinator,
    MediaProcessor,
}

/// Cluster capacity.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClusterCapacity {
    pub cpu_cores: u32,
    pub memory_gb: u64,
    pub disk_gb: u64,
    pub network_gbps: u64,
    pub gpu_count: u32,
    pub current_load: f64,
}

/// Broadcast ownership.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BroadcastOwnership {
    pub stream_id: String,
    pub owner_node: String,
    pub lease_expires: chrono::DateTime<chrono::Utc>,
    pub replicas: Vec<String>,
}

// ============================================================================
// Config API
// ============================================================================

/// Config reload request.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigReloadRequest {
    pub force: bool,
}

/// Config reload response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigReloadResponse {
    pub status: String,
    pub version: u64,
    pub changed: bool,
    pub message: Option<String>,
}

/// Config version response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigVersionResponse {
    pub version: u64,
    pub source: Option<String>,
    pub loaded_at: chrono::DateTime<chrono::Utc>,
    pub service: String,
}

// ============================================================================
// Metrics API
// ============================================================================

/// SLO status.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SloStatus {
    pub overall_status: SloOverallStatus,
    pub transports: HashMap<String, TransportSloStatus>,
    pub evaluated_at: chrono::DateTime<chrono::Utc>,
}

/// SLO overall status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SloOverallStatus {
    Healthy,
    Warning,
    Critical,
}

/// Transport SLO status.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransportSloStatus {
    pub transport: String,
    pub target_ms: u64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub status: SloTransportStatus,
    pub error_budget_remaining: f64,
    pub sample_count: u64,
}

/// SLO transport status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SloTransportStatus {
    Healthy,
    Warning,
    Critical,
}

/// Client sample request.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClientSampleRequest {
    pub transport: String,
    pub latency: DurationSecs,
    pub session_id: String,
    pub client_version: Option<String>,
    pub player_type: Option<String>,
    pub quality: Option<String>,
    pub region: Option<String>,
}

/// Duration in seconds (for JSON serialization).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct DurationSecs(pub f64);

impl From<std::time::Duration> for DurationSecs {
    fn from(d: std::time::Duration) -> Self {
        DurationSecs(d.as_secs_f64())
    }
}

impl From<DurationSecs> for std::time::Duration {
    fn from(d: DurationSecs) -> Self {
        std::time::Duration::from_secs_f64(d.0)
    }
}

// ============================================================================
// Admin API
// ============================================================================

/// System health.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SystemHealth {
    pub status: HealthStatus,
    pub components: HashMap<String, ComponentHealth>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Component health.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ComponentHealth {
    pub status: HealthStatus,
    pub message: String,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

/// Tenant info.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub status: TenantStatus,
    pub quotas: TenantQuotas,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Tenant status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TenantStatus {
    Active,
    Suspended,
    Deleted,
}

/// Tenant quotas.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TenantQuotas {
    pub max_streams: u32,
    pub max_viewers: u64,
    pub max_bandwidth_mbps: u64,
    pub max_storage_gb: u64,
    pub max_transcode_jobs: u32,
}

// ============================================================================
// Path/Query Parameters
// ============================================================================

/// Stream ID path parameter.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StreamIdPath {
    #[schema(pattern = "^[a-zA-Z0-9_-]+$")]
    pub stream_id: String,
}

/// Node ID path parameter.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NodeIdPath {
    #[schema(pattern = "^[a-zA-Z0-9_-]+$")]
    pub node_id: String,
}

/// Tenant ID path parameter.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TenantIdPath {
    #[schema(pattern = "^[a-zA-Z0-9_-]+$")]
    pub tenant_id: String,
}

/// Time range query.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TimeRangeQuery {
    pub start: Option<chrono::DateTime<chrono::Utc>>,
    pub end: Option<chrono::DateTime<chrono::Utc>>,
}

/// Metrics query.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MetricsQuery {
    pub metric: String,
    pub aggregation: Option<String>, // avg, sum, min, max, p50, p95, p99
    pub interval: Option<String>,    // 1m, 5m, 1h, 1d
    #[serde(flatten)]
    pub time_range: TimeRangeQuery,
}
