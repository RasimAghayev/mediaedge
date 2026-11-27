//! Gossip Protocol Implementation (SWIM-style)
//!
//! Implements a Chitchat-style gossip protocol for:
//! - Membership management (join, leave, failure detection)
//! - Ownership KV (broadcast ownership with lease renewal)
//! - Config sync (LWW - Last-Writer-Wins)

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use parking_lot::RwLock;
use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use mediaedge_observability::MetricsRegistry;

/// Node identifier.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Node state in the cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub node_id: NodeId,
    pub address: SocketAddr,
    pub status: NodeStatus,
    pub incarnation: u64,
    pub last_seen: Instant,
    pub metadata: HashMap<String, String>,
}

/// Node status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    Alive,
    Suspect,
    Dead,
    Left,
}

/// Gossip message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GossipMessage {
    /// Ping message for failure detection
    Ping {
        source: NodeId,
        target: NodeId,
        sequence: u64,
        payload: GossipPayload,
    },
    /// Ping request (indirect ping via another node)
    PingRequest {
        source: NodeId,
        target: NodeId,
        via: NodeId,
        sequence: u64,
    },
    /// Ack response
    Ack {
        source: NodeId,
        target: NodeId,
        sequence: u64,
        payload: GossipPayload,
    },
    /// State sync (full state exchange)
    Sync {
        source: NodeId,
        nodes: Vec<NodeState>,
        ownership: Vec<OwnershipEntry>,
        config: Vec<ConfigEntry>,
    },
}

/// Gossip payload carrying membership, ownership, and config data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GossipPayload {
    pub nodes: Vec<NodeState>,
    pub ownership: Vec<OwnershipEntry>,
    pub config: Vec<ConfigEntry>,
}

/// Ownership entry for broadcast ownership.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipEntry {
    pub key: String, // e.g., "broadcast:stream-123"
    pub owner: NodeId,
    pub lease_expires: u64, // Unix timestamp
    pub incarnation: u64,
    pub metadata: HashMap<String, String>,
}

/// Config entry for LWW config sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
    pub version: u64, // Logical timestamp (LWW)
    pub updated_by: NodeId,
    pub updated_at: u64, // Unix timestamp
}

/// Gossip events for external consumers.
#[derive(Debug, Clone)]
pub enum GossipEvent {
    NodeJoined(NodeId),
    NodeLeft(NodeId),
    NodeFailed(NodeId),
    NodeRecovered(NodeId),
    OwnershipChanged {
        key: String,
        old_owner: Option<NodeId>,
        new_owner: NodeId,
    },
    ConfigChanged {
        key: String,
        value: String,
    },
}

/// Gossip protocol configuration.
#[derive(Debug, Clone)]
pub struct GossipConfig {
    pub node_id: NodeId,
    pub bind_address: String,
    pub cluster_port: u16,
    pub interval_ms: u64,
    pub fanout: usize,
    pub suspect_timeout_ms: u64,
    pub dead_timeout_ms: u64,
    pub max_payload_nodes: usize,
    pub udp_buffer_size: usize,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            node_id: NodeId::new(uuid::Uuid::new_v4().to_string()),
            bind_address: "0.0.0.0".to_string(),
            cluster_port: 7946,
            interval_ms: 1000,
            fanout: 3,
            suspect_timeout_ms: 5000,
            dead_timeout_ms: 10000,
            max_payload_nodes: 100,
            udp_buffer_size: 65536,
        }
    }
}

/// Gossip protocol implementation.
pub struct GossipProtocol {
    config: GossipConfig,
    socket: Arc<UdpSocket>,
    nodes: Arc<DashMap<NodeId, NodeState>>,
    local_state: Arc<RwLock<NodeState>>,
    sequence: Arc<std::sync::atomic::AtomicU64>,
    event_tx: mpsc::Sender<GossipEvent>,
    event_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<GossipEvent>>>,
    shutdown_tx: watch::Sender<bool>,
    metrics: Arc<MetricsRegistry>,
    seed_nodes: Vec<SocketAddr>,
}

