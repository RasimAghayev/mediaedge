//! Fanout layer for dual export to Prometheus and OTLP.

use std::collections::HashMap;
use std::sync::Arc;

use metrics::{Counter, Gauge, Histogram, Key, Metadata, Recorder, SetRecorderError, Unit};
use metrics_exporter_prometheus::PrometheusHandle;
use opentelemetry::{
    KeyValue, global,
    metrics::{self as otel_metrics, Meter, MeterProvider as _},
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::metrics::{self as otel_sdk_metrics, SdkMeterProvider};
use parking_lot::RwLock;
use tracing::{debug, info, warn};

use crate::types::FanoutConfig;

/// Fanout recorder that exports to multiple backends.
pub struct FanoutRecorder {
    prometheus_handle: Option<PrometheusHandle>,
    otel_meter_provider: Option<SdkMeterProvider>,
    otel_meter: Option<Arc<dyn Meter + Send + Sync>>,
    enabled_prometheus: bool,
    enabled_otlp: bool,
    // Local caches for metrics
    counters: Arc<RwLock<HashMap<String, Counter>>>,
    gauges: Arc<RwLock<HashMap<String, Gauge>>>,
    histograms: Arc<RwLock<HashMap<String, Histogram>>>,
}

impl FanoutRecorder {
    /// Create a new fanout recorder.
    pub fn new(
        prometheus_handle: Option<PrometheusHandle>,
        enabled_prometheus: bool,
        enabled_otlp: bool,
    ) -> Self {
        let recorder = Self {
            prometheus_handle,
            otel_meter_provider: None,
            otel_meter: None,
            enabled_prometheus,
            enabled_otlp,
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
        };

        // Initialize OTLP if enabled
        if enabled_otlp {
            if let Err(e) = recorder.init_otlp() {
                warn!("Failed to initialize OTLP exporter: {}", e);
            }
        }

        recorder
    }

    /// Initialize OTLP metrics exporter.
    fn init_otlp(&self) -> anyhow::Result<()> {
        // Get endpoint from environment or config
        let endpoint = std::env::var("OTEL_EXPORTER_OTLP_METRICS_ENDPOINT")
            .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT"))
            .unwrap_or_else(|_| "http://localhost:4317".to_string());

        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(endpoint)
            .with_timeout(std::time::Duration::from_secs(10))
            .build_metrics_exporter()?;

        let reader = otel_sdk_metrics::PeriodicReader::builder(exporter)
            .with_interval(std::time::Duration::from_secs(60))
            .build();

        let provider = SdkMeterProvider::builder().with_reader(reader).build();

        let meter = provider.meter("mediaedge");

        // Store in self (would need interior mutability)
        // For now, we'll use global provider
        global::set_meter_provider(provider.clone());

        Ok(())
    }

    /// Register a counter.
    fn register_counter(&self, key: &Key, metadata: &Metadata) -> Counter {
        let name = key.name();
        let mut counters = self.counters.write();
        counters
            .entry(name.to_string())
            .or_insert_with(|| {
                if self.enabled_otlp {
                    // Also register with OTel
                    if let Some(meter) = &self.otel_meter {
                        let _ = meter
                            .u64_counter(name)
                            .with_description(metadata.description().unwrap_or(""))
                            .with_unit(metadata.unit().map(|u| u.as_str()).unwrap_or(""))
                            .init();
                    }
                }
                metrics::counter!(name, metadata.description().unwrap_or(""))
            })
            .clone()
    }

    /// Register a gauge.
    fn register_gauge(&self, key: &Key, metadata: &Metadata) -> Gauge {
        let name = key.name();
        let mut gauges = self.gauges.write();
        gauges
            .entry(name.to_string())
            .or_insert_with(|| {
                if self.enabled_otlp {
                    if let Some(meter) = &self.otel_meter {
                        let _ = meter
                            .f64_gauge(name)
                            .with_description(metadata.description().unwrap_or(""))
                            .with_unit(metadata.unit().map(|u| u.as_str()).unwrap_or(""))
                            .init();
                    }
                }
                metrics::gauge!(name, metadata.description().unwrap_or(""))
            })
            .clone()
    }

    /// Register a histogram.
    fn register_histogram(&self, key: &Key, metadata: &Metadata) -> Histogram {
        let name = key.name();
        let mut histograms = self.histograms.write();
        histograms
            .entry(name.to_string())
            .or_insert_with(|| {
                if self.enabled_otlp {
                    if let Some(meter) = &self.otel_meter {
                        let _ = meter
                            .f64_histogram(name)
                            .with_description(metadata.description().unwrap_or(""))
                            .with_unit(metadata.unit().map(|u| u.as_str()).unwrap_or(""))
                            .init();
                    }
                }
                metrics::histogram!(name, metadata.description().unwrap_or(""))
            })
            .clone()
    }
}

impl Recorder for FanoutRecorder {
    fn register_counter(
        &self,
        key: Key,
        metadata: Metadata,
        _description: &'static str,
    ) -> Counter {
        self.register_counter(&key, &metadata)
    }

    fn register_gauge(&self, key: Key, metadata: Metadata, _description: &'static str) -> Gauge {
        self.register_gauge(&key, &metadata)
    }

    fn register_histogram(
        &self,
        key: Key,
        metadata: Metadata,
        _description: &'static str,
    ) -> Histogram {
        self.register_histogram(&key, &metadata)
    }

    fn increment_counter(&self, key: Key, value: u64) {
        let name = key.name();

        // Prometheus
        if self.enabled_prometheus {
            if let Some(counter) = self.counters.read().get(name) {
                counter.increment(value);
            } else {
                // Fallback to global
                metrics::counter!(name).increment(value);
            }
        }

        // OTLP
        if self.enabled_otlp {
            global::get_meter_provider()
                .meter("mediaedge")
                .u64_counter(name)
                .init()
                .add(value, &[]);
        }
    }

    fn decrement_gauge(&self, key: Key, value: f64) {
        let name = key.name();

        if self.enabled_prometheus {
            if let Some(gauge) = self.gauges.read().get(name) {
                gauge.decrement(value);
            }
        }

        if self.enabled_otlp {
            global::get_meter_provider()
                .meter("mediaedge")
                .f64_gauge(name)
                .init()
                .record(-value, &[]);
        }
    }

    fn increment_gauge(&self, key: Key, value: f64) {
        let name = key.name();

        if self.enabled_prometheus {
            if let Some(gauge) = self.gauges.read().get(name) {
                gauge.increment(value);
            }
        }

        if self.enabled_otlp {
            global::get_meter_provider()
                .meter("mediaedge")
                .f64_gauge(name)
                .init()
                .record(value, &[]);
        }
    }

    fn set_gauge(&self, key: Key, value: f64) {
        let name = key.name();

        if self.enabled_prometheus {
            if let Some(gauge) = self.gauges.read().get(name) {
                gauge.set(value);
            }
        }

        if self.enabled_otlp {
            global::get_meter_provider()
                .meter("mediaedge")
                .f64_gauge(name)
                .init()
                .record(value, &[]);
        }
    }

    fn record_histogram(&self, key: Key, value: f64) {
        let name = key.name();

        if self.enabled_prometheus {
            if let Some(histogram) = self.histograms.read().get(name) {
                histogram.record(value);
            }
        }

        if self.enabled_otlp {
            global::get_meter_provider()
                .meter("mediaedge")
                .f64_histogram(name)
                .init()
                .record(value, &[]);
        }
    }
}

/// Fanout layer for tracing (dual export).
pub struct FanoutLayer {
    tracer: opentelemetry::trace::Tracer,
}

impl FanoutLayer {
    /// Create a new fanout layer.
    pub fn new(tracer: opentelemetry::trace::Tracer) -> Self {
        Self { tracer }
    }
}

impl<S> tracing_subscriber::Layer<S> for FanoutLayer
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // Create OpenTelemetry span
        let span_name = attrs.metadata().name();
        let otel_span = self.tracer.start(span_name.to_string());

        // Store OTel span in tracing span extensions
        if let Some(span) = ctx.span(id) {
            let mut extensions = span.extensions_mut();
            extensions.insert(otel_span);
        }
    }

    fn on_record(
        &self,
        id: &tracing::Id,
        values: &tracing::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if let Some(span) = ctx.span(id) {
            let mut extensions = span.extensions_mut();
            if let Some(otel_span) = extensions.get_mut::<opentelemetry::trace::Span>() {
                let mut visitor = OtelVisitor(otel_span);
                values.record(&mut visitor);
            }
        }
    }

    fn on_enter(&self, id: &tracing::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            let mut extensions = span.extensions_mut();
            if let Some(otel_span) = extensions.get_mut::<opentelemetry::trace::Span>() {
                otel_span.add_event("enter", vec![]);
            }
        }
    }

    fn on_exit(&self, id: &tracing::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            let mut extensions = span.extensions_mut();
            if let Some(otel_span) = extensions.remove::<opentelemetry::trace::Span>() {
                otel_span.end();
            }
        }
    }
}

