//! Ingest module - unified entry point for all ingest protocols.

pub mod rtmp;
pub mod srt;
pub mod rist;
pub mod whip;
pub mod rtsp;

use std::sync::Arc;

use crate::fragment::FragmentBroadcasterRegistry;
use mediaedge_config::types::GatewayConfig;
use mediaedge_observability::MetricsRegistry;

/// Start all configured ingest servers.
pub async fn start_all_ingest_servers(
    config: &GatewayConfig,
    registry: Arc<FragmentBroadcasterRegistry>,
    metrics: Arc<MetricsRegistry>,
) -> anyhow::Result<()> {
    // RTMP
    if let Some(rtmp_config) = &config.protocols.rtmp {
        if rtmp_config.enabled {
            let registry = registry.clone();
            let metrics = metrics.clone();
            let port = rtmp_config.port;
            tokio::spawn(async move {
                if let Err(e) = rtmp::run_rtmp_server(port, registry, metrics).await {
                    tracing::error!("RTMP server error: {}", e);
                }
            });
            tracing::info!("RTMP ingest server started on port {}", port);
        }
    }

    // SRT
    if let Some(srt_config) = &config.protocols.srt {
        if srt_config.enabled {
            let registry = registry.clone();
            let metrics = metrics.clone();
            let port = srt_config.port;
            tokio::spawn(async move {
                if let Err(e) = srt::run_srt_server(port, registry, metrics).await {
                    tracing::error!("SRT server error: {}", e);
                }
            });
            tracing::info!("SRT ingest server started on port {}", port);
        }
    }

    // RIST
    if let Some(rist_config) = &config.protocols.rist {
        if rist_config.enabled {
            let registry = registry.clone();
            let metrics = metrics.clone();
            let port = rist_config.port;
            tokio::spawn(async move {
                if let Err(e) = rist::run_rist_server(port, registry, metrics).await {
                    tracing::error!("RIST server error: {}", e);
                }
            });
            tracing::info!("RIST ingest server started on port {}", port);
        }
    }

    // WHIP (handled via HTTP in gateway)
    if let Some(whip_config) = &config.protocols.whip {
        if whip_config.enabled {
            tracing::info!("WHIP ingest enabled at path: {}", whip_config.path);
        }
    }

    // RTSP (pull mode - configured separately)
    // RTSP pull clients are started on-demand via API

    Ok(())
}

/// Ingest protocol type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IngestProtocol {
    Rtmp,
    Srt,
    Rist,
    Whip,
    Rtsp,
}

impl IngestProtocol {
    /// Get default port for protocol.
    pub fn default_port(&self) -> u16 {
        match self {
            IngestProtocol::Rtmp => 1935,
            IngestProtocol::Srt => 8888,
            IngestProtocol::Rist => 5000,
            IngestProtocol::Whip => 443, // HTTPS
            IngestProtocol::Rtsp => 554,
        }
    }

    /// Get protocol name.
    pub fn name(&self) -> &'static str {
        match self {
            IngestProtocol::Rtmp => "RTMP",
            IngestProtocol::Srt => "SRT",
            IngestProtocol::Rist => "RIST",
            IngestProtocol::Whip => "WHIP",
            IngestProtocol::Rtsp => "RTSP",
        }
    }
}

/// Ingest stream info.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IngestStreamInfo {
    pub stream_id: String,
    pub protocol: IngestProtocol,
    pub status: IngestStatus,
    pub peer_addr: Option<String>,
    pub bytes_received: u64,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub bitrate_bps: u64,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution: Option<String>,
}

/// Ingest status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IngestStatus {
    Connecting,
    Connected,
    Streaming,
    Reconnecting,
    Disconnected,
    Error,
}

/// Ingest manager for dynamic stream handling.
pub struct IngestManager {
    registry: Arc<FragmentBroadcasterRegistry>,
    metrics: Arc<MetricsRegistry>,
    active_streams: dashmap::DashMap<String, IngestStreamInfo>,
}

