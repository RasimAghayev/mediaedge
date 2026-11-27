//! Mesh Module - Browser Peer Mesh (WebRTC DataChannel)
//!
//! Implements WebRTC-based peer-to-peer mesh for CDN cost reduction
//! and edge offload. Features:
//! - Topology planner (capacity-aware)
//! - WebSocket signaling + server-side subscriber registration
//! - WebRTC DataChannel parent/child relay
//! - Client SDK integration points
//! - TURN deployment support

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock as TokioRwLock, mpsc};
use tokio::time::interval;
use tracing::{debug, info, warn};

use mediaedge_observability::MetricsRegistry;

/// Mesh node identifier.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeshNodeId(pub String);

impl MeshNodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Mesh peer state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshPeer {
    pub node_id: MeshNodeId,
    pub session_id: String,
    pub role: PeerRole,
    pub capacity: PeerCapacity,
    pub connected_at: u64,
    pub last_seen: u64,
    pub latency_ms: u32,
    pub streams_relayed: Vec<String>,
    pub parent: Option<MeshNodeId>,
    pub children: Vec<MeshNodeId>,
}

/// Peer role in mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PeerRole {
    Root,       // Origin server / entry point
    Parent,     // Relays to children
    Child,      // Receives from parent
    Standalone, // Not in mesh
}

/// Peer capacity for topology planning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerCapacity {
    pub max_bandwidth_mbps: u32,
    pub max_peers: u32,
    pub current_bandwidth_mbps: u32,
    pub current_peers: u32,
    pub cpu_cores: u32,
    pub memory_mb: u32,
    pub gpu_available: bool,
    pub region: String,
    pub datacenter: String,
}

impl PeerCapacity {
    /// Calculate available capacity score (0.0 - 1.0).
    pub fn capacity_score(&self) -> f32 {
        let bw_score =
            1.0 - (self.current_bandwidth_mbps as f32 / self.max_bandwidth_mbps.max(1) as f32);
        let peer_score = 1.0 - (self.current_peers as f32 / self.max_peers.max(1) as f32);
        (bw_score * 0.6 + peer_score * 0.4).clamp(0.0, 1.0)
    }

    /// Check if peer can accept more children.
    pub fn can_accept_children(&self, count: u32) -> bool {
        self.current_peers + count <= self.max_peers
    }
}

/// Mesh topology planner - builds optimal mesh topology.
pub struct TopologyPlanner {
    config: MeshPlannerConfig,
    peers: Arc<DashMap<MeshNodeId, MeshPeer>>,
    metrics: Arc<MetricsRegistry>,
}

/// Mesh planner configuration.
#[derive(Debug, Clone)]
pub struct MeshPlannerConfig {
    pub max_depth: u32,
    pub max_children_per_parent: u32,
    pub prefer_same_region: bool,
    pub prefer_same_datacenter: bool,
    pub min_capacity_score: f32,
    pub rebalance_interval_secs: u64,
}

impl Default for MeshPlannerConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_children_per_parent: 10,
            prefer_same_region: true,
            prefer_same_datacenter: true,
            min_capacity_score: 0.3,
            rebalance_interval_secs: 60,
        }
    }
}

impl TopologyPlanner {
    /// Create a new topology planner.
    pub fn new(config: MeshPlannerConfig, metrics: Arc<MetricsRegistry>) -> Self {
        Self {
            config,
            peers: Arc::new(DashMap::new()),
            metrics,
        }
    }

    /// Register a peer.
    pub fn register_peer(&self, peer: MeshPeer) {
        self.peers.insert(peer.node_id.clone(), peer);
        self.metrics
            .gauge("mesh_peers_total", "Total mesh peers")
            .increment(1);
    }

    /// Unregister a peer.
    pub fn unregister_peer(&self, node_id: &MeshNodeId) {
        if self.peers.remove(node_id).is_some() {
            self.metrics
                .gauge("mesh_peers_total", "Total mesh peers")
                .decrement(1);
        }
    }

