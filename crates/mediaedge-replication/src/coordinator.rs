//! Replication Coordinator - Raft-based coordinator for geo-replication
//!
//! Uses etcd-client for leader election and state management.
//! Manages replication plans, monitors replica health, and coordinates failover.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use etcd_client::{Client, Compare, CompareOp, TxnOp, TxnOpResponse, WatchOptions};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock as TokioRwLock, mpsc};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use mediaedge_config::types::ReplicationCoordinatorConfig;
use mediaedge_observability::MetricsRegistry;

/// Replication coordinator.
pub struct ReplicationCoordinator {
    config: ReplicationCoordinatorConfig,
    etcd: Arc<Client>,
    local_node_id: String,
    replication_plans: Arc<DashMap<String, ReplicationPlan>>,
    replica_states: Arc<DashMap<String, ReplicaState>>,
    event_tx: mpsc::Sender<CoordinatorEvent>,
    event_rx: Arc<TokioRwLock<mpsc::Receiver<CoordinatorEvent>>>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    metrics: Arc<MetricsRegistry>,
    is_leader: Arc<RwLock<bool>>,
}

/// Coordinator events.
#[derive(Debug, Clone)]
pub enum CoordinatorEvent {
    PlanCreated {
        plan_id: String,
    },
    PlanUpdated {
        plan_id: String,
    },
    PlanDeleted {
        plan_id: String,
    },
    ReplicaPromoted {
        plan_id: String,
        replica_id: String,
    },
    ReplicaFailed {
        plan_id: String,
        replica_id: String,
        reason: String,
    },
    LeadershipChanged {
        is_leader: bool,
    },
}

/// Replication plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationPlan {
    pub plan_id: String,
    pub stream_id: String,
    pub source_region: String,
    pub target_regions: Vec<TargetRegion>,
    pub erasure_config: ErasureConfigRef,
    pub staging: StagingConfig,
    pub demand_trigger: DemandTriggerConfig,
    pub created_at: u64,
    pub updated_at: u64,
    pub version: u64,
}

/// Target region for replication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetRegion {
    pub region: String,
    pub datacenters: Vec<DatacenterTarget>,
    pub priority: u32, // Lower = higher priority
}

/// Datacenter target within a region.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatacenterTarget {
    pub datacenter: String,
    pub node_ids: Vec<String>, // Specific nodes, empty = any
    pub storage_class: StorageClass,
    pub min_replicas: u32,
}

/// Storage class for tiering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageClass {
    Hot,  // NVMe, low latency
    Warm, // SSD, medium latency
    Cold, // S3/Glacier, high latency
}

/// Erasure coding config reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureConfigRef {
    pub k: u32, // Data shards
    pub m: u32, // Parity shards
    pub chunk_size: usize,
}

/// Staging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagingConfig {
    pub enabled: bool,
    pub continent_staging: bool,
    pub region_staging: bool,
    pub staging_ttl_secs: u64,
}

/// Demand trigger configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemandTriggerConfig {
    pub enabled: bool,
    pub min_viewers: u32,
    pub trigger_delay_secs: u64,
    pub cooldown_secs: u64,
}

/// Replica state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaState {
    pub replica_id: String,
    pub plan_id: String,
    pub node_id: String,
    pub region: String,
    pub datacenter: String,
    pub status: ReplicaStatus,
    pub storage_class: StorageClass,
    pub size_bytes: u64,
    pub last_heartbeat: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub health_score: f32, // 0.0 - 1.0
}

/// Replica status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplicaStatus {
    Creating,
    Active,
    Degraded,
    Failed,
    Promoting,
    Promoted,
    Removing,
}

impl ReplicationCoordinator {
    /// Create a new replication coordinator.
    pub async fn new(
        config: ReplicationCoordinatorConfig,
        local_node_id: String,
        metrics: Arc<MetricsRegistry>,
    ) -> anyhow::Result<Self> {
        let etcd_endpoints = config.etcd_endpoints.clone();
        let etcd = Arc::new(Client::connect(etcd_endpoints, None).await?);

        let (event_tx, event_rx) = mpsc::channel(1000);
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);

