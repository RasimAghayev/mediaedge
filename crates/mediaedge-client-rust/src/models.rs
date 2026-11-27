use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Stream models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stream {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub status: StreamStatus,
    pub tenant_id: Uuid,
    pub ingest_protocol: IngestProtocol,
    pub ingest_url: String,
    pub playback_urls: PlaybackUrls,
    pub transcoding_profile: Option<String>,
    pub recording_enabled: bool,
    pub dvr_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StreamStatus {
    Draft,
    Starting,
    Live,
    Stopping,
    Stopped,
    Error,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum IngestProtocol {
    Rtmp,
    Srt,
    Rist,
    Whip,
    Rtsp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackUrls {
    pub hls: Option<String>,
    pub dash: Option<String>,
    pub whep: Option<String>,
    pub moq: Option<String>,
    pub websocket_fmp4: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStreamRequest {
    pub name: String,
    pub slug: Option<String>,
    pub ingest_protocol: IngestProtocol,
    pub transcoding_profile: Option<String>,
    pub recording_enabled: Option<bool>,
    pub dvr_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStreamRequest {
    pub name: Option<String>,
    pub transcoding_profile: Option<String>,
    pub recording_enabled: Option<bool>,
    pub dvr_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMetrics {
    pub stream_id: Uuid,
    pub viewers_current: u64,
    pub viewers_peak: u64,
    pub bitrate_in: u64,
    pub bitrate_out: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub packets_lost: u64,
    pub rtt_ms: Option<f64>,
    pub timestamp: DateTime<Utc>,
}

/// Cluster models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    pub id: Uuid,
    pub name: String,
    pub endpoint: String,
    pub region: String,
    pub zone: String,
    pub capacity: NodeCapacity,
    pub load: NodeLoad,
    pub status: NodeStatus,
    pub roles: Vec<NodeRole>,
    pub last_heartbeat: DateTime<Utc>,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapacity {
    pub cpu_cores: u32,
    pub memory_gb: u32,
    pub storage_gb: u64,
    pub network_gbps: u32,
    pub max_streams: u32,
    pub max_viewers: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLoad {
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub storage_percent: f32,
    pub network_percent: f32,
    pub active_streams: u32,
    pub active_viewers: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Draining,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    Ingest,
    Transcode,
    Edge,
    Origin,
    Storage,
    Gateway,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Broadcast {
    pub id: Uuid,
    pub stream_id: Uuid,
    pub owner_node_id: Uuid,
    pub status: BroadcastStatus,
    pub viewers: u64,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BroadcastStatus {
    Active,
    Transferring,
    Failed,
}

/// Tenant models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub status: TenantStatus,
    pub quotas: TenantQuotas,
    pub branding: Option<TenantBranding>,
    pub settings: TenantSettings,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TenantStatus {
    Active,
    Trial,
    Suspended,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantQuotas {
    pub max_streams: u32,
    pub max_concurrent_viewers: u64,
    pub max_storage_gb: u64,
    pub max_bandwidth_gbps: u32,
    pub max_transcoding_hours: u32,
    pub custom_domains: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantBranding {
    pub logo_url: Option<String>,
    pub favicon_url: Option<String>,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub custom_css: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantSettings {
    pub allowed_protocols: Vec<IngestProtocol>,
    pub default_transcoding_profile: Option<String>,
    pub recording_enabled: bool,
    pub dvr_enabled: bool,
    pub geo_blocking: Option<GeoBlocking>,
    pub drm_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoBlocking {
    pub allowed_countries: Option<Vec<String>>,
    pub blocked_countries: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTenantRequest {
    pub name: String,
    pub slug: Option<String>,
    pub quotas: Option<TenantQuotas>,
    pub branding: Option<TenantBranding>,
    pub settings: Option<TenantSettings>,
}

/// Monetization models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastChannel {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub slug: String,
    pub status: FastChannelStatus,
    pub schedule: FastChannelSchedule,
    pub ad_breaks: AdBreakConfig,
    pub output_formats: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FastChannelStatus {
    Draft,
    Active,
    Paused,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastChannelSchedule {
    pub timezone: String,
    pub entries: Vec<ScheduleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub content_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub duration_seconds: u32,
    pub repeat: Option<RepeatRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepeatRule {
    pub frequency: RepeatFrequency,
    pub interval: u32,
    pub until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RepeatFrequency {
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdBreakConfig {
    pub enabled: bool,
    pub provider: AdProvider,
    pub tag_url: Option<String>,
    pub min_interval_seconds: u32,
    pub max_duration_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AdProvider {
    MediaTailor,
    Yospace,
    Broadpeak,
    GoogleAdManager,
    Prebid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvodProduct {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub poster_url: Option<String>,
    pub trailer_url: Option<String>,
    pub content_id: Uuid,
    pub rental_price_cents: Option<u32>,
    pub purchase_price_cents: Option<u32>,
    pub rental_duration_hours: Option<u32>,
    pub purchase_availability: Option<AvailabilityWindow>,
    pub rental_availability: Option<AvailabilityWindow>,
    pub territories: Vec<String>,
    pub status: TvodStatus,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TvodStatus {
    Draft,
    Active,
    Hidden,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailabilityWindow {
    pub start: DateTime<Utc>,
    pub end: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub bundle_type: BundleType,
    pub pricing: BundlePricing,
    pub included_products: Vec<BundleProduct>,
    pub partner_validation: Option<PartnerValidation>,
    pub status: BundleStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BundleType {
    StreamerBundle,
    TelcoBundle,
    AggregatorBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundlePricing {
    pub base_price_cents: u32,
    pub billing_cycle: BillingCycle,
    pub trial_days: Option<u32>,
    pub setup_fee_cents: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BillingCycle {
    Monthly,
    Quarterly,
    Yearly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleProduct {
    pub product_id: Uuid,
    pub product_type: ProductType,
    pub quantity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProductType {
    FastChannel,
    TvodProduct,
    LiveStream,
    Storage,
    Transcoding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartnerValidation {
    pub partner_id: String,
    pub validation_type: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BundleStatus {
    Draft,
    Active,
    Paused,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementCheck {
    pub user_id: Uuid,
    pub content_id: Uuid,
    pub content_type: ContentType,
    pub grant_type: Option<GrantType>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    Stream,
    Vod,
    FastChannel,
    Bundle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GrantType {
    Subscription,
    Rental,
    Purchase,
    Bundle,
    Promo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementResult {
    pub granted: bool,
    pub reason: Option<String>,
    pub entitlement: Option<Entitlement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entitlement {
    pub id: Uuid,
    pub user_id: Uuid,
    pub content_id: Uuid,
    pub content_type: ContentType,
    pub grant_type: GrantType,
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Billing models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub customer_id: Uuid,
    pub amount_cents: u64,
    pub currency: String,
    pub status: InvoiceStatus,
    pub line_items: Vec<InvoiceLineItem>,
    pub created_at: DateTime<Utc>,
    pub due_date: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum InvoiceStatus {
    Draft,
    Open,
    Paid,
    Void,
    Uncollectible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceLineItem {
    pub description: String,
    pub quantity: u32,
    pub unit_price_cents: u64,
    pub amount_cents: u64,
}

/// Auth models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub roles: Vec<String>,
    pub mfa_enabled: bool,
    pub status: UserStatus,
    pub tenant_id: Uuid,
    pub tenant_name: String,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    Active,
    Invited,
    Suspended,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub mfa_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: User,
    pub tenant: Tenant,
    pub requires_mfa: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteUserRequest {
    pub email: String,
    pub name: String,
    pub roles: Vec<String>,
    pub tenant_id: Uuid,
}

/// Metrics models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsQuery {
    pub metric: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub step: Option<String>,
    pub labels: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResult {
    pub metric: String,
    pub data: Vec<MetricsDataPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsDataPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub labels: serde_json::Value,
}

/// Pagination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub search: Option<String>,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    Desc,
}