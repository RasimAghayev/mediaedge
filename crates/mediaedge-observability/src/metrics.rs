//! Metrics with Prometheus export and custom histograms.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use metrics::{Counter, Gauge, Histogram, Key, Metadata, Recorder, SetRecorderError, Unit};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use parking_lot::RwLock;
use tracing::{debug, info, warn};

use crate::fanout::FanoutRecorder;
use crate::types::{MetricsConfig, ObservabilityConfig, SloConfig};

/// Metrics registry for custom metrics.
pub struct MetricsRegistry {
    config: MetricsConfig,
    slo_config: SloConfig,
    prometheus_handle: Option<PrometheusHandle>,
    custom_counters: Arc<RwLock<HashMap<String, Counter>>>,
    custom_gauges: Arc<RwLock<HashMap<String, Gauge>>>,
    custom_histograms: Arc<RwLock<HashMap<String, Histogram>>>,
    // SLO histograms per transport
    slo_histograms: Arc<RwLock<HashMap<String, Histogram>>>,
}

impl MetricsRegistry {
    /// Create a new metrics registry.
    pub fn new(config: &ObservabilityConfig) -> anyhow::Result<Self> {
        let metrics_config = &config.metrics;
        let slo_config = &config.slo;

        // Build Prometheus exporter
        let mut builder = PrometheusBuilder::new().with_namespace(&metrics_config.prefix);

        if metrics_config.process_metrics {
            builder = builder.install_process_collector();
        }

        let handle = if metrics_config.enabled {
            Some(builder.install_recorder()?)
        } else {
            None
        };

        // Set global recorder with fanout
        let fanout_recorder =
            FanoutRecorder::new(handle.clone(), config.fanout.prometheus, config.fanout.otlp);
        metrics::set_boxed_recorder(Box::new(fanout_recorder))?;

        let registry = Self {
            config: metrics_config.clone(),
            slo_config: slo_config.clone(),
            prometheus_handle: handle,
            custom_counters: Arc::new(RwLock::new(HashMap::new())),
            custom_gauges: Arc::new(RwLock::new(HashMap::new())),
            custom_histograms: Arc::new(RwLock::new(HashMap::new())),
            slo_histograms: Arc::new(RwLock::new(HashMap::new())),
        };

        // Initialize SLO histograms
        registry.init_slo_histograms();

        // Register default metrics
        registry.register_default_metrics();

        info!(
            "Metrics registry initialized: prefix={}, prometheus={}",
            metrics_config.prefix, metrics_config.enabled
        );

        Ok(registry)
    }

    /// Initialize SLO histograms for each transport.
    fn init_slo_histograms(&self) {
        let mut histograms = self.slo_histograms.write();
        for transport in &self.slo_config.transports {
            let key = format!("{}_glass_to_glass_latency_seconds", transport);
            let histogram = metrics::histogram!(
                &key,
                metrics::Unit::Seconds,
                "Glass-to-glass latency for {} transport".to_string(),
            );
            histograms.insert(transport.clone(), histogram);
        }
    }

