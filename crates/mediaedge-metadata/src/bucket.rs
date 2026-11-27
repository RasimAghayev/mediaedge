//! Bucket metadata operations.

use crate::keys;
use crate::schema::{BucketSchema, entity_key};
use etcd_client::Client;

/// Bucket operations.
pub struct BucketOps {
    client: Client,
}

impl BucketOps {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Create a bucket.
    pub async fn create(&mut self, bucket: BucketSchema) -> anyhow::Result<()> {
        let key = entity_key(keys::BUCKETS, &bucket.id);
        let value = serde_json::to_string(&bucket)?;
        self.client.put(key, value, None).await?;
        Ok(())
    }

    /// Get a bucket.
    pub async fn get(&mut self, id: &str) -> anyhow::Result<Option<BucketSchema>> {
        let key = entity_key(keys::BUCKETS, id);
        let resp = self.client.get(key, None).await?;
        Ok(resp
            .kvs()
            .first()
            .map(|kv| serde_json::from_slice(kv.value()).unwrap()))
    }

    /// Delete a bucket.
    pub async fn delete(&mut self, id: &str) -> anyhow::Result<()> {
        let key = entity_key(keys::BUCKETS, id);
        self.client.delete(key, None).await?;
        Ok(())
    }

    /// List all buckets.
    pub async fn list(&mut self) -> anyhow::Result<Vec<BucketSchema>> {
        let resp = self
            .client
            .get(
                keys::BUCKETS,
                Some(etcd_client::GetOptions::new().with_prefix()),
            )
            .await?;
        let mut buckets = Vec::new();
        for kv in resp.kvs() {
            if let Ok(bucket) = serde_json::from_slice(kv.value()) {
                buckets.push(bucket);
            }
        }
        Ok(buckets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bucket_ops() {
        // Requires etcd server
    }
}