    /// Update peer capacity.
    pub fn update_capacity(&self, node_id: &MeshNodeId, capacity: PeerCapacity) {
        if let Some(mut peer) = self.peers.get_mut(node_id) {
            peer.capacity = capacity;
        }
    }

    /// Get best parent for a new peer.
    pub fn find_best_parent(&self, new_peer: &MeshPeer) -> Option<MeshNodeId> {
        let mut candidates: Vec<&MeshPeer> = self
            .peers
            .iter()
            .filter(|p| p.value().role == PeerRole::Root || p.value().role == PeerRole::Parent)
            .filter(|p| p.value().capacity.can_accept_children(1))
            .filter(|p| p.value().capacity.capacity_score() >= self.config.min_capacity_score)
            .map(|p| p.value())
            .collect();

        // Sort by preference
        candidates.sort_by(|a, b| {
            let a_score = self.calculate_parent_score(a, new_peer);
            let b_score = self.calculate_parent_score(b, new_peer);
            b_score.partial_cmp(&a_score).unwrap()
        });

        candidates.first().map(|p| p.node_id.clone())
    }

    /// Calculate parent score for a new peer.
    fn calculate_parent_score(&self, parent: &MeshPeer, new_peer: &MeshPeer) -> f32 {
        let mut score = parent.capacity.capacity_score() * 100.0;

        // Prefer same region
        if self.config.prefer_same_region && parent.capacity.region == new_peer.capacity.region {
            score += 50.0;
        }

        // Prefer same datacenter
        if self.config.prefer_same_datacenter
            && parent.capacity.datacenter == new_peer.capacity.datacenter
        {
            score += 30.0;
        }

        // Prefer lower depth
        let depth = self.get_peer_depth(&parent.node_id);
        score -= depth as f32 * 10.0;

        // Prefer fewer children
        score -= parent.children.len() as f32 * 5.0;

        score
    }

    /// Get peer depth in tree.
    fn get_peer_depth(&self, node_id: &MeshNodeId) -> u32 {
        let mut depth = 0;
        let mut current = node_id.clone();

        while let Some(peer) = self.peers.get(&current) {
            if let Some(parent_id) = &peer.parent {
                current = parent_id.clone();
                depth += 1;
            } else {
                break;
            }

            if depth > self.config.max_depth {
                break;
            }
        }

        depth
    }

    /// Rebalance mesh topology.
    pub fn rebalance(&self) -> Vec<RebalanceAction> {
        let mut actions = Vec::new();

        // Find overloaded parents
        for peer in self.peers.iter() {
            if peer.role == PeerRole::Parent || peer.role == PeerRole::Root {
                let capacity = &peer.capacity;
                if capacity.current_peers > capacity.max_peers {
                    // Need to move some children to other parents
                    let excess = capacity.current_peers - capacity.max_peers;

                    // Find children to move (prefer those with lowest capacity score)
                    let mut children: Vec<MeshNodeId> = peer.children.clone();
                    children.sort_by(|a, b| {
                        let a_score = self
                            .peers
                            .get(a)
                            .map(|p| p.capacity.capacity_score())
                            .unwrap_or(1.0);
                        let b_score = self
                            .peers
                            .get(b)
                            .map(|p| p.capacity.capacity_score())
                            .unwrap_or(1.0);
                        a_score.partial_cmp(&b_score).unwrap()
                    });

                    for child_id in children.into_iter().take(excess as usize) {
                        if let Some(new_parent) = self.find_best_parent_for_child(&child_id) {
                            actions.push(RebalanceAction {
                                child_id: child_id.clone(),
                                old_parent: peer.node_id.clone(),
                                new_parent: new_parent.clone(),
                                reason: "Parent overloaded".to_string(),
                            });
                        }
                    }
                }
            }
        }

        actions
    }

    /// Find best parent for a specific child.
    fn find_best_parent_for_child(&self, child_id: &MeshNodeId) -> Option<MeshNodeId> {
        let child = self.peers.get(child_id)?;
        self.find_best_parent(child.value())
    }

    /// Get all peers.
    pub fn get_peers(&self) -> Vec<MeshPeer> {
        self.peers.iter().map(|p| p.value().clone()).collect()
    }

