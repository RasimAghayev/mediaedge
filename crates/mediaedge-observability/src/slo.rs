//! SLO (Service Level Objective) tracking with client-sample integration.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Body,
    extract::{Json as JsonExtract, State},
    http::StatusCode,
    response::Json,
    routing::post,
    Router,
};
use dashmap::DashMap;
use hdrhistogram::Histogram;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use crate::types::{ObservabilityConfig, SloConfig, AlertThresholds};
use crate::metrics::MetricsRegistry;

/// SLO tracker for latency objectives.
pub struct SloTracker {
    config: SloConfig,
    histograms: Arc<DashMap<String, Arc<Histogram<u64>>>>,
    // Client-sample histograms
    client_histograms: Arc<DashMap<String, Arc<Histogram<u64>>>>,
    // Alert state
    alert_state: Arc<DashMap<String, AlertState>>,
    // Broadcast for real-time updates
    update_sender: broadcast::Sender<SloUpdate>,
}

#[derive(Debug, Clone)]
struct AlertState {
    last_alert: Option<chrono::DateTime<chrono::Utc>>,
    consecutive_violations: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SloUpdate {
    pub transport: String,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub target_ms: u64,
    pub status: SloStatus,
    pub error_budget_remaining: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// SLO status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SloStatus {
    Healthy,
    Warning,
    Critical,
}

impl SloTracker {
    /// Create a new SLO tracker.
    pub fn new(config: SloConfig) -> Self {
        let histograms = Arc::new(DashMap::new());
        let client_histograms = Arc::new(DashMap::new());

        // Initialize histograms for each transport
        for transport in &config.transports {
            let histogram = Arc::new(Histogram::<u64>::new(3).expect("Failed to create histogram"));
            histograms.insert(transport.clone(), histogram);

            let client_histogram = Arc::new(Histogram::<u64>::new(3).expect("Failed to create histogram"));
            client_histograms.insert(transport.clone(), client_histogram);
        }

        let (update_sender, _) = broadcast::channel(1000);

        Self {
            config,
            histograms,
            client_histograms,
            alert_state: Arc::new(DashMap::new()),
            update_sender,
        }
    }

    /// Record server-side latency measurement.
    pub fn record_latency(&self, transport: &str, latency: Duration) {
        if let Some(histogram) = self.histograms.get(transport) {
            let micros = latency.as_micros() as u64;
            let _ = histogram.record(micros);
            self.check_and_alert(transport);
        }
    }

    /// Record client-sampled latency.
    pub fn record_client_sample(&self, transport: &str, latency: Duration) {
        if let Some(histogram) = self.client_histograms.get(transport) {
            let micros = latency.as_micros() as u64;
            let _ = histogram.record(micros);
        }
    }

    /// Record multiple client samples (batch).
    pub fn record_client_samples(&self, samples: Vec<ClientSample>) {
        for sample in samples {
            self.record_client_sample(&sample.transport, sample.latency);
        }
    }

    /// Check SLO and send alerts if needed.
    fn check_and_alert(&self, transport: &str) {
        if let Some(target_ms) = self.config.targets.get(transport) {
            if let Some(histogram) = self.histograms.get(transport) {
                let p99 = percentile(histogram, 99.0) as f64 / 1000.0; // Convert to ms
                let target = *target_ms as f64;

                let warning_threshold = target * self.config.alert_thresholds.warning_pct / 100.0;
                let critical_threshold = target * self.config.alert_thresholds.critical_pct / 100.0;

                let status = if p99 >= critical_threshold {
                    SloStatus::Critical
                } else if p99 >= warning_threshold {
                    SloStatus::Warning
                } else {
                    SloStatus::Healthy
                };

                // Update alert state
                let mut alert_state = self.alert_state.entry(transport.to_string()).or_insert(AlertState {
                    last_alert: None,
                    consecutive_violations: 0,
                });

                match status {
                    SloStatus::Healthy => {
                        alert_state.consecutive_violations = 0;
                    }
                    SloStatus::Warning | SloStatus::Critical => {
                        alert_state.consecutive_violations += 1;
                        let now = chrono::Utc::now();
                        
                        // Alert on first violation or every 10 violations
                        if alert_state.consecutive_violations == 1 || 
                           alert_state.consecutive_violations % 10 == 0 {
                            alert_state.last_alert = Some(now);
                            warn!("SLO {} for {}: p99={:.2}ms target={}ms", 
                                status as u8, transport, p99, target);
                        }
                    }
                }

                // Broadcast update
                let p50 = percentile(histogram, 50.0) as f64 / 1000.0;
                let p95 = percentile(histogram, 95.0) as f64 / 1000.0;
                let error_budget = if p99 >= target { 0.0 } else { 1.0 - p99 / target };

                let update = SloUpdate {
                    transport: transport.to_string(),
                    p50_ms: p50,
                    p95_ms: p95,
                    p99_ms: p99,
                    target_ms: *target_ms,
                    status,
                    error_budget_remaining: error_budget,
                    timestamp: chrono::Utc::now(),
                };

                let _ = self.update_sender.send(update);
            }
        }
    }

