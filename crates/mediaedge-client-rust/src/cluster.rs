use crate::{MediaEdgeClient, Result, models::*};

pub struct ClusterApi {
    client: MediaEdgeClient,
}

impl ClusterApi {
    pub fn new(client: MediaEdgeClient) -> Self {
        Self { client }
    }

    /// List cluster nodes
    pub async fn nodes(&self, params: Option<ListParams>) -> Result<PaginatedResponse<ClusterNode>> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(page) = p.page { query.push(("page", page.to_string())); }
            if let Some(per_page) = p.per_page { query.push(("per_page", per_page.to_string())); }
        }
        self.client.get_with_query("/api/v1/cluster/nodes", &query).await
    }

    /// Get a single node
    pub async fn node(&self, id: Uuid) -> Result<ClusterNode> {
        self.client.get(&format!("/api/v1/cluster/nodes/{}", id)).await
    }

    /// List broadcasts
    pub async fn broadcasts(&self, params: Option<ListParams>) -> Result<PaginatedResponse<Broadcast>> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(page) = p.page { query.push(("page", page.to_string())); }
            if let Some(per_page) = p.per_page { query.push(("per_page", per_page.to_string())); }
        }
        self.client.get_with_query("/api/v1/cluster/broadcasts", &query).await
    }

    /// Get cluster health
    pub async fn health(&self) -> Result<ClusterHealth> {
        self.client.get("/api/v1/cluster/health").await
    }

    /// Get federation status
    pub async fn federation(&self) -> Result<FederationStatus> {
        self.client.get("/api/v1/cluster/federation").await
    }

    /// Get mesh status
    pub async fn mesh(&self) -> Result<MeshStatus> {
        self.client.get("/api/v1/cluster/mesh").await
    }

    /// Trigger config sync
    pub async fn sync_config(&self) -> Result<SyncResult> {
        self.client.post("/api/v1/cluster/config/sync", &()).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterHealth {
    pub status: String,
    pub nodes_total: u32,
    pub nodes_healthy: u32,
    pub nodes_degraded: u32,
    pub nodes_offline: u32,
    pub streams_active: u32,
    pub viewers_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationStatus {
    pub clusters: Vec<FederatedCluster>,
    pub pull_connections: u32,
    pub push_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederatedCluster {
    pub id: Uuid,
    pub name: String,
    pub endpoint: String,
    pub status: String,
    pub latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshStatus {
    pub peers: u32,
    pub connections: u32,
    pub bandwidth_mbps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub success: bool,
    pub version: u64,
    pub nodes_synced: u32,
    pub errors: Vec<String>,
}

use crate::models::{ListParams, PaginatedResponse};
use uuid::Uuid;
use serde::{Serialize, Deserialize};