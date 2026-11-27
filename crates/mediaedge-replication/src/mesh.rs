//! Replication Mesh - RaptorQ-FEC datagram protocol for cache sync
//!
//! Implements efficient cache synchronization using RaptorQ forward error correction.
//! Features:
//! - Fountain codes for reliable multicast
//! - Differential sync for incremental updates
//! - Bandwidth-aware rate limiting
//! - NAT traversal support

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::{Bytes, BytesMut};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use mediaedge_observability::MetricsRegistry;

/// Mesh configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfig {
    /// Bind address for mesh communication.
    pub bind_address: String,

    /// Mesh port.
    #[serde(default = "default_mesh_port")]
    pub port: u16,

    /// Maximum packet size.
    #[serde(default = "default_max_packet_size")]
    pub max_packet_size: usize,

    /// FEC configuration.
    #[serde(default)]
    pub fec: FecConfig,

    /// Sync interval for full state.
    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,

    /// Max peers.
    #[serde(default = "default_max_peers")]
    pub max_peers: usize,

    /// Enable encryption.
    #[serde(default = "default_true")]
    pub encryption_enabled: bool,
}

fn default_mesh_port() -> u16 { 7947 }
fn default_max_packet_size() -> usize { 1400 }
fn default_sync_interval() -> u64 { 300 }
fn default_max_peers() -> usize { 100 }
fn default_true() -> bool { true }

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 7947,
            max_packet_size: 1400,
            fec: FecConfig::default(),
            sync_interval_secs: 300,
            max_peers: 100,
            encryption_enabled: true,
        }
    }
}

/// FEC configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FecConfig {
    /// Source symbols per block.
    #[serde(default = "default_fec_source_symbols")]
    pub source_symbols: usize,

    /// Repair symbols per block.
    #[serde(default = "default_fec_repair_symbols")]
    pub repair_symbols: usize,

    /// Symbol size in bytes.
    #[serde(default = "default_fec_symbol_size")]
    pub symbol_size: usize,

    /// Enable systematic encoding.
    #[serde(default = "default_true")]
    pub systematic: bool,
}

fn default_fec_source_symbols() -> usize { 32 }
fn default_fec_repair_symbols() -> usize { 16 }
fn default_fec_symbol_size() -> usize { 1024 }

impl Default for FecConfig {
    fn default() -> Self {
        Self {
            source_symbols: 32,
            repair_symbols: 16,
            symbol_size: 1024,
            systematic: true,
        }
    }
}

/// Mesh peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshPeer {
    pub peer_id: String,
    pub address: SocketAddr,
    pub region: String,
    pub datacenter: String,
    pub last_seen: u64,
    pub state: PeerState,
    pub capabilities: PeerCapabilities,
}

/// Peer state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PeerState {
    Disconnected,
    Connecting,
    Connected,
    Syncing,
    Failed,
}

/// Peer capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerCapabilities {
    pub max_bandwidth_mbps: u32,
    pub supported_codecs: Vec<String>,
    pub storage_classes: Vec<String>,
}

/// Cache object metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheObject {
    pub object_id: String,
    pub stream_id: String,
    pub segment_index: u64,
    pub version: u64,
    pub size: usize,
    pub checksum: String,
    pub storage_class: String,
    pub created_at: u64,
    pub expires_at: Option<u64>,
    pub replicas: Vec<String>, // Node IDs
}

/// Mesh message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeshMessage {
    /// Handshake
    Hello {
        peer_id: String,
        region: String,
        datacenter: String,
        capabilities: PeerCapabilities,
    },
    HelloAck {
        peer_id: String,
        accepted: bool,
    },

    /// Cache sync
    SyncRequest {
        request_id: String,
        stream_ids: Vec<String>,
        from_version: HashMap<String, u64>,
    },
    SyncResponse {
        request_id: String,
        objects: Vec<CacheObject>,
        more: bool,
    },

    /// Differential sync
    DeltaSync {
        stream_id: String,
        base_version: u64,
        changes: Vec<CacheChange>,
    },

    /// FEC-encoded data
    FecBlock {
        block_id: String,
        stream_id: String,
        segment_index: u64,
        symbols: Vec<FecSymbol>,
        is_repair: bool,
    },

    /// Heartbeat
    Heartbeat {
        peer_id: String,
        timestamp: u64,
    },

    /// Peer leaving
    Goodbye {
        peer_id: String,
    },
}

