//! AI Anomaly Detection
//!
//! Stream health anomalies (bitrate drops, error spikes).
//! Predictive failure (disk, GPU, network).
//! < 1min MTTR target.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Anomaly detection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetectionConfig {
    pub enabled: bool,
    pub detection_window_seconds: u64,
    pub min_data_points: usize,
    pub sensitivity: AnomalySensitivity,
    pub models: Vec<AnomalyModelConfig>,
    pub alerting: AlertingConfig,
}

/// Anomaly sensitivity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnomalySensitivity {
    Low,
    Medium,
    High,
    Custom,
}

/// Anomaly model configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyModelConfig {
    pub model_type: AnomalyModelType,
    pub metric_patterns: Vec<String>, // Regex patterns for metric names
    pub threshold_multiplier: f64,
    pub min_anomaly_score: f64,
    pub training_window_hours: u32,
    pub retrain_interval_hours: u32,
}

/// Anomaly model types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyModelType {
    Statistical,      // Z-score, IQR
    IsolationForest,  // Unsupervised
    LstmForecast,     // Time series forecasting
    Prophet,          // Facebook Prophet
    Custom,           // Custom model endpoint
}

/// Alerting configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingConfig {
    pub enabled: bool,
    pub channels: Vec<AlertChannel>,
    pub cooldown_seconds: u64,
    pub escalation: EscalationConfig,
}

/// Alert channels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertChannel {
    pub channel_type: AlertChannelType,
    pub config: HashMap<String, String>,
    pub severity_filter: Vec<AnomalySeverity>,
}

/// Alert channel types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertChannelType {
    Slack,
    PagerDuty,
    Opsgenie,
    Webhook,
    Email,
    Sms,
}

/// Escalation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationConfig {
    pub enabled: bool,
    pub levels: Vec<EscalationLevel>,
}

/// Escalation level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationLevel {
    pub after_seconds: u64,
    pub channels: Vec<AlertChannelType>,
    pub message_template: String,
}

/// Anomaly severity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AnomalySeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// Anomaly event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub metric_name: String,
    pub metric_labels: HashMap<String, String>,
    pub model_type: AnomalyModelType,
    pub anomaly_type: AnomalyType,
    pub severity: AnomalySeverity,
    pub score: f64,
    pub expected_value: f64,
    pub actual_value: f64,
    pub deviation: f64,
    pub context: AnomalyContext,
    pub resolved: bool,
    pub resolved_at: Option<DateTime<Utc>>,
}

/// Anomaly types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyType {
    Spike,           // Sudden increase
    Drop,            // Sudden decrease
    TrendChange,     // Change in trend
    SeasonalityBreak,// Break in seasonal pattern
    Outlier,         // Statistical outlier
    ForecastError,   // Prediction vs actual
    CorrelationBreak,// Correlation breakdown
}

/// Anomaly context for root cause analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyContext {
    pub related_metrics: Vec<RelatedMetric>,
    pub probable_causes: Vec<ProbableCause>,
    pub affected_services: Vec<String>,
    pub recommended_actions: Vec<String>,
    pub runbook_url: Option<String>,
}

/// Related metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedMetric {
    pub metric_name: String,
    pub correlation: f64,
    pub current_value: f64,
    pub anomaly_detected: bool,
}

/// Probable cause.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbableCause {
    pub cause: String,
    pub confidence: f64,
    pub evidence: Vec<String>,
}

/// Metric time series data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesData {
    pub metric_name: String,
    pub labels: HashMap<String, String>,
    pub points: VecDeque<DataPoint>,
    pub max_points: usize,
}

/// Data point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

/// Anomaly detector.
pub struct AnomalyDetector {
    config: AnomalyDetectionConfig,
    time_series: Arc<DashMap<String, TimeSeriesData>>, // metric_name+labels_hash -> data
    models: Arc<DashMap<String, Box<dyn AnomalyModel + Send + Sync>>>, // model_id -> model
    active_anomalies: Arc<DashMap<Uuid, AnomalyEvent>>,
    alert_state: Arc<DashMap<String, AlertState>>, // alert_key -> state
}

/// Alert state for cooldown tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertState {
    pub last_alert_at: DateTime<Utc>,
    pub alert_count: u32,
    pub escalation_level: usize,
}

