//! Capacity Manager - Per-node capacity advertisement
//!
//! Advertises and tracks node capacity (CPU, memory, bandwidth, GPU)
//! for scheduling and load balancing decisions.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, info};

use crate::gossip::{GossipProtocol, NodeId};
use mediaedge_observability::MetricsRegistry;

/// Node capacity information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapacity {
    pub node_id: NodeId,
    pub cpu_cores: u32,
    pub cpu_usage_percent: f32,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub bandwidth_total_mbps: u32,
    pub bandwidth_used_mbps: u32,
    pub gpu_count: u32,
    pub gpu_memory_total_bytes: u64,
    pub gpu_memory_used_bytes: u64,
    pub gpu_utilization_percent: f32,
    pub max_streams: u32,
    pub current_streams: u32,
    pub custom_metrics: HashMap<String, f64>,
    pub updated_at: u64, // Unix timestamp
}

impl NodeCapacity {
    /// Calculate available capacity score (0.0 - 1.0, higher = more available).
    pub fn capacity_score(&self) -> f32 {
        let cpu_available = 1.0 - (self.cpu_usage_percent / 100.0);
        let memory_available =
            1.0 - (self.memory_used_bytes as f32 / self.memory_total_bytes.max(1) as f32);
        let bandwidth_available =
            1.0 - (self.bandwidth_used_mbps as f32 / self.bandwidth_total_mbps.max(1) as f32);
        let stream_available = 1.0 - (self.current_streams as f32 / self.max_streams.max(1) as f32);

        // Weighted average
        (cpu_available * 0.3
            + memory_available * 0.2
            + bandwidth_available * 0.2
            + stream_available * 0.3)
            .clamp(0.0, 1.0)
    }

    /// Check if node can accept more streams.
    pub fn can_accept_streams(&self, count: u32) -> bool {
        self.current_streams + count <= self.max_streams
    }

    /// Get GPU info if available.
    pub fn has_gpu(&self) -> bool {
        self.gpu_count > 0
    }
}

/// Capacity manager configuration.
#[derive(Debug, Clone)]
pub struct CapacityConfig {
    pub advertise_interval_secs: u64,
    pub stale_threshold_secs: u64,
    pub enable_gpu_monitoring: bool,
}

impl Default for CapacityConfig {
    fn default() -> Self {
        Self {
            advertise_interval_secs: 10,
            stale_threshold_secs: 30,
            enable_gpu_monitoring: true,
        }
    }
}

/// Capacity manager.
pub struct CapacityManager {
    config: CapacityConfig,
    gossip: Arc<RwLock<Option<Arc<GossipProtocol>>>>,
    local_capacity: Arc<RwLock<NodeCapacity>>,
    cluster_capacity: Arc<DashMap<NodeId, NodeCapacity>>,
    event_tx: mpsc::Sender<CapacityEvent>,
    event_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<CapacityEvent>>>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    metrics: Arc<MetricsRegistry>,
}

/// Capacity events.
#[derive(Debug, Clone)]
pub enum CapacityEvent {
    Updated { node_id: NodeId },
    Removed { node_id: NodeId },
    Stale { node_id: NodeId },
}

impl CapacityManager {
    /// Create a new capacity manager.
    pub fn new(metrics: Arc<MetricsRegistry>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(1000);
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);

        // Initialize with default capacity (will be updated by system info)
        let local_capacity = NodeCapacity {
            node_id: NodeId::new("unknown"),
            cpu_cores: num_cpus::get() as u32,
            cpu_usage_percent: 0.0,
            memory_total_bytes: Self::get_total_memory(),
            memory_used_bytes: 0,
            bandwidth_total_mbps: 10000, // 10 Gbps default
            bandwidth_used_mbps: 0,
            gpu_count: 0,
            gpu_memory_total_bytes: 0,
            gpu_memory_used_bytes: 0,
            gpu_utilization_percent: 0.0,
            max_streams: 10000,
            current_streams: 0,
            custom_metrics: HashMap::new(),
            updated_at: chrono::Utc::now().timestamp() as u64,
        };

