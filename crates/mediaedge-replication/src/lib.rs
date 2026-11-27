//! MediaEdge Replication Crate
//!
//! Geo-replication, erasure coding, replication planning, and cache sync.
//!
//! Features:
//! - Raft-based replication coordinator (etcd)
//! - Reed-Solomon erasure coding (k=10, m=4 for 99.9999999% durability)
//! - Intelligent replication planner (continent/region staging, anti-affinity)
//! - RaptorQ-FEC mesh for cache synchronization

#![warn(missing_docs)]

pub mod coordinator;
pub mod erasure;
pub mod mesh;
pub mod planner;

use mediaedge_config::types::ReplicationCoordinatorConfig;
use mediaedge_observability::MetricsRegistry;

/// Replication service configuration.
#[derive(Debug, Clone)]
pub struct ReplicationConfig {
    pub coordinator: ReplicationCoordinatorConfig,
}

/// Main replication service entry point.
pub struct ReplicationService {
    config: ReplicationConfig,
    metrics: std::sync::Arc<MetricsRegistry>,
    coordinator: std::sync::Arc<coordinator::ReplicationCoordinator>,
    planner: std::sync::Arc<planner::ReplicationPlanner>,
    mesh: std::sync::Arc<mesh::MeshManager>,
}

impl ReplicationService {
    /// Create a new replication service.
    pub async fn new(
        config: ReplicationConfig,
        local_node_id: String,
        metrics: std::sync::Arc<MetricsRegistry>,
    ) -> anyhow::Result<Self> {
        let coordinator = std::sync::Arc::new(
            coordinator::ReplicationCoordinator::new(
                config.coordinator.clone(),
                local_node_id.clone(),
                metrics.clone(),
            )
            .await?,
        );

        let planner = std::sync::Arc::new(planner::ReplicationPlanner::new(
            config.coordinator.planner.clone().unwrap_or_default(),
            metrics.clone(),
        ));

        let mesh = std::sync::Arc::new(
            mesh::MeshManager::new(
                config.coordinator.mesh.clone().unwrap_or_default(),
                local_node_id,
                metrics.clone(),
            )
            .await?,
        );

        Ok(Self {
            config,
            metrics,
            coordinator,
            planner,
            mesh,
        })
    }

    /// Get the coordinator.
    pub fn coordinator(&self) -> std::sync::Arc<coordinator::ReplicationCoordinator> {
        self.coordinator.clone()
    }

    /// Get the planner.
    pub fn planner(&self) -> std::sync::Arc<planner::ReplicationPlanner> {
        self.planner.clone()
    }

    /// Get the mesh manager.
    pub fn mesh(&self) -> std::sync::Arc<mesh::MeshManager> {
        self.mesh.clone()
    }

    /// Get metrics registry.
    pub fn metrics(&self) -> std::sync::Arc<MetricsRegistry> {
        self.metrics.clone()
    }

    /// Start all replication components.
    pub async fn start(&self) -> anyhow::Result<()> {
        self.coordinator.start().await?;
        self.mesh.start().await?;
        info!("Replication service started");
        Ok(())
    }

    /// Stop all replication components.
    pub async fn stop(&self) -> anyhow::Result<()> {
        self.mesh.stop().await?;
        self.coordinator.stop().await?;
        info!("Replication service stopped");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replication_config() {
        // Placeholder test
    }
}