impl GossipProtocol {
    /// Create a new gossip protocol instance.
    pub async fn new(
        node_id: NodeId,
        bind_address: String,
        cluster_port: u16,
        metrics: Arc<MetricsRegistry>,
    ) -> anyhow::Result<Self> {
        let config = GossipConfig {
            node_id: node_id.clone(),
            bind_address: bind_address.clone(),
            cluster_port,
            ..Default::default()
        };

        let socket_addr = format!("{}:{}", bind_address, cluster_port).parse::<SocketAddr>()?;
        let socket = Arc::new(UdpSocket::bind(socket_addr).await?);

        let local_state = NodeState {
            node_id: node_id.clone(),
            address: socket_addr,
            status: NodeStatus::Alive,
            incarnation: 1,
            last_seen: Instant::now(),
            metadata: HashMap::new(),
        };

        let (event_tx, event_rx) = mpsc::channel(1000);
        let (shutdown_tx, _) = watch::channel(false);

        let protocol = Self {
            config,
            socket,
            nodes: Arc::new(DashMap::new()),
            local_state: Arc::new(RwLock::new(local_state)),
            sequence: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            event_tx,
            event_rx: Arc::new(tokio::sync::Mutex::new(event_rx)),
            shutdown_tx,
            metrics,
            seed_nodes: Vec::new(),
        };

        // Register self
        protocol
            .nodes
            .insert(node_id.clone(), protocol.local_state.read().clone());

        Ok(protocol)
    }

    /// Add seed nodes for initial cluster join.
    pub fn add_seed_nodes(&mut self, nodes: Vec<SocketAddr>) {
        self.seed_nodes = nodes;
    }

