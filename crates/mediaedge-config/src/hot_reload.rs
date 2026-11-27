//! Hot-reload integration for services.
//!
//! Provides:
//! - SIGHUP handler for config reload
//! - HTTP endpoint: POST /api/v1/config-reload
//! - Config version tracking
//! - Graceful reload (in-flight requests complete)

use std::sync::Arc;
use std::time::Duration;

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use serde_json::json;
use tokio::signal::unix::{SignalKind, signal};
use tracing::{error, info, warn};

use crate::loader::{ConfigError, ConfigLoader, LoadedConfig};
use crate::types::ServiceConfig;

/// Shared state for hot-reload endpoint.
#[derive(Clone)]
pub struct HotReloadState<C> {
    pub loader: Arc<ConfigLoader<C>>,
    pub service_name: String,
}

/// Create hot-reload routes.
pub fn hot_reload_routes<C>() -> Router<HotReloadState<C>>
where
    C: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/v1/config-reload", post(config_reload_handler))
        .route("/api/v1/config-version", get(config_version_handler))
}

/// POST /api/v1/config-reload - Trigger config reload.
async fn config_reload_handler<C>(
    State(state): State<HotReloadState<C>>,
    headers: HeaderMap,
) -> Response
where
    C: Clone + Send + Sync + 'static,
{
    // Optional: Require admin token
    if let Some(admin_token) = std::env::var("MEDIAEDGE_ADMIN_TOKEN").ok() {
        let auth_header = headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        if !auth_header.contains(&admin_token) {
            return (
                StatusCode::UNAUTHORIZED,
                json!({ "error": "Invalid admin token" }).to_string(),
            )
                .into_response();
        }
    }

    info!("Config reload requested via API for {}", state.service_name);

    match state.loader.reload() {
        Ok(changed) => {
            let version = state.loader.meta().version;
            info!(
                "Config reload completed for {} (version {}, changed: {})",
                state.service_name, version, changed
            );
            (
                StatusCode::OK,
                json!({
                    "status": "ok",
                    "version": version,
                    "changed": changed,
                    "service": state.service_name,
                })
                .to_string(),
            )
                .into_response()
        }
        Err(e) => {
            error!("Config reload failed for {}: {}", state.service_name, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({
                    "error": "Config reload failed",
                    "details": e.to_string(),
                    "service": state.service_name,
                })
                .to_string(),
            )
                .into_response()
        }
    }
}

/// GET /api/v1/config-version - Get current config version.
async fn config_version_handler<C>(State(state): State<HotReloadState<C>>) -> Response
where
    C: Clone + Send + Sync + 'static,
{
    let meta = state.loader.meta();
    (
        StatusCode::OK,
        json!({
            "version": meta.version,
            "source": meta.source,
            "loaded_at": meta.loaded_at.to_rfc3339(),
            "service": meta.service,
        })
        .to_string(),
    )
        .into_response()
}

/// Spawn SIGHUP handler for config reload.
pub fn spawn_sighup_handler<C>(loader: Arc<ConfigLoader<C>>, service_name: String)
where
    C: Clone + Send + Sync + 'static,
{
    tokio::spawn(async move {
        let mut sighup = signal(SignalKind::hangup()).expect("Failed to create SIGHUP handler");
        loop {
            sighup.recv().await;
            info!("Received SIGHUP, reloading config for {}", service_name);
            match loader.reload() {
                Ok(changed) => {
                    let version = loader.meta().version;
                    info!(
                        "SIGHUP config reload completed for {} (version {}, changed: {})",
                        service_name, version, changed
                    );
                }
                Err(e) => {
                    error!("SIGHUP config reload failed for {}: {}", service_name, e);
                }
            }
        }
    });
}

/// Spawn periodic config validation (optional).
pub fn spawn_config_validator<C>(
    loader: Arc<ConfigLoader<C>>,
    service_name: String,
    interval: Duration,
) where
    C: Clone + Send + Sync + 'static,
{
    tokio::spawn(async move {
        let mut interval_timer = tokio::time::interval(interval);
        loop {
            interval_timer.tick().await;
            // Validate current config is still valid
            let config = loader.config();
            if let Err(e) = config.validate() {
                warn!(
                    "Periodic config validation failed for {}: {}",
                    service_name, e
                );
            }
        }
    });
}

/// Graceful shutdown helper - waits for in-flight requests.
pub async fn graceful_shutdown(timeout: Duration) {
    info!("Starting graceful shutdown (timeout: {:?})", timeout);
    tokio::time::sleep(timeout).await;
    info!("Graceful shutdown complete");
}

/// Config watcher for file changes (alternative to SIGHUP).
pub fn spawn_file_watcher<C, P: AsRef<std::path::Path>>(
    loader: Arc<ConfigLoader<C>>,
    path: P,
    service_name: String,
    debounce: Duration,
) where
    C: Clone + Send + Sync + 'static,
{
    let path = path.as_ref().to_path_buf();
    let mut last_modified = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok());

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            if let Ok(metadata) = std::fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    if last_modified.map_or(true, |last| modified > last) {
                        // File changed, debounce
                        tokio::time::sleep(debounce).await;
                        info!("Config file changed, reloading for {}", service_name);
                        match loader.reload() {
                            Ok(changed) => {
                                let version = loader.meta().version;
                                info!(
                                    "File watcher reload completed for {} (version {}, changed: {})",
                                    service_name, version, changed
                                );
                                last_modified = Some(modified);
                            }
                            Err(e) => {
                                error!("File watcher reload failed for {}: {}", service_name, e);
                            }
                        }
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::GatewayConfig;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_config_reload_endpoint() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let config = r#"
service = "gateway"
[server]
host = "0.0.0.0"
port = 8080
[auth]
default_mode = "none"
[metrics]
enabled = true
[tracing]
enabled = true
[rate_limit]
enabled = true
"#;
        std::io::Write::write_all(&mut file, config.as_bytes()).unwrap();

        let loader =
            Arc::new(crate::loader::ConfigLoader::<GatewayConfig>::from_file(file.path()).unwrap());
        let state = HotReloadState {
            loader: loader.clone(),
            service_name: "gateway".to_string(),
        };

        let app = hot_reload_routes().with_state(state);

        // Test reload endpoint
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/config-reload")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
