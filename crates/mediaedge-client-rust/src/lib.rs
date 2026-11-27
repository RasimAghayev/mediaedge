//! MediaEdge Rust Client
//!
//! Async client library for interacting with MediaEdge API.
//! Provides type-safe access to all API endpoints.

#![warn(missing_docs)]

pub mod client;
pub mod error;
pub mod models;
pub mod streams;
pub mod cluster;
pub mod tenants;
pub mod monetization;
pub mod auth;
pub mod metrics;

use client::MediaEdgeClient;
pub use client::MediaEdgeClient;
pub use error::{MediaEdgeError, Result};
pub use models::*;

/// Prelude for convenient imports.
pub mod prelude {
    pub use crate::{
        MediaEdgeClient, MediaEdgeError, Result,
        models::*,
        streams::StreamsApi,
        cluster::ClusterApi,
        tenants::TenantsApi,
        monetization::MonetizationApi,
        auth::AuthApi,
        metrics::MetricsApi,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = MediaEdgeClient::new("http://localhost:8080").unwrap();
        assert_eq!(client.base_url(), "http://localhost:8080");
    }
}