//! MediaEdge Core Crate
//!
//! Core cluster functionality:
//! - Gossip-based cluster membership (SWIM protocol)
//! - Broadcast ownership with lease renewal
//! - Per-node capacity advertisement
//! - HTTP redirect-to-owner for HLS/DASH/RTSP
//! - LWW config sync via gossip
//! - Cross-cluster federation (MoQ-based)
//! - Browser peer mesh (WebRTC DataChannel)
//! - Multi-tenancy & Monetization (Phase 5)
//! - AI-Assisted Operations (Phase 5)

#![warn(missing_docs)]

pub mod ai;
pub mod cluster;
pub mod monetization;

use mediaedge_config::types::ServerConfig;
use mediaedge_observability::MetricsRegistry;

/// Core service configuration.
#[derive(Debug, Clone)]
pub struct CoreConfig {
    pub server: ServerConfig,
}

/// Main core service entry point.
pub struct CoreService {
    config: CoreConfig,
    metrics: std::sync::Arc<MetricsRegistry>,
    cluster: std::sync::Arc<cluster::ClusterManager>,
    monetization: std::sync::Arc<monetization::MonetizationManager>,
    ai: std::sync::Arc<ai::AiManager>,
}

impl CoreService {
    /// Create a new core service.
    pub fn new(config: CoreConfig, metrics: std::sync::Arc<MetricsRegistry>) -> Self {
        let cluster = std::sync::Arc::new(cluster::ClusterManager::new(
            config.server.clone(),
            metrics.clone(),
        ));

        let monetization = std::sync::Arc::new(monetization::MonetizationManager::new());
        let ai = std::sync::Arc::new(ai::AiManager::new());

        Self {
            config,
            metrics,
            cluster,
            monetization,
            ai,
        }
    }

    /// Get the cluster manager.
    pub fn cluster(&self) -> std::sync::Arc<cluster::ClusterManager> {
        self.cluster.clone()
    }

    /// Get monetization manager.
    pub fn monetization(&self) -> std::sync::Arc<monetization::MonetizationManager> {
        self.monetization.clone()
    }

    /// Get AI manager.
    pub fn ai(&self) -> std::sync::Arc<ai::AiManager> {
        self.ai.clone()
    }

    /// Get metrics registry.
    pub fn metrics(&self) -> std::sync::Arc<MetricsRegistry> {
        self.metrics.clone()
    }

    /// Start the core service.
    pub async fn start(&self) -> anyhow::Result<()> {
        self.cluster.start().await?;
        self.monetization.start().await?;
        self.ai.start().await?;
        Ok(())
    }

    /// Stop the core service.
    pub async fn stop(&self) -> anyhow::Result<()> {
        self.ai.stop().await?;
        self.monetization.stop().await?;
        self.cluster.stop().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_config() {
        // Placeholder test
    }
}
