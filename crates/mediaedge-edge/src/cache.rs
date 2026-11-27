//! Edge node cache implementation.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Simple in-memory cache for edge nodes.
pub struct EdgeCache<K, V> {
    data: Arc<RwLock<HashMap<K, V>>>,
}

impl<K, V> EdgeCache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get(&self, key: &K) -> Option<V> {
        self.data.read().await.get(key).cloned()
    }

    pub async fn insert(&self, key: K, value: V) {
        self.data.write().await.insert(key, value);
    }

    pub async fn remove(&self, key: &K) {
        self.data.write().await.remove(key);
    }
}

impl<K, V> Default for EdgeCache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache() {
        let cache = EdgeCache::new();
        cache.insert("key", "value").await;
        assert_eq!(cache.get(&"key").await, Some("value"));
    }
}