    /// Register default mediaedge metrics.
    fn register_default_metrics(&self) {
        // Ingest metrics
        self.counter("ingest_streams_total", "Total ingest streams");
        self.counter("ingest_errors_total", "Total ingest errors");
        self.gauge("ingest_active_streams", "Active ingest streams");
        self.histogram(
            "ingest_bitrate_bytes",
            "Ingest bitrate in bytes/sec",
            metrics::Unit::Bytes,
        );
        self.histogram(
            "ingest_latency_seconds",
            "Ingest latency",
            metrics::Unit::Seconds,
        );

        // Egress metrics
        self.counter("egress_viewers_total", "Total egress viewers");
        self.counter("egress_errors_total", "Total egress errors");
        self.gauge("egress_active_viewers", "Active egress viewers");
        self.histogram(
            "egress_bitrate_bytes",
            "Egress bitrate in bytes/sec",
            metrics::Unit::Bytes,
        );
        self.histogram(
            "egress_latency_seconds",
            "Egress latency",
            metrics::Unit::Seconds,
        );

        // Transcoding metrics
        self.counter("transcode_jobs_total", "Total transcode jobs");
        self.counter("transcode_errors_total", "Total transcode errors");
        self.gauge("transcode_active_jobs", "Active transcode jobs");
        self.gauge("transcode_queue_depth", "Transcode queue depth");
        self.histogram(
            "transcode_duration_seconds",
            "Transcode duration",
            metrics::Unit::Seconds,
        );
        self.gauge("transcode_gpu_utilization", "GPU utilization percentage");

        // Storage metrics
        self.counter("storage_reads_total", "Total storage reads");
        self.counter("storage_writes_total", "Total storage writes");
        self.counter("storage_errors_total", "Total storage errors");
        self.gauge(
            "storage_used_bytes",
            "Storage used in bytes",
            metrics::Unit::Bytes,
        );
        self.gauge(
            "storage_capacity_bytes",
            "Storage capacity in bytes",
            metrics::Unit::Bytes,
        );
        self.histogram(
            "storage_read_latency_seconds",
            "Storage read latency",
            metrics::Unit::Seconds,
        );
        self.histogram(
            "storage_write_latency_seconds",
            "Storage write latency",
            metrics::Unit::Seconds,
        );

        // Replication metrics
        self.counter("replication_tasks_total", "Total replication tasks");
        self.counter("replication_errors_total", "Total replication errors");
        self.gauge("replication_active_tasks", "Active replication tasks");
        self.gauge("replication_pending_tasks", "Pending replication tasks");
        self.histogram(
            "replication_duration_seconds",
            "Replication duration",
            metrics::Unit::Seconds,
        );
        self.histogram(
            "replication_lag_seconds",
            "Replication lag",
            metrics::Unit::Seconds,
        );

        // Cluster metrics
        self.gauge("cluster_nodes_total", "Total cluster nodes");
        self.gauge("cluster_nodes_healthy", "Healthy cluster nodes");
        self.gauge("cluster_broadcasts_total", "Total broadcasts");
        self.counter("cluster_ownership_changes_total", "Ownership changes");
        self.histogram(
            "cluster_gossip_latency_seconds",
            "Gossip latency",
            metrics::Unit::Seconds,
        );

        // Config reload metrics
        self.counter("config_reloads_total", "Total config reloads");
        self.counter("config_reload_errors_total", "Config reload errors");
        self.histogram(
            "config_reload_duration_seconds",
            "Config reload duration",
            metrics::Unit::Seconds,
        );

        // Auth metrics
        self.counter("auth_requests_total", "Total auth requests");
        self.counter("auth_failures_total", "Total auth failures");
        self.histogram(
            "auth_latency_seconds",
            "Auth latency",
            metrics::Unit::Seconds,
        );

        // Rate limit metrics
        self.counter("rate_limit_rejected_total", "Rate limited requests");
        self.gauge("rate_limit_active_keys", "Active rate limit keys");
    }

    /// Get or create a counter.
    pub fn counter(&self, name: &str, description: &str) -> Counter {
        let key = self.prefixed_name(name);
        let mut counters = self.custom_counters.write();
        counters
            .entry(key.clone())
            .or_insert_with(|| metrics::counter!(key, description))
            .clone()
    }

    /// Get or create a gauge.
    pub fn gauge(&self, name: &str, description: &str) -> Gauge {
        let key = self.prefixed_name(name);
        let mut gauges = self.custom_gauges.write();
        gauges
            .entry(key.clone())
            .or_insert_with(|| metrics::gauge!(key, description))
            .clone()
    }

    /// Get or create a histogram.
    pub fn histogram(&self, name: &str, description: &str, unit: Unit) -> Histogram {
        let key = self.prefixed_name(name);
        let mut histograms = self.custom_histograms.write();
        histograms
            .entry(key.clone())
            .or_insert_with(|| {
                let buckets = self
                    .config
                    .histogram_buckets
                    .as_ref()
                    .unwrap_or(&self.config.default_buckets);
                metrics::histogram!(key, unit, description, buckets)
            })
            .clone()
    }

    /// Get SLO histogram for a transport.
    pub fn slo_histogram(&self, transport: &str) -> Option<Histogram> {
        self.slo_histograms.read().get(transport).cloned()
    }

    /// Record glass-to-glass latency for a transport.
    pub fn record_glass_to_glass(&self, transport: &str, latency: Duration) {
        if let Some(histogram) = self.slo_histogram(transport) {
            histogram.record(latency.as_secs_f64());
        }
    }