    /// Get mesh topology as tree.
    pub fn get_topology(&self) -> MeshTopology {
        let mut roots = Vec::new();
        let mut nodes = HashMap::new();

        for peer in self.peers.iter() {
            let node = TopologyNode {
                node_id: peer.node_id.clone(),
                role: peer.role,
                capacity: peer.capacity.clone(),
                children: peer.children.clone(),
                parent: peer.parent.clone(),
            };
            nodes.insert(peer.node_id.clone(), node);

            if peer.parent.is_none() && peer.role != PeerRole::Standalone {
                roots.push(peer.node_id.clone());
            }
        }

        MeshTopology { roots, nodes }
    }
}

/// Rebalance action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceAction {
    pub child_id: MeshNodeId,
    pub old_parent: MeshNodeId,
    pub new_parent: MeshNodeId,
    pub reason: String,
}

/// Mesh topology tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshTopology {
    pub roots: Vec<MeshNodeId>,
    pub nodes: HashMap<MeshNodeId, TopologyNode>,
}

/// Topology node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyNode {
    pub node_id: MeshNodeId,
    pub role: PeerRole,
    pub capacity: PeerCapacity,
    pub children: Vec<MeshNodeId>,
    pub parent: Option<MeshNodeId>,
}

/// Mesh signaling manager - handles WebSocket signaling.
pub struct SignalingManager {
    config: SignalingConfig,
    sessions: Arc<DashMap<String, SignalingSession>>,
    planner: Arc<TopologyPlanner>,
    metrics: Arc<MetricsRegistry>,
}

/// Signaling configuration.
#[derive(Debug, Clone)]
pub struct SignalingConfig {
    pub websocket_path: String,
    pub heartbeat_interval_secs: u64,
    pub session_timeout_secs: u64,
}

impl Default for SignalingConfig {
    fn default() -> Self {
        Self {
            websocket_path: "/mesh/signaling".to_string(),
            heartbeat_interval_secs: 10,
            session_timeout_secs: 60,
        }
    }
}

/// Signaling session.
#[derive(Debug, Clone)]
pub struct SignalingSession {
    pub session_id: String,
    pub node_id: MeshNodeId,
    pub peer_role: PeerRole,
    pub connected_at: Instant,
    pub last_heartbeat: Instant,
    pub sdp_offer: Option<String>,
    pub sdp_answer: Option<String>,
    pub ice_candidates: Vec<String>,
}

impl SignalingManager {
    /// Create a new signaling manager.
    pub fn new(
        config: SignalingConfig,
        planner: Arc<TopologyPlanner>,
        metrics: Arc<MetricsRegistry>,
    ) -> Self {
        Self {
            config,
            sessions: Arc::new(DashMap::new()),
            planner,
            metrics,
        }
    }

    /// Handle new WebSocket connection.
    pub async fn handle_connection(
        &self,
        session_id: String,
        node_id: MeshNodeId,
        capacity: PeerCapacity,
    ) -> anyhow::Result<SignalingSession> {
        // Determine role based on capacity and current topology
        let role = self.determine_role(&node_id, &capacity).await;

        let session = SignalingSession {
            session_id: session_id.clone(),
            node_id: node_id.clone(),
            peer_role: role,
            connected_at: Instant::now(),
            last_heartbeat: Instant::now(),
            sdp_offer: None,
            sdp_answer: None,
            ice_candidates: Vec::new(),
        };

        // Register peer in planner
        let peer = MeshPeer {
            node_id: node_id.clone(),
            session_id: session_id.clone(),
            role,
            capacity,
            connected_at: chrono::Utc::now().timestamp() as u64,
            last_seen: chrono::Utc::now().timestamp() as u64,
            latency_ms: 0,
            streams_relayed: Vec::new(),
            parent: None,
            children: Vec::new(),
        };

        self.planner.register_peer(peer);
        self.sessions.insert(session_id.clone(), session.clone());

        self.metrics
            .counter("mesh_signaling_sessions_total", "Total signaling sessions")
            .increment(1);

        Ok(session)
    }

