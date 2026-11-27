//! etcd client wrapper.

use etcd_client::{Client, ConnectOptions, LockOptions};
use std::time::Duration;

/// Metadata client.
pub struct MetadataClient {
    client: Client,
}

impl MetadataClient {
    /// Create a new client.
    pub async fn new(endpoints: Vec<String>, timeout: Duration) -> anyhow::Result<Self> {
        let options = ConnectOptions::new()
            .with_endpoints(endpoints)
            .with_dial_timeout(timeout);
        let client = Client::connect(options).await?;
        Ok(Self { client })
    }

    /// Get the underlying client.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get a value by key.
    pub async fn get(&mut self, key: &str) -> anyhow::Result<Option<String>> {
        let resp = self.client.get(key, None).await?;
        Ok(resp
            .kvs()
            .first()
            .map(|kv| String::from_utf8_lossy(kv.value()).to_string()))
    }

    /// Put a key-value pair.
    pub async fn put(&mut self, key: &str, value: &str) -> anyhow::Result<()> {
        self.client.put(key, value, None).await?;
        Ok(())
    }

    /// Delete a key.
    pub async fn delete(&mut self, key: &str) -> anyhow::Result<()> {
        self.client.delete(key, None).await?;
        Ok(())
    }

    /// Watch a key prefix.
    pub async fn watch(&mut self, prefix: &str) -> anyhow::Result<etcd_client::WatchStream> {
        let (watcher, mut stream) = self.client.watch(prefix, None).await?;
        Ok(stream)
    }

    /// Acquire a lock.
    pub async fn lock(&mut self, key: &str, ttl: u64) -> anyhow::Result<etcd_client::LockKey> {
        let lock = self
            .client
            .lock(key, Some(LockOptions::new().with_ttl(ttl)))
            .await?;
        Ok(lock)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client() {
        // Requires etcd server
    }
}
