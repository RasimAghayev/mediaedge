//! Object metadata operations.

use crate::keys;
use crate::schema::{ObjectSchema, entity_key};
use etcd_client::Client;

/// Object operations.
pub struct ObjectOps {
    client: Client,
}

impl ObjectOps {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Create an object.
    pub async fn create(&mut self, object: ObjectSchema) -> anyhow::Result<()> {
        let key = entity_key(keys::OBJECTS, &object.id);
        let value = serde_json::to_string(&object)?;
        self.client.put(key, value, None).await?;
        Ok(())
    }

    /// Get an object.
    pub async fn get(&mut self, id: &str) -> anyhow::Result<Option<ObjectSchema>> {
        let key = entity_key(keys::OBJECTS, id);
        let resp = self.client.get(key, None).await?;
        Ok(resp
            .kvs()
            .first()
            .map(|kv| serde_json::from_slice(kv.value()).unwrap()))
    }

    /// Delete an object.
    pub async fn delete(&mut self, id: &str) -> anyhow::Result<()> {
        let key = entity_key(keys::OBJECTS, id);
        self.client.delete(key, None).await?;
        Ok(())
    }

    /// List objects in a bucket.
    pub async fn list_by_bucket(&mut self, bucket_id: &str) -> anyhow::Result<Vec<ObjectSchema>> {
        let prefix = format!("{}/{}", keys::OBJECTS, bucket_id);
        let resp = self
            .client
            .get(prefix, Some(etcd_client::GetOptions::new().with_prefix()))
            .await?;
        let mut objects = Vec::new();
        for kv in resp.kvs() {
            if let Ok(object) = serde_json::from_slice(kv.value()) {
                objects.push(object);
            }
        }
        Ok(objects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_object_ops() {
        // Requires etcd server
    }
}
