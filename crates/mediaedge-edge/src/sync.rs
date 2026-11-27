//! Edge node synchronization logic.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Sync state for a resource.
#[derive(Debug, Clone)]
pub struct SyncState {
    pub version: u64,
    pub data: Vec<u8>,
    pub last_sync: std::time::Instant,
}

/// Synchronization manager.
pub struct SyncManager {
    states: Arc<RwLock<HashMap<String, SyncState>>>,
}

impl SyncManager {
    pub fn new() -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_state(&self, key: &str) -> Option<SyncState> {
        self.states.read().await.get(key).cloned()
    }

    pub async fn set_state(&self, key: String, state: SyncState) {
        self.states.write().await.insert(key, state);
    }
}

impl Default for SyncManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sync() {
        let mgr = SyncManager::new();
        mgr.set_state(
            "test".into(),
            SyncState {
                version: 1,
                data: vec![1, 2, 3],
                last_sync: std::time::Instant::now(),
            },
        )
        .await;
        assert!(mgr.get_state("test").await.is_some());
    }
}
