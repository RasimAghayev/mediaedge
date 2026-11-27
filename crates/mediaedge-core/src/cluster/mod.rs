//! Cluster Module - Gossip-based cluster membership and ownership

pub mod capacity;
pub mod config_sync;
pub mod federation;
pub mod gossip;
pub mod mesh;
pub mod ownership;
pub mod redirect;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::capacity::CapacityManager;
use crate::config_sync::ConfigSyncManager;
use crate::federation::FederationManager;
use crate::gossip::{GossipEvent, GossipProtocol};
use crate::mesh::MeshManager;
use crate::ownership::OwnershipManager;
use crate::redirect::RedirectManager;
use mediaedge_config::types::ServerConfig;
use mediaedge_observability::MetricsRegistry;

/// Cluster manager - coordinates all cluster components.
pub struct ClusterManager {
    server_config: ServerConfig,
    metrics: Arc<MetricsRegistry>,
    gossip: Arc<RwLock<Option<GossipProtocol>>>,
    ownership: Arc<OwnershipManager>,
    capacity: Arc<CapacityManager>,
    redirect: Arc<RedirectManager>,
    config_sync: Arc<ConfigSyncManager>,
    federation: Arc<FederationManager>,
    mesh: Arc<MeshManager>,
    running: Arc<RwLock<bool>>,
}

impl ClusterManager {
    /// Create a new cluster manager.
    pub fn new(server_config: ServerConfig, metrics: Arc<MetricsRegistry>) -> Self {
        let ownership = Arc::new(OwnershipManager::new(metrics.clone()));
        let capacity = Arc::new(CapacityManager::new(metrics.clone()));
        let redirect = Arc::new(RedirectManager::new(metrics.clone()));
        let config_sync = Arc::new(ConfigSyncManager::new(metrics.clone()));
        let federation = Arc::new(FederationManager::new(
            server_config.federation.clone().unwrap_or_default(),
            server_config.node_id.clone(),
            metrics.clone(),
        ));
        let mesh = Arc::new(MeshManager::new(
            server_config.mesh.clone().unwrap_or_default(),
            metrics.clone(),
        ));

        Self {
            server_config,
            metrics,
            gossip: Arc::new(RwLock::new(None)),
            ownership,
            capacity,
            redirect,
            config_sync,
            federation,
            mesh,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the cluster manager.
    pub async fn start(&self) -> anyhow::Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }

        info!(
            "Starting cluster manager for node: {}",
            self.server_config.node_id
        );

        // Initialize gossip protocol
        let gossip = GossipProtocol::new(
            self.server_config.node_id.clone(),
            self.server_config.bind_address.clone(),
            self.server_config.cluster_port,
            self.metrics.clone(),
        )
        .await?;

        // Start gossip
        gossip.start().await?;

        // Initialize other managers with gossip
        self.ownership.set_gossip(gossip.clone());
        self.capacity.set_gossip(gossip.clone());
        self.redirect.set_ownership(self.ownership.clone());
        self.config_sync.set_gossip(gossip.clone());

        // Start managers
        self.ownership.start().await?;
        self.capacity.start().await?;
        self.config_sync.start().await?;
        self.mesh.start().await?;

        *self.gossip.write().await = Some(gossip);
        *running = true;

        info!("Cluster manager started");
        Ok(())
    }

    /// Stop the cluster manager.
    pub async fn stop(&self) -> anyhow::Result<()> {
        let mut running = self.running.write().await;
        if !*running {
            return Ok(());
        }

        info!("Stopping cluster manager");

        // Stop managers
        self.mesh.stop().await?;
        self.config_sync.stop().await?;
        self.capacity.stop().await?;
        self.ownership.stop().await?;

        // Stop gossip
        if let Some(gossip) = self.gossip.write().await.take() {
            gossip.stop().await?;
        }

        *running = false;
        info!("Cluster manager stopped");
        Ok(())
    }

    /// Get ownership manager.
    pub fn ownership(&self) -> Arc<OwnershipManager> {
        self.ownership.clone()
    }

    /// Get capacity manager.
    pub fn capacity(&self) -> Arc<CapacityManager> {
        self.capacity.clone()
    }

    /// Get redirect manager.
    pub fn redirect(&self) -> Arc<RedirectManager> {
        self.redirect.clone()
    }

    /// Get config sync manager.
    pub fn config_sync(&self) -> Arc<ConfigSyncManager> {
        self.config_sync.clone()
    }

    /// Get federation manager.
    pub fn federation(&self) -> Arc<FederationManager> {
        self.federation.clone()
    }

    /// Get mesh manager.
    pub fn mesh(&self) -> Arc<MeshManager> {
        self.mesh.clone()
    }

    /// Get gossip protocol.
    pub async fn gossip(&self) -> Option<Arc<GossipProtocol>> {
        self.gossip.read().await.clone()
    }

    /// Check if running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}