impl AnomalyDetector {
    /// Create a new anomaly detector.
    pub fn new(config: AnomalyDetectionConfig) -> Arc<Self> {
        let detector = Arc::new(Self {
            config,
            time_series: Arc::new(DashMap::new()),
            models: Arc::new(DashMap::new()),
            active_anomalies: Arc::new(DashMap::new()),
            alert_state: Arc::new(DashMap::new()),
        });

        // Initialize default models
        detector.init_default_models();

        detector
    }

    fn init_default_models(&self) {
        // Statistical model (Z-score based)
        self.models.insert(
            "statistical_default".to_string(),
            Box::new(StatisticalModel::new(AnomalyModelConfig {
                model_type: AnomalyModelType::Statistical,
                metric_patterns: vec![".*".to_string()],
                threshold_multiplier: 3.0,
                min_anomaly_score: 0.5,
                training_window_hours: 24,
                retrain_interval_hours: 24,
            }))
        );
    }

    /// Register a custom anomaly model.
    pub fn register_model(&self, model_id: String, model: Box<dyn AnomalyModel + Send + Sync>) {
        self.models.insert(model_id, model);
    }

    /// Ingest a metric data point.
    pub fn ingest_metric(
        &self,
        metric_name: String,
        labels: HashMap<String, String>,
        value: f64,
        timestamp: DateTime<Utc>,
    ) {
        let key = self.series_key(&metric_name, &labels);

        let mut series = self.time_series.entry(key.clone()).or_insert_with(|| TimeSeriesData {
            metric_name: metric_name.clone(),
            labels: labels.clone(),
            points: VecDeque::new(),
            max_points: (self.config.detection_window_seconds / 10) as usize, // ~10s resolution
        });

        series.points.push_back(DataPoint { timestamp, value });

        // Trim old points
        let cutoff = timestamp - ChronoDuration::seconds(self.config.detection_window_seconds as i64);
        while series.points.front().map(|p| p.timestamp < cutoff).unwrap_or(false) {
            series.points.pop_front();
        }

        // Run detection if enough data points
        if series.points.len() >= self.config.min_data_points {
            self.detect_anomalies(&key, &series);
        }
    }

    /// Detect anomalies in a time series.
    fn detect_anomalies(&self, key: &str, series: &TimeSeriesData) {
        for model in self.models.iter() {
            if self.model_matches(series, &model) {
                if let Ok(anomalies) = model.detect(series) {
                    for anomaly in anomalies {
                        self.handle_anomaly(anomaly);
                    }
                }
            }
        }
    }

    fn model_matches(&self, series: &TimeSeriesData, model: &Box<dyn AnomalyModel>) -> bool {
        // Check if model's metric patterns match this series
        // Simplified: all models match all series for now
        true
    }

    /// Handle detected anomaly.
    fn handle_anomaly(&self, mut anomaly: AnomalyEvent) {
        anomaly.id = Uuid::new_v4();
        anomaly.timestamp = Utc::now();

        // Check if similar anomaly already active
        let duplicate = self.active_anomalies.iter().any(|a| {
            a.metric_name == anomaly.metric_name
                && a.metric_labels == anomaly.metric_labels
                && a.anomaly_type == anomaly.anomaly_type
                && !a.resolved
                && (Utc::now() - a.timestamp) < ChronoDuration::minutes(5)
        });

        if duplicate {
            return; // Suppress duplicate
        }

        // Enrich with context
        anomaly.context = self.enrich_context(&anomaly);

        // Store active anomaly
        self.active_anomalies.insert(anomaly.id, anomaly.clone());

        // Trigger alerting
        if self.config.alerting.enabled {
            self.trigger_alert(&anomaly);
        }

        tracing::warn!(
            anomaly_id = %anomaly.id,
            metric = %anomaly.metric_name,
            type = ?anomaly.anomaly_type,
            severity = ?anomaly.severity,
            score = anomaly.score,
            "Anomaly detected"
        );
    }

