//! Multipart upload support.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Multipart upload ID.
pub type UploadId = String;

/// Part number (1-based).
pub type PartNumber = u32;

/// Multipart upload metadata.
#[derive(Debug, Clone)]
pub struct MultipartUpload {
    pub upload_id: UploadId,
    pub key: String,
    pub initiated_at: chrono::DateTime<chrono::Utc>,
    pub parts: HashMap<PartNumber, PartInfo>,
}

/// Part information.
#[derive(Debug, Clone)]
pub struct PartInfo {
    pub part_number: PartNumber,
    pub etag: String,
    pub size: u64,
    pub uploaded_at: chrono::DateTime<chrono::Utc>,
}

/// Multipart upload manager.
pub struct MultipartManager {
    uploads: Arc<RwLock<HashMap<UploadId, MultipartUpload>>>,
}

impl MultipartManager {
    pub fn new() -> Self {
        Self {
            uploads: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initiate a multipart upload.
    pub async fn create_upload(&self, key: String) -> UploadId {
        let upload_id = uuid::Uuid::new_v4().to_string();
        let upload = MultipartUpload {
            upload_id: upload_id.clone(),
            key,
            initiated_at: chrono::Utc::now(),
            parts: HashMap::new(),
        };
        self.uploads.write().await.insert(upload_id.clone(), upload);
        upload_id
    }

    /// Upload a part.
    pub async fn upload_part(
        &self,
        upload_id: &str,
        part_number: PartNumber,
        etag: String,
        size: u64,
    ) -> anyhow::Result<()> {
        let mut uploads = self.uploads.write().await;
        let upload = uploads
            .get_mut(upload_id)
            .ok_or_else(|| anyhow::anyhow!("Upload not found"))?;
        upload.parts.insert(
            part_number,
            PartInfo {
                part_number,
                etag,
                size,
                uploaded_at: chrono::Utc::now(),
            },
        );
        Ok(())
    }

    /// Complete a multipart upload.
    pub async fn complete_upload(&self, upload_id: &str) -> anyhow::Result<MultipartUpload> {
        let uploads = self.uploads.write().await;
        let upload = uploads
            .remove(upload_id)
            .ok_or_else(|| anyhow::anyhow!("Upload not found"))?;
        Ok(upload)
    }

    /// Abort a multipart upload.
    pub async fn abort_upload(&self, upload_id: &str) -> anyhow::Result<()> {
        self.uploads.write().await.remove(upload_id);
        Ok(())
    }

    /// Get upload info.
    pub async fn get_upload(&self, upload_id: &str) -> Option<MultipartUpload> {
        self.uploads.read().await.get(upload_id).cloned()
    }

    /// List all uploads.
    pub async fn list_uploads(&self) -> Vec<MultipartUpload> {
        self.uploads.read().await.values().cloned().collect()
    }
}

impl Default for MultipartManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_multipart() {
        let mgr = MultipartManager::new();
        let id = mgr.create_upload("test".into()).await;
        mgr.upload_part(&id, 1, "etag1".into(), 100).await.unwrap();
        mgr.upload_part(&id, 2, "etag2".into(), 200).await.unwrap();
        let upload = mgr.complete_upload(&id).await.unwrap();
        assert_eq!(upload.parts.len(), 2);
    }
}
