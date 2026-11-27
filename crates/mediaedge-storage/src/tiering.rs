//! Archive tiering: hot (NVMe) -> warm (SSD) -> cold (S3/Glacier).

use crate::store::{ObjectMeta, ObjectStore};
use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Storage tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageTier {
    Hot,
    Warm,
    Cold,
}

impl StorageTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageTier::Hot => "hot",
            StorageTier::Warm => "warm",
            StorageTier::Cold => "cold",
        }
    }
}

/// Tier configuration.
#[derive(Debug, Clone)]
pub struct TierConfig {
    pub tier: StorageTier,
    pub store: Arc<dyn ObjectStore>,
    pub max_size_bytes: u64,
    pub min_age: std::time::Duration,
}

/// Tiering manager.
pub struct TieringManager {
    tiers: Vec<TierConfig>,
    current_tier: Arc<RwLock<StorageTier>>,
}

impl TieringManager {
    pub fn new(tiers: Vec<TierConfig>) -> Self {
        Self {
            tiers,
            current_tier: Arc::new(RwLock::new(StorageTier::Hot)),
        }
    }

    /// Get the store for a tier.
    pub fn store(&self, tier: StorageTier) -> Option<Arc<dyn ObjectStore>> {
        self.tiers
            .iter()
            .find(|t| t.tier == tier)
            .map(|t| t.store.clone())
    }

    /// Get the hot store.
    pub fn hot_store(&self) -> Option<Arc<dyn ObjectStore>> {
        self.store(StorageTier::Hot)
    }

    /// Get the warm store.
    pub fn warm_store(&self) -> Option<Arc<dyn ObjectStore>> {
        self.store(StorageTier::Warm)
    }

    /// Get the cold store.
    pub fn cold_store(&self) -> Option<Arc<dyn ObjectStore>> {
        self.store(StorageTier::Cold)
    }

    /// Move an object between tiers.
    pub async fn move_object(
        &self,
        key: &str,
        from: StorageTier,
        to: StorageTier,
    ) -> anyhow::Result<()> {
        let from_store = self
            .store(from)
            .ok_or_else(|| anyhow::anyhow!("Source tier not found"))?;
        let to_store = self
            .store(to)
            .ok_or_else(|| anyhow::anyhow!("Target tier not found"))?;

        let (data, meta) = from_store
            .get(key)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Object not found"))?;
        to_store.put(key, data, meta).await?;
        from_store.delete(key).await?;

        Ok(())
    }

    /// Promote object to hot tier.
    pub async fn promote(&self, key: &str) -> anyhow::Result<()> {
        self.move_object(key, StorageTier::Warm, StorageTier::Hot)
            .await
    }

    /// Demote object to warm tier.
    pub async fn demote_to_warm(&self, key: &str) -> anyhow::Result<()> {
        self.move_object(key, StorageTier::Hot, StorageTier::Warm)
            .await
    }

    /// Demote object to cold tier.
    pub async fn demote_to_cold(&self, key: &str) -> anyhow::Result<()> {
        self.move_object(key, StorageTier::Warm, StorageTier::Cold)
            .await
    }

    /// Run tiering job based on age and size.
    pub async fn run_tiering_job(&self) -> anyhow::Result<u32> {
        let mut moved = 0;
        // In real implementation, would scan and move objects based on policy
        Ok(moved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tiering() {
        // Requires actual stores
    }
}
