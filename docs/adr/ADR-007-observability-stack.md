# ADR-007: Observability Stack

## Status
Proposed

## Context
Logs, metrics, traces needed across distributed services. Options: OpenTelemetry
vs custom.

## Decision
OpenTelemetry + `tracing` + `metrics`. Backends: Loki (logs), Prometheus
(metrics), Jaeger (traces), Alertmanager (alerting).

## Consequences
- OTel instrumentation across all services
- W3C tracecontext propagation
