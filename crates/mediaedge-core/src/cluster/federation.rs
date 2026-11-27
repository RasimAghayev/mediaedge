//! Federation Module - Cross-Cluster Federation
//!
//! Implements multi-region, multi-provider federation for disaster recovery.
//! Features:
//! - FederationLink: authenticated one-way MoQ pulls
//! - Exponential backoff reconnect with jitter
//! - mTLS between federated clusters
//! - Stream mirroring with offset compensation
//! - Per-link state monitoring

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use mediaedge_config::types::FederationConfig;
use mediaedge_observability::MetricsRegistry;

/// Federation link state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationLinkState {
    pub link_id: String,
    pub remote_cluster_id: String,
    pub remote_endpoint: String,
    pub status: FederationLinkStatus,
    pub last_connected: Option<u64>, // Unix timestamp
    pub last_error: Option<String>,
    pub reconnect_attempts: u32,
    pub next_reconnect_at: Option<u64>, // Unix timestamp
    pub streams_mirrored: Vec<String>,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Federation link status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FederationLinkStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
    Paused,
}

/// Federation link configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationLinkConfig {
    pub link_id: String,
    pub remote_cluster_id: String,
    pub remote_endpoint: String,
    pub streams: Vec<String>, // Stream patterns to mirror
    pub mTLS_enabled: bool,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub ca_path: Option<String>,
    pub reconnect_base_delay_secs: u64,
    pub reconnect_max_delay_secs: u64,
    pub reconnect_jitter_percent: u8,
    pub health_check_interval_secs: u64,
    pub offset_compensation_ms: i64,
}

/// Default federation link config.
impl Default for FederationLinkConfig {
    fn default() -> Self {
        Self {
            link_id: String::new(),
            remote_cluster_id: String::new(),
            remote_endpoint: String::new(),
            streams: Vec::new(),
            mTLS_enabled: true,
            cert_path: None,
            key_path: None,
            ca_path: None,
            reconnect_base_delay_secs: 1,
            reconnect_max_delay_secs: 60,
            reconnect_jitter_percent: 10,
            health_check_interval_secs: 30,
            offset_compensation_ms: 0,
        }
    }
}

/// Federation manager for cross-cluster replication.
pub struct FederationManager {
    config: FederationConfig,
    links: Arc<DashMap<String, FederationLink>>,
    local_cluster_id: String,
    event_tx: mpsc::Sender<FederationEvent>,
    event_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<FederationEvent>>>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    metrics: Arc<MetricsRegistry>,
}

/// Individual federation link.
struct FederationLink {
    config: FederationLinkConfig,
    state: FederationLinkState,
    connection: Option<FederationConnection>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
}

/// Federation connection (MoQ-based).
struct FederationConnection {
    // In real implementation: MoQ session, tracks, subscriptions
    connected_at: Instant,
    last_health_check: Instant,
}

/// Federation events.
#[derive(Debug, Clone)]
pub enum FederationEvent {
    LinkConnected { link_id: String },
    LinkDisconnected { link_id: String, reason: String },
    LinkFailed { link_id: String, error: String },
    StreamMirrored { link_id: String, stream_id: String },
    StreamUnmirrored { link_id: String, stream_id: String },
    ConfigChanged { link_id: String },
}

impl FederationManager {
    /// Create a new federation manager.
    pub fn new(
        config: FederationConfig,
        local_cluster_id: String,
        metrics: Arc<MetricsRegistry>,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::channel(1000);
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);

