//! Ownership Manager - Broadcast ownership with lease renewal
//!
//! Manages ownership of broadcasts/streams across the cluster.
//! Uses leases with automatic renewal and failover.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, info, warn};

use crate::gossip::{GossipPayload, GossipProtocol, NodeId, OwnershipEntry};
use mediaedge_observability::MetricsRegistry;

/// Ownership manager configuration.
#[derive(Debug, Clone)]
pub struct OwnershipConfig {
    pub lease_duration_secs: u64,
    pub renewal_interval_secs: u64,
    pub max_ownership_per_node: usize,
}

impl Default for OwnershipConfig {
    fn default() -> Self {
        Self {
            lease_duration_secs: 30,
            renewal_interval_secs: 10,
            max_ownership_per_node: 1000,
        }
    }
}

/// Ownership state for a key.
#[derive(Debug, Clone)]
pub struct OwnershipState {
    pub key: String,
    pub owner: Option<NodeId>,
    pub lease_expires: Option<Instant>,
    pub incarnation: u64,
    pub metadata: HashMap<String, String>,
    pub claimants: Vec<NodeId>, // Nodes wanting ownership
}

/// Ownership manager.
pub struct OwnershipManager {
    config: OwnershipConfig,
    gossip: Arc<RwLock<Option<Arc<GossipProtocol>>>>,
    ownership: Arc<DashMap<String, OwnershipState>>,
    local_ownership: Arc<DashMap<String, OwnershipState>>, // Keys we own
    event_tx: mpsc::Sender<OwnershipEvent>,
    event_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<OwnershipEvent>>>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    metrics: Arc<MetricsRegistry>,
}

/// Ownership events.
#[derive(Debug, Clone)]
pub enum OwnershipEvent {
    Acquired { key: String },
    Released { key: String },
    Lost { key: String, new_owner: NodeId },
    Renewed { key: String },
    Failed { key: String, reason: String },
}

impl OwnershipManager {
    /// Create a new ownership manager.
    pub fn new(metrics: Arc<MetricsRegistry>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(1000);
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);