    /// Determine peer role.
    async fn determine_role(&self, node_id: &MeshNodeId, capacity: &PeerCapacity) -> PeerRole {
        let peers = self.planner.get_peers();

        // If no peers, become root
        if peers.is_empty() {
            return PeerRole::Root;
        }

        // If capacity is 1, must be child (leaf)
        if capacity.max_peers <= 1 {
            return PeerRole::Child;
        }

        // Check if we can be a parent
        let root_count = peers.iter().filter(|p| p.role == PeerRole::Root).count();
        let parent_count = peers.iter().filter(|p| p.role == PeerRole::Parent).count();

        // Limit number of roots and parents
        if root_count == 0 {
            PeerRole::Root
        } else if parent_count < 3 && capacity.capacity_score() > 0.5 {
            PeerRole::Parent
        } else {
            PeerRole::Child
        }
    }

    /// Handle SDP offer.
    pub fn handle_offer(&self, session_id: &str, offer: String) -> anyhow::Result<()> {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.sdp_offer = Some(offer);
            session.last_heartbeat = Instant::now();
        }
        Ok(())
    }

    /// Handle SDP answer.
    pub fn handle_answer(&self, session_id: &str, answer: String) -> anyhow::Result<()> {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.sdp_answer = Some(answer);
            session.last_heartbeat = Instant::now();
        }
        Ok(())
    }

    /// Handle ICE candidate.
    pub fn handle_ice_candidate(&self, session_id: &str, candidate: String) -> anyhow::Result<()> {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.ice_candidates.push(candidate);
            session.last_heartbeat = Instant::now();
        }
        Ok(())
    }

    /// Handle heartbeat.
    pub fn handle_heartbeat(&self, session_id: &str) -> anyhow::Result<()> {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.last_heartbeat = Instant::now();
        }
        Ok(())
    }

    /// Get session.
    pub fn get_session(&self, session_id: &str) -> Option<SignalingSession> {
        self.sessions.get(session_id).map(|s| s.clone())
    }

    /// Remove session.
    pub fn remove_session(&self, session_id: &str) {
        if let Some((_, session)) = self.sessions.remove(session_id) {
            self.planner.unregister_peer(&session.node_id);
            self.metrics
                .counter(
                    "mesh_signaling_sessions_removed_total",
                    "Removed signaling sessions",
                )
                .increment(1);
        }
    }

    /// Clean up stale sessions.
    pub fn cleanup_stale(&self) {
        let now = Instant::now();
        let timeout = Duration::from_secs(self.config.session_timeout_secs);

        let stale: Vec<String> = self
            .sessions
            .iter()
            .filter(|s| now.duration_since(s.last_heartbeat) > timeout)
            .map(|s| s.session_id.clone())
            .collect();

        for session_id in stale {
            self.remove_session(&session_id);
        }
    }
}

/// WebRTC DataChannel manager for mesh relay.
pub struct WebRTCMeshManager {
    config: WebRTCMeshConfig,
    connections: Arc<DashMap<String, WebRTCConnection>>,
    metrics: Arc<MetricsRegistry>,
}

/// WebRTC mesh configuration.
#[derive(Debug, Clone)]
pub struct WebRTCMeshConfig {
    pub ice_servers: Vec<IceServerConfig>,
    pub max_data_channels: u32,
    pub data_channel_buffer_size: usize,
    pub reconnect_timeout_secs: u64,
}

impl Default for WebRTCMeshConfig {
    fn default() -> Self {
        Self {
            ice_servers: vec![IceServerConfig {
                urls: vec!["stun:stun.l.google.com:19302".to_string()],
                username: None,
                credential: None,
            }],
            max_data_channels: 100,
            data_channel_buffer_size: 65536,
            reconnect_timeout_secs: 30,
        }
    }
}

/// ICE server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceServerConfig {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

