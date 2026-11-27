use crate::{MediaEdgeClient, Result, models::*};

pub struct StreamsApi {
    client: MediaEdgeClient,
}

impl StreamsApi {
    pub fn new(client: MediaEdgeClient) -> Self {
        Self { client }
    }

    /// List streams with pagination and filters
    pub async fn list(&self, params: Option<ListParams>) -> Result<PaginatedResponse<Stream>> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(page) = p.page { query.push(("page", page.to_string())); }
            if let Some(per_page) = p.per_page { query.push(("per_page", per_page.to_string())); }
            if let Some(search) = p.search { query.push(("search", search)); }
            if let Some(sort) = p.sort { query.push(("sort", sort)); }
            if let Some(order) = p.order { query.push(("order", order.to_string())); }
        }
        self.client.get_with_query("/api/v1/streams", &query).await
    }

    /// Get a single stream by ID
    pub async fn get(&self, id: Uuid) -> Result<Stream> {
        self.client.get(&format!("/api/v1/streams/{}", id)).await
    }

    /// Create a new stream
    pub async fn create(&self, request: CreateStreamRequest) -> Result<Stream> {
        self.client.post("/api/v1/streams", &request).await
    }

    /// Update a stream
    pub async fn update(&self, id: Uuid, request: UpdateStreamRequest) -> Result<Stream> {
        self.client.patch(&format!("/api/v1/streams/{}", id), &request).await
    }

    /// Delete a stream
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.client.delete(&format!("/api/v1/streams/{}", id)).await
    }

    /// Start a stream
    pub async fn start(&self, id: Uuid) -> Result<Stream> {
        self.client.post(&format!("/api/v1/streams/{}/start", id), &()).await
    }

    /// Stop a stream
    pub async fn stop(&self, id: Uuid) -> Result<Stream> {
        self.client.post(&format!("/api/v1/streams/{}/stop", id), &()).await
    }

    /// Get stream metrics
    pub async fn metrics(&self, id: Uuid, params: Option<MetricsQueryParams>) -> Result<Vec<StreamMetrics>> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(start) = p.start { query.push(("start", start.to_rfc3339())); }
            if let Some(end) = p.end { query.push(("end", end.to_rfc3339())); }
            if let Some(interval) = p.interval { query.push(("interval", interval)); }
        }
        self.client.get_with_query(&format!("/api/v1/streams/{}/metrics", id), &query).await
    }

    /// Get stream playback URLs
    pub async fn playback_urls(&self, id: Uuid) -> Result<PlaybackUrls> {
        self.client.get(&format!("/api/v1/streams/{}/playback", id)).await
    }

    /// Get stream ingest URL
    pub async fn ingest_url(&self, id: Uuid) -> Result<String> {
        #[derive(Deserialize)]
        struct IngestUrlResponse { url: String }
        let resp: IngestUrlResponse = self.client.get(&format!("/api/v1/streams/{}/ingest", id)).await?;
        Ok(resp.url)
    }
}

#[derive(Default)]
pub struct MetricsQueryParams {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub interval: Option<String>,
}

use crate::models::{ListParams, PaginatedResponse};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::Deserialize;