        Self {
            config,
            links: Arc::new(DashMap::new()),
            local_cluster_id,
            event_tx,
            event_rx: Arc::new(tokio::sync::Mutex::new(event_rx)),
            shutdown_tx,
            metrics,
        }
    }

    /// Add a federation link.
    pub async fn add_link(&self, config: FederationLinkConfig) -> anyhow::Result<()> {
        info!(
            "Adding federation link: {} -> {}",
            config.link_id, config.remote_cluster_id
        );

        let state = FederationLinkState {
            link_id: config.link_id.clone(),
            remote_cluster_id: config.remote_cluster_id.clone(),
            remote_endpoint: config.remote_endpoint.clone(),
            status: FederationLinkStatus::Disconnected,
            last_connected: None,
            last_error: None,
            reconnect_attempts: 0,
            next_reconnect_at: None,
            streams_mirrored: Vec::new(),
            bytes_received: 0,
            bytes_sent: 0,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };

        let (shutdown_tx, _) = tokio::sync::watch::channel(false);

        let link = FederationLink {
            config: config.clone(),
            state,
            connection: None,
            shutdown_tx,
        };

        self.links.insert(config.link_id.clone(), link);

        // Start link connection task
        self.start_link_task(config.link_id.clone()).await;

        self.metrics
            .counter("federation_links_total", "Total federation links")
            .increment(1);
        Ok(())
    }

    /// Remove a federation link.
    pub async fn remove_link(&self, link_id: &str) -> anyhow::Result<()> {
        info!("Removing federation link: {}", link_id);

        if let Some((_, link)) = self.links.remove(link_id) {
            let _ = link.shutdown_tx.send(true);

            // Disconnect if connected
            if let Some(conn) = link.connection {
                // In real implementation: close MoQ connection
            }
        }

        self.metrics
            .counter("federation_links_removed_total", "Removed federation links")
            .increment(1);
        Ok(())
    }

    /// Pause a federation link.
    pub async fn pause_link(&self, link_id: &str) -> anyhow::Result<()> {
        if let Some(mut link) = self.links.get_mut(link_id) {
            link.state.status = FederationLinkStatus::Paused;
            link.state.updated_at = chrono::Utc::now().timestamp() as u64;

            // Disconnect if connected
            if let Some(conn) = link.connection.take() {
                // In real implementation: close MoQ connection
            }
        }
        Ok(())
    }

    /// Resume a federation link.
    pub async fn resume_link(&self, link_id: &str) -> anyhow::Result<()> {
        if let Some(mut link) = self.links.get_mut(link_id) {
            link.state.status = FederationLinkStatus::Disconnected;
            link.state.updated_at = chrono::Utc::now().timestamp() as u64;
            link.state.reconnect_attempts = 0;
            link.state.next_reconnect_at = None;
        }
        self.start_link_task(link_id.to_string()).await;
        Ok(())
    }

    /// Start the link connection task.
    async fn start_link_task(&self, link_id: String) {
        let links = self.links.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let metrics = self.metrics.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            Self::link_connection_loop(links, link_id, shutdown_rx, metrics, event_tx).await;
        });
    }

    /// Main connection loop for a federation link.
    async fn link_connection_loop(
        links: Arc<DashMap<String, FederationLink>>,
        link_id: String,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
        metrics: Arc<MetricsRegistry>,
        event_tx: mpsc::Sender<FederationEvent>,
    ) {
        let mut reconnect_interval = interval(Duration::from_secs(1));
        reconnect_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = reconnect_interval.tick() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }

                    let should_reconnect = {
                        let link = match links.get(&link_id) {
                            Some(l) => l,
                            None => break, // Link removed
                        };

                        match link.state.status {
                            FederationLinkStatus::Disconnected | FederationLinkStatus::Reconnecting => {
                                // Check if it's time to reconnect
                                let now = chrono::Utc::now().timestamp() as u64;
                                if let Some(next_reconnect) = link.state.next_reconnect_at {
                                    now >= next_reconnect
                                } else {
                                    true // First attempt
                                }
                            }
                            FederationLinkStatus::Paused => false,
                            _ => false, // Already connected or failed permanently
                        }
                    };

                    if should_reconnect {
                        Self::attempt_connection(&links, &link_id, &metrics, &event_tx).await;
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

    /// Attempt to connect to the remote cluster.
    async fn attempt_connection(
        links: &Arc<DashMap<String, FederationLink>>,
        link_id: &str,
        metrics: &Arc<MetricsRegistry>,
        event_tx: &mpsc::Sender<FederationEvent>,
    ) {
        let mut link = match links.get_mut(link_id) {
            Some(l) => l,
            None => return,
        };

        link.state.status = FederationLinkStatus::Connecting;
        link.state.updated_at = chrono::Utc::now().timestamp() as u64;

        info!(
            "Attempting federation connection to: {}",
            link.config.remote_endpoint
        );

        // In real implementation: establish MoQ connection with mTLS
        // For now, simulate connection
        let connection_result = Self::establish_moq_connection(&link.config).await;

        match connection_result {
            Ok(connection) => {
                link.connection = Some(connection);
                link.state.status = FederationLinkStatus::Connected;
                link.state.last_connected = Some(chrono::Utc::now().timestamp() as u64);
                link.state.last_error = None;
                link.state.reconnect_attempts = 0;
                link.state.next_reconnect_at = None;
                link.state.updated_at = chrono::Utc::now().timestamp() as u64;

                // Subscribe to streams
                Self::subscribe_to_streams(&mut link).await;

                let _ = event_tx.try_send(FederationEvent::LinkConnected {
                    link_id: link_id.to_string(),
                });

                metrics
                    .counter("federation_connections_total", "Federation connections")
                    .increment(1);

                // Start health check loop
                Self::start_health_check(links.clone(), link_id.to_string(), metrics.clone()).await;
            }
            Err(e) => {
                link.state.status = FederationLinkStatus::Reconnecting;
                link.state.last_error = Some(e.to_string());
                link.state.reconnect_attempts += 1;
                link.state.updated_at = chrono::Utc::now().timestamp() as u64;

                // Calculate next reconnect with exponential backoff and jitter
                let delay =
                    Self::calculate_reconnect_delay(&link.config, link.state.reconnect_attempts);
                link.state.next_reconnect_at =
                    Some(chrono::Utc::now().timestamp() as u64 + delay.as_secs());

                warn!(
                    "Federation connection failed for {}: {} (attempt {}, next in {:?})",
                    link_id, e, link.state.reconnect_attempts, delay
                );

                let _ = event_tx.try_send(FederationEvent::LinkFailed {
                    link_id: link_id.to_string(),
                    error: e.to_string(),
                });

                metrics
                    .counter(
                        "federation_connection_failures_total",
                        "Federation connection failures",
                    )
                    .increment(1);
            }
        }
    }

    /// Calculate reconnect delay with exponential backoff and jitter.
    fn calculate_reconnect_delay(config: &FederationLinkConfig, attempt: u32) -> Duration {
        let base = config.reconnect_base_delay_secs as f64;
        let max = config.reconnect_max_delay_secs as f64;
        let jitter = config.reconnect_jitter_percent as f64 / 100.0;

        // Exponential backoff: base * 2^attempt
        let delay = (base * 2_f64.powi(attempt as i32)).min(max);

        // Add jitter: ±jitter%
        let jitter_range = delay * jitter;
        let jitter_offset = rand::rng().random_range(-jitter_range..=jitter_range);
        let final_delay = (delay + jitter_offset).max(0.0);

        Duration::from_secs_f64(final_delay)
    }

    /// Establish MoQ connection to remote cluster.
    async fn establish_moq_connection(
        config: &FederationLinkConfig,
    ) -> anyhow::Result<FederationConnection> {
        // In real implementation:
        // 1. Parse remote endpoint (moq:// or https://)
        // 2. Set up TLS config (mTLS if enabled)
        // 3. Connect via MoQ transport
        // 4. Negotiate session
        // 5. Return connection handle

        // Simulate connection
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(FederationConnection {
            connected_at: Instant::now(),
            last_health_check: Instant::now(),
        })
    }

    /// Subscribe to mirrored streams.
    async fn subscribe_to_streams(link: &mut FederationLink) {
        // In real implementation:
        // 1. For each stream pattern in config.streams
        // 2. Create MoQ subscription with appropriate parameters
        // 3. Handle incoming objects and forward to local ingest

        for stream_pattern in &link.config.streams {
            link.state.streams_mirrored.push(stream_pattern.clone());
            debug!("Mirroring stream pattern: {}", stream_pattern);
        }
    }

    /// Start health check loop for a connected link.
    async fn start_health_check(
        links: Arc<DashMap<String, FederationLink>>,
        link_id: String,
        metrics: Arc<MetricsRegistry>,
    ) {
        let link_config = {
            let link = links.get(&link_id).expect("Link should exist");
            link.config.clone()
        };

        tokio::spawn(async move {
            let mut interval =
                interval(Duration::from_secs(link_config.health_check_interval_secs));

            loop {
                interval.tick().await;

                let mut link = match links.get_mut(&link_id) {
                    Some(l) => l,
                    None => break,
                };

                if link.state.status != FederationLinkStatus::Connected {
                    break;
                }

                // Perform health check (MoQ ping)
                let healthy = Self::perform_health_check(&mut link).await;

                if !healthy {
                    warn!("Health check failed for federation link: {}", link_id);
                    link.state.status = FederationLinkStatus::Reconnecting;
                    link.state.last_error = Some("Health check failed".to_string());
                    link.state.updated_at = chrono::Utc::now().timestamp() as u64;

                    let delay = Self::calculate_reconnect_delay(
                        &link.config,
                        link.state.reconnect_attempts,
                    );
                    link.state.next_reconnect_at =
                        Some(chrono::Utc::now().timestamp() as u64 + delay.as_secs());
                    break;
                }

                link.connection
                    .as_mut()
                    .map(|c| c.last_health_check = Instant::now());
            }
        });
    }

    /// Perform health check on connection.
    async fn perform_health_check(link: &mut FederationLink) -> bool {
        // In real implementation: send MoQ ping, wait for pong with timeout
        // For now, simulate
        if let Some(conn) = &link.connection {
            // Simulate network check
            true
        } else {
            false
        }
    }

    /// Get link state.
    pub fn get_link_state(&self, link_id: &str) -> Option<FederationLinkState> {
        self.links.get(link_id).map(|l| l.state.clone())
    }

    /// Get all link states.
    pub fn list_links(&self) -> Vec<FederationLinkState> {
        self.links.iter().map(|l| l.state.clone()).collect()
    }

    /// Get link by ID.
    pub fn get_link(&self, link_id: &str) -> Option<FederationLinkState> {
        self.links.get(link_id).map(|l| l.state.clone())
    }

    /// Subscribe to federation events.
    pub fn subscribe(&self) -> mpsc::Receiver<FederationEvent> {
        let (tx, rx) = mpsc::channel(100);
        rx
    }

    /// Get federation stats.
    pub fn stats(&self) -> FederationStats {
        let total = self.links.len();
        let connected = self
            .links
            .iter()
            .filter(|l| l.state.status == FederationLinkStatus::Connected)
            .count();
        let connecting = self
            .links
            .iter()
            .filter(|l| l.state.status == FederationLinkStatus::Connecting)
            .count();
        let failed = self
            .links
            .iter()
            .filter(|l| l.state.status == FederationLinkStatus::Failed)
            .count();
        let paused = self
            .links
            .iter()
            .filter(|l| l.state.status == FederationLinkStatus::Paused)
            .count();

        FederationStats {
            total_links: total,
            connected,
            connecting,
            failed,
            paused,
        }
    }
}