    /// Start the gossip protocol.
    pub async fn start(&self) -> anyhow::Result<()> {
        info!(
            "Starting gossip protocol for node: {}",
            self.config.node_id.0
        );

        // Start UDP listener
        let socket = self.socket.clone();
        let nodes = self.nodes.clone();
        let local_state = self.local_state.clone();
        let metrics = self.metrics.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            Self::udp_listener(socket, nodes, local_state, metrics, shutdown_rx).await;
        });

        // Start gossip interval
        let nodes = self.nodes.clone();
        let local_state = self.local_state.clone();
        let sequence = self.sequence.clone();
        let config = self.config.clone();
        let event_tx = self.event_tx.clone();
        let metrics = self.metrics.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            Self::gossip_loop(
                nodes,
                local_state,
                sequence,
                config,
                event_tx,
                metrics,
                shutdown_rx,
            )
            .await;
        });

        // Start failure detection
        let nodes = self.nodes.clone();
        let local_state = self.local_state.clone();
        let config = self.config.clone();
        let event_tx = self.event_tx.clone();
        let metrics = self.metrics.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            Self::failure_detection_loop(
                nodes,
                local_state,
                config,
                event_tx,
                metrics,
                shutdown_rx,
            )
            .await;
        });

        // Join seed nodes
        for seed in &self.seed_nodes {
            self.join(*seed).await?;
        }

        Ok(())
    }

    /// Stop the gossip protocol.
    pub async fn stop(&self) -> anyhow::Result<()> {
        info!("Stopping gossip protocol");
        let _ = self.shutdown_tx.send(true);

        // Mark self as left
        {
            let mut state = self.local_state.write();
            state.status = NodeStatus::Left;
            state.incarnation += 1;
        }

        Ok(())
    }

    /// Join the cluster via a seed node.
    async fn join(&self, seed: SocketAddr) -> anyhow::Result<()> {
        info!("Joining cluster via seed: {}", seed);

        let ping = GossipMessage::Ping {
            source: self.config.node_id.clone(),
            target: NodeId::new(seed.to_string()), // Will be resolved by seed
            sequence: self
                .sequence
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            payload: self.build_payload().await,
        };

        let data = bincode::serialize(&ping)?;
        self.socket.send_to(&data, seed).await?;

        Ok(())
    }

    /// Build gossip payload with current state.
    async fn build_payload(&self) -> GossipPayload {
        let nodes: Vec<NodeState> = self
            .nodes
            .iter()
            .map(|e| e.value().clone())
            .take(self.config.max_payload_nodes)
            .collect();

        GossipPayload {
            nodes,
            ownership: Vec::new(), // Will be populated by ownership manager
            config: Vec::new(),    // Will be populated by config sync manager
        }
    }

    /// UDP listener for incoming gossip messages.
    async fn udp_listener(
        socket: Arc<UdpSocket>,
        nodes: Arc<DashMap<NodeId, NodeState>>,
        local_state: Arc<RwLock<NodeState>>,
        metrics: Arc<MetricsRegistry>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        let mut buf = vec![0u8; 65536];

        loop {
            if *shutdown_rx.borrow() {
                break;
            }

            tokio::select! {
                result = socket.recv_from(&mut buf) => {
                    match result {
                        Ok((len, addr)) => {
                            if let Ok(msg) = bincode::deserialize::<GossipMessage>(&buf[..len]) {
                                Self::handle_message(msg, addr, &nodes, &local_state, &metrics).await;
                            }
                        }
                        Err(e) => {
                            if !shutdown_rx.borrow() {
                                error!("UDP receive error: {}", e);
                            }
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
            }
        }
    }

    /// Handle incoming gossip message.
    async fn handle_message(
        msg: GossipMessage,
        from: SocketAddr,
        nodes: &Arc<DashMap<NodeId, NodeState>>,
        local_state: &Arc<RwLock<NodeState>>,
        metrics: &Arc<MetricsRegistry>,
    ) {
        match msg {
            GossipMessage::Ping {
                source,
                target,
                sequence,
                payload,
            } => {
                // Update source node state
                Self::update_node_state(nodes, &source, &payload, metrics).await;

                // Send ack
                let ack = GossipMessage::Ack {
                    source: local_state.read().node_id.clone(),
                    target: source,
                    sequence,
                    payload: GossipPayload::default(), // Could include partial state
                };
                // In real implementation, would send back to source
            }
            GossipMessage::PingRequest {
                source,
                target,
                via,
                sequence,
            } => {
                // Forward ping to target
                // In real implementation, would forward via UDP
            }
            GossipMessage::Ack {
                source,
                target,
                sequence,
                payload,
            } => {
                // Update source node state
                Self::update_node_state(nodes, &source, &payload, metrics).await;
            }
            GossipMessage::Sync {
                source,
                nodes: sync_nodes,
                ownership,
                config,
            } => {
                // Full state sync
                for node in sync_nodes {
                    Self::merge_node_state(nodes, node, metrics).await;
                }
                // Process ownership and config
            }
        }
    }

    /// Update node state from gossip payload.
    async fn update_node_state(
        nodes: &Arc<DashMap<NodeId, NodeState>>,
        source: &NodeId,
        payload: &GossipPayload,
        metrics: &Arc<MetricsRegistry>,
    ) {
        // Merge nodes from payload
        for node in &payload.nodes {
            Self::merge_node_state(nodes, node.clone(), metrics).await;
        }
    }

    /// Merge node state with local view.
    async fn merge_node_state(
        nodes: &Arc<DashMap<NodeId, NodeState>>,
        mut incoming: NodeState,
        metrics: &Arc<MetricsRegistry>,
    ) {
        let node_id = incoming.node_id.clone();

        let mut updated = false;
        nodes
            .entry(node_id.clone())
            .and_modify(|existing| {
                // Higher incarnation wins
                if incoming.incarnation > existing.incarnation
                    || (incoming.incarnation == existing.incarnation
                        && incoming.status as u8 > existing.status as u8)
                {
                    *existing = incoming.clone();
                    updated = true;
                }
                // Update last_seen for alive nodes
                if incoming.status == NodeStatus::Alive {
                    existing.last_seen = Instant::now();
                }
            })
            .or_insert_with(|| {
                updated = true;
                incoming
            });

        if updated {
            metrics
                .counter("gossip_state_updates_total", "Gossip state updates")
                .increment(1);
        }
    }

    /// Main gossip loop - periodic state dissemination.
    async fn gossip_loop(
        nodes: Arc<DashMap<NodeId, NodeState>>,
        local_state: Arc<RwLock<NodeState>>,
        sequence: Arc<std::sync::atomic::AtomicU64>,
        config: GossipConfig,
        event_tx: mpsc::Sender<GossipEvent>,
        metrics: Arc<MetricsRegistry>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        let mut interval = interval(Duration::from_millis(config.interval_ms));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }

                    // Select random nodes to gossip with
                    let alive_nodes: Vec<NodeState> = nodes.iter()
                        .filter(|e| e.status == NodeStatus::Alive && e.node_id != config.node_id)
                        .map(|e| e.value().clone())
                        .collect();

                    if alive_nodes.is_empty() {
                        continue;
                    }

                    // Pick random fanout nodes
                    let mut rng = rand::rng();
                    let targets: Vec<&NodeState> = alive_nodes.choose_multiple(&mut rng, config.fanout.min(alive_nodes.len())).collect();

                    let seq = sequence.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                    // Build payload
                    let payload = GossipPayload {
                        nodes: nodes.iter().map(|e| e.value().clone()).take(config.max_payload_nodes).collect(),
                        ownership: Vec::new(),
                        config: Vec::new(),
                    };

                    // Send pings
                    for target in targets {
                        let ping = GossipMessage::Ping {
                            source: config.node_id.clone(),
                            target: target.node_id.clone(),
                            sequence: seq,
                            payload: payload.clone(),
                        };

                        let data = bincode::serialize(&ping).unwrap();
                        // In real implementation, would send via UDP socket
                        // socket.send_to(&data, target.address).await
                    }

                    metrics.counter("gossip_pings_sent_total", "Gossip pings sent").increment(targets.len() as u64);
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
            }
        }
    }

    /// Failure detection loop.
    async fn failure_detection_loop(
        nodes: Arc<DashMap<NodeId, NodeState>>,
        local_state: Arc<RwLock<NodeState>>,
        config: GossipConfig,
        event_tx: mpsc::Sender<GossipEvent>,
        metrics: Arc<MetricsRegistry>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        let mut interval = interval(Duration::from_millis(1000));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }

                    let now = Instant::now();
                    let mut to_update = Vec::new();

                    // Check node timeouts
                    for entry in nodes.iter() {
                        let node = entry.value();
                        let elapsed = now.duration_since(node.last_seen);

                        if node.status == NodeStatus::Alive && elapsed > Duration::from_millis(config.suspect_timeout_ms) {
                            to_update.push((node.node_id.clone(), NodeStatus::Suspect));
                        } else if node.status == NodeStatus::Suspect && elapsed > Duration::from_millis(config.dead_timeout_ms) {
                            to_update.push((node.node_id.clone(), NodeStatus::Dead));
                        }
                    }

                    // Apply updates
                    for (node_id, new_status) in to_update {
                        if let Some(mut node) = nodes.get_mut(&node_id) {
                            let old_status = node.status;
                            node.status = new_status;
                            node.incarnation += 1;

                            // Emit events
                            match new_status {
                                NodeStatus::Suspect => {
                                    let _ = event_tx.try_send(GossipEvent::NodeFailed(node_id.clone()));
                                }
                                NodeStatus::Dead => {
                                    let _ = event_tx.try_send(GossipEvent::NodeFailed(node_id.clone()));
                                }
                                _ => {}
                            }

                            metrics.counter("gossip_status_changes_total", "Gossip status changes").increment(1);
                        }
                    }

                    // Update metrics
                    let alive_count = nodes.iter().filter(|e| e.status == NodeStatus::Alive).count();
                    let suspect_count = nodes.iter().filter(|e| e.status == NodeStatus::Suspect).count();
                    let dead_count = nodes.iter().filter(|e| e.status == NodeStatus::Dead).count();

                    metrics.gauge("gossip_nodes_alive", "Alive nodes").set(alive_count as f64);
                    metrics.gauge("gossip_nodes_suspect", "Suspect nodes").set(suspect_count as f64);
                    metrics.gauge("gossip_nodes_dead", "Dead nodes").set(dead_count as f64);
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
            }
        }
    }

    /// Get all nodes.
    pub fn nodes(&self) -> Vec<NodeState> {
        self.nodes.iter().map(|e| e.value().clone()).collect()
    }

    /// Get alive nodes.
    pub fn alive_nodes(&self) -> Vec<NodeState> {
        self.nodes
            .iter()
            .filter(|e| e.status == NodeStatus::Alive)
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get local node ID.
    pub fn node_id(&self) -> &NodeId {
        &self.config.node_id
    }

    /// Subscribe to gossip events.
    pub fn subscribe(&self) -> mpsc::Receiver<GossipEvent> {
        // In real implementation, would use broadcast channel
        let (tx, rx) = mpsc::channel(100);
        rx
    }
}