/// Visitor for recording attributes to OTel span.
struct OtelVisitor<'a>(&'a mut opentelemetry::trace::Span);

impl tracing::field::Visit for OtelVisitor<'_> {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.0.set_attribute(KeyValue::new(field.name(), value));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.set_attribute(KeyValue::new(field.name(), value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.set_attribute(KeyValue::new(field.name(), value));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.set_attribute(KeyValue::new(field.name(), value));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.set_attribute(KeyValue::new(field.name(), value));
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.0
            .set_attribute(KeyValue::new(field.name(), value.to_string()));
        self.0.record_error(value);
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0
            .set_attribute(KeyValue::new(field.name(), format!("{:?}", value)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metrics_exporter_prometheus::PrometheusBuilder;

    #[test]
    fn test_fanout_recorder_creation() {
        let builder = PrometheusBuilder::new();
        let handle = builder.install_recorder().unwrap();

        let recorder = FanoutRecorder::new(Some(handle), true, false);
        assert!(recorder.enabled_prometheus);
        assert!(!recorder.enabled_otlp);
    }

    #[test]
    fn test_fanout_recorder_both() {
        let builder = PrometheusBuilder::new();
        let handle = builder.install_recorder().unwrap();

        let recorder = FanoutRecorder::new(Some(handle), true, true);
        assert!(recorder.enabled_prometheus);
        assert!(recorder.enabled_otlp);
    }
}
