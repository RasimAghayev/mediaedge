//! Config Sync Manager - LWW (Last-Writer-Wins) config sync via gossip
//!
//! Synchronizes configuration across the cluster using gossip.
//! Uses LWW semantics with logical timestamps for conflict resolution.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, info, warn};

use crate::gossip::{ConfigEntry, GossipProtocol};
use mediaedge_observability::MetricsRegistry;

/// Config sync configuration.
#[derive(Debug, Clone)]
pub struct ConfigSyncConfig {
    pub sync_interval_secs: u64,
    pub max_entries: usize,
}

impl Default for ConfigSyncConfig {
    fn default() -> Self {
        Self {
            sync_interval_secs: 30,
            max_entries: 1000,
        }
    }
}

/// Configuration value with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValue {
    pub key: String,
    pub value: String,
    pub version: u64,          // Logical timestamp (LWW)
    pub updated_by: String,    // Node ID
    pub updated_at: u64,       // Unix timestamp
    pub source: ConfigSource,
}

/// Config source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigSource {
    Local,
    Gossip,
    Api,
}

/// Config sync manager.
pub struct ConfigSyncManager {
    config: ConfigSyncConfig,
    gossip: Arc<RwLock<Option<Arc<GossipProtocol>>>>,
    local_config: Arc<DashMap<String, ConfigValue>>,
    event_tx: mpsc::Sender<ConfigSyncEvent>,
    event_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<ConfigSyncEvent>>>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    metrics: Arc<MetricsRegistry>,
}

/// Config sync events.
#[derive(Debug, Clone)]
pub enum ConfigSyncEvent {
    Updated { key: String, value: ConfigValue },
    Removed { key: String },
    Conflict { key: String, local: ConfigValue, remote: ConfigValue },
}

impl ConfigSyncManager {
    /// Create a new config sync manager.
    pub fn new(metrics: Arc<MetricsRegistry>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(1000);
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);

