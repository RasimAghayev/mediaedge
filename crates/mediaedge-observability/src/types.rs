//! Observability configuration types.

use serde::{Deserialize, Serialize};
use validator::Validate;

/// Observability configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct ObservabilityConfig {
    /// Service name for telemetry.
    #[validate(length(min = 1))]
    pub service_name: String,

    /// Environment (dev, staging, prod).
    #[serde(default = "default_environment")]
    #[validate(length(min = 1))]
    pub environment: String,

    /// Tracing configuration.
    #[validate]
    pub tracing: TracingConfig,

    /// Metrics configuration.
    #[validate]
    pub metrics: MetricsConfig,

    /// Logging configuration.
    #[validate]
    pub logging: LoggingConfig,

    /// SLO configuration.
    #[validate]
    pub slo: SloConfig,

    /// Fanout configuration (dual export).
    #[validate]
    pub fanout: FanoutConfig,

    /// AI configuration (anomaly detection, etc.)
    #[validate]
    pub ai: AiConfig,
}

fn default_environment() -> String {
    "development".to_string()
}

/// Tracing configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct TracingConfig {
    /// Enable tracing.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Sampling ratio (0.0 - 1.0).
    #[serde(default = "default_sampling_ratio")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub sampling_ratio: f64,

    /// OpenTelemetry configuration.
    #[validate]
    pub otel: OtelConfig,

    /// Jaeger configuration (legacy).
    pub jaeger: Option<JaegerConfig>,

    /// Propagation formats.
    #[serde(default = "default_propagation")]
    pub propagation: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_sampling_ratio() -> f64 {
    0.1
}

fn default_propagation() -> Vec<String> {
    vec!["tracecontext".to_string(), "baggage".to_string()]
}

/// OpenTelemetry configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct OtelConfig {
    /// Enable OTLP export.
    #[serde(default)]
    pub enabled: bool,

    /// OTLP endpoint (gRPC).
    #[validate(url)]
    pub endpoint: Option<String>,

    /// Export protocol: grpc, http.
    #[serde(default = "default_otel_protocol")]
    pub protocol: String,

    /// Compression: gzip, none.
    #[serde(default = "default_otel_compression")]
    pub compression: String,

    /// Headers for OTLP endpoint.
    pub headers: std::collections::HashMap<String, String>,

    /// Resource attributes.
    pub resource_attributes: std::collections::HashMap<String, String>,

    /// Batch export configuration.
    #[validate]
    pub batch: BatchConfig,
}

fn default_otel_protocol() -> String {
    "grpc".to_string()
}

fn default_otel_compression() -> String {
    "gzip".to_string()
}

/// Batch export configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct BatchConfig {
    /// Max queue size.
    #[serde(default = "default_batch_queue_size")]
    #[validate(range(min = 1))]
    pub max_queue_size: usize,

    /// Batch timeout in milliseconds.
    #[serde(default = "default_batch_timeout")]
    #[validate(range(min = 1))]
    pub scheduled_delay_millis: u64,

    /// Max export batch size.
    #[serde(default = "default_batch_export_size")]
    #[validate(range(min = 1))]
    pub max_export_batch_size: usize,
}

fn default_batch_queue_size() -> usize {
    2048
}

fn default_batch_timeout() -> u64 {
    5000
}

fn default_batch_export_size() -> usize {
    512
}

/// Jaeger configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct JaegerConfig {
    /// Enable Jaeger export.
    #[serde(default)]
    pub enabled: bool,

    /// Agent host.
    #[serde(default = "default_jaeger_host")]
    pub agent_host: String,

    /// Agent port.
    #[serde(default = "default_jaeger_port")]
    #[validate(range(min = 1, max = 65535))]
    pub agent_port: u16,

    /// Sampler type: const, probabilistic, rate_limiting, remote.
    #[serde(default = "default_sampler_type")]
    pub sampler_type: String,

    /// Sampler param.
    #[serde(default = "default_sampler_param")]
    #[validate(range(min = 0.0))]
    pub sampler_param: f64,
}

fn default_jaeger_host() -> String {
    "localhost".to_string()
}

fn default_jaeger_port() -> u16 {
    6831
}

fn default_sampler_type() -> String {
    "probabilistic".to_string()
}

fn default_sampler_param() -> f64 {
    0.1
}

/// Metrics configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct MetricsConfig {
    /// Enable Prometheus metrics.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Metrics endpoint path.
    #[serde(default = "default_metrics_path")]
    pub path: String,

    /// Separate metrics port (optional).
    pub port: Option<u16>,

    /// Enable process metrics (CPU, memory, etc.).
    #[serde(default = "default_true")]
    pub process_metrics: bool,

    /// Custom metrics prefix.
    #[serde(default = "default_metrics_prefix")]
    pub prefix: String,

    /// Histogram buckets for latency metrics.
    pub histogram_buckets: Option<Vec<f64>>,

    /// Default histogram buckets (if not specified).
    #[serde(default = "default_histogram_buckets")]
    pub default_buckets: Vec<f64>,
}