    /// Get current SLO status for all transports.
    pub fn get_status(&self) -> SloStatusReport {
        let mut transports = HashMap::new();
        let mut overall = SloStatus::Healthy;

        for transport in &self.config.transports {
            if let Some(histogram) = self.histograms.get(transport) {
                let target_ms = *self.config.targets.get(transport).unwrap_or(&0);
                let status = self.calculate_status(transport, histogram, target_ms);
                
                if status == SloStatus::Critical {
                    overall = SloStatus::Critical;
                } else if status == SloStatus::Warning && overall == SloStatus::Healthy {
                    overall = SloStatus::Warning;
                }

                let p50 = percentile(histogram, 50.0) as f64 / 1000.0;
                let p95 = percentile(histogram, 95.0) as f64 / 1000.0;
                let p99 = percentile(histogram, 99.0) as f64 / 1000.0;
                let error_budget = if p99 >= target_ms as f64 { 0.0 } else { 1.0 - p99 / target_ms as f64 };

                transports.insert(transport.clone(), TransportSloStatus {
                    transport: transport.clone(),
                    target_ms,
                    p50_ms: p50,
                    p95_ms: p95,
                    p99_ms: p99,
                    status,
                    error_budget_remaining: error_budget,
                    sample_count: histogram.len() as u64,
                });
            }
        }

        SloStatusReport {
            overall_status: overall,
            transports,
            evaluated_at: chrono::Utc::now(),
        }
    }

    fn calculate_status(&self, transport: &str, histogram: &Histogram<u64>, target_ms: u64) -> SloStatus {
        let p99 = percentile(histogram, 99.0) as f64 / 1000.0;
        let target = target_ms as f64;

        if p99 >= target * self.config.alert_thresholds.critical_pct / 100.0 {
            SloStatus::Critical
        } else if p99 >= target * self.config.alert_thresholds.warning_pct / 100.0 {
            SloStatus::Warning
        } else {
            SloStatus::Healthy
        }
    }

    /// Subscribe to SLO updates.
    pub fn subscribe(&self) -> broadcast::Receiver<SloUpdate> {
        self.update_sender.subscribe()
    }

    /// Get histogram for a transport (for metrics export).
    pub fn get_histogram(&self, transport: &str) -> Option<Arc<Histogram<u64>>> {
        self.histograms.get(transport).map(|h| h.clone())
    }

