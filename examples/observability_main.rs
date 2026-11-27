//! Example service demonstrating observability integration.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    middleware,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use tokio::signal;
use tracing::{info, instrument, Level};
use tracing_subscriber::FmtSubscriber;

use mediaedge_observability::{
    init_observability, ObservabilityConfig, ObservabilityHandles,
    MetricsRegistry, SloTracker, MetricsTimerExt,
    prelude::*,
};
use mediaedge_config::ConfigLoader;

#[derive(Clone)]
struct AppState {
    observability: ObservabilityHandles,
    config_loader: Arc<ConfigLoader<mediaedge_config::types::GatewayConfig>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration
    let config_path = std::env::var("CONFIG_FILE")
        .unwrap_or_else(|_| "config/gateway.toml".to_string());
    let config_loader = Arc::new(ConfigLoader::<mediaedge_config::types::GatewayConfig>::from_file(&config_path)?);

    // Build observability config from service config
    let service_config = config_loader.config();
    let obs_config = build_observability_config(&service_config);

    // Initialize observability stack
    let observability = init_observability(&obs_config)?;

    info!("Starting Gateway with observability");

    let app_state = AppState {
        observability,
        config_loader,
    };

    // Build router with observability middleware
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/ingest/:stream", post(ingest_stream))
        .route("/api/v1/stream/:stream/segment", get(get_segment))
        .merge(app_state.observability.routes(&obs_config))
        .layer(middleware::from_fn_with_state(
            app_state.observability.metrics_registry.clone(),
            metrics_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            app_state.observability.metrics_registry.clone(),
            trace_context_middleware,
        ))
        .with_state(app_state);

    // Start server
    let addr = format!("{}:{}", service_config.server.host, service_config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Gateway listening on {}", addr);

    // Graceful shutdown
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal());

    if let Err(e) = server.await {
        tracing::error!("Server error: {}", e);
    }

    info!("Gateway stopped");
    Ok(())
}

/// Build observability config from service config.
fn build_observability_config(service_config: &mediaedge_config::types::GatewayConfig) -> ObservabilityConfig {
    ObservabilityConfig {
        service_name: "gateway".to_string(),
        environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
        tracing: service_config.tracing.clone().into(),
        metrics: service_config.metrics.clone().into(),
        logging: mediaedge_observability::types::LoggingConfig {
            level: service_config.tracing.level.clone(),
            format: service_config.tracing.format.clone(),
            live_tail: true,
            tail_buffer_size: 10000,
            include_span_context: true,
            targets: None,
        },
        slo: mediaedge_observability::types::SloConfig::default(),
        fanout: mediaedge_observability::types::FanoutConfig::default(),
    }
}

/// Health check endpoint.
async fn health_check(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "gateway",
        "version": state.config_loader.meta().version,
    }))
}

/// Ingest stream endpoint with metrics.
#[instrument(skip(state))]
async fn ingest_stream(
    State(state): State<AppState>,
    Path(stream): Path<String>,
    body: Body,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let timer = state.observability.metrics_registry.timer("ingest_request_duration_seconds");
    
    // Simulate ingest processing
    let active_streams = state.observability.metrics_registry.gauge("ingest_active_streams", "Active ingest streams");
    active_streams.increment(1.0);

    // Process stream...
    tokio::time::sleep(Duration::from_millis(10)).await;

    active_streams.decrement(1.0);
    timer.stop();

    // Record SLO latency
    state.observability.slo_tracker.record_latency("hls", Duration::from_millis(50));

    Ok(Json(json!({
        "status": "ok",
        "stream": stream,
    })))
}

/// Get segment endpoint with SLO tracking.
#[instrument(skip(state))]
async fn get_segment(
    State(state): State<AppState>,
    Path(stream): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let timer = state.observability.metrics_registry.timer("egress_request_duration_seconds");
    
    let active_viewers = state.observability.metrics_registry.gauge("egress_active_viewers", "Active egress viewers");
    active_viewers.increment(1.0);

    // Simulate segment delivery
    tokio::time::sleep(Duration::from_millis(5)).await;

    active_viewers.decrement(1.0);
    timer.stop();

    // Record SLO latency for HLS
    state.observability.slo_tracker.record_latency("hls", Duration::from_millis(20));

    Ok(Json(json!({
        "status": "ok",
        "stream": stream,
        "segment": "segment_001.ts",
    })))
}

/// Shutdown signal handler.
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C"),
        _ = terminate => info!("Received SIGTERM"),
    }
}

// Trait implementations for config conversion
impl From<mediaedge_config::types::TracingConfig> for mediaedge_observability::types::TracingConfig {
    fn from(config: mediaedge_config::types::TracingConfig) -> Self {
        Self {
            enabled: config.enabled,
            sampling_ratio: 0.1,
            otel: mediaedge_observability::types::OtelConfig::default(),
            jaeger: config.jaeger.map(|j| mediaedge_observability::types::JaegerConfig {
                enabled: j.enabled,
                agent_host: j.agent_host,
                agent_port: j.agent_port,
                sampler_type: j.sampler_type,
                sampler_param: j.sampler_param,
            }),
            propagation: config.propagation.unwrap_or_else(|| vec!["tracecontext".to_string(), "baggage".to_string()]),
        }
    }
}

impl From<mediaedge_config::types::MetricsConfig> for mediaedge_observability::types::MetricsConfig {
    fn from(config: mediaedge_config::types::MetricsConfig) -> Self {
        Self {
            enabled: config.enabled,
            path: config.path,
            port: config.port,
            process_metrics: true,
            prefix: "mediaedge".to_string(),
            histogram_buckets: None,
            default_buckets: vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0,
            ],
        }
    }
}