        Ok(Self {
            config,
            etcd,
            local_node_id,
            replication_plans: Arc::new(DashMap::new()),
            replica_states: Arc::new(DashMap::new()),
            event_tx,
            event_rx: Arc::new(TokioRwLock::new(event_rx)),
            shutdown_tx,
            metrics,
            is_leader: Arc::new(RwLock::new(false)),
        })
    }

    /// Start the coordinator.
    pub async fn start(&self) -> anyhow::Result<()> {
        info!(
            "Starting replication coordinator for node: {}",
            self.local_node_id
        );

        // Start leader election
        self.start_leader_election().await?;

        // Start plan watcher
        self.start_plan_watcher().await?;

        // Start replica health monitor
        self.start_health_monitor().await?;

        // Start demand trigger evaluator
        self.start_demand_evaluator().await?;

        // Load existing plans
        self.load_plans().await?;

        info!("Replication coordinator started");
        Ok(())
    }

    /// Stop the coordinator.
    pub async fn stop(&self) -> anyhow::Result<()> {
        info!("Stopping replication coordinator");
        let _ = self.shutdown_tx.send(true);

        // Resign leadership if we're leader
        self.resign_leadership().await?;

        Ok(())
    }

    /// Start leader election using etcd.
    async fn start_leader_election(&self) -> anyhow::Result<()> {
        let etcd = self.etcd.clone();
        let node_id = self.local_node_id.clone();
        let is_leader = self.is_leader.clone();
        let event_tx = self.event_tx.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let lease_ttl = 10; // seconds

        tokio::spawn(async move {
            let mut election_key = format!("mediaedge/replication/leader");
            let mut interval = interval(Duration::from_secs(5));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }

                        // Try to become leader
                        let mut txn = etcd.txn();
                        txn = txn
                            .when(vec![Compare::version(election_key.clone(), CompareOp::Equal, 0)])
                            .and_then(vec![TxnOp::put(election_key.clone(), node_id.clone(), None)])
                            .or_else(vec![]);

                        match txn.commit().await {
                            Ok(resp) => {
                                if resp.succeeded() {
                                    *is_leader.write() = true;
                                    let _ = event_tx.try_send(CoordinatorEvent::LeadershipChanged { is_leader: true });
                                    info!("Became replication coordinator leader");
                                }
                            }
                            Err(e) => {
                                warn!("Leader election error: {}", e);
                            }
                        }

                        // If not leader, check who is
                        if !*is_leader.read() {
                            if let Ok(resp) = etcd.get(election_key.clone(), None).await {
                                if let Some(kv) = resp.kvs().first() {
                                    let leader = String::from_utf8_lossy(kv.value()).to_string();
                                    if leader != node_id {
                                        // Leader exists, keep watching
                                    } else {
                                        // We are leader but flag not set
                                        *is_leader.write() = true;
                                        let _ = event_tx.try_send(CoordinatorEvent::LeadershipChanged { is_leader: true });
                                    }
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

    /// Resign leadership.
    async fn resign_leadership(&self) -> anyhow::Result<()> {
        if *self.is_leader.read() {
            let election_key = "mediaedge/replication/leader";
            let compare = Compare::value(
                election_key,
                CompareOp::Equal,
                self.local_node_id.as_bytes().to_vec(),
            );
            let txn = self
                .etcd
                .txn()
                .when(vec![compare])
                .and_then(vec![TxnOp::delete(election_key)])
                .commit()
                .await?;

            if txn.succeeded() {
                *self.is_leader.write() = false;
                let _ = self
                    .event_tx
                    .try_send(CoordinatorEvent::LeadershipChanged { is_leader: false });
                info!("Resigned replication coordinator leadership");
            }
        }
        Ok(())
    }

    /// Start watching for plan changes.
    async fn start_plan_watcher(&self) -> anyhow::Result<()> {
        let etcd = self.etcd.clone();
        let plans = self.replication_plans.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let watch_key = "mediaedge/replication/plans/";
            let mut watch_stream = etcd
                .watch(watch_key, Some(WatchOptions::new().with_prefix()))
                .await
                .unwrap();

            while let Some(resp) = watch_stream.message().await.unwrap() {
                if *shutdown_rx.borrow() {
                    break;
                }

                for event in resp.events() {
                    match event.event_type() {
                        etcd_client::EventType::Put => {
                            if let Some(kv) = event.kv() {
                                let key = String::from_utf8_lossy(kv.key()).to_string();
                                let plan_id = key
                                    .strip_prefix("mediaedge/replication/plans/")
                                    .unwrap_or(&key);

                                if let Ok(plan) =
                                    serde_json::from_slice::<ReplicationPlan>(kv.value())
                                {
                                    plans.insert(plan_id.to_string(), plan.clone());

                                    let is_new = !plans.contains_key(plan_id);
                                    if is_new {
                                        let _ = event_tx.try_send(CoordinatorEvent::PlanCreated {
                                            plan_id: plan_id.to_string(),
                                        });
                                    } else {
                                        let _ = event_tx.try_send(CoordinatorEvent::PlanUpdated {
                                            plan_id: plan_id.to_string(),
                                        });
                                    }
                                }
                            }
                        }
                        etcd_client::EventType::Delete => {
                            if let Some(kv) = event.kv() {
                                let key = String::from_utf8_lossy(kv.key()).to_string();
                                let plan_id = key
                                    .strip_prefix("mediaedge/replication/plans/")
                                    .unwrap_or(&key);

                                plans.remove(plan_id);
                                let _ = event_tx.try_send(CoordinatorEvent::PlanDeleted {
                                    plan_id: plan_id.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Start replica health monitor.
    async fn start_health_monitor(&self) -> anyhow::Result<()> {
        let replicas = self.replica_states.clone();
        let plans = self.replication_plans.clone();
        let event_tx = self.event_tx.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let metrics = self.metrics.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }

                        let now = chrono::Utc::now().timestamp() as u64;
                        let mut failed_replicas = Vec::new();

                        for entry in replicas.iter() {
                            let replica = entry.value();

                            // Check heartbeat timeout (2 minutes)
                            if now.saturating_sub(replica.last_heartbeat) > 120 {
                                if replica.status == ReplicaStatus::Active || replica.status == ReplicaStatus::Degraded {
                                    failed_replicas.push((replica.plan_id.clone(), replica.replica_id.clone(), replica.node_id.clone()));
                                }
                            }

                            // Update health score based on recency
                            let age = now.saturating_sub(replica.last_heartbeat);
                            let health = if age < 30 { 1.0 } else if age < 60 { 0.7 } else if age < 120 { 0.4 } else { 0.1 };

                            if let Some(mut r) = replicas.get_mut(entry.key()) {
                                r.health_score = health;
                                if health < 0.5 && r.status == ReplicaStatus::Active {
                                    r.status = ReplicaStatus::Degraded;
                                }
                            }
                        }

                        // Handle failed replicas
                        for (plan_id, replica_id, node_id) in failed_replicas {
                            warn!("Replica failed: plan={}, replica={}, node={}", plan_id, replica_id, node_id);

                            if let Some(mut replica) = replicas.get_mut(&replica_id) {
                                replica.status = ReplicaStatus::Failed;
                                replica.updated_at = now;
                            }

                            let _ = event_tx.try_send(CoordinatorEvent::ReplicaFailed {
                                plan_id: plan_id.clone(),
                                replica_id: replica_id.clone(),
                                reason: "Heartbeat timeout".to_string()
                            });

                            // Trigger failover if we're leader
                            // This would be handled by the leader
                            metrics.counter("replication_replica_failures_total", "Replica failures").increment(1);
                        }

                        // Update metrics
                        let active = replicas.iter().filter(|r| r.status == ReplicaStatus::Active).count();
                        let degraded = replicas.iter().filter(|r| r.status == ReplicaStatus::Degraded).count();
                        let failed = replicas.iter().filter(|r| r.status == ReplicaStatus::Failed).count();

                        metrics.gauge("replication_replicas_active", "Active replicas").set(active as f64);
                        metrics.gauge("replication_replicas_degraded", "Degraded replicas").set(degraded as f64);
                        metrics.gauge("replication_replicas_failed", "Failed replicas").set(failed as f64);
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

    /// Start demand trigger evaluator.
    async fn start_demand_evaluator(&self) -> anyhow::Result<()> {
        let plans = self.replication_plans.clone();
        let replicas = self.replica_states.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }

                        // Check each plan for demand triggers
                        for entry in plans.iter() {
                            let plan = entry.value();

                            if !plan.demand_trigger.enabled {
                                continue;
                            }

                            // In real implementation, would query metrics for viewer count
                            // For now, simulate
                            let viewer_count = 0; // Would come from metrics

                            if viewer_count >= plan.demand_trigger.min_viewers {
                                // Trigger demand-based replication
                                Self::trigger_demand_replication(&plan, &replicas).await;
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

    /// Trigger demand-based replication.
    async fn trigger_demand_replication(
        plan: &ReplicationPlan,
        replicas: &Arc<DashMap<String, ReplicaState>>,
    ) {
        info!("Demand trigger activated for plan: {}", plan.plan_id);

        // In real implementation, would create new replicas in target regions
        // based on the demand trigger configuration
    }

    /// Load existing plans from etcd.
    async fn load_plans(&self) -> anyhow::Result<()> {
        let resp = self
            .etcd
            .get(
                "mediaedge/replication/plans/",
                Some(WatchOptions::new().with_prefix()),
            )
            .await?;

        for kv in resp.kvs() {
            let key = String::from_utf8_lossy(kv.key()).to_string();
            let plan_id = key
                .strip_prefix("mediaedge/replication/plans/")
                .unwrap_or(&key);

            if let Ok(plan) = serde_json::from_slice::<ReplicationPlan>(kv.value()) {
                self.replication_plans.insert(plan_id.to_string(), plan);
            }
        }

        info!("Loaded {} replication plans", self.replication_plans.len());
        Ok(())
    }

    /// Create a new replication plan.
    pub async fn create_plan(&self, mut plan: ReplicationPlan) -> anyhow::Result<()> {
        if !*self.is_leader.read() {
            return Err(anyhow::anyhow!("Not leader"));
        }

        plan.version = 1;
        plan.created_at = chrono::Utc::now().timestamp() as u64;
        plan.updated_at = plan.created_at;

        let key = format!("mediaedge/replication/plans/{}", plan.plan_id);
        let value = serde_json::to_vec(&plan)?;

        self.etcd.put(key, value, None).await?;
        self.replication_plans
            .insert(plan.plan_id.clone(), plan.clone());

        let _ = self.event_tx.try_send(CoordinatorEvent::PlanCreated {
            plan_id: plan.plan_id,
        });
        self.metrics
            .counter("replication_plans_created_total", "Plans created")
            .increment(1);

        Ok(())
    }

    /// Update a replication plan.
    pub async fn update_plan(&self, mut plan: ReplicationPlan) -> anyhow::Result<()> {
        if !*self.is_leader.read() {
            return Err(anyhow::anyhow!("Not leader"));
        }

        plan.version += 1;
        plan.updated_at = chrono::Utc::now().timestamp() as u64;

        let key = format!("mediaedge/replication/plans/{}", plan.plan_id);
        let value = serde_json::to_vec(&plan)?;

        self.etcd.put(key, value, None).await?;
        self.replication_plans
            .insert(plan.plan_id.clone(), plan.clone());

        let _ = self.event_tx.try_send(CoordinatorEvent::PlanUpdated {
            plan_id: plan.plan_id,
        });
        Ok(())
    }

    /// Delete a replication plan.
    pub async fn delete_plan(&self, plan_id: &str) -> anyhow::Result<()> {
        if !*self.is_leader.read() {
            return Err(anyhow::anyhow!("Not leader"));
        }

        let key = format!("mediaedge/replication/plans/{}", plan_id);
        self.etcd.delete(key, None).await?;
        self.replication_plans.remove(plan_id);

        let _ = self.event_tx.try_send(CoordinatorEvent::PlanDeleted {
            plan_id: plan_id.to_string(),
        });
        self.metrics
            .counter("replication_plans_deleted_total", "Plans deleted")
            .increment(1);

        Ok(())
    }

    /// Get a replication plan.
    pub fn get_plan(&self, plan_id: &str) -> Option<ReplicationPlan> {
        self.replication_plans.get(plan_id).map(|p| p.clone())
    }

    /// List all replication plans.
    pub fn list_plans(&self) -> Vec<ReplicationPlan> {
        self.replication_plans
            .iter()
            .map(|p| p.value().clone())
            .collect()
    }

    /// Register a replica.
    pub async fn register_replica(&self, replica: ReplicaState) -> anyhow::Result<()> {
        let key = format!("mediaedge/replication/replicas/{}", replica.replica_id);
        let value = serde_json::to_vec(&replica)?;

        self.etcd.put(key, value, None).await?;
        self.replica_states
            .insert(replica.replica_id.clone(), replica.clone());

        Ok(())
    }

    /// Update replica heartbeat.
    pub async fn replica_heartbeat(&self, replica_id: &str, size_bytes: u64) -> anyhow::Result<()> {
        if let Some(mut replica) = self.replica_states.get_mut(replica_id) {
            replica.last_heartbeat = chrono::Utc::now().timestamp() as u64;
            replica.size_bytes = size_bytes;
            replica.updated_at = replica.last_heartbeat;

            if replica.status == ReplicaStatus::Creating
                || replica.status == ReplicaStatus::Degraded
            {
                replica.status = ReplicaStatus::Active;
            }
        }
        Ok(())
    }

    /// Promote a replica (failover).
    pub async fn promote_replica(&self, plan_id: &str, replica_id: &str) -> anyhow::Result<()> {
        if !*self.is_leader.read() {
            return Err(anyhow::anyhow!("Not leader"));
        }

        if let Some(mut replica) = self.replica_states.get_mut(replica_id) {
            if replica.plan_id != plan_id {
                return Err(anyhow::anyhow!("Replica not in plan"));
            }

            replica.status = ReplicaStatus::Promoting;
            replica.updated_at = chrono::Utc::now().timestamp() as u64;

            // In real implementation, would coordinate with storage nodes
            // to promote the replica to primary

            replica.status = ReplicaStatus::Promoted;

            let _ = self.event_tx.try_send(CoordinatorEvent::ReplicaPromoted {
                plan_id: plan_id.to_string(),
                replica_id: replica_id.to_string(),
            });

            self.metrics
                .counter("replication_promotions_total", "Replica promotions")
                .increment(1);
        }

        Ok(())
    }

    /// Check if leader.
    pub fn is_leader(&self) -> bool {
        *self.is_leader.read()
    }

    /// Get coordinator stats.
    pub fn stats(&self) -> CoordinatorStats {
        let plans = self.replication_plans.len();
        let replicas = self.replica_states.len();
        let active = self
            .replica_states
            .iter()
            .filter(|r| r.status == ReplicaStatus::Active)
            .count();
        let degraded = self
            .replica_states
            .iter()
            .filter(|r| r.status == ReplicaStatus::Degraded)
            .count();
        let failed = self
            .replica_states
            .iter()
            .filter(|r| r.status == ReplicaStatus::Failed)
            .count();

        CoordinatorStats {
            is_leader: self.is_leader(),
            total_plans: plans,
            total_replicas: replicas,
            active_replicas: active,
            degraded_replicas: degraded,
            failed_replicas: failed,
        }
    }

    /// Subscribe to coordinator events.
    pub fn subscribe(&self) -> mpsc::Receiver<CoordinatorEvent> {
        let (tx, rx) = mpsc::channel(100);
        rx
    }
}

/// Coordinator statistics.
#[derive(Debug, Clone, Serialize)]
pub struct CoordinatorStats {
    pub is_leader: bool,
    pub total_plans: usize,
    pub total_replicas: usize,
    pub active_replicas: usize,
    pub degraded_replicas: usize,
    pub failed_replicas: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replication_plan_serialization() {
        let plan = ReplicationPlan {
            plan_id: "plan-1".to_string(),
            stream_id: "stream-1".to_string(),
            source_region: "us-east-1".to_string(),
            target_regions: vec![TargetRegion {
                region: "eu-west-1".to_string(),
                datacenters: vec![DatacenterTarget {
                    datacenter: "dc1".to_string(),
                    node_ids: vec![],
                    storage_class: StorageClass::Hot,
                    min_replicas: 2,
                }],
                priority: 1,
            }],
            erasure_config: ErasureConfigRef {
                k: 10,
                m: 4,
                chunk_size: 1024 * 1024,
            },
            staging: StagingConfig {
                enabled: true,
                continent_staging: true,
                region_staging: true,
                staging_ttl_secs: 3600,
            },
            demand_trigger: DemandTriggerConfig {
                enabled: true,
                min_viewers: 100,
                trigger_delay_secs: 300,
                cooldown_secs: 3600,
            },
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
            version: 1,
        };

        let json = serde_json::to_string(&plan).unwrap();
        let parsed: ReplicationPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.plan_id, "plan-1");
        assert_eq!(parsed.erasure_config.k, 10);
        assert_eq!(parsed.erasure_config.m, 4);
    }

    #[test]
    fn test_replica_state() {
        let replica = ReplicaState {
            replica_id: "replica-1".to_string(),
            plan_id: "plan-1".to_string(),
            node_id: "node-1".to_string(),
            region: "us-east-1".to_string(),
            datacenter: "dc1".to_string(),
            status: ReplicaStatus::Active,
            storage_class: StorageClass::Hot,
            size_bytes: 1024 * 1024 * 100,
            last_heartbeat: chrono::Utc::now().timestamp() as u64,
            created_at: chrono::Utc::now().timestamp() as u64,
            updated_at: chrono::Utc::now().timestamp() as u64,
            health_score: 1.0,
        };

        assert_eq!(replica.status, ReplicaStatus::Active);
        assert_eq!(replica.storage_class, StorageClass::Hot);
    }
}