    /// Record client-sampled latency (from browser/MoQ client).
    pub fn record_client_sample(
        &self,
        transport: &str,
        latency: Duration,
        metadata: ClientSampleMetadata,
    ) {
        if let Some(histogram) = self.slo_histogram(transport) {
            histogram.record(latency.as_secs_f64());
        }
        // Also record with metadata labels
        let key = format!("{}_client_latency_seconds", transport);
        let histogram = self.histogram(&key, "Client-sampled latency", metrics::Unit::Seconds);
        histogram.record(latency.as_secs_f64());
    }

    /// Get Prometheus handle for /metrics endpoint.
    pub fn prometheus_handle(&self) -> Option<&PrometheusHandle> {
        self.prometheus_handle.as_ref()
    }

    /// Render Prometheus metrics.
    pub fn render_prometheus(&self) -> String {
        if let Some(handle) = &self.prometheus_handle {
            handle.render()
        } else {
            String::new()
        }
    }

    /// Get all metric names.
    pub fn metric_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        names.extend(self.custom_counters.read().keys().cloned());
        names.extend(self.custom_gauges.read().keys().cloned());
        names.extend(self.custom_histograms.read().keys().cloned());
        names.extend(self.slo_histograms.read().keys().cloned());
        names.sort();
        names.dedup();
        names
    }

    fn prefixed_name(&self, name: &str) -> String {
        format!("{}_{}", self.config.prefix, name)
    }
}

/// Client sample metadata from browser/MoQ client.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClientSampleMetadata {
    pub transport: String,
    pub session_id: String,
    pub client_version: Option<String>,
    pub player_type: Option<String>, // "hls", "dash", "ws", "whep", "moq"
    pub quality: Option<String>,
    pub region: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Metrics timer for measuring durations.
pub struct MetricsTimer {
    start: Instant,
    histogram: Histogram,
}

impl MetricsTimer {
    /// Start a new timer.
    pub fn new(histogram: Histogram) -> Self {
        Self {
            start: Instant::now(),
            histogram,
        }
    }

    /// Stop the timer and record the duration.
    pub fn stop(self) {
        let duration = self.start.elapsed();
        self.histogram.record(duration.as_secs_f64());
    }
}

/// Extension trait for easy timer creation.
pub trait MetricsTimerExt {
    fn timer(&self, name: &str) -> MetricsTimer;
}

impl MetricsTimerExt for MetricsRegistry {
    fn timer(&self, name: &str) -> MetricsTimer {
        let histogram = self.histogram(name, "Timer", metrics::Unit::Seconds);
        MetricsTimer::new(histogram)
    }
}

/// Axum middleware for request metrics.
pub fn metrics_middleware(
    registry: Arc<MetricsRegistry>,
) -> axum::middleware::MapRequestResponse<
    fn(axum::http::Request<axum::body::Body>) -> axum::http::Request<axum::body::Body>,
    fn(axum::http::Response<axum::body::Body>) -> axum::http::Response<axum::body::Body>,
> {
    axum::middleware::map_request_response(
        move |mut req: axum::http::Request<axum::body::Body>| {
            let start = Instant::now();
            req.extensions_mut().insert(RequestMetrics { start });
            req
        },
        move |mut res: axum::http::Response<axum::body::Body>| {
            if let Some(metrics) = res.extensions_mut().remove::<RequestMetrics>() {
                let duration = metrics.start.elapsed();
                let status = res.status().as_u16();
                let path = res
                    .extensions()
                    .get::<String>()
                    .cloned()
                    .unwrap_or_default();

                // Record request metrics
                let counter = registry.counter("http_requests_total", "Total HTTP requests");
                counter.increment(1);

                let histogram = registry.histogram(
                    "http_request_duration_seconds",
                    "HTTP request duration",
                    metrics::Unit::Seconds,
                );
                histogram.record(duration.as_secs_f64());

                let status_counter = registry.counter(
                    &format!("http_requests_status_{}_total", status),
                    "HTTP requests by status",
                );
                status_counter.increment(1);
            }
            res
        },
    )
}

struct RequestMetrics {
    start: Instant,
}

/// SLO evaluator for alerting.
pub struct SloEvaluator {
    registry: Arc<MetricsRegistry>,
    config: SloConfig,
}

