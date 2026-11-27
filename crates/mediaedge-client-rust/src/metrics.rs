use crate::{MediaEdgeClient, Result, models::*};

pub struct MetricsApi {
    client: MediaEdgeClient,
}

impl MetricsApi {
    pub fn new(client: MediaEdgeClient) -> Self {
        Self { client }
    }

    /// Query Prometheus metrics
    pub async fn query(&self, query: &str, time: Option<DateTime<Utc>>) -> Result<MetricsQueryResult> {
        let mut params = vec![("query", query.to_string())];
        if let Some(t) = time {
            params.push(("time", t.to_rfc3339()));
        }
        self.client.get_with_query("/api/v1/metrics/query", &params).await
    }

    /// Query range
    pub async fn query_range(&self, request: MetricsQuery) -> Result<MetricsResult> {
        let mut params = Vec::new();
        params.push(("metric", request.metric));
        params.push(("start", request.start.to_rfc3339()));
        params.push(("end", request.end.to_rfc3339()));
        if let Some(step) = request.step {
            params.push(("step", step));
        }
        if let Some(labels) = request.labels {
            params.push(("labels", labels.to_string()));
        }
        self.client.get_with_query("/api/v1/metrics/query_range", &params).await
    }

    /// Get stream metrics
    pub async fn stream_metrics(&self, stream_id: Uuid, params: Option<MetricsQueryParams>) -> Result<Vec<StreamMetrics>> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(start) = p.start { query.push(("start", start.to_rfc3339())); }
            if let Some(end) = p.end { query.push(("end", end.to_rfc3339())); }
            if let Some(interval) = p.interval { query.push(("interval", interval)); }
        }
        self.client.get_with_query(&format!("/api/v1/streams/{}/metrics", stream_id), &query).await
    }

    /// Get cluster metrics
    pub async fn cluster_metrics(&self, params: Option<MetricsQueryParams>) -> Result<ClusterMetrics> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(start) = p.start { query.push(("start", start.to_rfc3339())); }
            if let Some(end) = p.end { query.push(("end", end.to_rfc3339())); }
            if let Some(interval) = p.interval { query.push(("interval", interval)); }
        }
        self.client.get_with_query("/api/v1/cluster/metrics", &query).await
    }

    /// Get tenant metrics
    pub async fn tenant_metrics(&self, tenant_id: Uuid, params: Option<MetricsQueryParams>) -> Result<TenantMetrics> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(start) = p.start { query.push(("start", start.to_rfc3339())); }
            if let Some(end) = p.end { query.push(("end", end.to_rfc3339())); }
            if let Some(interval) = p.interval { query.push(("interval", interval)); }
        }
        self.client.get_with_query(&format!("/api/v1/tenants/{}/metrics", tenant_id), &query).await
    }

    /// Get anomaly alerts
    pub async fn anomalies(&self, params: Option<ListParams>) -> Result<PaginatedResponse<AnomalyAlert>> {
        let mut query = Vec::new();
        if let Some(p) = params {
            if let Some(page) = p.page { query.push(("page", page.to_string())); }
            if let Some(per_page) = p.per_page { query.push(("per_page", per_page.to_string())); }
        }
        self.client.get_with_query("/api/v1/observability/anomalies", &query).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsQueryResult {
    pub status: String,
    pub data: MetricsData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsData {
    pub result_type: String,
    pub result: Vec<MetricsSeries>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSeries {
    pub metric: serde_json::Value,
    pub value: (f64, String), // (timestamp, value)
}

#[derive(Default)]
pub struct MetricsQueryParams {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub interval: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterMetrics {
    pub total_streams: u32,
    pub active_streams: u32,
    pub total_viewers: u64,
    pub total_bandwidth_mbps: f64,
    pub total_storage_gb: f64,
    pub transcoding_hours: f64,
    pub nodes_healthy: u32,
    pub nodes_total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantMetrics {
    pub tenant_id: Uuid,
    pub streams: u32,
    pub viewers: u64,
    pub bandwidth_mbps: f64,
    pub storage_gb: f64,
    pub transcoding_hours: f64,
    pub revenue_cents: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyAlert {
    pub id: Uuid,
    pub stream_id: Option<Uuid>,
    pub tenant_id: Uuid,
    pub anomaly_type: String,
    pub severity: String,
    pub message: String,
    pub metric_value: f64,
    pub threshold: f64,
    pub detected_at: DateTime<Utc>,
    pub acknowledged: bool,
}

use crate::models::{ListParams, PaginatedResponse};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Serialize, Deserialize};