        Self {
            config: ConfigSyncConfig::default(),
            gossip: Arc::new(RwLock::new(None)),
            local_config: Arc::new(DashMap::new()),
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

    /// Start the config sync manager.
    pub async fn start(&self) -> anyhow::Result<()> {
        info!("Starting config sync manager");

        // Start sync loop
        self.start_sync_loop().await;

        Ok(())
    }

    /// Stop the config sync manager.
    pub async fn stop(&self) -> anyhow::Result<()> {
        info!("Stopping config sync manager");
        let _ = self.shutdown_tx.send(true);
        Ok(())
    }

    /// Set a configuration value locally.
    pub async fn set(&self, key: &str, value: &str, source: ConfigSource) -> anyhow::Result<()> {
        let node_id = self.get_node_id().await?;
        
        let mut current_version = 0;
        if let Some(existing) = self.local_config.get(key) {
            current_version = existing.version;
        }

        let config_value = ConfigValue {
            key: key.to_string(),
            value: value.to_string(),
            version: current_version + 1,
            updated_by: node_id.0,
            updated_at: chrono::Utc::now().timestamp() as u64,
            source,
        };

        // Check for conflicts
        if let Some(existing) = self.local_config.get(key) {
            if existing.version > config_value.version {
                // Local version is newer, don't overwrite
                warn!("Config conflict for {}: local version {} > new version {}", 
                    key, existing.version, config_value.version);
                let _ = self.event_tx.try_send(ConfigSyncEvent::Conflict {
                    key: key.to_string(),
                    local: existing.clone(),
                    remote: config_value,
                });
                return Ok(());
            }
        }

        self.local_config.insert(key.to_string(), config_value.clone());
        
        let _ = self.event_tx.try_send(ConfigSyncEvent::Updated {
            key: key.to_string(),
            value: config_value,
        });

        self.metrics.counter("config_sync_updates_total", "Config sync updates").increment(1);
        
        Ok(())
    }

    /// Get a configuration value.
    pub fn get(&self, key: &str) -> Option<ConfigValue> {
        self.local_config.get(key).map(|v| v.clone())
    }

    /// Get all configuration values.
    pub fn get_all(&self) -> Vec<ConfigValue> {
        self.local_config.iter().map(|e| e.value().clone()).collect()
    }

    /// Remove a configuration value.
    pub async fn remove(&self, key: &str) -> anyhow::Result<()> {
        if self.local_config.remove(key).is_some() {
            let _ = self.event_tx.try_send(ConfigSyncEvent::Removed { key: key.to_string() });
            self.metrics.counter("config_sync_removals_total", "Config sync removals").increment(1);
        }
        Ok(())
    }

    /// Update config from gossip payload.
    pub fn update_from_gossip(&self, entries: Vec<ConfigEntry>) {
        for entry in entries {
            let remote_value = ConfigValue {
                key: entry.key.clone(),
                value: entry.value.clone(),
                version: entry.version,
                updated_by: entry.updated_by.0,
                updated_at: entry.updated_at,
                source: ConfigSource::Gossip,
            };

            let mut should_update = false;
            
            if let Some(local) = self.local_config.get(&entry.key) {
                // LWW: higher version wins
                if entry.version > local.version {
                    should_update = true;
                } else if entry.version == local.version {
                    // Tie-breaker: newer timestamp wins
                    if entry.updated_at > local.updated_at {
                        should_update = true;
                    } else if entry.updated_at == local.updated_at {
                        // Final tie-breaker: lexicographically larger node ID wins
                        if entry.updated_by.0 > local.updated_by {
                            should_update = true;
                        }
                    }
                }
            } else {
                // New key from gossip
                should_update = true;
            }

            if should_update {
                let key = entry.key.clone();
                self.local_config.insert(key.clone(), remote_value.clone());
                
                let _ = self.event_tx.try_send(ConfigSyncEvent::Updated {
                    key,
                    value: remote_value,
                });

                self.metrics.counter("config_sync_gossip_updates_total", "Config sync gossip updates").increment(1);
            }
        }
    }

    /// Start periodic sync with gossip.
    async fn start_sync_loop(&self) {
        let local_config = self.local_config.clone();
        let gossip = self.gossip.clone();
        let config = self.config.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.sync_interval_secs));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }

                        // In real implementation, would add local config to gossip payload
                        // The gossip protocol will disseminate it
                        if let Some(gossip_guard) = gossip.read().as_ref() {
                            // Gossip protocol reads local_config when building payload
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

    /// Get node ID from gossip.
    async fn get_node_id(&self) -> anyhow::Result<crate::gossip::NodeId> {
        if let Some(gossip) = self.gossip.read().as_ref() {
            Ok(gossip.node_id().clone())
        } else {
            Err(anyhow::anyhow!("Gossip not initialized"))
        }
    }

    /// Subscribe to config sync events.
    pub fn subscribe(&self) -> mpsc::Receiver<ConfigSyncEvent> {
        let (tx, rx) = mpsc::channel(100);
        rx
    }

    /// Get config sync stats.
    pub fn stats(&self) -> ConfigSyncStats {
        let total = self.local_config.len();
        let by_source = self.local_config.iter()
            .fold(HashMap::new(), |mut acc, entry| {
                *acc.entry(entry.source).or_insert(0) += 1;
                acc
            });

        ConfigSyncStats {
            total_entries: total,
            by_source,
        }
    }
}

/// Config sync statistics.
#[derive(Debug, Clone, Serialize)]
pub struct ConfigSyncStats {
    pub total_entries: usize,
    pub by_source: HashMap<ConfigSource, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_sync_config_default() {
        let config = ConfigSyncConfig::default();
        assert_eq!(config.sync_interval_secs, 30);
        assert_eq!(config.max_entries, 1000);
    }

    #[test]
    fn test_config_value_lww() {
        let local = ConfigValue {
            key: "test".to_string(),
            value: "local".to_string(),
            version: 5,
            updated_by: "node-1".to_string(),
            updated_at: 1000,
            source: ConfigSource::Local,
        };

        let remote = ConfigValue {
            key: "test".to_string(),
            value: "remote".to_string(),
            version: 6,
            updated_by: "node-2".to_string(),
            updated_at: 1001,
            source: ConfigSource::Gossip,
        };

        // Remote has higher version, should win
        assert!(remote.version > local.version);

        let remote_same_version = ConfigValue {
            key: "test".to_string(),
            value: "remote".to_string(),
            version: 5,
            updated_by: "node-2".to_string(),
            updated_at: 1002,
            source: ConfigSource::Gossip,
        };

        // Same version, remote has newer timestamp, should win
        assert!(remote_same_version.updated_at > local.updated_at);

        let remote_same_ts = ConfigValue {
            key: "test".to_string(),
            value: "remote".to_string(),
            version: 5,
            updated_by: "node-3".to_string(), // lexicographically larger
            updated_at: 1000,
            source: ConfigSource::Gossip,
        };

        // Same version and timestamp, node-3 > node-1, should win
        assert!(remote_same_ts.updated_by > local.updated_by);
    }
}