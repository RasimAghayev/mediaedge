//! Structured logging with JSON output and live log tail via SSE.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    Router,
    body::Body,
    extract::State,
    http::StatusCode,
    response::{Sse, sse::Event},
    routing::get,
};
use futures::stream::{self, Stream};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{Level, Subscriber, info};
use tracing_subscriber::{
    layer::{Context, Layer},
    registry::LookupSpan,
};

use crate::types::{LoggingConfig, ObservabilityConfig};

/// Log entry for structured logging and live tail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
    pub span_context: Option<SpanContext>,
    pub fields: serde_json::Value,
}

/// Span context for correlation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
}

/// Log buffer for live tail.
pub struct LogBuffer {
    buffer: RwLock<VecDeque<LogEntry>>,
    max_size: usize,
    sender: broadcast::Sender<LogEntry>,
}

impl LogBuffer {
    /// Create a new log buffer.
    pub fn new(max_size: usize) -> Self {
        let (sender, _) = broadcast::channel(max_size);
        Self {
            buffer: RwLock::new(VecDeque::with_capacity(max_size)),
            max_size,
            sender,
        }
    }

    /// Add a log entry.
    pub fn push(&self, entry: LogEntry) {
        // Add to buffer
        let mut buffer = self.buffer.write();
        if buffer.len() >= self.max_size {
            buffer.pop_front();
        }
        buffer.push_back(entry.clone());

        // Broadcast to subscribers
        let _ = self.sender.send(entry);
    }

    /// Get recent logs.
    pub fn recent(&self, limit: usize) -> Vec<LogEntry> {
        let buffer = self.buffer.read();
        buffer.iter().rev().take(limit).cloned().collect()
    }

    /// Subscribe to live log stream.
    pub fn subscribe(&self) -> BroadcastStream<LogEntry> {
        BroadcastStream::new(self.sender.subscribe())
    }

    /// Get current buffer size.
    pub fn size(&self) -> usize {
        self.buffer.read().len()
    }
}

/// Log layer for capturing structured logs.
pub struct LogLayer {
    buffer: Arc<LogBuffer>,
    include_span_context: bool,
}

impl LogLayer {
    /// Create a new log layer.
    pub fn new(buffer: Arc<LogBuffer>, include_span_context: bool) -> Self {
        Self {
            buffer,
            include_span_context,
        }
    }
}

impl<S> Layer<S> for LogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
        // Extract metadata
        let metadata = event.metadata();
        let level = metadata.level().to_string();
        let target = metadata.target().to_string();

        // Extract fields
        let mut fields = serde_json::Map::new();
        let mut visitor = JsonVisitor(&mut fields);
        event.record(&mut visitor);

        // Extract message
        let message = fields
            .remove("message")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();

        // Extract span context
        let span_context = if self.include_span_context {
            ctx.lookup_current().map(|span| {
                let ext = span.extensions();
                let trace_id = ext
                    .get::<opentelemetry::trace::TraceId>()
                    .map(|id| id.to_string())
                    .unwrap_or_default();
                let span_id = ext
                    .get::<opentelemetry::trace::SpanId>()
                    .map(|id| id.to_string())
                    .unwrap_or_default();
                let parent_span_id = ext
                    .get::<opentelemetry::trace::SpanId>()
                    .map(|id| id.to_string());

                SpanContext {
                    trace_id,
                    span_id,
                    parent_span_id,
                }
            })
        } else {
            None
        };

        let entry = LogEntry {
            timestamp: chrono::Utc::now(),
            level,
            target,
            message,
            span_context,
            fields: serde_json::Value::Object(fields),
        };

        self.buffer.push(entry);
    }
}

/// JSON visitor for extracting structured fields.
struct JsonVisitor<'a>(&'a mut serde_json::Map<String, serde_json::Value>);

impl tracing::field::Visit for JsonVisitor<'_> {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.0.insert(field.name().to_string(), json!(value));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(field.name().to_string(), json!(value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.name().to_string(), json!(value));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(field.name().to_string(), json!(value));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().to_string(), json!(value));
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.0
            .insert(field.name().to_string(), json!(value.to_string()));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), json!(format!("{:?}", value)));
    }
}

/// Initialize logging with live tail buffer.
pub fn init_logging(config: &ObservabilityConfig) -> Arc<LogBuffer> {
    let logging_config = &config.logging;
    let buffer = Arc::new(LogBuffer::new(logging_config.tail_buffer_size));

    // Create log layer
    let log_layer = LogLayer::new(buffer.clone(), logging_config.include_span_context);

    // Build fmt layer for console output
    let fmt_layer = build_fmt_layer(logging_config);

    // Combine with existing subscriber (would be done in main init)
    // This is a placeholder - actual init happens in tracing.rs

    info!(
        "Logging initialized: level={}, format={}, live_tail={}",
        logging_config.level, logging_config.format, logging_config.live_tail
    );

    buffer
}

