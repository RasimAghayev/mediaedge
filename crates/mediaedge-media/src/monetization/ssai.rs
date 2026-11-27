//! SSAI (Server-Side Ad Insertion) + Ad Server Integration
//!
//! AWS MediaTailor, Yospace, Broadpeak, Google Ad Manager, Prebid.js header bidding.
//! SCTE-35 marker-based ad insertion.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// SSAI provider.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SsaiProvider {
    AwsMediaTailor,
    Yospace,
    Broadpeak,
    GoogleAdManager,
    Prebid,
    Custom,
}

/// SSAI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsaiConfig {
    pub provider: SsaiProvider,
    pub endpoint: String,
    pub api_key: String,
    pub configuration_id: Option<String>, // MediaTailor configuration alias
    pub origin_server: String,            // Origin manifest URL
    pub ad_server: AdServerConfig,
    pub cdn_config: CdnConfig,
    pub slate: SlateConfig,
    pub tracking: TrackingConfig,
}

/// Ad server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdServerConfig {
    pub provider: AdServerProvider,
    pub endpoint: String,
    pub network_code: Option<String>, // Google Ad Manager
    pub api_key: String,
    pub vast_version: VastVersion,
    pub ad_pod_max_duration_seconds: u32,
    pub ad_pod_max_ads: u32,
    pub header_bidding: HeaderBiddingConfig,
}

/// Ad server providers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdServerProvider {
    GoogleAdManager,
    Yospace,
    Broadpeak,
    AwsMediaTailor,
    Prebid,
    Custom,
}

/// VAST version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VastVersion {
    Vast3,
    Vast4,
}

/// Header bidding configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderBiddingConfig {
    pub enabled: bool,
    pub prebid_config: Option<PrebidConfig>,
    pub timeout_ms: u32,
    pub bidders: Vec<BidderConfig>,
}

/// Prebid.js configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrebidConfig {
    pub endpoint: String,
    pub account_id: String,
    pub bidders: Vec<String>,
    pub price_granularity: PriceGranularity,
}

/// Price granularity for Prebid.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PriceGranularity {
    Low,
    Medium,
    High,
    Auto,
    Dense,
    Custom,
}

/// Bidder configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BidderConfig {
    pub name: String,
    pub endpoint: String,
    pub adapter: String,
    pub params: HashMap<String, serde_json::Value>,
}

/// CDN configuration for SSAI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdnConfig {
    pub enabled: bool,
    pub cdn_domain: Option<String>,
    pub manifest_manipulation: bool,
    pub segment_prefix: Option<String>,
}

/// Slate configuration (filler content).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlateConfig {
    pub enabled: bool,
    pub slate_asset_id: Option<Uuid>,
    pub slate_url: Option<String>,
    pub duration_seconds: u32,
}

/// Tracking configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingConfig {
    pub client_tracking: bool,
    pub server_tracking: bool,
    pub beacon_endpoint: Option<String>,
    pub tracking_events: Vec<TrackingEvent>,
}

/// Tracking events.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrackingEvent {
    Impression,
    Start,
    FirstQuartile,
    Midpoint,
    ThirdQuartile,
    Complete,
    Pause,
    Resume,
    Skip,
    Click,
    Close,
    Error,
}

/// Ad break opportunity (SCTE-35 signaled).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdBreakOpportunity {
    pub id: Uuid,
    pub stream_id: Uuid,
    pub tenant_id: Uuid,
    pub splice_insert: SpliceInsert,
    pub segmentation_descriptor: Option<SegmentationDescriptor>,
    pub avail_start: DateTime<Utc>,
    pub avail_duration_seconds: Option<u32>,
    pub avail_num: u8,
    pub avails_expected: u8,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Splice insert (simplified).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpliceInsert {
    pub splice_event_id: u32,
    pub out_of_network_indicator: bool,
    pub program_splice_flag: bool,
    pub duration_flag: bool,
    pub break_duration_ticks: Option<u64>, // 90kHz
    pub unique_program_id: u16,
    pub avail_num: u8,
    pub avails_expected: u8,
}

/// Segmentation descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentationDescriptor {
    pub segmentation_event_id: u32,
    pub segmentation_type_id: u8,
    pub segment_num: u8,
    pub segments_expected: u8,
}