fn default_metrics_path() -> String {
    "/metrics".to_string()
}

fn default_metrics_prefix() -> String {
    "mediaedge".to_string()
}

fn default_histogram_buckets() -> Vec<f64> {
    vec![
        0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0,
    ]
}

/// Logging configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct LoggingConfig {
    /// Log level: trace, debug, info, warn, error.
    #[serde(default = "default_log_level")]
    #[validate(length(min = 1))]
    pub level: String,

    /// Output format: json, pretty, compact.
    #[serde(default = "default_log_format")]
    #[validate(length(min = 1))]
    pub format: String,

    /// Enable live log tail via SSE.
    #[serde(default = "default_true")]
    pub live_tail: bool,

    /// Live tail buffer size (lines).
    #[serde(default = "default_tail_buffer")]
    #[validate(range(min = 100))]
    pub tail_buffer_size: usize,

    /// Include span context in logs.
    #[serde(default = "default_true")]
    pub include_span_context: bool,

    /// Log targets filter (comma-separated).
    pub targets: Option<String>,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "json".to_string()
}

fn default_tail_buffer() -> usize {
    10000
}

/// SLO configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct SloConfig {
    /// Enable SLO tracking.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Transport types to track.
    #[serde(default = "default_transports")]
    pub transports: Vec<String>,

    /// SLO targets per transport (p99 latency in ms).
    pub targets: std::collections::HashMap<String, u64>,

    /// Client sample endpoint path.
    #[serde(default = "default_client_sample_path")]
    pub client_sample_path: String,

    /// Client sample max payload size.
    #[serde(default = "default_client_sample_size")]
    #[validate(range(min = 1024))]
    pub client_sample_max_size: usize,

    /// SLO evaluation window in seconds.
    #[serde(default = "default_slo_window")]
    #[validate(range(min = 60))]
    pub evaluation_window_secs: u64,

    /// Alert thresholds (percentage of SLO target).
    #[serde(default = "default_alert_thresholds")]
    pub alert_thresholds: AlertThresholds,
}

fn default_transports() -> Vec<String> {
    vec!["hls".to_string(), "dash".to_string(), "ws".to_string(), "whep".to_string(), "moq".to_string()]
}

fn default_client_sample_path() -> String {
    "/api/v1/slo/client-sample".to_string()
}

fn default_client_sample_size() -> usize {
    65536 // 64 KB
}

fn default_slo_window() -> u64 {
    300 // 5 minutes
}

fn default_alert_thresholds() -> AlertThresholds {
    AlertThresholds::default()
}

/// Alert thresholds for SLO violations.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct AlertThresholds {
    /// Warning threshold (% of SLO target).
    #[serde(default = "default_warning_threshold")]
    #[validate(range(min = 0.0, max = 100.0))]
    pub warning_pct: f64,

    /// Critical threshold (% of SLO target).
    #[serde(default = "default_critical_threshold")]
    #[validate(range(min = 0.0, max = 100.0))]
    pub critical_pct: f64,

    /// Burn rate threshold (multiplier).
    #[serde(default = "default_burn_rate")]
    #[validate(range(min = 1.0))]
    pub burn_rate: f64,
}

fn default_warning_threshold() -> f64 {
    50.0
}

fn default_critical_threshold() -> f64 {
    90.0
}

fn default_burn_rate() -> f64 {
    2.0
}

/// Fanout configuration (dual export to Prometheus + OTLP).
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct FanoutConfig {
    /// Enable fanout.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Export to Prometheus.
    #[serde(default = "default_true")]
    pub prometheus: bool,

    /// Export to OTLP.
    #[serde(default)]
    pub otlp: bool,

    /// Export to Jaeger.
    #[serde(default)]
    pub jaeger: bool,

    /// Custom exporters.
    pub custom: Vec<String>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            service_name: "mediaedge".to_string(),
            environment: "development".to_string(),
            tracing: TracingConfig::default(),
            metrics: MetricsConfig::default(),
            logging: LoggingConfig::default(),
            slo: SloConfig::default(),
            fanout: FanoutConfig::default(),
        }
    }
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sampling_ratio: 0.1,
            otel: OtelConfig::default(),
            jaeger: None,
            propagation: vec!["tracecontext".to_string(), "baggage".to_string()],
        }
    }
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            protocol: "grpc".to_string(),
            compression: "gzip".to_string(),
            headers: std::collections::HashMap::new(),
            resource_attributes: std::collections::HashMap::new(),
            batch: BatchConfig::default(),
        }
    }
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 2048,
            scheduled_delay_millis: 5000,
            max_export_batch_size: 512,
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: "/metrics".to_string(),
            port: None,
            process_metrics: true,
            prefix: "mediaedge".to_string(),
            histogram_buckets: None,
            default_buckets: vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0,
            ],
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "json".to_string(),
            live_tail: true,
            tail_buffer_size: 10000,
            include_span_context: true,
            targets: None,
        }
    }
}