/// Build fmt layer for console output.
fn build_fmt_layer(
    config: &LoggingConfig,
) -> Box<dyn Layer<tracing_subscriber::Registry> + Send + Sync> {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true);

    let fmt_layer = match config.format.as_str() {
        "json" => fmt_layer.json().flatten_event(true).boxed(),
        "pretty" => fmt_layer.pretty().boxed(),
        "compact" => fmt_layer.compact().boxed(),
        _ => fmt_layer.json().flatten_event(true).boxed(),
    };

    fmt_layer
}

/// Create live tail routes.
pub fn live_tail_routes(buffer: Arc<LogBuffer>, config: &LoggingConfig) -> Router {
    let path = if config.live_tail {
        "/api/v1/logs"
    } else {
        return Router::new(); // Empty router if disabled
    };

    Router::new()
        .route(path, get(live_tail_handler))
        .route(&format!("{}/recent", path), get(recent_logs_handler))
        .with_state(buffer)
}

/// GET /api/v1/logs - Live log tail via SSE.
async fn live_tail_handler(
    State(buffer): State<Arc<LogBuffer>>,
    axum::extract::Query(params): axum::extract::Query<LiveTailParams>,
) -> Sse<impl Stream<Item = Result<Event, std::io::Error>>> {
    let level_filter = params.level.map(|l| l.to_uppercase());
    let target_filter = params.target;
    let since = params.since;

    let stream = buffer.subscribe().filter_map(move |entry| {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => return None,
        };

        // Apply filters
        if let Some(ref level) = level_filter {
            if entry.level != *level {
                return None;
            }
        }
        if let Some(ref target) = target_filter {
            if !entry.target.contains(target) {
                return None;
            }
        }
        if let Some(since) = since {
            if entry.timestamp <= since {
                return None;
            }
        }

        // Format as SSE event
        let data = serde_json::to_string(&entry).unwrap_or_default();
        Some(Ok(Event::default().data(data)))
    });

    Sse::new(stream)
}

/// GET /api/v1/logs/recent - Get recent logs.
async fn recent_logs_handler(
    State(buffer): State<Arc<LogBuffer>>,
    axum::extract::Query(params): axum::extract::Query<RecentLogsParams>,
) -> Result<axum::Json<Vec<LogEntry>>, StatusCode> {
    let limit = params.limit.unwrap_or(100).min(10000);
    let logs = buffer.recent(limit);
    Ok(axum::Json(logs))
}

/// Query parameters for live tail.
#[derive(Debug, Deserialize)]
pub struct LiveTailParams {
    /// Filter by log level (trace, debug, info, warn, error).
    pub level: Option<String>,
    /// Filter by target (substring match).
    pub target: Option<String>,
    /// Only show logs after this timestamp.
    pub since: Option<chrono::DateTime<chrono::Utc>>,
}

/// Query parameters for recent logs.
#[derive(Debug, Deserialize)]
pub struct RecentLogsParams {
    /// Maximum number of logs to return (default 100, max 10000).
    pub limit: Option<usize>,
}

/// Log query for filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogQuery {
    pub level: Option<String>,
    pub target: Option<String>,
    pub since: Option<chrono::DateTime<chrono::Utc>>,
    pub until: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: Option<usize>,
    pub search: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_buffer() {
        let buffer = LogBuffer::new(100);

        let entry = LogEntry {
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
        assert_eq!(recent[0].message, "test message");
    }

    #[test]
    fn test_log_buffer_overflow() {
        let buffer = LogBuffer::new(3);

        for i in 0..5 {
            let entry = LogEntry {
                timestamp: chrono::Utc::now(),
                level: "info".to_string(),
                target: "test".to_string(),
                message: format!("message {}", i),
                span_context: None,
                fields: json!({}),
            };
            buffer.push(entry);
        }

        assert_eq!(buffer.size(), 3);
        let recent = buffer.recent(10);
        assert_eq!(recent.len(), 3);
        // Should have last 3 messages
        assert_eq!(recent[0].message, "message 4");
        assert_eq!(recent[1].message, "message 3");
        assert_eq!(recent[2].message, "message 2");
    }

    #[test]
    fn test_log_entry_serialization() {
        let entry = LogEntry {
            timestamp: chrono::Utc::now(),
            level: "info".to_string(),
            target: "test".to_string(),
            message: "test message".to_string(),
            span_context: Some(SpanContext {
                trace_id: "trace123".to_string(),
                span_id: "span456".to_string(),
                parent_span_id: Some("parent789".to_string()),
            }),
            fields: json!({"key": "value"}),
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("trace123"));
        assert!(json.contains("span456"));
        assert!(json.contains("parent789"));
        assert!(json.contains("key"));
    }
}