/// Ad decision from ad server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdDecision {
    pub opportunity_id: Uuid,
    pub ads: Vec<AdCreative>,
    pub ad_pod_duration_seconds: u32,
    pub vast_xml: Option<String>,
    pub vmaps_xml: Option<String>,
    pub tracking_urls: HashMap<TrackingEvent, Vec<String>>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Ad creative.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdCreative {
    pub id: String,
    pub sequence: u32,
    pub duration_seconds: u32,
    pub media_files: Vec<AdMediaFile>,
    pub tracking_events: HashMap<TrackingEvent, Vec<String>>,
    pub clickthrough_url: Option<String>,
    pub vast_ad_id: Option<String>,
}

/// Ad media file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdMediaFile {
    pub url: String,
    pub delivery: AdDeliveryMethod,
    pub mime_type: String,
    pub bitrate: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub codec: Option<String>,
}

/// Ad delivery method.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdDeliveryMethod {
    Progressive,
    Streaming,
}

/// SSAI session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsaiSession {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub stream_id: Uuid,
    pub viewer_id: Option<Uuid>,
    pub session_token: String,
    pub manifest_url: String,
    pub ad_decisions: Vec<AdDecision>,
    pub current_ad_break: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// SSAI service trait.
#[async_trait]
pub trait SsaiProviderClient: Send + Sync {
    async fn create_session(&self, config: &SsaiConfig, stream_id: Uuid, viewer_id: Option<Uuid>) -> Result<SsaiSession>;
    async fn get_manifest(&self, session_id: Uuid, query_params: HashMap<String, String>) -> Result<String>;
    async fn report_ad_event(&self, session_id: Uuid, ad_break_id: Uuid, event: TrackingEvent, position_seconds: f64) -> Result<()>;
    async fn close_session(&self, session_id: Uuid) -> Result<()>;
}

/// Ad server client trait.
#[async_trait]
pub trait AdServerClient: Send + Sync {
    async fn request_ads(&self, opportunity: &AdBreakOpportunity, viewer_profile: &ViewerProfile) -> Result<AdDecision>;
    async fn report_impression(&self, creative_id: String, session_id: Uuid) -> Result<()>;
    async fn report_click(&self, creative_id: String, session_id: Uuid) -> Result<()>;
}

/// Viewer profile for ad targeting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerProfile {
    pub viewer_id: Option<Uuid>,
    pub tenant_id: Uuid,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub geo: Option<GeoInfo>,
    pub demographics: HashMap<String, String>,
    pub consent: ConsentFlags,
    pub custom: HashMap<String, serde_json::Value>,
}

/// Geo information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoInfo {
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub postal_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub timezone: Option<String>,
}

/// Consent flags (GDPR, CCPA, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentFlags {
    pub gdpr_consent: bool,
    pub ccpa_opt_out: bool,
    pub tcf_string: Option<String>,
    pub usp_string: Option<String>,
}

/// SSAI manager orchestrating ad insertion.
pub struct SsaiManager {
    configs: Arc<DashMap<Uuid, SsaiConfig>>, // tenant_id -> config
    sessions: Arc<DashMap<Uuid, SsaiSession>>,
    provider_clients: Arc<DashMap<SsaiProvider, Arc<dyn SsaiProviderClient>>>,
    ad_server_clients: Arc<DashMap<AdServerProvider, Arc<dyn AdServerClient>>>,
}