    /// Enrich anomaly with context for root cause analysis.
    fn enrich_context(&self, anomaly: &AnomalyEvent) -> AnomalyContext {
        let mut related = Vec::new();
        let mut causes = Vec::new();

        // Check related metrics (correlation-based)
        for entry in self.time_series.iter() {
            let series = entry.value();
            if series.metric_name != anomaly.metric_name && series.points.len() > 10 {
                let correlation = self.calculate_correlation(anomaly, series);
                if correlation.abs() > 0.7 {
                    let current_value = series.points.back().map(|p| p.value).unwrap_or(0.0);
                    related.push(RelatedMetric {
                        metric_name: series.metric_name.clone(),
                        correlation,
                        current_value,
                        anomaly_detected: self.active_anomalies.iter().any(|a| {
                            a.metric_name == series.metric_name && !a.resolved
                        }),
                    });
                }
            }
        }

        // Generate probable causes based on anomaly type and metric
        causes = self.generate_probable_causes(anomaly);

        AnomalyContext {
            related_metrics: related,
            probable_causes: causes,
            affected_services: self.identify_affected_services(anomaly),
            recommended_actions: self.generate_recommended_actions(anomaly),
            runbook_url: self.get_runbook_url(anomaly),
        }
    }

    fn calculate_correlation(&self, anomaly: &AnomalyEvent, series: &TimeSeriesData) -> f64 {
        // Simplified correlation calculation
        // In production, use proper statistical correlation
        0.0
    }

    fn generate_probable_causes(&self, anomaly: &AnomalyEvent) -> Vec<ProbableCause> {
        let mut causes = Vec::new();

        match anomaly.anomaly_type {
            AnomalyType::Drop => {
                if anomaly.metric_name.contains("bitrate") {
                    causes.push(ProbableCause {
                        cause: "Encoder failure or network issue".to_string(),
                        confidence: 0.8,
                        evidence: vec!["Bitrate dropped significantly".to_string()],
                    });
                } else if anomaly.metric_name.contains("viewers") {
                    causes.push(ProbableCause {
                        cause: "CDN outage or stream failure".to_string(),
                        confidence: 0.7,
                        evidence: vec!["Concurrent viewers dropped".to_string()],
                    });
                }
            }
            AnomalyType::Spike => {
                if anomaly.metric_name.contains("error") || anomaly.metric_name.contains("latency") {
                    causes.push(ProbableCause {
                        cause: "Downstream service degradation".to_string(),
                        confidence: 0.75,
                        evidence: vec!["Error rate or latency spiked".to_string()],
                    });
                }
            }
            AnomalyType::ForecastError => {
                causes.push(ProbableCause {
                    cause: "Unusual traffic pattern or scheduled event".to_string(),
                    confidence: 0.6,
                    evidence: vec!["Actual vs forecast deviation".to_string()],
                });
            }
            _ => {}
        }

        // Add generic causes
        causes.push(ProbableCause {
            cause: "Infrastructure issue (CPU, memory, disk, network)".to_string(),
            confidence: 0.5,
            evidence: vec!["System resource pressure".to_string()],
        });

        causes
    }

    fn identify_affected_services(&self, anomaly: &AnomalyEvent) -> Vec<String> {
        let mut services = Vec::new();

        if anomaly.metric_name.contains("ingest") {
            services.push("ingest".to_string());
        }
        if anomaly.metric_name.contains("egress") || anomaly.metric_name.contains("hls") || anomaly.metric_name.contains("dash") {
            services.push("egress".to_string());
        }
        if anomaly.metric_name.contains("transcode") {
            services.push("transcode".to_string());
        }
        if anomaly.metric_name.contains("storage") || anomaly.metric_name.contains("disk") {
            services.push("storage".to_string());
        }
        if anomaly.metric_name.contains("cluster") || anomaly.metric_name.contains("node") {
            services.push("cluster".to_string());
        }

        if services.is_empty() {
            services.push("unknown".to_string());
        }

        services
    }