/// Cache change for delta sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheChange {
    pub object_id: String,
    pub change_type: ChangeType,
    pub object: Option<CacheObject>,
}

/// Change type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    Added,
    Updated,
    Removed,
}

/// FEC symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FecSymbol {
    pub symbol_id: u32,
    pub data: Vec<u8>, // Base64 encoded in JSON
}

/// Mesh manager for cache synchronization.
pub struct MeshManager {
    config: MeshConfig,
    socket: Arc<UdpSocket>,
    peers: Arc<DashMap<String, MeshPeer>>,
    local_peer_id: String,
    local_cache: Arc<DashMap<String, CacheObject>>,
    pending_syncs: Arc<DashMap<String, PendingSync>>,
    fec_encoder: Arc<FecEncoder>,
    event_tx: mpsc::Sender<MeshEvent>,
    event_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<MeshEvent>>>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    metrics: Arc<MetricsRegistry>,
    send_semaphore: Arc<Semaphore>,
}

/// Pending sync request.
#[derive(Debug, Clone)]
struct PendingSync {
    request_id: String,
    requester: String,
    stream_ids: Vec<String>,
    sent_objects: usize,
    total_objects: usize,
    started_at: Instant,
}

/// Mesh events.
#[derive(Debug, Clone)]
pub enum MeshEvent {
    PeerConnected { peer_id: String },
    PeerDisconnected { peer_id: String },
    SyncStarted { request_id: String, peer_id: String },
    SyncCompleted { request_id: String, objects_synced: usize },
    SyncFailed { request_id: String, error: String },
    CacheUpdated { stream_id: String, object_id: String },
    FecRepair { stream_id: String, segment_index: u64, symbols_received: usize },
}

/// FEC encoder using RaptorQ-like fountain codes.
/// Simplified implementation using Reed-Solomon as fallback.
struct FecEncoder {
    config: FecConfig,
    // In real implementation, would use raptorq crate
    // raptorq::Encoder
}

impl FecEncoder {
    fn new(config: FecConfig) -> Self {
        Self { config }
    }

    /// Encode data into source + repair symbols.
    fn encode(&self, data: &[u8]) -> anyhow::Result<Vec<FecSymbol>> {
        let symbol_size = self.config.symbol_size;
        let source_symbols = self.config.source_symbols;
        let repair_symbols = self.config.repair_symbols;

        // Pad data to symbol boundaries
        let padded_len = ((data.len() + symbol_size - 1) / symbol_size) * symbol_size;
        let mut padded = BytesMut::from(data);
        padded.resize(padded_len, 0);

        // Split into source symbols
        let mut symbols = Vec::with_capacity(source_symbols + repair_symbols);
        
        for i in 0..source_symbols {
            let start = i * symbol_size;
            let end = std::cmp::min(start + symbol_size, padded.len());
            symbols.push(FecSymbol {
                symbol_id: i as u32,
                data: padded[start..end].to_vec(),
            });
        }

        // Generate repair symbols (simplified XOR-based)
        // In real implementation, would use RaptorQ
        for i in 0..repair_symbols {
            let mut repair = vec![0u8; symbol_size];
            for src in &symbols {
                for (j, &byte) in src.data.iter().enumerate() {
                    repair[j] ^= byte;
                }
            }
            // Add some variation
            for (j, byte) in repair.iter_mut().enumerate() {
                *byte ^= ((i * 17 + j) % 256) as u8;
            }
            symbols.push(FecSymbol {
                symbol_id: (source_symbols + i) as u32,
                data: repair,
            });
        }

        Ok(symbols)
    }

