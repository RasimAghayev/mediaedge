//! Integration tests for mediaedge-observability.

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, HeaderValue, Request, StatusCode},
    routing::get,
};
use futures::StreamExt;
use mediaedge_observability::{
    LogBuffer, MetricsRegistry, ObservabilityConfig, SloTracker, init_logging, init_observability,
    live_tail_routes,
    prelude::*,
    slo_routes,
    types::{LoggingConfig, MetricsConfig, SloConfig, TracingConfig},
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tower::ServiceExt;

fn create_test_obs_config() -> ObservabilityConfig {
    let mut config = ObservabilityConfig::default();
    config.service_name = "test-service".to_string();
    config.environment = "test".to_string();
    config.tracing.enabled = true;
    config.tracing.sampling_ratio = 1.0;
    config.tracing.otel.enabled = false;
    config.tracing.jaeger = None;
    config.metrics.enabled = true;
    config.logging.live_tail = true;
    config.logging.tail_buffer_size = 100;
    config.slo.enabled = true;
    config.slo.transports = vec!["hls".to_string(), "moq".to_string()];
    config.slo.targets.insert("hls".to_string(), 2000);
    config.slo.targets.insert("moq".to_string(), 100);
    config.fanout.prometheus = true;
    config.fanout.otlp = false;
    config
}

#[tokio::test]
async fn test_observability_init() {
    let config = create_test_obs_config();
    let handles = init_observability(&config).unwrap();

    // Verify components are initialized
    assert!(handles.log_buffer.size() == 0);
    assert!(!handles.metrics_registry.metric_names().is_empty());

    let slo_report = handles.slo_tracker.get_status();
    assert!(slo_report.transports.contains_key("hls"));
    assert!(slo_report.transports.contains_key("moq"));
}

#[tokio::test]
async fn test_metrics_registry() {
    let config = create_test_obs_config();
    let registry = MetricsRegistry::new(&config).unwrap();

    // Test counter
    let counter = registry.counter("test_counter", "Test counter");
    counter.increment(5);

    // Test gauge
    let gauge = registry.gauge("test_gauge", "Test gauge");
    gauge.set(42.0);
    gauge.increment(1.0);
    assert_eq!(gauge.get(), 43.0);

    // Test histogram
    let histogram = registry.histogram("test_histogram", "Test histogram", metrics::Unit::Seconds);
    histogram.record(0.5);
    histogram.record(1.0);

    // Test SLO histograms
    let hls_hist = registry.slo_histogram("hls");
    assert!(hls_hist.is_some());

    let moq_hist = registry.slo_histogram("moq");
    assert!(moq_hist.is_some());

    // Test Prometheus render
    let output = registry.render_prometheus();
    assert!(output.contains("mediaedge_test_counter"));
    assert!(output.contains("mediaedge_test_gauge"));
}

#[tokio::test]
async fn test_metrics_timer() {
    let config = create_test_obs_config();
    let registry = MetricsRegistry::new(&config).unwrap();

    let timer = registry.timer("test_timer");
    std::thread::sleep(Duration::from_millis(10));
    timer.stop();

    // Verify histogram recorded
    let output = registry.render_prometheus();
    assert!(output.contains("mediaedge_test_timer"));
}

#[tokio::test]
async fn test_slo_tracker() {
    let config = SloConfig {
        enabled: true,
        transports: vec!["hls".to_string(), "moq".to_string()],
        targets: {
            let mut t = std::collections::HashMap::new();
            t.insert("hls".to_string(), 2000);
            t.insert("moq".to_string(), 100);
            t
        },
        client_sample_path: "/api/v1/slo/client-sample".to_string(),
        client_sample_max_size: 65536,
        evaluation_window_secs: 300,
        alert_thresholds: Default::default(),
    };

    let tracker = SloTracker::new(config);

    // Record healthy latencies
    for _ in 0..100 {
        tracker.record_latency("hls", Duration::from_millis(500));
        tracker.record_latency("moq", Duration::from_millis(50));
    }

    let report = tracker.get_status();
    assert_eq!(report.overall_status, super::SloStatus::Healthy);
    assert_eq!(report.transports["hls"].status, super::SloStatus::Healthy);
    assert_eq!(report.transports["moq"].status, super::SloStatus::Healthy);
}

#[tokio::test]
async fn test_slo_violation() {
    let config = SloConfig {
        enabled: true,
        transports: vec!["hls".to_string()],
        targets: {
            let mut t = std::collections::HashMap::new();
            t.insert("hls".to_string(), 100);
            t
        },
        client_sample_path: "/api/v1/slo/client-sample".to_string(),
        client_sample_max_size: 65536,
        evaluation_window_secs: 300,
        alert_thresholds: {
            let mut a = mediaedge_observability::types::AlertThresholds::default();
            a.warning_pct = 50.0;
            a.critical_pct = 90.0;
            a
        },
    };

    let tracker = SloTracker::new(config);

    // Record violating latencies
    for _ in 0..100 {
        tracker.record_latency("hls", Duration::from_millis(200));
    }

    let report = tracker.get_status();
    assert_eq!(report.overall_status, super::SloStatus::Critical);
    assert_eq!(report.transports["hls"].status, super::SloStatus::Critical);
}

#[tokio::test]
async fn test_client_samples() {
    let config = SloConfig {
        enabled: true,
        transports: vec!["whep".to_string()],
        targets: {
            let mut t = std::collections::HashMap::new();
            t.insert("whep".to_string(), 500);
            t
        },
        client_sample_path: "/api/v1/slo/client-sample".to_string(),
        client_sample_max_size: 65536,
        evaluation_window_secs: 300,
        alert_thresholds: Default::default(),
    };

    let tracker = SloTracker::new(config);

    let sample = mediaedge_observability::slo::ClientSample {
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

#[tokio::test]
async fn test_log_buffer() {
    let buffer = LogBuffer::new(100);

    let entry = mediaedge_observability::logging::LogEntry {
        timestamp: chrono::Utc::now(),
        level: "info".to_string(),
        target: "test".to_string(),
        message: "test message".to_string(),
        span_context: None,
        fields: json!({}),
    };

    buffer.push(entry.clone());
    assert_eq!(buffer.size(), 1);

    let recent = buffer.recent(10);
    assert_eq!(recent.len(), 1);
}

#[tokio::test]
async fn test_live_tail_sse() {
    let config = create_test_obs_config();
    let buffer = Arc::new(LogBuffer::new(config.logging.tail_buffer_size));

    let app = live_tail_routes(buffer.clone(), &config.logging);

    // Add some log entries
    for i in 0..5 {
        buffer.push(mediaedge_observability::logging::LogEntry {
            timestamp: chrono::Utc::now(),
            level: "info".to_string(),
            target: "test".to_string(),
            message: format!("message {}", i),
            span_context: None,
            fields: json!({}),
        });
    }

    // Test SSE stream
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/logs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Check content type is SSE
    let content_type = response.headers().get("content-type").unwrap();
    assert!(content_type.to_str().unwrap().contains("text/event-stream"));
}

#[tokio::test]
async fn test_recent_logs_endpoint() {
    let config = create_test_obs_config();
    let buffer = Arc::new(LogBuffer::new(config.logging.tail_buffer_size));

    let app = live_tail_routes(buffer.clone(), &config.logging);

    // Add some log entries
    for i in 0..5 {
        buffer.push(mediaedge_observability::logging::LogEntry {
            timestamp: chrono::Utc::now(),
            level: "info".to_string(),
            target: "test".to_string(),
            message: format!("message {}", i),
            span_context: None,
            fields: json!({}),
        });
    }

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/logs/recent?limit=3")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let logs: Vec<mediaedge_observability::logging::LogEntry> =
        serde_json::from_slice(&body).unwrap();
    assert_eq!(logs.len(), 3);
}

#[tokio::test]
async fn test_slo_endpoints() {
    let config = create_test_obs_config();
    let tracker = Arc::new(SloTracker::new(config.slo.clone()));

    // Record some latencies
    for _ in 0..50 {
        tracker.record_latency("hls", Duration::from_millis(500));
    }

    let app = slo_routes(tracker.clone());

    // Test /api/v1/slo/status
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/slo/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let report: mediaedge_observability::slo::SloStatusReport =
        serde_json::from_slice(&body).unwrap();
    assert!(report.transports.contains_key("hls"));
    assert!(report.transports.contains_key("moq"));

    // Test /api/v1/slo/status/hls
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/slo/status/hls")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let status: mediaedge_observability::slo::TransportSloStatus =
        serde_json::from_slice(&body).unwrap();
    assert_eq!(status.transport, "hls");
}

#[tokio::test]
async fn test_client_sample_endpoint() {
    let config = create_test_obs_config();
    let tracker = Arc::new(SloTracker::new(config.slo.clone()));

    let app = slo_routes(tracker.clone());

    let sample = json!({
        "transport": "hls",
        "latency": { "secs": 0, "nanos": 50000000 }, // 50ms
        "session_id": "session123",
        "client_version": "1.0",
        "player_type": "hls",
        "quality": "1080p",
        "region": "us-east",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/slo/client-sample")
                .header("content-type", "application/json")
                .body(Body::from(sample.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_prometheus_metrics_endpoint() {
    let config = create_test_obs_config();
    let handles = init_observability(&config).unwrap();

    let app = Router::new()
        .route(
            &config.metrics.path,
            get(mediaedge_observability::metrics_handler),
        )
        .with_state(handles.metrics_registry.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&config.metrics.path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let output = String::from_utf8(body.to_vec()).unwrap();
    assert!(output.contains("mediaedge_"));
}

#[tokio::test]
async fn test_log_filtering() {
    let buffer = Arc::new(LogBuffer::new(100));

    // Add entries with different levels
    buffer.push(mediaedge_observability::logging::LogEntry {
        timestamp: chrono::Utc::now(),
        level: "info".to_string(),
        target: "test".to_string(),
        message: "info message".to_string(),
        span_context: None,
        fields: json!({}),
    });
    buffer.push(mediaedge_observability::logging::LogEntry {
        timestamp: chrono::Utc::now(),
        level: "error".to_string(),
        target: "test".to_string(),
        message: "error message".to_string(),
        span_context: None,
        fields: json!({}),
    });
    buffer.push(mediaedge_observability::logging::LogEntry {
        timestamp: chrono::Utc::now(),
        level: "debug".to_string(),
        target: "other".to_string(),
        message: "debug message".to_string(),
        span_context: None,
        fields: json!({}),
    });

    let app = live_tail_routes(buffer.clone(), &LoggingConfig::default());

    // Test level filter - should only get error
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/logs?level=error")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_metrics_middleware() {
    let config = create_test_obs_config();
    let handles = init_observability(&config).unwrap();

    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .layer(axum::middleware::from_fn_with_state(
            handles.metrics_registry.clone(),
            mediaedge_observability::metrics_middleware,
        ))
        .with_state(handles.metrics_registry.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Check metrics were recorded
    let output = handles.metrics_registry.render_prometheus();
    assert!(output.contains("mediaedge_http_requests_total"));
    assert!(output.contains("mediaedge_http_request_duration_seconds"));
}
