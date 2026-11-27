//! Redirect Manager - HTTP 302 redirect-to-owner for HLS/DASH/RTSP
//!
//! Handles redirecting clients to the node that owns a broadcast stream.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::ownership::OwnershipManager;
use mediaedge_observability::MetricsRegistry;

/// Redirect manager configuration.
#[derive(Debug, Clone)]
pub struct RedirectConfig {
    pub enabled: bool,
    pub redirect_path_prefix: String,
    pub preserve_query_params: bool,
    pub max_redirect_hops: u32,
}

impl Default for RedirectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            redirect_path_prefix: "/".to_string(),
            preserve_query_params: true,
            max_redirect_hops: 3,
        }
    }
}

/// Redirect manager.
pub struct RedirectManager {
    config: RedirectConfig,
    ownership: Arc<RwLock<Option<Arc<OwnershipManager>>>>,
    node_addresses: Arc<RwLock<HashMap<String, String>>>, // node_id -> base_url
    metrics: Arc<MetricsRegistry>,
    router: Router,
}

impl RedirectManager {
    /// Create a new redirect manager.
    pub fn new(metrics: Arc<MetricsRegistry>) -> Self {
        let config = RedirectConfig::default();
        let ownership = Arc::new(RwLock::new(None));
        let node_addresses = Arc::new(RwLock::new(HashMap::new()));

        let router = Self::build_router(
            config.clone(),
            ownership.clone(),
            node_addresses.clone(),
            metrics.clone(),
        );

        Self {
            config,
            ownership,
            node_addresses,
            metrics,
            router,
        }
    }

    /// Build the redirect router.
    fn build_router(
        config: RedirectConfig,
        ownership: Arc<RwLock<Option<Arc<OwnershipManager>>>>,
        node_addresses: Arc<RwLock<HashMap<String, String>>>,
        metrics: Arc<MetricsRegistry>,
    ) -> Router {
        Router::new()
            .route("/redirect/:key", get(Self::handle_redirect))
            .route(
                "/redirect/hls/:stream_id/*path",
                get(Self::handle_hls_redirect),
            )
            .route(
                "/redirect/dash/:stream_id/*path",
                get(Self::handle_dash_redirect),
            )
            .route("/redirect/rtsp/:stream_id", get(Self::handle_rtsp_redirect))
            .with_state(RedirectState {
                config,
                ownership,
                node_addresses,
                metrics,
            })
    }

    /// Get the router for integration.
    pub fn router(&self) -> Router {
        self.router.clone()
    }

    /// Set the ownership manager.
    pub fn set_ownership(&self, ownership: Arc<OwnershipManager>) {
        *self.ownership.write() = Some(ownership);
    }

    /// Register a node's base URL.
    pub fn register_node_address(&self, node_id: String, base_url: String) {
        self.node_addresses.write().insert(node_id, base_url);
    }

    /// Remove a node's address.
    pub fn remove_node_address(&self, node_id: &str) {
        self.node_addresses.write().remove(node_id);
    }

    /// Handle generic redirect.
    async fn handle_redirect(
        State(state): State<RedirectState>,
        Path(key): Path<String>,
        Query(params): Query<HashMap<String, String>>,
        headers: HeaderMap,
    ) -> Response {
        if !state.config.enabled {
            return (StatusCode::NOT_FOUND, "Redirect disabled").into_response();
        }

        // Check redirect hop count
        let hop_count = headers
            .get("x-redirect-hop")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        if hop_count >= state.config.max_redirect_hops {
            warn!("Max redirect hops exceeded for key: {}", key);
            return (StatusCode::TOO_MANY_REQUESTS, "Max redirect hops exceeded").into_response();
        }

        // Get ownership
        let owner = if let Some(ownership) = state.ownership.read().as_ref() {
            ownership.get_owner(&key)
        } else {
            None
        };

        let owner = match owner {
            Some(o) => o,
            None => {
                debug!("No owner found for key: {}", key);
                return (StatusCode::NOT_FOUND, "No owner found").into_response();
            }
        };

        // Get node address
        let target_url = state.node_addresses.read().get(&owner.0).cloned();

        let target_url = match target_url {
            Some(url) => url,
            None => {
                warn!("No address registered for owner: {}", owner.0);
                return (StatusCode::SERVICE_UNAVAILABLE, "Owner address unknown").into_response();
            }
        };

        // Build redirect URL
        let mut redirect_url =
            format!("{}{}{}", target_url, state.config.redirect_path_prefix, key);

        if state.config.preserve_query_params && !params.is_empty() {
            let query = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            redirect_url = format!("{}?{}", redirect_url, query);
        }

        // Add redirect hop header
        let mut response = Redirect::temporary(&redirect_url).into_response();
        response.headers_mut().insert(
            "x-redirect-hop",
            (hop_count + 1).to_string().parse().unwrap(),
        );

        state
            .metrics
            .counter("redirect_total", "Total redirects")
            .increment(1);
        info!(
            "Redirecting {} to {} (hop {})",
            key,
            redirect_url,
            hop_count + 1
        );

        response
    }

