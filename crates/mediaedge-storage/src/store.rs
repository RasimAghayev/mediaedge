//! Storage abstraction trait and implementations.

use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;

/// Storage object metadata.
#[derive(Debug, Clone)]
pub struct ObjectMeta {
    pub key: String,
    pub size: u64,
    pub etag: String,
    pub content_type: Option<String>,
    pub last_modified: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, String>,
}

/// Storage backend trait.
#[async_trait]
pub trait ObjectStore: Send + Sync {
    /// Put an object.
    async fn put(&self, key: &str, data: Bytes, meta: ObjectMeta) -> anyhow::Result<()>;

    /// Get an object.
    async fn get(&self, key: &str) -> anyhow::Result<Option<(Bytes, ObjectMeta)>>;

    /// Delete an object.
    async fn delete(&self, key: &str) -> anyhow::Result<()>;

    /// List objects with prefix.
    async fn list(&self, prefix: &str) -> anyhow::Result<Vec<ObjectMeta>>;

    /// Check if object exists.
    async fn exists(&self, key: &str) -> anyhow::Result<bool> {
        self.get(key).await.map(|r| r.is_some())
    }

    /// Get object metadata only.
    async fn head(&self, key: &str) -> anyhow::Result<Option<ObjectMeta>> {
        self.get(key).await.map(|r| r.map(|(_, m)| m))
    }
}

/// Memory store for testing.
pub struct MemoryStore {
    data:
        std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, (Bytes, ObjectMeta)>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            data: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
}

#[async_trait]
impl ObjectStore for MemoryStore {
    async fn put(&self, key: &str, data: Bytes, meta: ObjectMeta) -> anyhow::Result<()> {
        self.data
            .write()
            .await
            .insert(key.to_string(), (data, meta));
        Ok(())
    }

    async fn get(&self, key: &str) -> anyhow::Result<Option<(Bytes, ObjectMeta)>> {
        Ok(self.data.read().await.get(key).cloned())
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.data.write().await.remove(key);
        Ok(())
    }

    async fn list(&self, prefix: &str) -> anyhow::Result<Vec<ObjectMeta>> {
        let data = self.data.read().await;
        Ok(data
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(_, (_, m))| m.clone())
            .collect())
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[tokio::test]
    async fn test_memory_store() {
        let store = MemoryStore::new();
        let meta = ObjectMeta {
            key: "test".into(),
            size: 4,
            etag: "abcd".into(),
            content_type: None,
            last_modified: chrono::Utc::now(),
            metadata: Default::default(),
        };
        store
            .put("test", Bytes::from("data"), meta.clone())
            .await
            .unwrap();
        let (data, m) = store.get("test").await.unwrap().unwrap();
        assert_eq!(data, Bytes::from("data"));
        assert_eq!(m.etag, "abcd");
    }
}