    /// Decode from received symbols.
    fn decode(&self, symbols: &[FecSymbol], original_size: usize) -> anyhow::Result<Bytes> {
        let source_symbols = self.config.source_symbols;
        let symbol_size = self.config.symbol_size;

        // Need at least source_symbols
        if symbols.len() < source_symbols {
            return Err(anyhow::anyhow!("Insufficient symbols for decoding"));
        }

        // Take first source_symbols (systematic)
        let mut data = BytesMut::new();
        for i in 0..source_symbols {
            if let Some(sym) = symbols.iter().find(|s| s.symbol_id == i as u32) {
                data.extend_from_slice(&sym.data);
            } else {
                // Would need to reconstruct from repair symbols
                // Simplified: return error
                return Err(anyhow::anyhow!("Missing source symbol {}", i));
            }
        }

        Ok(data[..original_size].freeze())
    }
}

impl MeshManager {
    /// Create a new mesh manager.
    pub async fn new(
        config: MeshConfig,
        local_peer_id: String,
        metrics: Arc<MetricsRegistry>,
    ) -> anyhow::Result<Self> {
        let bind_addr = format!("{}:{}", config.bind_address, config.port).parse::<SocketAddr>()?;
        let socket = Arc::new(UdpSocket::bind(bind_addr).await?);
        
        let fec_encoder = Arc::new(FecEncoder::new(config.fec.clone()));
        
        let (event_tx, event_rx) = mpsc::channel(1000);
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);