/// WebRTC connection.
#[derive(Debug, Clone)]
pub struct WebRTCConnection {
    pub connection_id: String,
    pub local_node_id: MeshNodeId,
    pub remote_node_id: MeshNodeId,
    pub role: PeerRole,
    pub data_channels: HashMap<String, DataChannelState>,
    pub connected_at: Instant,
    pub last_activity: Instant,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Data channel state.
#[derive(Debug, Clone)]
pub struct DataChannelState {
    pub label: String,
    pub stream_id: String,
    pub ordered: bool,
    pub max_retransmits: Option<u16>,
    pub state: DataChannelConnectionState,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Data channel connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataChannelConnectionState {
    Connecting,
    Open,
    Closing,
    Closed,
    Error,
}

impl WebRTCMeshManager {
    /// Create a new WebRTC mesh manager.
    pub fn new(config: WebRTCMeshConfig, metrics: Arc<MetricsRegistry>) -> Self {
        Self {
            config,
            connections: Arc::new(DashMap::new()),
            metrics,
        }
    }

    /// Create a new parent connection (outgoing to parent).
    pub async fn connect_to_parent(
        &self,
        local_node_id: MeshNodeId,
        parent_node_id: MeshNodeId,
        signaling_server: String,
    ) -> anyhow::Result<String> {
        // In real implementation:
        // 1. Connect to signaling server
        // 2. Create PeerConnection with ICE servers
        // 3. Create data channels for each stream
        // 4. Exchange SDP offer/answer via signaling
        // 5. Handle ICE candidates

        let connection_id = uuid::Uuid::new_v4().to_string();

        let connection = WebRTCConnection {
            connection_id: connection_id.clone(),
            local_node_id: local_node_id.clone(),
            remote_node_id: parent_node_id.clone(),
            role: PeerRole::Child,
            data_channels: HashMap::new(),
            connected_at: Instant::now(),
            last_activity: Instant::now(),
            bytes_sent: 0,
            bytes_received: 0,
        };

        self.connections.insert(connection_id.clone(), connection);
        self.metrics
            .counter("mesh_webrtc_connections_total", "Total WebRTC connections")
            .increment(1);

        info!(
            "WebRTC mesh connection initiated: {} -> {} (child)",
            local_node_id.0, parent_node_id.0
        );

        Ok(connection_id)
    }

    /// Accept incoming child connection (incoming from child).
    pub async fn accept_child(
        &self,
        local_node_id: MeshNodeId,
        child_node_id: MeshNodeId,
    ) -> anyhow::Result<String> {
        let connection_id = uuid::Uuid::new_v4().to_string();

        let connection = WebRTCConnection {
            connection_id: connection_id.clone(),
            local_node_id: local_node_id.clone(),
            remote_node_id: child_node_id.clone(),
            role: PeerRole::Parent,
            data_channels: HashMap::new(),
            connected_at: Instant::now(),
            last_activity: Instant::now(),
            bytes_sent: 0,
            bytes_received: 0,
        };

        self.connections.insert(connection_id.clone(), connection);
        self.metrics
            .counter("mesh_webrtc_connections_total", "Total WebRTC connections")
            .increment(1);

        info!(
            "WebRTC mesh connection accepted: {} <- {} (parent)",
            local_node_id.0, child_node_id.0
        );

        Ok(connection_id)
    }

    /// Create data channel for a stream.
    pub fn create_data_channel(
        &self,
        connection_id: &str,
        stream_id: String,
        ordered: bool,
        max_retransmits: Option<u16>,
    ) -> anyhow::Result<()> {
        if let Some(mut conn) = self.connections.get_mut(connection_id) {
            let channel = DataChannelState {
                label: format!("stream-{}", stream_id),
                stream_id: stream_id.clone(),
                ordered,
                max_retransmits,
                state: DataChannelConnectionState::Connecting,
                messages_sent: 0,
                messages_received: 0,
                bytes_sent: 0,
                bytes_received: 0,
            };

            conn.data_channels.insert(stream_id, channel);
        }
        Ok(())
    }