    /// Get client histogram for a transport.
    pub fn get_client_histogram(&self, transport: &str) -> Option<Arc<Histogram<u64>>> {
        self.client_histograms.get(transport).map(|h| h.clone())
    }
}

/// Calculate percentile from HDR histogram.
fn percentile(histogram: &Histogram<u64>, percentile: f64) -> u64 {
    histogram.value_at_quantile(percentile / 100.0)
}

/// Client sample from browser/MoQ client.
#[derive(Debug, Clone, Deserialize)]
pub struct ClientSample {
    pub transport: String,
    pub latency: Duration,
    pub session_id: String,
    pub client_version: Option<String>,
    pub player_type: Option<String>,
    pub quality: Option<String>,
    pub region: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Batch client samples request.
#[derive(Debug, Deserialize)]
pub struct ClientSampleBatch {
    pub samples: Vec<ClientSample>,
}

/// SLO status report.
#[derive(Debug, Serialize)]
pub struct SloStatusReport {
    pub overall_status: SloStatus,
    pub transports: HashMap<String, TransportSloStatus>,
    pub evaluated_at: chrono::DateTime<chrono::Utc>,
}

/// Transport SLO status.
#[derive(Debug, Serialize)]
pub struct TransportSloStatus {
    pub transport: String,
    pub target_ms: u64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub status: SloStatus,
    pub error_budget_remaining: f64,
    pub sample_count: u64,
}

/// Create SLO routes.
pub fn slo_routes(tracker: Arc<SloTracker>) -> Router {
    Router::new()
        .route("/api/v1/slo/client-sample", post(client_sample_handler))
        .route("/api/v1/slo/client-sample/batch", post(client_sample_batch_handler))
        .route("/api/v1/slo/status", get(slo_status_handler))
        .route("/api/v1/slo/status/:transport", get(transport_slo_status_handler))
        .route("/api/v1/slo/stream", get(slo_stream_handler))
        .with_state(tracker)
}

/// POST /api/v1/slo/client-sample - Single client sample.
async fn client_sample_handler(
    State(tracker): State<Arc<SloTracker>>,
    JsonExtract(sample): JsonExtract<ClientSample>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    tracker.record_client_sample(&sample.transport, sample.latency);
    Ok(Json(json!({ "status": "ok" })))
}

/// POST /api/v1/slo/client-sample/batch - Batch client samples.
async fn client_sample_batch_handler(
    State(tracker): State<Arc<SloTracker>>,
    JsonExtract(batch): JsonExtract<ClientSampleBatch>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    tracker.record_client_samples(batch.samples);
    Ok(Json(json!({ "status": "ok", "count": batch.samples.len() })))
}

/// GET /api/v1/slo/status - Overall SLO status.
async fn slo_status_handler(
    State(tracker): State<Arc<SloTracker>>,
) -> Json<SloStatusReport> {
    Json(tracker.get_status())
}

/// GET /api/v1/slo/status/:transport - Transport-specific SLO status.
async fn transport_slo_status_handler(
    State(tracker): State<Arc<SloTracker>>,
    axum::extract::Path(transport): axum::extract::Path<String>,
) -> Result<Json<TransportSloStatus>, StatusCode> {
    let report = tracker.get_status();
    report.transports.get(&transport)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// GET /api/v1/slo/stream - Real-time SLO updates via SSE.
async fn slo_stream_handler(
    State(tracker): State<Arc<SloTracker>>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::io::Error>>> {
    let mut receiver = tracker.subscribe();
    
    let stream = async_stream::stream! {
        while let Ok(update) = receiver.recv().await {
            let data = serde_json::to_string(&update).unwrap_or_default();
            yield Ok(Event::default().data(data));
        }
    };

    Sse::new(stream)
}

use axum::response::sse::Event;

/// SLO evaluator for integration with metrics.
pub struct SloEvaluator {
    tracker: Arc<SloTracker>,
    metrics_registry: Arc<MetricsRegistry>,
}

impl SloEvaluator {
    pub fn new(tracker: Arc<SloTracker>, metrics_registry: Arc<MetricsRegistry>) -> Self {
        Self { tracker, metrics_registry }
    }

    /// Evaluate and return SLO status.
    pub fn evaluate(&self) -> SloStatusReport {
        self.tracker.get_status()
    }

    /// Record latency from metrics registry.
    pub fn record_from_metrics(&self, transport: &str, latency: Duration) {
        self.tracker.record_latency(transport, latency);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SloConfig;

    #[test]
    fn test_slo_tracker() {
        let mut config = SloConfig::default();
        config.transports = vec!["hls".to_string(), "moq".to_string()];
        config.targets.insert("hls".to_string(), 2000);
        config.targets.insert("moq".to_string(), 100);

        let tracker = SloTracker::new(config);

        // Record some latencies
        for _ in 0..100 {
            tracker.record_latency("hls", Duration::from_millis(500));
            tracker.record_latency("moq", Duration::from_millis(50));
        }

        let report = tracker.get_status();
        assert_eq!(report.overall_status, SloStatus::Healthy);
        assert!(report.transports.contains_key("hls"));
        assert!(report.transports.contains_key("moq"));
    }

    #[test]
    fn test_slo_violation() {
        let mut config = SloConfig::default();
        config.transports = vec!["hls".to_string()];
        config.targets.insert("hls".to_string(), 100); // 100ms target
        config.alert_thresholds.warning_pct = 50.0;
        config.alert_thresholds.critical_pct = 90.0;

        let tracker = SloTracker::new(config);

        // Record latencies exceeding target
        for _ in 0..100 {
            tracker.record_latency("hls", Duration::from_millis(200)); // 200ms > 100ms
        }

        let report = tracker.get_status();
        assert_eq!(report.overall_status, SloStatus::Critical);
        assert_eq!(report.transports["hls"].status, SloStatus::Critical);
    }

    #[test]
    fn test_client_samples() {
        let mut config = SloConfig::default();
        config.transports = vec!["whep".to_string()];
        config.targets.insert("whep".to_string(), 500);

        let tracker = SloTracker::new(config);

        let sample = ClientSample {
            transport: "whep".to_string(),
            latency: Duration::from_millis(100),
            session_id: "session123".to_string(),
            client_version: Some("1.0".to_string()),
            player_type: Some("whep".to_string()),
            quality: Some("1080p".to_string()),
            region: Some("us-east".to_string()),
            timestamp: chrono::Utc::now(),
        };

        tracker.record_client_sample(&sample.transport, sample.latency);
        
        let client_hist = tracker.get_client_histogram("whep");
        assert!(client_hist.is_some());
        assert_eq!(client_hist.unwrap().len(), 1);
    }

    #[test]
    fn test_percentile() {
        let histogram = Histogram::<u64>::new(3).unwrap();
        for i in 1..=100 {
            histogram.record(i).unwrap();
        }
        
        let p50 = percentile(&histogram, 50.0);
        let p95 = percentile(&histogram, 95.0);
        let p99 = percentile(&histogram, 99.0);
        
        assert!(p50 >= 45 && p50 <= 55);
        assert!(p95 >= 90 && p95 <= 100);
        assert!(p99 >= 95 && p99 <= 100);
    }
}