        Self {
            config: OwnershipConfig::default(),
            gossip: Arc::new(RwLock::new(None)),
            ownership: Arc::new(DashMap::new()),
            local_ownership: Arc::new(DashMap::new()),
            event_tx,
            event_rx: Arc::new(tokio::sync::Mutex::new(event_rx)),
            shutdown_tx,
            metrics,
        }
    }

    /// Set the gossip protocol.
    pub fn set_gossip(&self, gossip: Arc<GossipProtocol>) {
        *self.gossip.write() = Some(gossip);
    }

    /// Start the ownership manager.
    pub async fn start(&self) -> anyhow::Result<()> {
        info!("Starting ownership manager");

        // Start lease renewal loop
        self.start_renewal_loop().await;

        // Start lease monitoring loop
        self.start_monitoring_loop().await;

        Ok(())
    }

    /// Stop the ownership manager.
    pub async fn stop(&self) -> anyhow::Result<()> {
        info!("Stopping ownership manager");
        let _ = self.shutdown_tx.send(true);

        // Release all local ownership
        let keys: Vec<String> = self
            .local_ownership
            .iter()
            .map(|e| e.key().clone())
            .collect();
        for key in keys {
            self.release(&key).await?;
        }

        Ok(())
    }

    /// Try to acquire ownership of a key.
    pub async fn acquire(
        &self,
        key: &str,
        metadata: HashMap<String, String>,
    ) -> anyhow::Result<bool> {
        let node_id = self.get_node_id().await?;

        // Check if already owned locally
        if self.local_ownership.contains_key(key) {
            return Ok(true);
        }

        // Check current ownership
        if let Some(state) = self.ownership.get(key) {
            if let Some(owner) = &state.owner {
                if owner == &node_id {
                    // We already own it (maybe from gossip)
                    self.claim_local(key, state.clone()).await?;
                    return Ok(true);
                }

                // Check if lease expired
                if let Some(expires) = state.lease_expires {
                    if Instant::now() > expires {
                        // Lease expired, we can claim
                    } else {
                        // Someone else owns it with valid lease
                        return Ok(false);
                    }
                }
            }
        }

        // Claim ownership
        self.claim_local(
            key,
            OwnershipState {
                key: key.to_string(),
                owner: Some(node_id.clone()),
                lease_expires: Some(
                    Instant::now() + Duration::from_secs(self.config.lease_duration_secs),
                ),
                incarnation: 1,
                metadata,
                claimants: Vec::new(),
            },
        )
        .await?;

        // Broadcast ownership via gossip
        self.broadcast_ownership(key).await?;

        let _ = self.event_tx.try_send(OwnershipEvent::Acquired {
            key: key.to_string(),
        });
        self.metrics
            .counter("ownership_acquired_total", "Ownership acquired")
            .increment(1);

        Ok(true)
    }

    /// Release ownership of a key.
    pub async fn release(&self, key: &str) -> anyhow::Result<()> {
        if self.local_ownership.remove(key).is_some() {
            // Update state to released
            if let Some(mut state) = self.ownership.get_mut(key) {
                state.owner = None;
                state.lease_expires = None;
                state.incarnation += 1;
            }

            // Broadcast release
            self.broadcast_ownership(key).await?;

            let _ = self.event_tx.try_send(OwnershipEvent::Released {
                key: key.to_string(),
            });
            self.metrics
                .counter("ownership_released_total", "Ownership released")
                .increment(1);
        }
        Ok(())
    }

    /// Force release ownership (e.g., on node shutdown).
    pub async fn force_release_all(&self) -> anyhow::Result<()> {
        let keys: Vec<String> = self
            .local_ownership
            .iter()
            .map(|e| e.key().clone())
            .collect();
        for key in keys {
            self.release(&key).await?;
        }
        Ok(())
    }

    /// Get current owner of a key.
    pub fn get_owner(&self, key: &str) -> Option<NodeId> {
        self.ownership.get(key).and_then(|s| s.owner.clone())
    }

    /// Check if we own a key.
    pub fn is_owner(&self, key: &str) -> bool {
        self.local_ownership.contains_key(key)
    }

    /// Get all keys we own.
    pub fn owned_keys(&self) -> Vec<String> {
        self.local_ownership
            .iter()
            .map(|e| e.key().clone())
            .collect()
    }

    /// Get ownership state for a key.
    pub fn get_state(&self, key: &str) -> Option<OwnershipState> {
        self.ownership.get(key).map(|s| s.clone())
    }

    /// Renew lease for a key we own.
    async fn renew_lease(&self, key: &str) -> anyhow::Result<()> {
        if let Some(mut state) = self.local_ownership.get_mut(key) {
            state.lease_expires =
                Some(Instant::now() + Duration::from_secs(self.config.lease_duration_secs));
            state.incarnation += 1;

            // Update global state
            if let Some(mut global_state) = self.ownership.get_mut(key) {
                global_state.lease_expires = state.lease_expires;
                global_state.incarnation = state.incarnation;
            }

            // Broadcast renewal
            self.broadcast_ownership(key).await?;

            let _ = self.event_tx.try_send(OwnershipEvent::Renewed {
                key: key.to_string(),
            });
            self.metrics
                .counter("ownership_renewed_total", "Ownership renewed")
                .increment(1);
        }
        Ok(())
    }

    /// Start lease renewal loop.
    async fn start_renewal_loop(&self) {
        let local_ownership = self.local_ownership.clone();
        let ownership = self.ownership.clone();
        let config = self.config.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let gossip = self.gossip.clone();
        let metrics = self.metrics.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.renewal_interval_secs));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }

                        let keys: Vec<String> = local_ownership.iter().map(|e| e.key().clone()).collect();
                        for key in keys {
                            if let Err(e) = Self::renew_lease_static(&local_ownership, &ownership, &gossip, &key, &config).await {
                                warn!("Failed to renew lease for {}: {}", key, e);
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
    }

    /// Static version for use in spawned task.
    async fn renew_lease_static(
        local_ownership: &Arc<DashMap<String, OwnershipState>>,
        ownership: &Arc<DashMap<String, OwnershipState>>,
        gossip: &Arc<RwLock<Option<Arc<GossipProtocol>>>>,
        key: &str,
        config: &OwnershipConfig,
    ) -> anyhow::Result<()> {
        if let Some(mut state) = local_ownership.get_mut(key) {
            state.lease_expires =
                Some(Instant::now() + Duration::from_secs(config.lease_duration_secs));
            state.incarnation += 1;

            if let Some(mut global_state) = ownership.get_mut(key) {
                global_state.lease_expires = state.lease_expires;
                global_state.incarnation = state.incarnation;
            }
        }
        Ok(())
    }

    /// Start lease monitoring loop (detect expired leases).
    async fn start_monitoring_loop(&self) {
        let ownership = self.ownership.clone();
        let local_ownership = self.local_ownership.clone();
        let event_tx = self.event_tx.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let metrics = self.metrics.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }

                        let now = Instant::now();
                        let mut expired_keys = Vec::new();

                        // Find expired leases
                        for entry in ownership.iter() {
                            let state = entry.value();
                            if let Some(expires) = state.lease_expires {
                                if now > expires && state.owner.is_some() {
                                    expired_keys.push((entry.key().clone(), state.owner.clone().unwrap()));
                                }
                            }
                        }

                        // Process expired leases
                        for (key, old_owner) in expired_keys {
                            if let Some(mut state) = ownership.get_mut(&key) {
                                state.owner = None;
                                state.lease_expires = None;
                                state.incarnation += 1;

                                // If we were the owner, we lost it
                                if local_ownership.remove(&key).is_some() {
                                    let _ = event_tx.try_send(OwnershipEvent::Lost {
                                        key: key.clone(),
                                        new_owner: NodeId::new("unknown")
                                    });
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
    }

    /// Claim ownership locally.
    async fn claim_local(&self, key: &str, state: OwnershipState) -> anyhow::Result<()> {
        self.local_ownership.insert(key.to_string(), state.clone());
        self.ownership.insert(key.to_string(), state);
        Ok(())
    }

    /// Broadcast ownership state via gossip.
    async fn broadcast_ownership(&self, key: &str) -> anyhow::Result<()> {
        if let Some(gossip_guard) = self.gossip.read().as_ref() {
            // In real implementation, would add to gossip payload
            // The gossip protocol will pick it up in its next payload
        }
        Ok(())
    }

    /// Update ownership from gossip payload.
    pub fn update_from_gossip(&self, entries: Vec<OwnershipEntry>) {
        for entry in entries {
            let mut state =
                self.ownership
                    .entry(entry.key.clone())
                    .or_insert_with(|| OwnershipState {
                        key: entry.key.clone(),
                        owner: None,
                        lease_expires: None,
                        incarnation: 0,
                        metadata: HashMap::new(),
                        claimants: Vec::new(),
                    });

            // LWW based on incarnation
            if entry.incarnation > state.incarnation {
                state.owner = Some(entry.owner);
                state.lease_expires = Some(Instant::now() + Duration::from_secs(30));
                state.incarnation = entry.incarnation;
                state.metadata = entry.metadata;
            }
        }
    }

    /// Get node ID from gossip.
    async fn get_node_id(&self) -> anyhow::Result<NodeId> {
        if let Some(gossip) = self.gossip.read().as_ref() {
            Ok(gossip.node_id().clone())
        } else {
            Err(anyhow::anyhow!("Gossip not initialized"))
        }
    }

    /// Subscribe to ownership events.
    pub fn subscribe(&self) -> mpsc::Receiver<OwnershipEvent> {
        let (tx, rx) = mpsc::channel(100);
        rx
    }

    /// Get ownership stats.
    pub fn stats(&self) -> OwnershipStats {
        let total = self.ownership.len();
        let local = self.local_ownership.len();
        let expired = self
            .ownership
            .iter()
            .filter(|e| e.lease_expires.map_or(false, |exp| Instant::now() > exp))
            .count();

        OwnershipStats {
            total_keys: total,
            local_keys: local,
            expired_keys: expired,
        }
    }
}

/// Ownership statistics.
#[derive(Debug, Clone, Serialize)]
pub struct OwnershipStats {
    pub total_keys: usize,
    pub local_keys: usize,
    pub expired_keys: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ownership_config_default() {
        let config = OwnershipConfig::default();
        assert_eq!(config.lease_duration_secs, 30);
        assert_eq!(config.renewal_interval_secs, 10);
    }

    #[test]
    fn test_ownership_state() {
        let state = OwnershipState {
            key: "test".to_string(),
            owner: Some(NodeId::new("node-1")),
            lease_expires: Some(Instant::now() + Duration::from_secs(30)),
            incarnation: 1,
            metadata: HashMap::new(),
            claimants: Vec::new(),
        };
        assert!(state.owner.is_some());
    }
}