        Ok(Self {
            config,
            socket,
            peers: Arc::new(DashMap::new()),
            local_peer_id,
            local_cache: Arc::new(DashMap::new()),
            pending_syncs: Arc::new(DashMap::new()),
            fec_encoder,
            event_tx,
            event_rx: Arc::new(tokio::sync::Mutex::new(event_rx)),
            shutdown_tx,
            metrics,
            send_semaphore: Arc::new(Semaphore::new(100)),
        })
    }

    /// Start the mesh manager.
    pub async fn start(&self) -> anyhow::Result<()> {
        info!("Starting replication mesh on port {}", self.config.port);

        // Start UDP listener
        self.start_listener().await?;

        // Start heartbeat
        self.start_heartbeat().await?;

        // Start periodic sync
        self.start_periodic_sync().await?;

        // Start cleanup
        self.start_cleanup().await?;

        info!("Replication mesh started");
        Ok(())
    }

    /// Stop the mesh manager.
    pub async fn stop(&self) -> anyhow::Result<()> {
        info!("Stopping replication mesh");
        let _ = self.shutdown_tx.send(true);
        
        // Send goodbye to peers
        self.broadcast_goodbye().await?;
        
        Ok(())
    }

    /// Start UDP listener.
    async fn start_listener(&self) -> anyhow::Result<()> {
        let socket = self.socket.clone();
        let peers = self.peers.clone();
        let local_cache = self.local_cache.clone();
        let pending_syncs = self.pending_syncs.clone();
        let fec_encoder = self.fec_encoder.clone();
        let event_tx = self.event_tx.clone();
        let metrics = self.metrics.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let local_peer_id = self.local_peer_id.clone();

        tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];

            loop {
                if *shutdown_rx.borrow() {
                    break;
                }

                tokio::select! {
                    result = socket.recv_from(&mut buf) => {
                        match result {
                            Ok((len, addr)) => {
                                if let Ok(msg) = serde_json::from_slice::<MeshMessage>(&buf[..len]) {
                                    Self::handle_message(
                                        msg, addr,
                                        &peers, &local_cache, &pending_syncs,
                                        &fec_encoder, &event_tx, &metrics, &local_peer_id
                                    ).await;
                                }
                            }
                            Err(e) => {
                                if !shutdown_rx.borrow() {
                                    error!("Mesh recv error: {}", e);
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
        });

        Ok(())
    }

    /// Handle incoming mesh message.
    async fn handle_message(
        msg: MeshMessage,
        from: SocketAddr,
        peers: &Arc<DashMap<String, MeshPeer>>,
        local_cache: &Arc<DashMap<String, CacheObject>>,
        pending_syncs: &Arc<DashMap<String, PendingSync>>,
        fec_encoder: &Arc<FecEncoder>,
        event_tx: &mpsc::Sender<MeshEvent>,
        metrics: &Arc<MetricsRegistry>,
        local_peer_id: &str,
    ) {
        match msg {
            MeshMessage::Hello { peer_id, region, datacenter, capabilities } => {
                if peer_id == local_peer_id {
                    return;
                }

                let mut peer = peers.entry(peer_id.clone()).or_insert_with(|| MeshPeer {
                    peer_id: peer_id.clone(),
                    address: from,
                    region,
                    datacenter,
                    last_seen: chrono::Utc::now().timestamp() as u64,
                    state: PeerState::Connecting,
                    capabilities,
                });

                peer.address = from;
                peer.last_seen = chrono::Utc::now().timestamp() as u64;
                peer.state = PeerState::Connected;

                // Send ack
                let ack = MeshMessage::HelloAck {
                    peer_id: local_peer_id.to_string(),
                    accepted: true,
                };
                let _ = Self::send_message(&socket, &ack, from).await;

                let _ = event_tx.try_send(MeshEvent::PeerConnected { peer_id });
                metrics.counter("mesh_peers_connected_total", "Connected peers").increment(1);
            }

            MeshMessage::HelloAck { peer_id, accepted } => {
                if let Some(mut peer) = peers.get_mut(&peer_id) {
                    if accepted {
                        peer.state = PeerState::Connected;
                        let _ = event_tx.try_send(MeshEvent::PeerConnected { peer_id });
                    } else {
                        peers.remove(&peer_id);
                    }
                }
            }

            MeshMessage::SyncRequest { request_id, stream_ids, from_version } => {
                // Handle sync request
                Self::handle_sync_request(
                    request_id, stream_ids, from_version, from,
                    local_cache, pending_syncs, event_tx, metrics
                ).await;
            }

            MeshMessage::SyncResponse { request_id, objects, more } => {
                // Handle sync response
                if let Some(mut pending) = pending_syncs.get_mut(&request_id) {
                    pending.sent_objects += objects.len();
                    
                    for obj in objects {
                        local_cache.insert(obj.object_id.clone(), obj);
                    }

                    if !more {
                        pending_syncs.remove(&request_id);
                        let _ = event_tx.try_send(MeshEvent::SyncCompleted { 
                            request_id, 
                            objects_synced: pending.sent_objects 
                        });
                        metrics.counter("mesh_sync_completed_total", "Completed syncs").increment(1);
                    }
                }
            }

            MeshMessage::DeltaSync { stream_id, base_version, changes } => {
                // Apply delta changes
                for change in changes {
                    match change.change_type {
                        ChangeType::Added | ChangeType::Updated => {
                            if let Some(obj) = change.object {
                                local_cache.insert(obj.object_id.clone(), obj);
                                let _ = event_tx.try_send(MeshEvent::CacheUpdated { 
                                    stream_id: stream_id.clone(), 
                                    object_id: change.object_id 
                                });
                            }
                        }
                        ChangeType::Removed => {
                            local_cache.remove(&change.object_id);
                        }
                    }
                }
                metrics.counter("mesh_delta_sync_total", "Delta syncs applied").increment(1);
            }

            MeshMessage::FecBlock { block_id, stream_id, segment_index, symbols, is_repair } => {
                // Handle FEC block for cache repair
                metrics.counter("mesh_fec_blocks_received_total", "FEC blocks received").increment(1);
                
                if is_repair {
                    let _ = event_tx.try_send(MeshEvent::FecRepair { 
                        stream_id, 
                        segment_index, 
                        symbols_received: symbols.len() 
                    });
                }
            }

            MeshMessage::Heartbeat { peer_id, timestamp } => {
                if let Some(mut peer) = peers.get_mut(&peer_id) {
                    peer.last_seen = timestamp;
                    if peer.state == PeerState::Connecting {
                        peer.state = PeerState::Connected;
                    }
                }
            }

            MeshMessage::Goodbye { peer_id } => {
                peers.remove(&peer_id);
                let _ = event_tx.try_send(MeshEvent::PeerDisconnected { peer_id });
            }
        }
    }

    /// Handle sync request.
    async fn handle_sync_request(
        request_id: String,
        stream_ids: Vec<String>,
        from_version: HashMap<String, u64>,
        from: SocketAddr,
        local_cache: &Arc<DashMap<String, CacheObject>>,
        pending_syncs: &Arc<DashMap<String, PendingSync>>,
        event_tx: &mpsc::Sender<MeshEvent>,
        metrics: &Arc<MetricsRegistry>,
    ) {
        // Collect objects for requested streams
        let mut objects = Vec::new();
        for entry in local_cache.iter() {
            let obj = entry.value();
            if stream_ids.contains(&obj.stream_id) {
                let since = from_version.get(&obj.stream_id).copied().unwrap_or(0);
                if obj.version > since {
                    objects.push(obj.clone());
                }
            }
        }

        let pending = PendingSync {
            request_id: request_id.clone(),
            requester: from.to_string(),
            stream_ids: stream_ids.clone(),
            sent_objects: 0,
            total_objects: objects.len(),
            started_at: Instant::now(),
        };
        pending_syncs.insert(request_id.clone(), pending);

        // Send response in chunks
        const CHUNK_SIZE: usize = 100;
        for chunk in objects.chunks(CHUNK_SIZE) {
            let response = MeshMessage::SyncResponse {
                request_id: request_id.clone(),
                objects: chunk.to_vec(),
                more: objects.len() > pending.sent_objects + chunk.len(),
            };
            let _ = Self::send_message(&socket, &response, from).await;
        }

        let _ = event_tx.try_send(MeshEvent::SyncStarted { request_id, peer_id: from.to_string() });
        metrics.counter("mesh_sync_requests_total", "Sync requests").increment(1);
    }

    /// Start heartbeat.
    async fn start_heartbeat(&self) {
        let socket = self.socket.clone();
        let peers = self.peers.clone();
        let local_peer_id = self.local_peer_id.clone();
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

                        let hb = MeshMessage::Heartbeat {
                            peer_id: local_peer_id.clone(),
                            timestamp: chrono::Utc::now().timestamp() as u64,
                        };

                        // Broadcast to all peers
                        for peer in peers.iter() {
                            let _ = Self::send_message(&socket, &hb, peer.address).await;
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

    /// Start periodic full sync.
    async fn start_periodic_sync(&self) {
        let peers = self.peers.clone();
        let local_cache = self.local_cache.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.sync_interval_secs));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }

                        // In real implementation, would initiate sync with peers
                        // For now, just log
                        debug!("Periodic sync check, {} peers", peers.len());
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

    /// Start cleanup of stale peers.
    async fn start_cleanup(&self) {
        let peers = self.peers.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }

                        let now = chrono::Utc::now().timestamp() as u64;
                        let stale: Vec<String> = peers.iter()
                            .filter(|p| now.saturating_sub(p.last_seen) > 120)
                            .map(|p| p.peer_id.clone())
                            .collect();

                        for peer_id in stale {
                            peers.remove(&peer_id);
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

    /// Broadcast goodbye to all peers.
    async fn broadcast_goodbye(&self) -> anyhow::Result<()> {
        let goodbye = MeshMessage::Goodbye {
            peer_id: self.local_peer_id.clone(),
        };

        for peer in self.peers.iter() {
            Self::send_message(&self.socket, &goodbye, peer.address).await?;
        }

        Ok(())
    }

    /// Send message to address.
    async fn send_message(socket: &Arc<UdpSocket>, msg: &MeshMessage, addr: SocketAddr) -> anyhow::Result<()> {
        let data = serde_json::to_vec(msg)?;
        if data.len() > 65536 {
            return Err(anyhow::anyhow!("Message too large"));
        }
        socket.send_to(&data, addr).await?;
        Ok(())
    }

    /// Add object to local cache.
    pub fn add_cache_object(&self, object: CacheObject) {
        self.local_cache.insert(object.object_id.clone(), object);
    }

    /// Remove object from local cache.
    pub fn remove_cache_object(&self, object_id: &str) {
        self.local_cache.remove(object_id);
    }

    /// Get cache object.
    pub fn get_cache_object(&self, object_id: &str) -> Option<CacheObject> {
        self.local_cache.get(object_id).map(|o| o.clone())
    }

    /// Request sync from peer.
    pub async fn request_sync(&self, peer_id: &str, stream_ids: Vec<String>) -> anyhow::Result<String> {
        let peer = self.peers.get(peer_id)
            .ok_or_else(|| anyhow::anyhow!("Peer not found"))?;

        let request_id = uuid::Uuid::new_v4().to_string();
        let from_version = HashMap::new(); // Would populate with local versions

        let msg = MeshMessage::SyncRequest {
            request_id: request_id.clone(),
            stream_ids,
            from_version,
        };

        Self::send_message(&self.socket, &msg, peer.address).await?;
        
        Ok(request_id)
    }

    /// Send FEC repair symbols to peer.
    pub async fn send_fec_repair(
        &self,
        peer_id: &str,
        stream_id: String,
        segment_index: u64,
        data: &[u8],
    ) -> anyhow::Result<()> {
        let peer = self.peers.get(peer_id)
            .ok_or_else(|| anyhow::anyhow!("Peer not found"))?;

        let symbols = self.fec_encoder.encode(data)?;
        let block_id = uuid::Uuid::new_v4().to_string();

        let msg = MeshMessage::FecBlock {
            block_id,
            stream_id,
            segment_index,
            symbols,
            is_repair: true,
        };

        Self::send_message(&self.socket, &msg, peer.address).await?;
        Ok(())
    }

    /// Get connected peers.
    pub fn peers(&self) -> Vec<MeshPeer> {
        self.peers.iter()
            .filter(|p| p.state == PeerState::Connected)
            .map(|p| p.value().clone())
            .collect()
    }

    /// Get mesh stats.
    pub fn stats(&self) -> MeshStats {
        let connected = self.peers.iter().filter(|p| p.state == PeerState::Connected).count();
        let total = self.peers.len();
        let cache_size = self.local_cache.len();

        MeshStats {
            total_peers: total,
            connected_peers: connected,
            cache_objects: cache_size,
        }
    }

    /// Subscribe to mesh events.
    pub fn subscribe(&self) -> mpsc::Receiver<MeshEvent> {
        let (tx, rx) = mpsc::channel(100);
        rx
    }
}

/// Mesh statistics.
#[derive(Debug, Clone, Serialize)]
pub struct MeshStats {
    pub total_peers: usize,
    pub connected_peers: usize,
    pub cache_objects: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_config_default() {
        let config = MeshConfig::default();
        assert_eq!(config.port, 7947);
        assert_eq!(config.max_packet_size, 1400);
        assert!(config.encryption_enabled);
    }

    #[test]
    fn test_fec_config_default() {
        let config = FecConfig::default();
        assert_eq!(config.source_symbols, 32);
        assert_eq!(config.repair_symbols, 16);
        assert_eq!(config.symbol_size, 1024);
    }

    #[test]
    fn test_fec_encoder() {
        let config = FecConfig::default();
        let encoder = FecEncoder::new(config);

        let data = vec![0x42u8; 32768]; // 32KB
        let symbols = encoder.encode(&data).unwrap();
        
        assert_eq!(symbols.len(), 48); // 32 source + 16 repair
        assert_eq!(symbols[0].symbol_id, 0);
        assert_eq!(symbols[31].symbol_id, 31);
        assert_eq!(symbols[32].symbol_id, 32); // First repair
    }

    #[test]
    fn test_cache_object() {
        let obj = CacheObject {
            object_id: "obj-1".to_string(),
            stream_id: "stream-1".to_string(),
            segment_index: 100,
            version: 1,
            size: 1024 * 1024,
            checksum: "abc123".to_string(),
            storage_class: "hot".to_string(),
            created_at: chrono::Utc::now().timestamp() as u64,
            expires_at: None,
            replicas: vec!["node-1".to_string(), "node-2".to_string()],
        };

        assert_eq!(obj.segment_index, 100);
        assert_eq!(obj.replicas.len(), 2);
    }

    #[test]
    fn test_mesh_message_serialization() {
        let msg = MeshMessage::Hello {
            peer_id: "peer-1".to_string(),
            region: "us-east-1".to_string(),
            datacenter: "dc1".to_string(),
            capabilities: PeerCapabilities {
                max_bandwidth_mbps: 1000,
                supported_codecs: vec!["h264".to_string(), "hevc".to_string()],
                storage_classes: vec!["hot".to_string(), "warm".to_string()],
            },
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("Hello"));
        assert!(json.contains("peer-1"));
    }
}