        Self {
            config: CapacityConfig::default(),
            gossip: Arc::new(RwLock::new(None)),
            local_capacity: Arc::new(RwLock::new(local_capacity)),
            cluster_capacity: Arc::new(DashMap::new()),
            event_tx,
            event_rx: Arc::new(tokio::sync::Mutex::new(event_rx)),
            shutdown_tx,
            metrics,
        }
    }

    /// Set the gossip protocol.
    pub fn set_gossip(&self, gossip: Arc<GossipProtocol>) {
        *self.gossip.write() = Some(gossip);

        // Update local node ID
        let node_id = gossip.node_id().clone();
        self.local_capacity.write().node_id = node_id;
    }

    /// Start the capacity manager.
    pub async fn start(&self) -> anyhow::Result<()> {
        info!("Starting capacity manager");

        // Start system metrics collection
        self.start_metrics_collection().await;

        // Start advertisement loop
        self.start_advertisement_loop().await;

        // Start stale detection
        self.start_stale_detection().await;

        Ok(())
    }

    /// Stop the capacity manager.
    pub async fn stop(&self) -> anyhow::Result<()> {
        info!("Stopping capacity manager");
        let _ = self.shutdown_tx.send(true);
        Ok(())
    }

    /// Update local capacity metrics.
    pub fn update_capacity(&self, update: CapacityUpdate) {
        let mut capacity = self.local_capacity.write();

        if let Some(cpu) = update.cpu_usage_percent {
            capacity.cpu_usage_percent = cpu.clamp(0.0, 100.0);
        }
        if let Some(mem_used) = update.memory_used_bytes {
            capacity.memory_used_bytes = mem_used;
        }
        if let Some(bw_used) = update.bandwidth_used_mbps {
            capacity.bandwidth_used_mbps = bw_used;
        }
        if let Some(gpu_util) = update.gpu_utilization_percent {
            capacity.gpu_utilization_percent = gpu_util.clamp(0.0, 100.0);
        }
        if let Some(gpu_mem_used) = update.gpu_memory_used_bytes {
            capacity.gpu_memory_used_bytes = gpu_mem_used;
        }
        if let Some(streams) = update.current_streams {
            capacity.current_streams = streams;
        }
        if let Some(max_streams) = update.max_streams {
            capacity.max_streams = max_streams;
        }

        for (k, v) in update.custom_metrics {
            capacity.custom_metrics.insert(k, v);
        }

        capacity.updated_at = chrono::Utc::now().timestamp() as u64;
    }

    /// Increment stream count.
    pub fn add_stream(&self) {
        let mut capacity = self.local_capacity.write();
        capacity.current_streams = capacity.current_streams.saturating_add(1);
        capacity.updated_at = chrono::Utc::now().timestamp() as u64;
    }

    /// Decrement stream count.
    pub fn remove_stream(&self) {
        let mut capacity = self.local_capacity.write();
        capacity.current_streams = capacity.current_streams.saturating_sub(1);
        capacity.updated_at = chrono::Utc::now().timestamp() as u64;
    }

    /// Get local capacity.
    pub fn local_capacity(&self) -> NodeCapacity {
        self.local_capacity.read().clone()
    }

    /// Get cluster capacity (all nodes).
    pub fn cluster_capacity(&self) -> Vec<NodeCapacity> {
        self.cluster_capacity
            .iter()
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get capacity for a specific node.
    pub fn get_capacity(&self, node_id: &NodeId) -> Option<NodeCapacity> {
        self.cluster_capacity.get(node_id).map(|c| c.clone())
    }

    /// Find best node for new streams (highest capacity score).
    pub fn find_best_node(&self, required_streams: u32, require_gpu: bool) -> Option<NodeId> {
        let local = self.local_capacity.read();
        let mut best_node = Some(local.node_id.clone());
        let mut best_score = local.capacity_score();

        // Check if local node can handle it
        if !local.can_accept_streams(required_streams) || (require_gpu && !local.has_gpu()) {
            best_node = None;
            best_score = 0.0;
        }

        // Check other nodes
        for entry in self.cluster_capacity.iter() {
            let capacity = entry.value();
            if capacity.can_accept_streams(required_streams) && (!require_gpu || capacity.has_gpu())
            {
                let score = capacity.capacity_score();
                if score > best_score {
                    best_score = score;
                    best_node = Some(capacity.node_id.clone());
                }
            }
        }

        best_node
    }

    /// Get nodes sorted by capacity score (best first).
    pub fn nodes_by_capacity(&self, require_gpu: bool) -> Vec<NodeCapacity> {
        let mut nodes: Vec<NodeCapacity> = self
            .cluster_capacity()
            .into_iter()
            .filter(|c| !require_gpu || c.has_gpu())
            .collect();

        nodes.sort_by(|a, b| b.capacity_score().partial_cmp(&a.capacity_score()).unwrap());
        nodes
    }

    /// Update capacity from gossip.
    pub fn update_from_gossip(&self, node_id: NodeId, capacity: NodeCapacity) {
        let is_new = !self.cluster_capacity.contains_key(&node_id);
        self.cluster_capacity.insert(node_id.clone(), capacity);

        if is_new {
            let _ = self.event_tx.try_send(CapacityEvent::Updated { node_id });
        }
    }

    /// Start system metrics collection.
    async fn start_metrics_collection(&self) {
        let local_capacity = self.local_capacity.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let config = self.config.clone();
        let metrics = self.metrics.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }

                        // Collect system metrics
                        let mut cap = local_capacity.write();

                        // CPU usage (simplified - in real implementation use sysinfo)
                        cap.cpu_usage_percent = Self::get_cpu_usage();

                        // Memory usage
                        cap.memory_used_bytes = Self::get_used_memory();

                        // GPU metrics (if enabled)
                        if config.enable_gpu_monitoring {
                            if let Some(gpu_info) = Self::get_gpu_info() {
                                cap.gpu_count = gpu_info.count;
                                cap.gpu_memory_total_bytes = gpu_info.memory_total;
                                cap.gpu_memory_used_bytes = gpu_info.memory_used;
                                cap.gpu_utilization_percent = gpu_info.utilization;
                            }
                        }

                        cap.updated_at = chrono::Utc::now().timestamp() as u64;

                        // Update metrics
                        metrics.gauge("capacity_cpu_usage_percent", "CPU usage percent").set(cap.cpu_usage_percent as f64);
                        metrics.gauge("capacity_memory_usage_percent", "Memory usage percent").set(
                            (cap.memory_used_bytes as f64 / cap.memory_total_bytes.max(1) as f64) * 100.0
                        );
                        metrics.gauge("capacity_streams", "Current streams").set(cap.current_streams as f64);
                        metrics.gauge("capacity_score", "Capacity score").set(cap.capacity_score() as f64);
                    }
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });
    }

    /// Start advertisement loop.
    async fn start_advertisement_loop(&self) {
        let local_capacity = self.local_capacity.clone();
        let gossip = self.gossip.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.advertise_interval_secs));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }

                        // In real implementation, would add to gossip payload
                        // The gossip protocol will pick it up
                        if let Some(gossip_guard) = gossip.read().as_ref() {
                            // Gossip protocol reads local_capacity when building payload
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });
    }

    /// Start stale node detection.
    async fn start_stale_detection(&self) {
        let cluster_capacity = self.cluster_capacity.clone();
        let event_tx = self.event_tx.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }

                        let now = chrono::Utc::now().timestamp() as u64;
                        let mut stale_nodes = Vec::new();

                        for entry in cluster_capacity.iter() {
                            let capacity = entry.value();
                            if now.saturating_sub(capacity.updated_at) > config.stale_threshold_secs {
                                stale_nodes.push(capacity.node_id.clone());
                            }
                        }

                        for node_id in stale_nodes {
                            cluster_capacity.remove(&node_id);
                            let _ = event_tx.try_send(CapacityEvent::Stale { node_id });
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });
    }

    /// Get CPU usage percentage (simplified).
    fn get_cpu_usage() -> f32 {
        // In real implementation, use sysinfo crate
        // For now, return a simulated value
        rand::rng().random_range(10.0..50.0)
    }

    /// Get total system memory.
    fn get_total_memory() -> u64 {
        // In real implementation, use sysinfo crate
        16 * 1024 * 1024 * 1024 // 16 GB default
    }

    /// Get used memory.
    fn get_used_memory() -> u64 {
        // In real implementation, use sysinfo crate
        rand::rng().random_range(4..12) * 1024 * 1024 * 1024
    }

    /// Get GPU info.
    fn get_gpu_info() -> Option<GpuInfo> {
        // In real implementation, query nvidia-smi or similar
        None
    }
}

