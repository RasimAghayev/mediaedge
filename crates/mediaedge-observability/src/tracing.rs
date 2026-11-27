//! Tracing setup with OpenTelemetry, Jaeger, and OTLP.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use opentelemetry::{
    global, trace::TracerProvider as _, KeyValue,
};
use opentelemetry_jaeger::Uninstall;
use opentelemetry_otlp::{
    WithExportConfig, WithTonicConfig, WithHttpConfig,
};
use opentelemetry_sdk::{
    trace::{self, Sampler, SpanProcessor},
    Resource,
};
use tracing::{info, warn, error, Level};
use tracing_subscriber::{
    layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};
use tracing_subscriber::fmt::format::JsonFields;
use tracing_subscriber::fmt::FormatEvent;
use tracing_subscriber::fmt::FormatFields;
use tracing_subscriber::registry::LookupSpan;

use crate::types::{ObservabilityConfig, TracingConfig, OtelConfig, JaegerConfig, BatchConfig};
use crate::fanout::FanoutLayer;

/// Initialize tracing with OpenTelemetry and fanout.
pub fn init_tracing(config: &ObservabilityConfig) -> anyhow::Result<TracingGuard> {
    let tracing_config = &config.tracing;
    
    if !tracing_config.enabled {
        info!("Tracing disabled");
        return Ok(TracingGuard { _jaeger: None, _otel: None });
    }

    // Build resource
    let resource = build_resource(config);

    // Create tracer provider with fanout
    let (tracer_provider, jaeger_uninstall, otel_shutdown) = build_tracer_provider(
        &tracing_config,
        &resource,
    )?;

    // Set global tracer provider
    global::set_tracer_provider(tracer_provider.clone());

    // Build tracing subscriber with fanout layer
    let subscriber = build_subscriber(tracing_config, tracer_provider, config)?;

    // Initialize subscriber
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Tracing initialized: service={}, sampling={}", 
        config.service_name, tracing_config.sampling_ratio);

    Ok(TracingGuard {
        _jaeger: jaeger_uninstall,
        _otel: otel_shutdown,
    })
}

/// Build OpenTelemetry resource.
fn build_resource(config: &ObservabilityConfig) -> Resource {
    let mut attributes = vec![
        KeyValue::new("service.name", config.service_name.clone()),
        KeyValue::new("service.namespace", "mediaedge"),
        KeyValue::new("deployment.environment", config.environment.clone()),
    ];

    // Add custom resource attributes
    if let Some(otel_config) = config.tracing.otel.resource_attributes.get("custom") {
        // Custom attributes would be merged here
    }

    // Add all resource attributes from config
    for (k, v) in &config.tracing.otel.resource_attributes {
        attributes.push(KeyValue::new(k.clone(), v.clone()));
    }

    Resource::new(attributes)
}

/// Build tracer provider with fanout to multiple exporters.
fn build_tracer_provider(
    config: &TracingConfig,
    resource: &Resource,
) -> anyhow::Result<(opentelemetry_sdk::trace::TracerProvider, Option<Uninstall>, Option<Box<dyn FnOnce() + Send + Sync>>)> {
    let mut exporters: Vec<Box<dyn SpanProcessor + Send + Sync>> = Vec::new();
    let mut jaeger_uninstall = None;
    let mut otel_shutdown: Option<Box<dyn FnOnce() + Send + Sync>> = None;

    // Jaeger exporter (legacy)
    if let Some(jaeger_config) = &config.jaeger {
        if jaeger_config.enabled {
            let (exporter, uninstall) = build_jaeger_exporter(jaeger_config)?;
            let processor = trace::BatchSpanProcessor::builder(exporter)
                .with_batch_config(build_batch_config(&config.otel.batch))
                .build();
            exporters.push(Box::new(processor));
            jaeger_uninstall = Some(uninstall);
        }
    }

    // OTLP exporter
    if config.otel.enabled {
        if let Some(endpoint) = &config.otel.endpoint {
            let (exporter, shutdown) = build_otlp_exporter(&config.otel, endpoint)?;
            let processor = trace::BatchSpanProcessor::builder(exporter)
                .with_batch_config(build_batch_config(&config.otel.batch))
                .build();
            exporters.push(Box::new(processor));
            otel_shutdown = Some(Box::new(shutdown));
        }
    }

    // Build sampler
    let sampler = build_sampler(config);

    // Create tracer provider
    let mut provider_builder = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_resource(resource.clone())
        .with_sampler(sampler);

    for exporter in exporters {
        provider_builder = provider_builder.with_span_processor(exporter);
    }

    let provider = provider_builder.build();

    Ok((provider, jaeger_uninstall, otel_shutdown))
}

/// Build Jaeger exporter.
fn build_jaeger_exporter(
    config: &JaegerConfig,
) -> anyhow::Result<(opentelemetry_jaeger::Exporter, Uninstall)> {
    let agent_endpoint = format!("{}:{}", config.agent_host, config.agent_port);
    
    let exporter = opentelemetry_jaeger::Exporter::builder()
        .with_agent_endpoint(agent_endpoint)
        .with_process(opentelemetry_jaeger::Process::new(
            "mediaedge".to_string(),
            vec![],
        ))
        .init()?;

    Ok((exporter.0, exporter.1))
}