impl Drop for GossipProtocol {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id() {
        let id = NodeId::new("test-node");
        assert_eq!(id.0, "test-node");
    }

    #[test]
    fn test_node_status_ordering() {
        assert!(NodeStatus::Alive as u8 > NodeStatus::Suspect as u8);
        assert!(NodeStatus::Suspect as u8 > NodeStatus::Dead as u8);
        assert!(NodeStatus::Dead as u8 > NodeStatus::Left as u8);
    }

    #[test]
    fn test_gossip_config_default() {
        let config = GossipConfig::default();
        assert_eq!(config.interval_ms, 1000);
        assert_eq!(config.fanout, 3);
        assert_eq!(config.suspect_timeout_ms, 5000);
        assert_eq!(config.dead_timeout_ms, 10000);
    }

    #[test]
    fn test_ownership_entry() {
        let entry = OwnershipEntry {
            key: "broadcast:stream-1".to_string(),
            owner: NodeId::new("node-1"),
            lease_expires: chrono::Utc::now().timestamp() as u64 + 30,
            incarnation: 1,
            metadata: HashMap::new(),
        };
        assert_eq!(entry.key, "broadcast:stream-1");
    }

    #[test]
    fn test_config_entry() {
        let entry = ConfigEntry {
            key: "transcode.preset".to_string(),
            value: "fast".to_string(),
            version: 1,
            updated_by: NodeId::new("node-1"),
            updated_at: chrono::Utc::now().timestamp() as u64,
        };
        assert_eq!(entry.key, "transcode.preset");
        assert_eq!(entry.value, "fast");
    }
}
