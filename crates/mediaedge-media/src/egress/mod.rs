//! Egress Module - Unified interface for all egress protocols
//!
//! This module provides a unified interface for running multiple egress protocols
//! simultaneously for a single stream.

pub mod hls;
pub mod dash;
pub mod whep;
pub mod moq;
pub mod ws_fmp4;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::fragment::{FragmentBroadcasterRegistry, StreamId};
use mediaedge_config::types::HttpProtocolConfig;
use mediaedge_observability::MetricsRegistry;

/// Egress protocol type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EgressProtocol {
    Hls,
    Dash,
    Whep,
    Moq,
    WsFmp4,
}

/// Egress configuration for a stream.
#[derive(Debug, Clone)]
pub struct EgressConfig {
    pub stream_id: StreamId,
    pub protocols: Vec<EgressProtocol>,
    pub output_root: PathBuf,
    pub base_url: String,
    pub hls_config: Option<hls::HlsEgressConfig>,
    pub dash_config: Option<dash::DashEgressConfig>,
    pub whep_config: Option<whep::WhepEgressConfig>,
    pub moq_config: Option<moq::MoqEgressConfig>,
    pub ws_fmp4_config: Option<ws_fmp4::WsFmp4EgressConfig>,
}

impl Default for EgressConfig {
    fn default() -> Self {
        Self {
            stream_id: StreamId::new("default"),
            protocols: vec![
                EgressProtocol::Hls,
                EgressProtocol::Dash,
            ],
            output_root: PathBuf::from("/data/egress"),
            base_url: "https://cdn.example.com".to_string(),
            hls_config: None,
            dash_config: None,
            whep_config: None,
            moq_config: None,
            ws_fmp4_config: None,
        }
    }
}

/// Egress manager for a stream.
pub struct EgressManager {
    config: EgressConfig,
    registry: Arc<FragmentBroadcasterRegistry>,
    metrics: Arc<MetricsRegistry>,
    http_config: HttpProtocolConfig,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl EgressManager {
    /// Create a new egress manager.
    pub fn new(
        config: EgressConfig,
        registry: Arc<FragmentBroadcasterRegistry>,
        metrics: Arc<MetricsRegistry>,
        http_config: HttpProtocolConfig,
    ) -> Self {
        Self {
            config,
            registry,
            metrics,
            http_config,
            shutdown_tx: None,
        }
    }

    /// Start all configured egress protocols.
    pub async fn start(&mut self) -> anyhow::Result<()> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        let mut handles = Vec::new();

        for protocol in &self.config.protocols {
            let handle = self.start_protocol(*protocol).await?;
            handles.push(handle);
        }

        info!("Started {} egress protocols for stream {}", 
            handles.len(), self.config.stream_id.0);

        // Wait for shutdown signal
        shutdown_rx.recv().await;

        // Graceful shutdown
        for handle in handles {
            handle.abort();
        }

        info!("Egress manager stopped for stream {}", self.config.stream_id.0);
        Ok(())
    }

    /// Start a specific egress protocol.
    async fn start_protocol(
        &self,
        protocol: EgressProtocol,
    ) -> anyhow::Result<tokio::task::JoinHandle<()>> {
        let stream_id = self.config.stream_id.clone();
        let registry = self.registry.clone();
        let metrics = self.metrics.clone();
        let http_config = self.http_config.clone();
        let output_root = self.config.output_root.clone();
        let base_url = self.config.base_url.clone();

        let handle = match protocol {
            EgressProtocol::Hls => {
                let hls_config = self.config.hls_config.clone().unwrap_or_default();
                tokio::spawn(async move {
                    if let Err(e) = hls::run_hls_egress(
                        stream_id,
                        registry,
                        metrics,
                        http_config,
                        output_root,
                        base_url,
                    ).await {
                        error!("HLS egress error: {}", e);
                    }
                })
            }
            EgressProtocol::Dash => {
                let dash_config = self.config.dash_config.clone().unwrap_or_default();
                tokio::spawn(async move {
                    if let Err(e) = dash::run_dash_egress(
                        stream_id,
                        registry,
                        metrics,
                        http_config,
                        output_root,
                        base_url,
                    ).await {
                        error!("DASH egress error: {}", e);
                    }
                })
            }
            EgressProtocol::Whep => {
                let whep_config = self.config.whep_config.clone().unwrap_or_default();
                tokio::spawn(async move {
                    if let Err(e) = whep::run_whep_egress(
                        stream_id,
                        registry,
                        metrics,
                        http_config,
                        output_root,
                        base_url,
                    ).await {
                        error!("WHEP egress error: {}", e);
                    }
                })
            }
            EgressProtocol::Moq => {
                let moq_config = self.config.moq_config.clone().unwrap_or_default();
                tokio::spawn(async move {
                    if let Err(e) = moq::run_moq_egress(
                        stream_id,
                        registry,
                        metrics,
                        http_config,
                        output_root,
                        base_url,
                    ).await {
                        error!("MoQ egress error: {}", e);
                    }
                })
            }
            EgressProtocol::WsFmp4 => {
                let ws_config = self.config.ws_fmp4_config.clone().unwrap_or_default();
                tokio::spawn(async move {
                    if let Err(e) = ws_fmp4::run_ws_fmp4_egress(
                        stream_id,
                        registry,
                        metrics,
                        http_config,
                        output_root,
                        base_url,
                    ).await {
                        error!("WebSocket fMP4 egress error: {}", e);
                    }
                })
            }
        };

        Ok(handle)
    }