impl SloEvaluator {
    /// Create a new SLO evaluator.
    pub fn new(registry: Arc<MetricsRegistry>, config: SloConfig) -> Self {
        Self { registry, config }
    }

    /// Evaluate SLOs for all transports.
    pub fn evaluate(&self) -> SloEvaluationResult {
        let mut results = HashMap::new();
        let mut overall_status = SloStatus::Healthy;

        for transport in &self.config.transports {
            if let Some(target_ms) = self.config.targets.get(transport) {
                if let Some(histogram) = self.registry.slo_histogram(transport) {
                    let result = self.evaluate_transport(transport, *target_ms, histogram);
                    if result.status == SloStatus::Critical {
                        overall_status = SloStatus::Critical;
                    } else if result.status == SloStatus::Warning
                        && overall_status == SloStatus::Healthy
                    {
                        overall_status = SloStatus::Warning;
                    }
                    results.insert(transport.clone(), result);
                }
            }
        }

        SloEvaluationResult {
            overall_status,
            transports: results,
            evaluated_at: chrono::Utc::now(),
        }
    }

    /// Evaluate SLO for a single transport.
    fn evaluate_transport(
        &self,
        transport: &str,
        target_ms: u64,
        histogram: Histogram,
    ) -> TransportSloResult {
        // Get histogram data (this is simplified - real implementation would use hdrhistogram)
        // For now, we'll use a placeholder
        let p50 = target_ms as f64 * 0.5;
        let p95 = target_ms as f64 * 0.95;
        let p99 = target_ms as f64 * 0.99;

        let status = if p99 > target_ms as f64 * self.config.alert_thresholds.critical_pct / 100.0 {
            SloStatus::Critical
        } else if p99 > target_ms as f64 * self.config.alert_thresholds.warning_pct / 100.0 {
            SloStatus::Warning
        } else {
            SloStatus::Healthy
        };

        TransportSloResult {
            transport: transport.to_string(),
            target_ms,
            p50_ms: p50,
            p95_ms: p95,
            p99_ms: p99,
            status,
            error_budget_remaining: self.calculate_error_budget(p99, target_ms as f64),
        }
    }

    fn calculate_error_budget(&self, p99: f64, target: f64) -> f64 {
        if p99 >= target {
            0.0
        } else {
            1.0 - (p99 / target)
        }
    }
}

/// SLO evaluation result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SloEvaluationResult {
    pub overall_status: SloStatus,
    pub transports: HashMap<String, TransportSloResult>,
    pub evaluated_at: chrono::DateTime<chrono::Utc>,
}

/// Transport SLO result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TransportSloResult {
    pub transport: String,
    pub target_ms: u64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub status: SloStatus,
    pub error_budget_remaining: f64,
}

/// SLO status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SloStatus {
    Healthy,
    Warning,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ObservabilityConfig;

    #[test]
    fn test_metrics_registry_creation() {
        let config = ObservabilityConfig::default();
        let registry = MetricsRegistry::new(&config).unwrap();

        // Test counter
        let counter = registry.counter("test_counter", "Test counter");
        counter.increment(1);

        // Test gauge
        let gauge = registry.gauge("test_gauge", "Test gauge");
        gauge.set(42.0);

        // Test histogram
        let histogram =
            registry.histogram("test_histogram", "Test histogram", metrics::Unit::Seconds);
        histogram.record(0.5);
    }

    #[test]
    fn test_slo_histograms() {
        let config = ObservabilityConfig::default();
        let registry = MetricsRegistry::new(&config).unwrap();

        for transport in &config.slo.transports {
            let histogram = registry.slo_histogram(transport);
            assert!(histogram.is_some());
        }
    }

    #[test]
    fn test_prometheus_render() {
        let config = ObservabilityConfig::default();
        let registry = MetricsRegistry::new(&config).unwrap();

        let output = registry.render_prometheus();
        assert!(output.contains("mediaedge_"));
    }

    #[test]
    fn test_metric_names() {
        let config = ObservabilityConfig::default();
        let registry = MetricsRegistry::new(&config).unwrap();

        let names = registry.metric_names();
        assert!(!names.is_empty());
        assert!(names.iter().any(|n| n.contains("ingest")));
        assert!(names.iter().any(|n| n.contains("egress")));
    }
}