    /// Send data on a stream.
    pub fn send_stream_data(
        &self,
        connection_id: &str,
        stream_id: &str,
        data: &[u8],
    ) -> anyhow::Result<()> {
        if let Some(mut conn) = self.connections.get_mut(connection_id) {
            if let Some(channel) = conn.data_channels.get_mut(stream_id) {
                if channel.state == DataChannelConnectionState::Open {
                    channel.messages_sent += 1;
                    channel.bytes_sent += data.len() as u64;
                    conn.bytes_sent += data.len() as u64;
                    conn.last_activity = Instant::now();

                    // In real implementation: send via WebRTC DataChannel
                    // dc.send(data).await?;
                }
            }
        }
        Ok(())
    }

    /// Handle incoming data.
    pub fn handle_incoming_data(&self, connection_id: &str, stream_id: &str, data: &[u8]) {
        if let Some(mut conn) = self.connections.get_mut(connection_id) {
            if let Some(channel) = conn.data_channels.get_mut(stream_id) {
                channel.messages_received += 1;
                channel.bytes_received += data.len() as u64;
                conn.bytes_received += data.len() as u64;
                conn.last_activity = Instant::now();
            }
        }
    }

    /// Close connection.
    pub fn close_connection(&self, connection_id: &str) {
        if let Some((_, conn)) = self.connections.remove(connection_id) {
            self.metrics
                .counter(
                    "mesh_webrtc_connections_closed_total",
                    "Closed WebRTC connections",
                )
                .increment(1);
            info!("WebRTC mesh connection closed: {}", connection_id);
        }
    }

    /// Get connection stats.
    pub fn get_stats(&self) -> WebRTCMeshStats {
        let total = self.connections.len();
        let mut by_role = HashMap::new();
        let mut total_bytes_sent = 0;
        let mut total_bytes_received = 0;
        let mut total_data_channels = 0;

        for conn in self.connections.iter() {
            *by_role.entry(conn.role).or_insert(0) += 1;
            total_bytes_sent += conn.bytes_sent;
            total_bytes_received += conn.bytes_received;
            total_data_channels += conn.data_channels.len();
        }

        WebRTCMeshStats {
            total_connections: total,
            by_role,
            total_data_channels,
            total_bytes_sent,
            total_bytes_received,
        }
    }
}

/// WebRTC mesh statistics.
#[derive(Debug, Clone, Serialize)]
pub struct WebRTCMeshStats {
    pub total_connections: usize,
    pub by_role: HashMap<PeerRole, usize>,
    pub total_data_channels: usize,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
}

/// Mesh manager - coordinates all mesh components.
pub struct MeshManager {
    planner: Arc<TopologyPlanner>,
    signaling: Arc<SignalingManager>,
    webrtc: Arc<WebRTCMeshManager>,
    config: MeshManagerConfig,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    metrics: Arc<MetricsRegistry>,
}

/// Mesh manager configuration.
#[derive(Debug, Clone)]
pub struct MeshManagerConfig {
    pub enabled: bool,
    pub max_peers: u32,
    pub root_peer_count: u32,
    pub planner_config: MeshPlannerConfig,
    pub signaling_config: SignalingConfig,
    pub webrtc_config: WebRTCMeshConfig,
}

impl Default for MeshManagerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_peers: 1000,
            root_peer_count: 3,
            planner_config: MeshPlannerConfig::default(),
            signaling_config: SignalingConfig::default(),
            webrtc_config: WebRTCMeshConfig::default(),
        }
    }
}

impl MeshManager {
    /// Create a new mesh manager.
    pub fn new(config: MeshManagerConfig, metrics: Arc<MetricsRegistry>) -> Self {
        let planner = Arc::new(TopologyPlanner::new(
            config.planner_config.clone(),
            metrics.clone(),
        ));
        let signaling = Arc::new(SignalingManager::new(
            config.signaling_config.clone(),
            planner.clone(),
            metrics.clone(),
        ));
        let webrtc = Arc::new(WebRTCMeshManager::new(
            config.webrtc_config.clone(),
            metrics.clone(),
        ));

        let (shutdown_tx, _) = tokio::sync::watch::channel(false);

        Self {
            planner,
            signaling,
            webrtc,
            config,
            shutdown_tx,
            metrics,
        }
    }