    /// Request shutdown.
    pub fn shutdown(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.try_send(());
        }
    }
}

/// Egress server that manages multiple streams.
pub struct EgressServer {
    http_config: HttpProtocolConfig,
    registry: Arc<FragmentBroadcasterRegistry>,
    metrics: Arc<MetricsRegistry>,
    managers: DashMap<String, EgressManager>,
    default_config: EgressConfig,
}

impl EgressServer {
    /// Create a new egress server.
    pub fn new(
        http_config: HttpProtocolConfig,
        registry: Arc<FragmentBroadcasterRegistry>,
        metrics: Arc<MetricsRegistry>,
        default_config: EgressConfig,
    ) -> Self {
        Self {
            http_config,
            registry,
            metrics,
            managers: DashMap::new(),
            default_config,
        }
    }

    /// Start egress for a stream.
    pub async fn start_stream(&self, stream_id: StreamId) -> anyhow::Result<()> {
        let mut config = self.default_config.clone();
        config.stream_id = stream_id.clone();

        let manager = EgressManager::new(
            config,
            self.registry.clone(),
            self.metrics.clone(),
            self.http_config.clone(),
        );

        let stream_key = stream_id.0.clone();
        self.managers.insert(stream_key.clone(), manager);

        // Start in background
        let manager = self.managers.get_mut(&stream_key).unwrap();
        tokio::spawn(async move {
            if let Err(e) = manager.start().await {
                error!("Egress manager error for {}: {}", stream_key, e);
            }
        });

        info!("Egress started for stream: {}", stream_key);
        Ok(())
    }

    /// Stop egress for a stream.
    pub fn stop_stream(&self, stream_id: &StreamId) {
        if let Some((_, manager)) = self.managers.remove(&stream_id.0) {
            manager.shutdown();
            info!("Egress stopped for stream: {}", stream_id.0);
        }
    }

    /// Get active stream count.
    pub fn active_stream_count(&self) -> usize {
        self.managers.len()
    }

    /// List active streams.
    pub fn list_streams(&self) -> Vec<String> {
        self.managers.iter().map(|m| m.key().clone()).collect()
    }
}

/// Egress statistics.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EgressStats {
    pub stream_id: String,
    pub protocols: Vec<ProtocolStats>,
    pub total_bytes: u64,
    pub total_segments: u64,
    pub uptime_secs: u64,
}

/// Per-protocol statistics.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProtocolStats {
    pub protocol: String,
    pub status: String,
    pub bytes_sent: u64,
    pub segments_sent: u64,
    pub active_connections: u32,
    pub errors: u64,
    pub last_segment_time: Option<String>,
}

impl EgressServer {
    /// Get statistics for all streams.
    pub async fn get_stats(&self) -> Vec<EgressStats> {
        // In a real implementation, would collect stats from each manager
        Vec::new()
    }
}

use dashmap::DashMap;
use tracing::error;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_egress_config() {
        let config = EgressConfig {
            stream_id: StreamId::new("test"),
            protocols: vec![EgressProtocol::Hls, EgressProtocol::Dash],
            output_root: PathBuf::from("/data"),
            base_url: "https://cdn.example.com".to_string(),
            hls_config: None,
            dash_config: None,
            whep_config: None,
            moq_config: None,
            ws_fmp4_config: None,
        };

        assert_eq!(config.protocols.len(), 2);
        assert!(config.protocols.contains(&EgressProtocol::Hls));
        assert!(config.protocols.contains(&EgressProtocol::Dash));
    }

    #[test]
    fn test_egress_protocol_enum() {
        let protocols = vec![
            EgressProtocol::Hls,
            EgressProtocol::Dash,
            EgressProtocol::Whep,
            EgressProtocol::Moq,
            EgressProtocol::WsFmp4,
        ];

        assert_eq!(protocols.len(), 5);
    }
}