/// Build OTLP exporter.
fn build_otlp_exporter(
    config: &OtelConfig,
    endpoint: &str,
) -> anyhow::Result<(opentelemetry_otlp::SpanExporter, Box<dyn FnOnce() + Send + Sync>)> {
    let mut exporter_builder = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(endpoint.to_string())
        .with_timeout(Duration::from_secs(10));

    // Add headers
    for (k, v) in &config.headers {
        exporter_builder = exporter_builder.with_header(k.clone(), v.clone());
    }

    // Compression
    if config.compression == "gzip" {
        exporter_builder = exporter_builder.with_compression(opentelemetry_otlp::Compression::Gzip);
    }

    let exporter = exporter_builder.build_span_exporter()?;

    // Return shutdown function
    let shutdown = Box::new(move || {
        let _ = exporter.shutdown();
    });

    Ok((exporter, shutdown))
}

/// Build batch config.
fn build_batch_config(config: &BatchConfig) -> trace::BatchConfig {
    trace::BatchConfig::default()
        .with_max_queue_size(config.max_queue_size)
        .with_scheduled_delay(Duration::from_millis(config.scheduled_delay_millis))
        .with_max_export_batch_size(config.max_export_batch_size)
}

/// Build sampler.
fn build_sampler(config: &TracingConfig) -> Sampler {
    match config.sampling_ratio {
        r if r >= 1.0 => Sampler::AlwaysOn,
        r if r <= 0.0 => Sampler::AlwaysOff,
        r => Sampler::TraceIdRatioBased(r),
    }
}

/// Build tracing subscriber with fanout layer.
fn build_subscriber(
    config: &TracingConfig,
    tracer_provider: opentelemetry_sdk::trace::TracerProvider,
    obs_config: &ObservabilityConfig,
) -> anyhow::Result<impl tracing::Subscriber + Send + Sync> {
    // Get tracer
    let tracer = tracer_provider.tracer("mediaedge");

    // Create fanout layer for dual export
    let fanout_layer = FanoutLayer::new(tracer);

    // Build fmt layer
    let fmt_layer = build_fmt_layer(&obs_config.logging)?;

    // Build env filter
    let env_filter = build_env_filter(&obs_config.logging)?;

    // Combine layers
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(fanout_layer)
        .with(fmt_layer);

    Ok(subscriber)
}

/// Build fmt layer for console/file output.
fn build_fmt_layer(
    logging_config: &crate::types::LoggingConfig,
) -> anyhow::Result<Box<dyn Layer<tracing_subscriber::Registry> + Send + Sync>> {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true);

    let fmt_layer = match logging_config.format.as_str() {
        "json" => fmt_layer.json().flatten_event(true).boxed(),
        "pretty" => fmt_layer.pretty().boxed(),
        "compact" => fmt_layer.compact().boxed(),
        _ => fmt_layer.json().flatten_event(true).boxed(),
    };

    Ok(fmt_layer)
}

/// Build env filter.
fn build_env_filter(
    logging_config: &crate::types::LoggingConfig,
) -> anyhow::Result<EnvFilter> {
    let mut filter = EnvFilter::try_new(&logging_config.level)?;

    if let Some(targets) = &logging_config.targets {
        for target in targets.split(',') {
            let target = target.trim();
            if !target.is_empty() {
                filter = filter.add_directive(target.parse()?);
            }
        }
    }

    Ok(filter)
}

/// Tracing guard for cleanup.
pub struct TracingGuard {
    _jaeger: Option<Uninstall>,
    _otel: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl Drop for TracingGuard {
    fn drop(&mut self) {
        // Jaeger uninstall happens automatically via Drop
        // OTLP shutdown
        if let Some(shutdown) = self._otel.take() {
            shutdown();
        }
    }
}

/// Extract trace context from headers.
pub fn extract_trace_context(headers: &axum::http::HeaderMap) -> opentelemetry::Context {
    global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(headers))
    })
}

/// Inject trace context into headers.
pub fn inject_trace_context(headers: &mut axum::http::HeaderMap) {
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&opentelemetry::Context::current(), &mut HeaderInjector(headers))
    })
}

/// Header extractor for OpenTelemetry propagation.
struct HeaderExtractor<'a>(&'a axum::http::HeaderMap);

impl<'a> opentelemetry::propagation::Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

/// Header injector for OpenTelemetry propagation.
struct HeaderInjector<'a>(&'a mut axum::http::HeaderMap);

impl<'a> opentelemetry::propagation::Injector for HeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(header_name) = axum::http::HeaderName::from_bytes(key.as_bytes()) {
            if let Ok(header_value) = axum::http::HeaderValue::from_str(&value) {
                self.0.insert(header_name, header_value);
            }
        }
    }
}

/// Axum middleware for trace context propagation.
pub fn trace_context_middleware() -> axum::middleware::MapRequestResponse<fn(axum::http::Request<axum::body::Body>) -> axum::http::Request<axum::body::Body>, fn(axum::http::Response<axum::body::Body>) -> axum::http::Response<axum::body::Body>> {
    axum::middleware::map_request_response(
        |mut req: axum::http::Request<axum::body::Body>| {
            // Extract trace context from incoming headers
            let context = extract_trace_context(req.headers());
            // Store in request extensions for handlers to access
            req.extensions_mut().insert(context);
            req
        },
        |mut res: axum::http::Response<axum::body::Body>| {
            // Inject trace context into response headers
            inject_trace_context(res.headers_mut());
            res
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ObservabilityConfig;

    #[test]
    fn test_build_resource() {
        let config = ObservabilityConfig::default();
        let resource = build_resource(&config);
        assert!(resource.iter().any(|kv| kv.key == "service.name"));
        assert!(resource.iter().any(|kv| kv.key == "deployment.environment"));
    }

    #[test]
    fn test_build_sampler() {
        let config = TracingConfig {
            enabled: true,
            sampling_ratio: 0.5,
            otel: OtelConfig::default(),
            jaeger: None,
            propagation: vec![],
        };
        let sampler = build_sampler(&config);
        // Can't easily test sampler type, but verify it doesn't panic
    }
}