    /// Start the mesh manager.
    pub async fn start(&self) -> anyhow::Result<()> {
        if !self.config.enabled {
            info!("Mesh disabled, not starting");
            return Ok(());
        }

        info!("Starting mesh manager");

        // Start rebalancing loop
        self.start_rebalance_loop().await;

        // Start cleanup loop
        self.start_cleanup_loop().await;

        Ok(())
    }

    /// Stop the mesh manager.
    pub async fn stop(&self) -> anyhow::Result<()> {
        info!("Stopping mesh manager");
        let _ = self.shutdown_tx.send(true);
        Ok(())
    }

    /// Get planner.
    pub fn planner(&self) -> Arc<TopologyPlanner> {
        self.planner.clone()
    }

    /// Get signaling manager.
    pub fn signaling(&self) -> Arc<SignalingManager> {
        self.signaling.clone()
    }

    /// Get WebRTC manager.
    pub fn webrtc(&self) -> Arc<WebRTCMeshManager> {
        self.webrtc.clone()
    }

    /// Start rebalancing loop.
    async fn start_rebalance_loop(&self) {
        let planner = self.planner.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let config = self.config.clone();
        let metrics = self.metrics.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(
                config.planner_config.rebalance_interval_secs,
            ));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }

                        let actions = planner.rebalance();
                        for action in actions {
                            info!("Rebalancing: move {} from {} to {} ({})",
                                action.child_id.0, action.old_parent.0, action.new_parent.0, action.reason);

                            // In real implementation: trigger WebRTC reconnection
                            metrics.counter("mesh_rebalance_actions_total", "Rebalance actions").increment(1);
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

    /// Start cleanup loop.
    async fn start_cleanup_loop(&self) {
        let signaling = self.signaling.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(
                config.signaling_config.heartbeat_interval_secs,
            ));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }
                        signaling.cleanup_stale();
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

    /// Get mesh stats.
    pub fn stats(&self) -> MeshStats {
        MeshStats {
            planner: self.planner.get_peers().len(),
            signaling: self.signaling.sessions.len(),
            webrtc: self.webrtc.get_stats(),
        }
    }
}

/// Mesh statistics.
#[derive(Debug, Clone, Serialize)]
pub struct MeshStats {
    pub planner: usize,
    pub signaling: usize,
    pub webrtc: WebRTCMeshStats,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_capacity_score() {
        let capacity = PeerCapacity {
            max_bandwidth_mbps: 1000,
            max_peers: 10,
            current_bandwidth_mbps: 500,
            current_peers: 5,
            cpu_cores: 8,
            memory_mb: 16384,
            gpu_available: false,
            region: "us-east-1".to_string(),
            datacenter: "dc1".to_string(),
        };

        let score = capacity.capacity_score();
        assert!((score - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_can_accept_children() {
        let capacity = PeerCapacity {
            max_bandwidth_mbps: 1000,
            max_peers: 10,
            current_bandwidth_mbps: 100,
            current_peers: 5,
            cpu_cores: 8,
            memory_mb: 16384,
            gpu_available: false,
            region: "us-east-1".to_string(),
            datacenter: "dc1".to_string(),
        };

        assert!(capacity.can_accept_children(3));
        assert!(capacity.can_accept_children(5));
        assert!(!capacity.can_accept_children(6));
    }

    #[test]
    fn test_mesh_planner_config_default() {
        let config = MeshPlannerConfig::default();
        assert_eq!(config.max_depth, 3);
        assert_eq!(config.max_children_per_parent, 10);
        assert!(config.prefer_same_region);
    }

    #[test]
    fn test_ice_server_config() {
        let ice = IceServerConfig {
            urls: vec!["stun:stun.example.com:19302".to_string()],
            username: None,
            credential: None,
        };
        assert_eq!(ice.urls.len(), 1);
    }

    #[test]
    fn test_webrtc_mesh_config_default() {
        let config = WebRTCMeshConfig::default();
        assert_eq!(config.ice_servers.len(), 1);
        assert_eq!(config.max_data_channels, 100);
    }
}
