//! MediaEdge Observability Stack
//!
//! Complete observability for MediaEdge services:
//! - OpenTelemetry tracing (Jaeger + OTLP gRPC)
//! - Prometheus metrics with custom histograms
//! - Structured JSON logging with live tail via SSE
//! - SLO tracking with 5 transport buckets (hls, dash, ws, whep, moq)
//! - Client-sample endpoint for browser/MoQ latency
//! - FanoutBuilder: single instrumentation → Prometheus + OTLP simultaneously
//! - AI Anomaly Detection - Phase 5

#![warn(missing_docs)]

pub mod ai;
pub mod fanout;
pub mod logging;
pub mod metrics;
pub mod slo;
pub mod tracing;
pub mod types;

// Re-export main types
pub use types::{
    AlertThresholds, BatchConfig, FanoutConfig, JaegerConfig, LoggingConfig, MetricsConfig,
    ObservabilityConfig, OtelConfig, SloConfig, TracingConfig,
};

pub use tracing::{
    TracingGuard, extract_trace_context, init_tracing, inject_trace_context,
    trace_context_middleware,
};

pub use metrics::{
    ClientSampleMetadata, MetricsRegistry, MetricsTimer, MetricsTimerExt, SloEvaluationResult,
    SloEvaluator, SloStatus, TransportSloResult, metrics_middleware,
};

pub use logging::{
    LiveTailParams, LogBuffer, LogEntry, LogLayer, LogQuery, RecentLogsParams, init_logging,
    live_tail_routes,
};

pub use slo::{
    ClientSample, ClientSampleBatch, SloEvaluator, SloStatus, SloStatusReport, SloTracker,
    TransportSloStatus, slo_routes,
};

pub use fanout::{FanoutLayer, FanoutRecorder};

pub use ai::{
    AnomalyDetectionConfig, AnomalyDetector, AnomalyEvent, AnomalyModel, AnomalyModelType,
    AnomalySeverity, AnomalyType, StatisticalModel,
};

/// Prelude for convenient imports.
pub mod prelude {
    pub use crate::{
        AnomalyDetectionConfig, AnomalyDetector, AnomalyEvent, LogBuffer, MetricsRegistry,
        MetricsTimerExt, ObservabilityConfig, SloEvaluator, SloTracker, init_logging, init_tracing,
        live_tail_routes, slo_routes,
    };
}

/// Initialize complete observability stack.
pub fn init_observability(config: &ObservabilityConfig) -> anyhow::Result<ObservabilityHandles> {
    // Initialize tracing
    let tracing_guard = tracing::init_tracing(config)?;

    // Initialize logging
    let log_buffer = logging::init_logging(config);

    // Initialize metrics
    let metrics_registry = Arc::new(metrics::MetricsRegistry::new(config)?);

    // Initialize SLO tracker
    let slo_tracker = Arc::new(slo::SloTracker::new(config.slo.clone()));

    // Initialize SLO evaluator
    let slo_evaluator = Arc::new(slo::SloEvaluator::new(
        slo_tracker.clone(),
        metrics_registry.clone(),
    ));

    // Initialize anomaly detector
    let anomaly_detector = Arc::new(ai::AnomalyDetector::new(
        config.ai.anomaly_detection.clone(),
    ));

    info!(
        "Observability stack initialized for service: {}",
        config.service_name
    );

    Ok(ObservabilityHandles {
        tracing_guard,
        log_buffer,
        metrics_registry,
        slo_tracker,
        slo_evaluator,
        anomaly_detector,
    })
}

/// Handles for observability components.
pub struct ObservabilityHandles {
    pub tracing_guard: tracing::TracingGuard,
    pub log_buffer: Arc<LogBuffer>,
    pub metrics_registry: Arc<MetricsRegistry>,
    pub slo_tracker: Arc<SloTracker>,
    pub slo_evaluator: Arc<SloEvaluator>,
    pub anomaly_detector: Arc<AnomalyDetector>,
}

impl ObservabilityHandles {
    /// Create all observability routes.
    pub fn routes(&self, config: &ObservabilityConfig) -> axum::Router {
        axum::Router::new()
            // Metrics
            .route(&config.metrics.path, axum::routing::get(metrics_handler))
            // Live log tail
            .merge(logging::live_tail_routes(
                self.log_buffer.clone(),
                &config.logging,
            ))
            // SLO
            .merge(slo::slo_routes(self.slo_tracker.clone()))
    }
}

/// GET /metrics handler.
async fn metrics_handler(
    State(registry): State<Arc<MetricsRegistry>>,
) -> Result<String, axum::http::StatusCode> {
    Ok(registry.render_prometheus())
}

use axum::{extract::State, routing::get};

use crate::ai::AnomalyDetector;
use crate::logging::LogBuffer;
use crate::metrics::MetricsRegistry;
use crate::slo::SloTracker;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ObservabilityConfig;

    #[test]
    fn test_observability_config_default() {
        let config = ObservabilityConfig::default();
        assert_eq!(config.service_name, "mediaedge");
        assert!(config.tracing.enabled);
        assert!(config.metrics.enabled);
        assert!(config.logging.live_tail);
        assert!(config.slo.enabled);
        assert!(config.fanout.enabled);
    }

    #[test]
    fn test_slo_targets() {
        let config = ObservabilityConfig::default();
        assert!(config.slo.targets.contains_key("hls"));
        assert!(config.slo.targets.contains_key("dash"));
        assert!(config.slo.targets.contains_key("ws"));
        assert!(config.slo.targets.contains_key("whep"));
        assert!(config.slo.targets.contains_key("moq"));
    }
}