    fn generate_recommended_actions(&self, anomaly: &AnomalyEvent) -> Vec<String> {
        let mut actions = Vec::new();

        match anomaly.severity {
            AnomalySeverity::Critical | AnomalySeverity::Emergency => {
                actions.push("Page on-call engineer immediately".to_string());
                actions.push("Check service health dashboards".to_string());
            }
            AnomalySeverity::Warning => {
                actions.push("Investigate within 15 minutes".to_string());
                actions.push("Check related metrics".to_string());
            }
            AnomalySeverity::Info => {
                actions.push("Monitor for escalation".to_string());
            }
        }

        match anomaly.anomaly_type {
            AnomalyType::Drop => {
                actions.push("Verify stream source health".to_string());
                actions.push("Check encoder status".to_string());
            }
            AnomalyType::Spike => {
                actions.push("Check error logs".to_string());
                actions.push("Verify downstream capacity".to_string());
            }
            _ => {}
        }

        actions
    }

    fn get_runbook_url(&self, anomaly: &AnomalyEvent) -> Option<String> {
        // Would map to actual runbook URLs
        Some(format!("https://runbooks.mediaedge.io/anomalies/{}", anomaly.anomaly_type as u8))
    }

    /// Trigger alert for anomaly.
    fn trigger_alert(&self, anomaly: &AnomalyEvent) {
        let alert_key = format!("{}:{}", anomaly.metric_name, anomaly.anomaly_type as u8);
        let now = Utc::now();

        let mut state = self.alert_state.entry(alert_key.clone()).or_insert(AlertState {
            last_alert_at: DateTime::from_timestamp(0, 0).unwrap(),
            alert_count: 0,
            escalation_level: 0,
        });

        // Check cooldown
        if (now - state.last_alert_at).num_seconds() < self.config.alerting.cooldown_seconds as i64 {
            return;
        }

        // Check severity filter
        let should_alert = self.config.alerting.channels.iter().any(|c| {
            c.severity_filter.contains(&anomaly.severity)
        });

        if !should_alert {
            return;
        }

        state.last_alert_at = now;
        state.alert_count += 1;

        // Send alerts (in production, would call actual alerting services)
        for channel in &self.config.alerting.channels {
            if channel.severity_filter.contains(&anomaly.severity) {
                self.send_alert(channel, anomaly, &state);
            }
        }

        // Check escalation
        if self.config.alerting.escalation.enabled {
            for level in &self.config.alerting.escalation.levels {
                if state.alert_count as u64 * self.config.alerting.cooldown_seconds >= level.after_seconds {
                    if state.escalation_level < self.config.alerting.escalation.levels.iter().position(|l| l.after_seconds == level.after_seconds).unwrap_or(0) {
                        state.escalation_level += 1;
                        for ch in &level.channels {
                            // Send escalation alert
                        }
                    }
                }
            }
        }
    }

    fn send_alert(&self, channel: &AlertChannel, anomaly: &AnomalyEvent, state: &AlertState) {
        // In production, implement actual alert sending
        tracing::info!(
            channel = ?channel.channel_type,
            anomaly_id = %anomaly.id,
            severity = ?anomaly.severity,
            "Sending alert"
        );
    }

    /// Resolve an anomaly.
    pub fn resolve_anomaly(&self, anomaly_id: Uuid) {
        if let Some(mut anomaly) = self.active_anomalies.get_mut(&anomaly_id) {
            anomaly.resolved = true;
            anomaly.resolved_at = Some(Utc::now());
        }
    }

    /// Get active anomalies.
    pub fn get_active_anomalies(&self) -> Vec<AnomalyEvent> {
        self.active_anomalies.iter()
            .filter(|a| !a.resolved)
            .map(|a| a.clone())
            .collect()
    }

    /// Get anomaly history.
    pub fn get_anomaly_history(&self, since: DateTime<Utc>, limit: usize) -> Vec<AnomalyEvent> {
        self.active_anomalies.iter()
            .filter(|a| a.timestamp >= since)
            .map(|a| a.clone())
            .collect::<Vec<_>>()
            .into_iter()
            .take(limit)
            .collect()
    }

    fn series_key(&self, metric_name: &str, labels: &HashMap<String, String>) -> String {
        let mut label_pairs: Vec<_> = labels.iter().collect();
        label_pairs.sort_by_key(|(k, _)| *k);
        let label_str = label_pairs.iter()
            .map(|(k, v)| format!("{}=\"{}\"", k, v))
            .collect::<Vec<_>>()
            .join(",");
        format!("{}{{{}}}", metric_name, label_str)
    }
}

