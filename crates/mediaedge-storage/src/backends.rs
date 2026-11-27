//! Storage backends (FS, S3, AzBlob, GCS, Memory).

use crate::store::{ObjectMeta, ObjectStore};
use async_trait::async_trait;
use bytes::Bytes;
use opendal::{
    Operator,
    layers::LoggingLayer,
    services::{AzBlob, Fs, Gcs, Memory, S3},
};

/// Filesystem backend.
pub struct FsBackend {
    operator: Operator,
}

impl FsBackend {
    pub fn new(root: &str) -> anyhow::Result<Self> {
        let mut builder = Fs::default();
        builder = builder.root(root);
        let op = Operator::new(builder)?
            .layer(LoggingLayer::default())
            .finish();
        Ok(Self { operator: op })
    }
}

#[async_trait]
impl ObjectStore for FsBackend {
    async fn put(&self, key: &str, data: Bytes, meta: ObjectMeta) -> anyhow::Result<()> {
        let mut writer = self.operator.writer(key).await?;
        writer.write(data).await?;
        writer.close().await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> anyhow::Result<Option<(Bytes, ObjectMeta)>> {
        let reader = match self.operator.reader(key).await {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let data = reader.read_all().await?;
        let meta = ObjectMeta {
            key: key.into(),
            size: data.len() as u64,
            etag: format!("\"{}\"", blake3::hash(&data).to_hex()),
            content_type: None,
            last_modified: chrono::Utc::now(),
            metadata: Default::default(),
        };
        Ok(Some((data, meta)))
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.operator.delete(key).await?;
        Ok(())
    }

    async fn list(&self, prefix: &str) -> anyhow::Result<Vec<ObjectMeta>> {
        let mut entries = Vec::new();
        let mut lister = self.operator.lister(prefix).await?;
        while let Some(entry) = lister.next().await {
            let entry = entry?;
            entries.push(ObjectMeta {
                key: entry.path().to_string(),
                size: entry.metadata().content_length().unwrap_or(0),
                etag: entry.metadata().etag().unwrap_or_default().to_string(),
                content_type: entry.metadata().content_type().map(|s| s.to_string()),
                last_modified: entry
                    .metadata()
                    .last_modified()
                    .map(|t| t.into())
                    .unwrap_or_else(chrono::Utc::now),
                metadata: Default::default(),
            });
        }
        Ok(entries)
    }
}

/// S3 backend.
pub struct S3Backend {
    operator: Operator,
}

impl S3Backend {
    pub fn new(
        bucket: &str,
        region: &str,
        access_key: &str,
        secret_key: &str,
    ) -> anyhow::Result<Self> {
        let mut builder = S3::default();
        builder = builder
            .bucket(bucket)
            .region(region)
            .access_key_id(access_key)
            .secret_access_key(secret_key);
        let op = Operator::new(builder)?
            .layer(LoggingLayer::default())
            .finish();
        Ok(Self { operator: op })
    }
}

#[async_trait]
impl ObjectStore for S3Backend {
    async fn put(&self, key: &str, data: Bytes, meta: ObjectMeta) -> anyhow::Result<()> {
        let mut writer = self.operator.writer(key).await?;
        writer.write(data).await?;
        writer.close().await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> anyhow::Result<Option<(Bytes, ObjectMeta)>> {
        let reader = match self.operator.reader(key).await {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let data = reader.read_all().await?;
        let meta = ObjectMeta {
            key: key.into(),
            size: data.len() as u64,
            etag: format!("\"{}\"", blake3::hash(&data).to_hex()),
            content_type: None,
            last_modified: chrono::Utc::now(),
            metadata: Default::default(),
        };
        Ok(Some((data, meta)))
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.operator.delete(key).await?;
        Ok(())
    }

    async fn list(&self, prefix: &str) -> anyhow::Result<Vec<ObjectMeta>> {
        let mut entries = Vec::new();
        let mut lister = self.operator.lister(prefix).await?;
        while let Some(entry) = lister.next().await {
            let entry = entry?;
            entries.push(ObjectMeta {
                key: entry.path().to_string(),
                size: entry.metadata().content_length().unwrap_or(0),
                etag: entry.metadata().etag().unwrap_or_default().to_string(),
                content_type: entry.metadata().content_type().map(|s| s.to_string()),
                last_modified: entry
                    .metadata()
                    .last_modified()
                    .map(|t| t.into())
                    .unwrap_or_else(chrono::Utc::now),
                metadata: Default::default(),
            });
        }
        Ok(entries)
    }
}

/// Azure Blob backend.
pub struct AzBlobBackend {
    operator: Operator,
}

impl AzBlobBackend {
    pub fn new(container: &str, account: &str, key: &str) -> anyhow::Result<Self> {
        let mut builder = AzBlob::default();
        builder = builder
            .container(container)
            .account_name(account)
            .account_key(key);
        let op = Operator::new(builder)?
            .layer(LoggingLayer::default())
            .finish();
        Ok(Self { operator: op })
    }
}

#[async_trait]
impl ObjectStore for AzBlobBackend {
    async fn put(&self, key: &str, data: Bytes, meta: ObjectMeta) -> anyhow::Result<()> {
        let mut writer = self.operator.writer(key).await?;
        writer.write(data).await?;
        writer.close().await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> anyhow::Result<Option<(Bytes, ObjectMeta)>> {
        let reader = match self.operator.reader(key).await {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let data = reader.read_all().await?;
        let meta = ObjectMeta {
            key: key.into(),
            size: data.len() as u64,
            etag: format!("\"{}\"", blake3::hash(&data).to_hex()),
            content_type: None,
            last_modified: chrono::Utc::now(),
            metadata: Default::default(),
        };
        Ok(Some((data, meta)))
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.operator.delete(key).await?;
        Ok(())
    }

    async fn list(&self, prefix: &str) -> anyhow::Result<Vec<ObjectMeta>> {
        let mut entries = Vec::new();
        let mut lister = self.operator.lister(prefix).await?;
        while let Some(entry) = lister.next().await {
            let entry = entry?;
            entries.push(ObjectMeta {
                key: entry.path().to_string(),
                size: entry.metadata().content_length().unwrap_or(0),
                etag: entry.metadata().etag().unwrap_or_default().to_string(),
                content_type: entry.metadata().content_type().map(|s| s.to_string()),
                last_modified: entry
                    .metadata()
                    .last_modified()
                    .map(|t| t.into())
                    .unwrap_or_else(chrono::Utc::now),
                metadata: Default::default(),
            });
        }
        Ok(entries)
    }
}

/// GCS backend.
pub struct GcsBackend {
    operator: Operator,
}

impl GcsBackend {
    pub fn new(bucket: &str, credentials: &str) -> anyhow::Result<Self> {
        let mut builder = Gcs::default();
        builder = builder.bucket(bucket).credential(credentials);
        let op = Operator::new(builder)?
            .layer(LoggingLayer::default())
            .finish();
        Ok(Self { operator: op })
    }
}

#[async_trait]
impl ObjectStore for GcsBackend {
    async fn put(&self, key: &str, data: Bytes, meta: ObjectMeta) -> anyhow::Result<()> {
        let mut writer = self.operator.writer(key).await?;
        writer.write(data).await?;
        writer.close().await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> anyhow::Result<Option<(Bytes, ObjectMeta)>> {
        let reader = match self.operator.reader(key).await {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let data = reader.read_all().await?;
        let meta = ObjectMeta {
            key: key.into(),
            size: data.len() as u64,
            etag: format!("\"{}\"", blake3::hash(&data).to_hex()),
            content_type: None,
            last_modified: chrono::Utc::now(),
            metadata: Default::default(),
        };
        Ok(Some((data, meta)))
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.operator.delete(key).await?;
        Ok(())
    }

    async fn list(&self, prefix: &str) -> anyhow::Result<Vec<ObjectMeta>> {
        let mut entries = Vec::new();
        let mut lister = self.operator.lister(prefix).await?;
        while let Some(entry) = lister.next().await {
            let entry = entry?;
            entries.push(ObjectMeta {
                key: entry.path().to_string(),
                size: entry.metadata().content_length().unwrap_or(0),
                etag: entry.metadata().etag().unwrap_or_default().to_string(),
                content_type: entry.metadata().content_type().map(|s| s.to_string()),
                last_modified: entry
                    .metadata()
                    .last_modified()
                    .map(|t| t.into())
                    .unwrap_or_else(chrono::Utc::now),
                metadata: Default::default(),
            });
        }
        Ok(entries)
    }
}

/// Memory backend.
pub struct MemoryBackend {
    store: crate::store::MemoryStore,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self {
            store: crate::store::MemoryStore::new(),
        }
    }
}

#[async_trait]
impl ObjectStore for MemoryBackend {
    async fn put(&self, key: &str, data: Bytes, meta: ObjectMeta) -> anyhow::Result<()> {
        self.store.put(key, data, meta).await
    }

    async fn get(&self, key: &str) -> anyhow::Result<Option<(Bytes, ObjectMeta)>> {
        self.store.get(key).await
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.store.delete(key).await
    }

    async fn list(&self, prefix: &str) -> anyhow::Result<Vec<ObjectMeta>> {
        self.store.list(prefix).await
    }
}

impl Default for MemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fs_backend() {
        let tmp = tempfile::tempdir().unwrap();
        let backend = FsBackend::new(tmp.path().to_str().unwrap()).unwrap();
        let meta = ObjectMeta {
            key: "test".into(),
            size: 4,
            etag: "abcd".into(),
            content_type: None,
            last_modified: chrono::Utc::now(),
            metadata: Default::default(),
        };
        backend
            .put("test", Bytes::from("data"), meta)
            .await
            .unwrap();
        let (data, _) = backend.get("test").await.unwrap().unwrap();
        assert_eq!(data, Bytes::from("data"));
    }
}