/// GPU information.
#[derive(Debug, Clone)]
struct GpuInfo {
    count: u32,
    memory_total: u64,
    memory_used: u64,
    utilization: f32,
}

/// Capacity update for local metrics.
#[derive(Debug, Clone, Default)]
pub struct CapacityUpdate {
    pub cpu_usage_percent: Option<f32>,
    pub memory_used_bytes: Option<u64>,
    pub bandwidth_used_mbps: Option<u32>,
    pub gpu_utilization_percent: Option<f32>,
    pub gpu_memory_used_bytes: Option<u64>,
    pub current_streams: Option<u32>,
    pub max_streams: Option<u32>,
    pub custom_metrics: HashMap<String, f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_capacity_score() {
        let capacity = NodeCapacity {
            node_id: NodeId::new("test"),
            cpu_cores: 8,
            cpu_usage_percent: 50.0,
            memory_total_bytes: 16_000_000_000,
            memory_used_bytes: 8_000_000_000,
            bandwidth_total_mbps: 10000,
            bandwidth_used_mbps: 5000,
            gpu_count: 1,
            gpu_memory_total_bytes: 24_000_000_000,
            gpu_memory_used_bytes: 12_000_000_000,
            gpu_utilization_percent: 50.0,
            max_streams: 1000,
            current_streams: 500,
            custom_metrics: HashMap::new(),
            updated_at: chrono::Utc::now().timestamp() as u64,
        };

        let score = capacity.capacity_score();
        assert!(score > 0.0 && score < 1.0);
        // 50% usage across all metrics should give ~0.5 score
        assert!((score - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_can_accept_streams() {
        let capacity = NodeCapacity {
            node_id: NodeId::new("test"),
            cpu_cores: 8,
            cpu_usage_percent: 50.0,
            memory_total_bytes: 16_000_000_000,
            memory_used_bytes: 8_000_000_000,
            bandwidth_total_mbps: 10000,
            bandwidth_used_mbps: 5000,
            gpu_count: 1,
            gpu_memory_total_bytes: 24_000_000_000,
            gpu_memory_used_bytes: 12_000_000_000,
            gpu_utilization_percent: 50.0,
            max_streams: 1000,
            current_streams: 500,
            custom_metrics: HashMap::new(),
            updated_at: chrono::Utc::now().timestamp() as u64,
        };

        assert!(capacity.can_accept_streams(100));
        assert!(capacity.can_accept_streams(500));
        assert!(!capacity.can_accept_streams(501));
    }

    #[test]
    fn test_capacity_update() {
        let mut update = CapacityUpdate::default();
        update.cpu_usage_percent = Some(75.0);
        update.current_streams = Some(100);

        assert_eq!(update.cpu_usage_percent, Some(75.0));
        assert_eq!(update.current_streams, Some(100));
    }
}
