//! Tracing, metrics, logging, and health checks.
//!
//! Wraps `tracing`, OpenTelemetry, and Prometheus metrics. Called by all
//! services and crates (see `services/gateway/src/main.rs` for init).

#![warn(missing_docs)]

pub mod tracing;
pub mod metrics;
pub mod logging;
pub mod health;
