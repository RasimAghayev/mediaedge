//! MediaEdge Media Crate
//!
//! Core media processing functionality:
//! - Protocol ingest (RTMP, SRT, RIST, WHIP, RTSP)
//! - Protocol egress (HLS, DASH, WHEP, MoQ, WebSocket fMP4)
//! - Transcoding pipeline (FFmpeg-next with HW accel)
//! - Fragment broadcaster registry (unified fragment model)
//! - Monetization (SSAI, Ad Server) - Phase 5
//! - AI Content Moderation - Phase 5

#![warn(missing_docs)]

pub mod ingest;
pub mod egress;
pub mod transcode;
pub mod fragment;
pub mod monetization;

use mediaedge_config::types::GatewayConfig;
use mediaedge_observability::MetricsRegistry;

/// Media service configuration.
#[derive(Debug, Clone)]
pub struct MediaConfig {
    pub gateway: GatewayConfig,
}

/// Main media service entry point.
pub struct MediaService {
    config: MediaConfig,
    metrics: std::sync::Arc<MetricsRegistry>,
    fragment_registry: std::sync::Arc<fragment::FragmentBroadcasterRegistry>,
    egress_server: std::sync::Arc<egress::EgressServer>,
    monetization: std::sync::Arc<monetization::MonetizationManager>,
}

impl MediaService {
    /// Create a new media service.
    pub fn new(config: MediaConfig, metrics: std::sync::Arc<MetricsRegistry>) -> Self {
        let fragment_registry = std::sync::Arc::new(fragment::FragmentBroadcasterRegistry::new());
        let egress_server = std::sync::Arc::new(egress::EgressServer::new(
            config.gateway.protocols.http.clone().unwrap_or_default(),
            fragment_registry.clone(),
            metrics.clone(),
            egress::EgressConfig::default(),
        ));

        let monetization = std::sync::Arc::new(monetization::MonetizationManager::new());

        Self {
            config,
            metrics,
            fragment_registry,
            egress_server,
            monetization,
        }
    }

    /// Get the fragment registry.
    pub fn fragment_registry(&self) -> std::sync::Arc<fragment::FragmentBroadcasterRegistry> {
        self.fragment_registry.clone()
    }

    /// Get metrics registry.
    pub fn metrics(&self) -> std::sync::Arc<MetricsRegistry> {
        self.metrics.clone()
    }

    /// Get egress server.
    pub fn egress_server(&self) -> std::sync::Arc<egress::EgressServer> {
        self.egress_server.clone()
    }

    /// Get monetization manager.
    pub fn monetization(&self) -> std::sync::Arc<monetization::MonetizationManager> {
        self.monetization.clone()
    }

    /// Start all ingest servers.
    pub async fn start_ingest_servers(&self) -> anyhow::Result<()> {
        let rtmp_config = self.config.gateway.protocols.rtmp.as_ref();
        let srt_config = self.config.gateway.protocols.srt.as_ref();
        let rist_config = self.config.gateway.protocols.rist.as_ref();
        let whip_config = self.config.gateway.protocols.whip.as_ref();

        // Start RTMP ingest
        if let Some(cfg) = rtmp_config {
            if cfg.enabled {
                let registry = self.fragment_registry.clone();
                let metrics = self.metrics.clone();
                let port = cfg.port;
                tokio::spawn(async move {
                    if let Err(e) = ingest::rtmp::run_rtmp_server(port, registry, metrics).await {
                        tracing::error!("RTMP server error: {}", e);
                    }
                });
                tracing::info!("RTMP ingest server started on port {}", port);
            }
        }

        // Start SRT ingest
        if let Some(cfg) = srt_config {
            if cfg.enabled {
                let registry = self.fragment_registry.clone();
                let metrics = self.metrics.clone();
                let port = cfg.port;
                tokio::spawn(async move {
                    if let Err(e) = ingest::srt::run_srt_server(port, registry, metrics).await {
                        tracing::error!("SRT server error: {}", e);
                    }
                });
                tracing::info!("SRT ingest server started on port {}", port);
            }
        }

        // Start RIST ingest
        if let Some(cfg) = rist_config {
            if cfg.enabled {
                let registry = self.fragment_registry.clone();
                let metrics = self.metrics.clone();
                let port = cfg.port;
                tokio::spawn(async move {
                    if let Err(e) = ingest::rist::run_rist_server(port, registry, metrics).await {
                        tracing::error!("RIST server error: {}", e);
                    }
                });
                tracing::info!("RIST ingest server started on port {}", port);
            }
        }

        // Start WHIP ingest (handled via HTTP in gateway)
        if let Some(cfg) = whip_config {
            if cfg.enabled {
                tracing::info!("WHIP ingest enabled at path: {}", cfg.path);
            }
        }

        Ok(())
    }

    /// Start egress for a stream.
    pub async fn start_egress(&self, stream_id: fragment::StreamId) -> anyhow::Result<()> {
        self.egress_server.start_stream(stream_id).await
    }

    /// Stop egress for a stream.
    pub fn stop_egress(&self, stream_id: &fragment::StreamId) {
        self.egress_server.stop_stream(stream_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_config() {
        // Placeholder test
    }
}