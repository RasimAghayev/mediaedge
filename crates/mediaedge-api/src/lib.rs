//! HTTP routes, OpenAPI schema, and request/response types for MediaEdge.
//!
//! Defines the Axum route tree for the S3-compatible API and MediaEdge
//! extensions. Called by `services/gateway` (see
//! `services/gateway/src/main.rs`).

#![warn(missing_docs)]

pub mod routes;
pub mod types;
pub mod openapi;