/// Federation statistics.
#[derive(Debug, Clone, Serialize)]
pub struct FederationStats {
    pub total_links: usize,
    pub connected: usize,
    pub connecting: usize,
    pub failed: usize,
    pub paused: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_federation_link_config_default() {
        let config = FederationLinkConfig::default();
        assert_eq!(config.reconnect_base_delay_secs, 1);
        assert_eq!(config.reconnect_max_delay_secs, 60);
        assert_eq!(config.reconnect_jitter_percent, 10);
    }

    #[test]
    fn test_calculate_reconnect_delay() {
        let config = FederationLinkConfig {
            reconnect_base_delay_secs: 1,
            reconnect_max_delay_secs: 60,
            reconnect_jitter_percent: 0, // No jitter for test
            ..Default::default()
        };

        let delay1 = FederationManager::calculate_reconnect_delay(&config, 0);
        assert_eq!(delay1.as_secs(), 1);

        let delay2 = FederationManager::calculate_reconnect_delay(&config, 1);
        assert_eq!(delay2.as_secs(), 2);

        let delay3 = FederationManager::calculate_reconnect_delay(&config, 2);
        assert_eq!(delay3.as_secs(), 4);

        let delay10 = FederationManager::calculate_reconnect_delay(&config, 10);
        // Should be capped at max (60)
        assert_eq!(delay10.as_secs(), 60);
    }

    #[test]
    fn test_federation_link_state() {
        let state = FederationLinkState {
            link_id: "link-1".to_string(),
            remote_cluster_id: "cluster-2".to_string(),
            remote_endpoint: "moq://cluster-2.example.com".to_string(),
            status: FederationLinkStatus::Connected,
            last_connected: Some(chrono::Utc::now().timestamp() as u64),
            last_error: None,
            reconnect_attempts: 0,
            next_reconnect_at: None,
            streams_mirrored: vec!["stream-*".to_string()],
            bytes_received: 0,
            bytes_sent: 0,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
        };

        assert_eq!(state.status, FederationLinkStatus::Connected);
        assert_eq!(state.streams_mirrored.len(), 1);
    }
}
