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
pub mod types;
pub mod tracing;
pub mod metrics;
pub mod logging;
pub mod slo;
pub mod fanout;

// Re-export main types
pub use types::{
    ObservabilityConfig, TracingConfig, OtelConfig, JaegerConfig, BatchConfig,
    MetricsConfig, LoggingConfig, SloConfig, AlertThresholds, FanoutConfig,
};

pub use tracing::{
    init_tracing, TracingGuard, extract_trace_context, inject_trace_context,
    trace_context_middleware,
};

pub use metrics::{
    MetricsRegistry, MetricsTimer, MetricsTimerExt, ClientSampleMetadata,
    metrics_middleware, SloEvaluator, SloEvaluationResult, TransportSloResult, SloStatus,
};

pub use logging::{
    LogEntry, LogBuffer, LogLayer, init_logging, live_tail_routes,
    LiveTailParams, RecentLogsParams, LogQuery,
};

pub use slo::{
    SloTracker, ClientSample, ClientSampleBatch, SloStatusReport, TransportSloStatus,
    SloStatus, slo_routes, SloEvaluator,
};

pub use fanout::{
    FanoutRecorder, FanoutLayer,
};

pub use ai::{
    AnomalyDetector, AnomalyDetectionConfig, AnomalyEvent, AnomalyType, AnomalySeverity,
    AnomalyModel, AnomalyModelType, StatisticalModel,
};

/// Prelude for convenient imports.
pub mod prelude {
    pub use crate::{
        ObservabilityConfig, MetricsRegistry, SloTracker, LogBuffer,
        init_tracing, init_logging, live_tail_routes, slo_routes,
        MetricsTimerExt, SloEvaluator,
        AnomalyDetector, AnomalyDetectionConfig, AnomalyEvent,
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
    let slo_evaluator = Arc::new(slo::SloEvaluator::new(slo_tracker.clone(), metrics_registry.clone()));

    // Initialize anomaly detector
    let anomaly_detector = Arc::new(ai::AnomalyDetector::new(config.ai.anomaly_detection.clone()));

    info!("Observability stack initialized for service: {}", config.service_name);

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
            .merge(logging::live_tail_routes(self.log_buffer.clone(), &config.logging))
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

use axum::{
    extract::State,
    routing::get,
};

use crate::logging::LogBuffer;
use crate::metrics::MetricsRegistry;
use crate::slo::SloTracker;
use crate::ai::AnomalyDetector;

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