impl IngestManager {
    pub fn new(registry: Arc<FragmentBroadcasterRegistry>, metrics: Arc<MetricsRegistry>) -> Self {
        Self {
            registry,
            metrics,
            active_streams: dashmap::DashMap::new(),
        }
    }

    /// Register a new ingest stream.
    pub fn register_stream(&self, stream_id: String, protocol: IngestProtocol, peer_addr: Option<String>) {
        let info = IngestStreamInfo {
            stream_id: stream_id.clone(),
            protocol,
            status: IngestStatus::Connecting,
            peer_addr,
            bytes_received: 0,
            started_at: chrono::Utc::now(),
            bitrate_bps: 0,
            video_codec: None,
            audio_codec: None,
            resolution: None,
        };
        self.active_streams.insert(stream_id, info);
        self.metrics.counter("ingest_streams_total", "Total ingest streams").increment(1);
    }

    /// Update stream status.
    pub fn update_status(&self, stream_id: &str, status: IngestStatus) {
        if let Some(mut info) = self.active_streams.get_mut(stream_id) {
            info.status = status;
        }
    }

    /// Update stream stats.
    pub fn update_stats(&self, stream_id: &str, bytes: u64, bitrate: u64) {
        if let Some(mut info) = self.active_streams.get_mut(stream_id) {
            info.bytes_received += bytes;
            info.bitrate_bps = bitrate;
        }
    }

    /// Remove a stream.
    pub fn remove_stream(&self, stream_id: &str) {
        self.active_streams.remove(stream_id);
        self.metrics.counter("ingest_active_streams", "Active ingest streams").decrement(1);
    }

    /// Get stream info.
    pub fn get_stream(&self, stream_id: &str) -> Option<IngestStreamInfo> {
        self.active_streams.get(stream_id).map(|v| v.clone())
    }

    /// List all active streams.
    pub fn list_streams(&self) -> Vec<IngestStreamInfo> {
        self.active_streams.iter().map(|v| v.clone()).collect()
    }

    /// Get stream count.
    pub fn stream_count(&self) -> usize {
        self.active_streams.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ingest_protocol_ports() {
        assert_eq!(IngestProtocol::Rtmp.default_port(), 1935);
        assert_eq!(IngestProtocol::Srt.default_port(), 8888);
        assert_eq!(IngestProtocol::Rist.default_port(), 5000);
        assert_eq!(IngestProtocol::Whip.default_port(), 443);
        assert_eq!(IngestProtocol::Rtsp.default_port(), 554);
    }

    #[test]
    fn test_ingest_protocol_names() {
        assert_eq!(IngestProtocol::Rtmp.name(), "RTMP");
        assert_eq!(IngestProtocol::Srt.name(), "SRT");
        assert_eq!(IngestProtocol::Rist.name(), "RIST");
        assert_eq!(IngestProtocol::Whip.name(), "WHIP");
        assert_eq!(IngestProtocol::Rtsp.name(), "RTSP");
    }

    #[test]
    fn test_ingest_manager() {
        let registry = Arc::new(crate::fragment::FragmentBroadcasterRegistry::new());
        let metrics = Arc::new(mediaedge_observability::MetricsRegistry::new(&Default::default()).unwrap());
        let manager = IngestManager::new(registry, metrics);

        manager.register_stream("test-stream".to_string(), IngestProtocol::Rtmp, Some("192.168.1.1:1234".to_string()));
        
        assert_eq!(manager.stream_count(), 1);
        assert_eq!(manager.get_stream("test-stream").unwrap().protocol, IngestProtocol::Rtmp);
        
        manager.update_status("test-stream", IngestStatus::Streaming);
        assert_eq!(manager.get_stream("test-stream").unwrap().status, IngestStatus::Streaming);
        
        manager.remove_stream("test-stream");
        assert_eq!(manager.stream_count(), 0);
    }
}