    /// Handle HLS redirect (playlist.m3u8, segments).
    async fn handle_hls_redirect(
        State(state): State<RedirectState>,
        Path(stream_id): Path<String>,
        Path(path): Path<String>,
        Query(params): Query<HashMap<String, String>>,
        headers: HeaderMap,
    ) -> Response {
        let key = format!("broadcast:{}", stream_id);
        let new_path = format!("/hls/{}/{}", stream_id, path);

        Self::do_redirect(state, &key, &new_path, params, headers).await
    }

    /// Handle DASH redirect (manifest.mpd, segments).
    async fn handle_dash_redirect(
        State(state): State<RedirectState>,
        Path(stream_id): Path<String>,
        Path(path): Path<String>,
        Query(params): Query<HashMap<String, String>>,
        headers: HeaderMap,
    ) -> Response {
        let key = format!("broadcast:{}", stream_id);
        let new_path = format!("/dash/{}/{}", stream_id, path);

        Self::do_redirect(state, &key, &new_path, params, headers).await
    }

    /// Handle RTSP redirect.
    async fn handle_rtsp_redirect(
        State(state): State<RedirectState>,
        Path(stream_id): Path<String>,
        Query(params): Query<HashMap<String, String>>,
        headers: HeaderMap,
    ) -> Response {
        let key = format!("broadcast:{}", stream_id);
        let new_path = format!("/rtsp/{}", stream_id);

        Self::do_redirect(state, &key, &new_path, params, headers).await
    }

    /// Generic redirect handler.
    async fn do_redirect(
        state: RedirectState,
        key: &str,
        path: &str,
        params: HashMap<String, String>,
        headers: HeaderMap,
    ) -> Response {
        if !state.config.enabled {
            return (StatusCode::NOT_FOUND, "Redirect disabled").into_response();
        }

        let hop_count = headers
            .get("x-redirect-hop")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        if hop_count >= state.config.max_redirect_hops {
            return (StatusCode::TOO_MANY_REQUESTS, "Max redirect hops exceeded").into_response();
        }

        let owner = if let Some(ownership) = state.ownership.read().as_ref() {
            ownership.get_owner(key)
        } else {
            None
        };

        let owner = match owner {
            Some(o) => o,
            None => return (StatusCode::NOT_FOUND, "No owner found").into_response(),
        };

        let target_url = state
            .node_addresses
            .read()
            .get(&owner.0)
            .cloned()
            .unwrap_or_else(|| {
                warn!("No address for owner: {}", owner.0);
                String::new()
            });

        if target_url.is_empty() {
            return (StatusCode::SERVICE_UNAVAILABLE, "Owner address unknown").into_response();
        }

        let mut redirect_url = format!("{}{}", target_url, path);

        if state.config.preserve_query_params && !params.is_empty() {
            let query = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            redirect_url = format!("{}?{}", redirect_url, query);
        }

        let mut response = Redirect::temporary(&redirect_url).into_response();
        response.headers_mut().insert(
            "x-redirect-hop",
            (hop_count + 1).to_string().parse().unwrap(),
        );

        state
            .metrics
            .counter("redirect_total", "Total redirects")
            .increment(1);
        debug!("Redirecting {} to {}", key, redirect_url);

        response
    }
}

/// Shared state for redirect handlers.
#[derive(Clone)]
struct RedirectState {
    config: RedirectConfig,
    ownership: Arc<RwLock<Option<Arc<OwnershipManager>>>>,
    node_addresses: Arc<RwLock<HashMap<String, String>>>,
    metrics: Arc<MetricsRegistry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redirect_config_default() {
        let config = RedirectConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_redirect_hops, 3);
    }
}
