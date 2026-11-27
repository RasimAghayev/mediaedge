//! Example service main demonstrating config integration.
//!
//! This shows how to:
//! 1. Load config from file with env/CLI overrides
//! 2. Set up hot-reload (SIGHUP + HTTP endpoint)
//! 3. Access config in handlers via Axum State
//! 4. Subscribe to config changes for reactive components

use std::sync::Arc;
use std::time::Duration;

use axum::{
    routing::get,
    Router,
    extract::State,
    response::Json,
};
use serde_json::json;
use tokio::signal;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use mediaedge_config::{
    ConfigLoader, HotReloadState, hot_reload_routes, spawn_sighup_handler,
    spawn_config_validator, graceful_shutdown,
    types::{GatewayConfig, ServiceConfig},
    prelude::*,
};

#[derive(Clone)]
struct AppState {
    config_loader: Arc<ConfigLoader<GatewayConfig>>,
    // Other service state...
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json()
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting MediaEdge Gateway");

    // Load configuration
    // Priority: defaults < config file < env vars < CLI args
    let config_path = std::env::var("CONFIG_FILE")
        .unwrap_or_else(|_| "config/gateway.toml".to_string());

    let config_loader = Arc::new(ConfigLoader::<GatewayConfig>::from_file(&config_path)?);
    let config = config_loader.config();

    info!("Loaded config: service={}, version={}", 
        config.server.port, config_loader.meta().version);

    // Set up hot-reload
    // 1. SIGHUP handler
    spawn_sighup_handler(config_loader.clone(), "gateway".to_string());

    // 2. Periodic validation (every 5 minutes)
    spawn_config_validator(config_loader.clone(), "gateway".to_string(), Duration::from_secs(300));

    // 3. Optional: File watcher (alternative to SIGHUP)
    // spawn_file_watcher(config_loader.clone(), &config_path, "gateway".to_string(), Duration::from_secs(2));

    // Create app state
    let app_state = AppState {
        config_loader: config_loader.clone(),
    };

    // Create hot-reload state for HTTP endpoint
    let hot_reload_state = HotReloadState {
        loader: config_loader.clone(),
        service_name: "gateway".to_string(),
    };

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/config", get(get_config))
        .merge(hot_reload_routes().with_state(hot_reload_state))
        .with_state(app_state);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Gateway listening on {}", addr);

    // Graceful shutdown
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal());

    if let Err(e) = server.await {
        error!("Server error: {}", e);
    }

    // Wait for in-flight requests
    graceful_shutdown(Duration::from_secs(30)).await;

    info!("Gateway stopped");
    Ok(())
}

/// Health check endpoint.
async fn health_check(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.config_loader.config();
    Json(json!({
        "status": "healthy",
        "service": "gateway",
        "version": state.config_loader.meta().version,
        "port": config.server.port,
    }))
}

/// Get current config (sanitized - no secrets).
async fn get_config(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.config_loader.config();
    let meta = state.config_loader.meta();

    // Sanitize config (remove secrets)
    let sanitized = json!({
        "server": {
            "host": config.server.host,
            "port": config.server.port,
            "http2": config.server.http2,
        },
        "auth": {
            "default_mode": format!("{:?}", config.auth.default_mode),
        },
        "metrics": {
            "enabled": config.metrics.enabled,
            "path": config.metrics.path,
        },
        "protocols": {
            "rtmp": config.protocols.rtmp.as_ref().map(|c| c.enabled),
            "srt": config.protocols.srt.as_ref().map(|c| c.enabled),
            "rist": config.protocols.rist.as_ref().map(|c| c.enabled),
            "whip": config.protocols.whip.as_ref().map(|c| c.enabled),
        },
        "meta": {
            "version": meta.version,
            "loaded_at": meta.loaded_at.to_rfc3339(),
            "source": meta.source,
        },
    });

    Json(sanitized)
}

/// Shutdown signal handler.
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C"),
        _ = terminate => info!("Received SIGTERM"),
    }
}

/// Example: Reactive component that responds to config changes.
struct ReactiveComponent {
    config_subscription: mediaedge_config::ConfigSubscription<GatewayConfig>,
    current_config: GatewayConfig,
}

impl ReactiveComponent {
    fn new(loader: &Arc<ConfigLoader<GatewayConfig>>) -> Self {
        let subscription = loader.subscribe();
        let current_config = loader.config();
        Self {
            config_subscription: subscription,
            current_config,
        }
    }

    /// Call periodically to check for config changes.
    fn check_config_changes(&mut self) -> bool {
        if let Some(new_config) = self.config_subscription.check() {
            info!("Config changed! Version: {}", new_config.meta.version);
            self.current_config = new_config.config;
            // React to changes (e.g., update rate limiter, reconnect to upstreams)
            self.on_config_change();
            true
        } else {
            false
        }
    }

    fn on_config_change(&self) {
        // Update rate limiter
        // Reconnect to upstreams if changed
        // Reload TLS certs if changed
        info!("Reactive component updated for config change");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn test_service_startup() {
        let mut file = NamedTempFile::new().unwrap();
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
        file.write_all(config.as_bytes()).unwrap();

        let loader = ConfigLoader::<GatewayConfig>::from_file(file.path()).unwrap();
        assert_eq!(loader.config().server.port, 8080);
    }
}