impl Default for SloConfig {
    fn default() -> Self {
        let mut targets = std::collections::HashMap::new();
        targets.insert("hls".to_string(), 2000);   // 2s
        targets.insert("dash".to_string(), 2000);  // 2s
        targets.insert("ws".to_string(), 500);     // 500ms
        targets.insert("whep".to_string(), 500);   // 500ms
        targets.insert("moq".to_string(), 100);    // 100ms

        Self {
            enabled: true,
            transports: vec!["hls".to_string(), "dash".to_string(), "ws".to_string(), "whep".to_string(), "moq".to_string()],
            targets,
            client_sample_path: "/api/v1/slo/client-sample".to_string(),
            client_sample_max_size: 65536,
            evaluation_window_secs: 300,
            alert_thresholds: AlertThresholds::default(),
        }
    }
}

impl Default for FanoutConfig {
        fn default() -> Self {
            Self {
                enabled: true,
                prometheus: true,
                otlp: false,
                jaeger: false,
                custom: Vec::new(),
            }
        }
    }
}

/// AI configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct AiConfig {
    /// Anomaly detection configuration.
    #[validate]
    pub anomaly_detection: AnomalyDetectionConfig,
}

/// Anomaly detection configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct AnomalyDetectionConfig {
    /// Enable anomaly detection.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Detection window in seconds.
    #[serde(default = "default_detection_window")]
    #[validate(range(min = 60))]
    pub detection_window_seconds: u64,

    /// Minimum data points for detection.
    #[serde(default = "default_min_data_points")]
    #[validate(range(min = 10))]
    pub min_data_points: usize,

    /// Detection sensitivity.
    #[serde(default)]
    pub sensitivity: String, // "low", "medium", "high", "custom"

    /// Model configurations.
    pub models: Vec<AnomalyModelConfig>,

    /// Alerting configuration.
    #[validate]
    pub alerting: AlertingConfig,
}

/// Anomaly model configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct AnomalyModelConfig {
    /// Model type: statistical, isolation_forest, lstm_forecast, prophet, custom.
    #[serde(default = "default_model_type")]
    pub model_type: String,

    /// Metric patterns to apply this model to.
    #[serde(default)]
    pub metric_patterns: Vec<String>,

    /// Threshold multiplier for statistical models.
    #[serde(default = "default_threshold_multiplier")]
    #[validate(range(min = 1.0))]
    pub threshold_multiplier: f64,

    /// Minimum anomaly score.
    #[serde(default = "default_min_anomaly_score")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub min_anomaly_score: f64,

    /// Training window in hours.
    #[serde(default = "default_training_window_hours")]
    #[validate(range(min = 1))]
    pub training_window_hours: u32,

    /// Retrain interval in hours.
    #[serde(default = "default_retrain_interval_hours")]
    #[validate(range(min = 1))]
    pub retrain_interval_hours: u32,
}

fn default_model_type() -> String {
    "statistical".to_string()
}

fn default_threshold_multiplier() -> f64 {
    3.0
}

fn default_min_anomaly_score() -> f64 {
    0.5
}

fn default_training_window_hours() -> u32 {
    24
}

fn default_retrain_interval_hours() -> u32 {
    24
}

fn default_detection_window() -> u64 {
    3600 // 1 hour
}

fn default_min_data_points() -> usize {
    30
}

/// Alerting configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct AlertingConfig {
    /// Enable alerting.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Alert channels.
    #[serde(default)]
    pub channels: Vec<AlertChannelConfig>,

    /// Cooldown in seconds between alerts.
    #[serde(default = "default_cooldown")]
    #[validate(range(min = 1))]
    pub cooldown_seconds: u64,

    /// Escalation configuration.
    #[validate]
    pub escalation: EscalationConfig,
}

/// Alert channel configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct AlertChannelConfig {
    /// Channel type: slack, pagerduty, opsgenie, webhook, email, sms.
    pub channel_type: String,

    /// Channel-specific configuration.
    #[serde(default)]
    pub config: std::collections::HashMap<String, String>,

    /// Severity filter.
    #[serde(default)]
    pub severity_filter: Vec<String>, // "info", "warning", "critical", "emergency"
}

/// Escalation configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct EscalationConfig {
    /// Enable escalation.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Escalation levels.
    #[serde(default)]
    pub levels: Vec<EscalationLevelConfig>,
}

/// Escalation level configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Validate, Default)]
pub struct EscalationLevelConfig {
    /// Seconds after first alert to escalate.
    #[validate(range(min = 1))]
    pub after_seconds: u64,

    /// Channels to use for escalation.
    #[serde(default)]
    pub channels: Vec<String>,

    /// Message template.
    #[serde(default)]
    pub message_template: String,
}
    }
}