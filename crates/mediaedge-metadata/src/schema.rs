//! etcd schema definitions.

use serde::{Deserialize, Serialize};

/// Stream metadata schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSchema {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub tenant_id: String,
    pub status: String,
    pub settings: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Tenant metadata schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantSchema {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub status: String,
    pub quotas: serde_json::Value,
    pub branding: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Cluster node schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSchema {
    pub id: String,
    pub region: String,
    pub datacenter: String,
    pub rack: Option<String>,
    pub status: String,
    pub capacity: NodeCapacity,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Node capacity schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapacity {
    pub max_streams: u32,
    pub max_viewers: u64,
    pub max_bandwidth_bps: u64,
    pub storage_capacity_bytes: u64,
}

/// Bucket metadata schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketSchema {
    pub id: String,
    pub name: String,
    pub tenant_id: String,
    pub region: String,
    pub storage_class: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Object metadata schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSchema {
    pub id: String,
    pub bucket_id: String,
    pub key: String,
    pub size: u64,
    pub etag: String,
    pub content_type: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Key prefixes for etcd.
pub mod keys {
    pub const STREAMS: &str = "/mediaedge/streams";
    pub const TENANTS: &str = "/mediaedge/tenants";
    pub const NODES: &str = "/mediaedge/nodes";
    pub const BUCKETS: &str = "/mediaedge/buckets";
    pub const OBJECTS: &str = "/mediaedge/objects";
    pub const CONFIG: &str = "/mediaedge/config";
    pub const LOCKS: &str = "/mediaedge/locks";
}

/// Build a key for an entity.
pub fn entity_key(prefix: &str, id: &str) -> String {
    format!("{}/{}", prefix.trim_end_matches('/'), id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_key() {
        assert_eq!(entity_key(keys::STREAMS, "123"), "/mediaedge/streams/123");
    }
}