/// Anomaly model trait.
pub trait AnomalyModel: Send + Sync {
    fn detect(&self, series: &TimeSeriesData) -> Result<Vec<AnomalyEvent>>;
    fn train(&mut self, series: &TimeSeriesData) -> Result<()>;
    fn model_type(&self) -> AnomalyModelType;
}

/// Statistical anomaly model (Z-score based).
pub struct StatisticalModel {
    config: AnomalyModelConfig,
    mean: Option<f64>,
    std_dev: Option<f64>,
}

impl StatisticalModel {
    pub fn new(config: AnomalyModelConfig) -> Self {
        Self {
            config,
            mean: None,
            std_dev: None,
        }
    }
}

impl AnomalyModel for StatisticalModel {
    fn detect(&self, series: &TimeSeriesData) -> Result<Vec<AnomalyEvent>> {
        let mut anomalies = Vec::new();

        if series.points.len() < self.config.min_anomaly_score as usize {
            return Ok(anomalies);
        }

        let values: Vec<f64> = series.points.iter().map(|p| p.value).collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return Ok(anomalies);
        }

        // Check last few points for anomalies
        let recent_points = series.points.iter().rev().take(5);
        for point in recent_points {
            let z_score = (point.value - mean).abs() / std_dev;
            if z_score > self.config.threshold_multiplier {
                let anomaly_type = if point.value > mean {
                    AnomalyType::Spike
                } else {
                    AnomalyType::Drop
                };

                let severity = match z_score {
                    z if z > 5.0 => AnomalySeverity::Emergency,
                    z if z > 4.0 => AnomalySeverity::Critical,
                    z if z > 3.0 => AnomalySeverity::Warning,
                    _ => AnomalySeverity::Info,
                };

                anomalies.push(AnomalyEvent {
                    id: Uuid::new_v4(),
                    timestamp: point.timestamp,
                    metric_name: series.metric_name.clone(),
                    metric_labels: series.labels.clone(),
                    model_type: AnomalyModelType::Statistical,
                    anomaly_type,
                    severity,
                    score: z_score,
                    expected_value: mean,
                    actual_value: point.value,
                    deviation: point.value - mean,
                    context: AnomalyContext::default(),
                    resolved: false,
                    resolved_at: None,
                });
            }
        }

        Ok(anomalies)
    }

    fn train(&mut self, series: &TimeSeriesData) -> Result<()> {
        let values: Vec<f64> = series.points.iter().map(|p| p.value).collect();
        if values.is_empty() {
            return Ok(());
        }

        self.mean = Some(values.iter().sum::<f64>() / values.len() as f64);
        let variance = values.iter()
            .map(|v| (v - self.mean.unwrap()).powi(2))
            .sum::<f64>() / values.len() as f64;
        self.std_dev = Some(variance.sqrt());

        Ok(())
    }

    fn model_type(&self) -> AnomalyModelType {
        AnomalyModelType::Statistical
    }
}

impl Default for AnomalyContext {
    fn default() -> Self {
        Self {
            related_metrics: Vec::new(),
            probable_causes: Vec::new(),
            affected_services: Vec::new(),
            recommended_actions: Vec::new(),
            runbook_url: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statistical_model() {
        let config = AnomalyModelConfig {
            model_type: AnomalyModelType::Statistical,
            metric_patterns: vec!["test.*".to_string()],
            threshold_multiplier: 2.0,
            min_anomaly_score: 0.5,
            training_window_hours: 24,
            retrain_interval_hours: 24,
        };

        let mut model = StatisticalModel::new(config);

        let series = TimeSeriesData {
            metric_name: "test.metric".to_string(),
            labels: HashMap::new(),
            points: (0..100).map(|i| DataPoint {
                timestamp: Utc::now() - ChronoDuration::seconds(100 - i as i64),
                value: 100.0 + (i as f64 * 0.1),
            }).collect(),
            max_points: 1000,
        };

        model.train(&series).unwrap();
        assert!(model.mean.is_some());
        assert!(model.std_dev.is_some());
    }

    #[test]
    fn test_anomaly_severity_ordering() {
        assert!(AnomalySeverity::Info < AnomalySeverity::Warning);
        assert!(AnomalySeverity::Warning < AnomalySeverity::Critical);
        assert!(AnomalySeverity::Critical < AnomalySeverity::Emergency);
    }
}