impl SsaiManager {
    /// Create a new SSAI manager.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            configs: Arc::new(DashMap::new()),
            sessions: Arc::new(DashMap::new()),
            provider_clients: Arc::new(DashMap::new()),
            ad_server_clients: Arc::new(DashMap::new()),
        })
    }

    /// Register SSAI provider client.
    pub fn register_provider(&self, provider: SsaiProvider, client: Arc<dyn SsaiProviderClient>) {
        self.provider_clients.insert(provider, client);
    }

    /// Register ad server client.
    pub fn register_ad_server(&self, provider: AdServerProvider, client: Arc<dyn AdServerClient>) {
        self.ad_server_clients.insert(provider, client);
    }

    /// Configure SSAI for a tenant.
    pub async fn configure_tenant(&self, tenant_id: Uuid, config: SsaiConfig) -> Result<()> {
        // Validate provider client exists
        if !self.provider_clients.contains_key(&config.provider) {
            return Err(anyhow::anyhow!("Provider client not registered: {:?}", config.provider));
        }

        self.configs.insert(tenant_id, config);
        Ok(())
    }

    /// Create SSAI session for a stream.
    pub async fn create_session(
        &self,
        tenant_id: Uuid,
        stream_id: Uuid,
        viewer_id: Option<Uuid>,
        query_params: HashMap<String, String>,
    ) -> Result<SsaiSession> {
        let config = self.configs.get(&tenant_id)
            .ok_or_else(|| anyhow::anyhow!("SSAI not configured for tenant"))?;

        let client = self.provider_clients.get(&config.provider)
            .ok_or_else(|| anyhow::anyhow!("Provider client not available"))?;

        let session = client.create_session(&config, stream_id, viewer_id).await?;
        self.sessions.insert(session.id, session.clone());

        Ok(session)
    }

    /// Get personalized manifest for session.
    pub async fn get_manifest(&self, session_id: Uuid, query_params: HashMap<String, String>) -> Result<String> {
        let session = self.sessions.get(&session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

        let config = self.configs.get(&session.tenant_id)
            .ok_or_else(|| anyhow::anyhow!("Config not found"))?;

        let client = self.provider_clients.get(&config.provider)
            .ok_or_else(|| anyhow::anyhow!("Provider client not available"))?;

        client.get_manifest(session_id, query_params).await
    }

    /// Process SCTE-35 ad break opportunity.
    pub async fn process_ad_break(
        &self,
        opportunity: AdBreakOpportunity,
        viewer_profile: ViewerProfile,
    ) -> Result<AdDecision> {
        let config = self.configs.get(&opportunity.tenant_id)
            .ok_or_else(|| anyhow::anyhow!("SSAI not configured for tenant"))?;

        // Try header bidding first
        let ad_decision = if config.ad_server.header_bidding.enabled {
            self.request_header_bidding(&config, &opportunity, &viewer_profile).await?
        } else {
            self.request_direct_ads(&config, &opportunity, &viewer_profile).await?
        };

        Ok(ad_decision)
    }

    /// Request ads via header bidding.
    async fn request_header_bidding(
        &self,
        config: &SsaiConfig,
        opportunity: &AdBreakOpportunity,
        viewer_profile: &ViewerProfile,
    ) -> Result<AdDecision> {
        // In production, would call Prebid.js server-side
        // For now, fall back to direct
        self.request_direct_ads(config, opportunity, viewer_profile).await
    }

    /// Request ads directly from ad server.
    async fn request_direct_ads(
        &self,
        config: &SsaiConfig,
        opportunity: &AdBreakOpportunity,
        viewer_profile: &ViewerProfile,
    ) -> Result<AdDecision> {
        let ad_server = self.ad_server_clients.get(&config.ad_server.provider)
            .ok_or_else(|| anyhow::anyhow!("Ad server client not available"))?;

        ad_server.request_ads(opportunity, viewer_profile).await
    }

    /// Report ad tracking event.
    pub async fn report_tracking(
        &self,
        session_id: Uuid,
        ad_break_id: Uuid,
        event: TrackingEvent,
        position_seconds: f64,
    ) -> Result<()> {
        let session = self.sessions.get(&session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

        let config = self.configs.get(&session.tenant_id)
            .ok_or_else(|| anyhow::anyhow!("Config not found"))?;

        let client = self.provider_clients.get(&config.provider)
            .ok_or_else(|| anyhow::anyhow!("Provider client not available"))?;

        client.report_ad_event(session_id, ad_break_id, event, position_seconds).await
    }

    /// Close SSAI session.
    pub async fn close_session(&self, session_id: Uuid) -> Result<()> {
        let session = self.sessions.get(&session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

        let config = self.configs.get(&session.tenant_id)
            .ok_or_else(|| anyhow::anyhow!("Config not found"))?;

        let client = self.provider_clients.get(&config.provider)
            .ok_or_else(|| anyhow::anyhow!("Provider client not available"))?;

        client.close_session(session_id).await?;
        self.sessions.remove(&session_id);

        Ok(())
    }

    /// Get active sessions for a tenant.
    pub fn get_tenant_sessions(&self, tenant_id: Uuid) -> Vec<SsaiSession> {
        self.sessions.iter()
            .filter(|s| s.tenant_id == tenant_id)
            .map(|s| s.clone())
            .collect()
    }
}

/// AWS MediaTailor client implementation (simplified).
pub struct MediaTailorClient {
    config: SsaiConfig,
    http_client: reqwest::Client,
}

impl MediaTailorClient {
    pub fn new(config: SsaiConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SsaiProviderClient for MediaTailorClient {
    async fn create_session(&self, config: &SsaiConfig, stream_id: Uuid, viewer_id: Option<Uuid>) -> Result<SsaiSession> {
        let session_token = Uuid::new_v4().to_string();
        let manifest_url = format!(
            "{}/v1/master/{}/{}?session_token={}",
            config.endpoint.trim_end_matches('/'),
            config.configuration_id.as_deref().unwrap_or("default"),
            stream_id,
            session_token
        );

        Ok(SsaiSession {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(), // Would come from context
            stream_id,
            viewer_id,
            session_token,
            manifest_url,
            ad_decisions: Vec::new(),
            current_ad_break: None,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(24),
        })
    }

    async fn get_manifest(&self, session_id: Uuid, query_params: HashMap<String, String>) -> Result<String> {
        // Proxy to MediaTailor manifest endpoint
        // For now, return placeholder
        Ok("#EXTM3U\n#EXT-X-VERSION:6\n".to_string())
    }

    async fn report_ad_event(&self, session_id: Uuid, ad_break_id: Uuid, event: TrackingEvent, position_seconds: f64) -> Result<()> {
        // Send to MediaTailor tracking endpoint
        Ok(())
    }

    async fn close_session(&self, session_id: Uuid) -> Result<()> {
        Ok(())
    }
}

/// Google Ad Manager client (simplified).
pub struct GoogleAdManagerClient {
    config: AdServerConfig,
    http_client: reqwest::Client,
}

impl GoogleAdManagerClient {
    pub fn new(config: AdServerConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl AdServerClient for GoogleAdManagerClient {
    async fn request_ads(&self, opportunity: &AdBreakOpportunity, viewer_profile: &ViewerProfile) -> Result<AdDecision> {
        // Call GAM API (DAI)
        // Return ad pod decision
        Ok(AdDecision {
            opportunity_id: opportunity.id,
            ads: Vec::new(),
            ad_pod_duration_seconds: config.ad_server.ad_pod_max_duration_seconds,
            vast_xml: None,
            vmaps_xml: None,
            tracking_urls: HashMap::new(),
            metadata: HashMap::new(),
        })
    }

    async fn report_impression(&self, creative_id: String, session_id: Uuid) -> Result<()> {
        Ok(())
    }

    async fn report_click(&self, creative_id: String, session_id: Uuid) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssai_config() {
        let config = SsaiConfig {
            provider: SsaiProvider::AwsMediaTailor,
            endpoint: "https://mediatailor.us-east-1.amazonaws.com".to_string(),
            api_key: "test".to_string(),
            configuration_id: Some("my-config".to_string()),
            origin_server: "https://origin.example.com".to_string(),
            ad_server: AdServerConfig {
                provider: AdServerProvider::GoogleAdManager,
                endpoint: "https://admanager.google.com".to_string(),
                network_code: Some("123456".to_string()),
                api_key: "test".to_string(),
                vast_version: VastVersion::Vast4,
                ad_pod_max_duration_seconds: 120,
                ad_pod_max_ads: 4,
                header_bidding: HeaderBiddingConfig {
                    enabled: false,
                    prebid_config: None,
                    timeout_ms: 500,
                    bidders: Vec::new(),
                },
            },
            cdn_config: CdnConfig {
                enabled: true,
                cdn_domain: Some("cdn.example.com".to_string()),
                manifest_manipulation: true,
                segment_prefix: None,
            },
            slate: SlateConfig {
                enabled: true,
                slate_asset_id: None,
                slate_url: Some("https://slate.example.com/slate.mp4".to_string()),
                duration_seconds: 30,
            },
            tracking: TrackingConfig {
                client_tracking: true,
                server_tracking: true,
                beacon_endpoint: Some("https://tracking.example.com/beacon".to_string()),
                tracking_events: vec![
                    TrackingEvent::Impression,
                    TrackingEvent::Start,
                    TrackingEvent::Complete,
                ],
            },
        };

        assert_eq!(config.provider, SsaiProvider::AwsMediaTailor);
        assert!(config.ad_server.header_bidding.enabled == false);
    }
}