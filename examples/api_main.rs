//! Example service demonstrating API layer integration.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    middleware,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use tokio::signal;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use mediaedge_api::{
    openapi::{OpenApiConfig, create_openapi_router},
    validation::{ValidationConfig, ValidatedJson},
    versioning::{VersioningConfig, versioning_layer},
    rate_limit::{RateLimitConfig, rate_limit_layer, KeyExtractor},
    middleware::{ApiMiddlewareConfig, create_full_api_router, CorsConfig, cors_layer},
    openapi::{
        CreateStreamRequest, Stream, StreamStatus, StreamHealth,
        IngestProtocol, EgressProtocol, StreamIngestInfoResponse,
        HlsPlaybackInfo, DashPlaybackInfo,
    },
};
use mediaedge_config::ConfigLoader;

#[derive(Clone)]
struct AppState {
    config_loader: Arc<ConfigLoader<mediaedge_config::types::GatewayConfig>>,
    streams: Arc<dashmap::DashMap<String, Stream>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json()
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting MediaEdge Gateway with API Layer");

    // Load configuration
    let config_path = std::env::var("CONFIG_FILE")
        .unwrap_or_else(|_| "config/gateway.toml".to_string());
    let config_loader = Arc::new(ConfigLoader::<mediaedge_config::types::GatewayConfig>::from_file(&config_path)?);

    // Build API middleware config
    let api_config = ApiMiddlewareConfig::new()
        .with_service_name("MediaEdge Gateway".to_string())
        .with_version("1.0.0".to_string())
        .with_validation(ValidationConfig::default())
        .with_versioning(VersioningConfig::new()
            .with_default_version(mediaedge_api::openapi::ApiVersion::V1)
            .with_supported_versions(vec![mediaedge_api::openapi::ApiVersion::V1]))
        .with_rate_limit(RateLimitConfig::new()
            .with_requests_per_window(1000)
            .with_window(std::time::Duration::from_secs(60))
            .with_key_extractor(KeyExtractor::Ip));

    // Create router with all middleware
    let app = create_full_api_router(api_config)
        .merge(create_openapi_router(&OpenApiConfig::default()))
        // Stream endpoints
        .route("/api/v1/streams", get(list_streams).post(create_stream))
        .route("/api/v1/streams/:stream_id", get(get_stream).delete(delete_stream))
        .route("/api/v1/streams/:stream_id/ingest", get(get_ingest_info))
        .route("/api/v1/streams/:stream_id/egress", get(get_egress_info))
        .with_state(AppState {
            config_loader,
            streams: Arc::new(dashmap::DashMap::new()),
        });

    // Start server
    let addr = "0.0.0.0:8080";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Gateway listening on {}", addr);

    // Graceful shutdown
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal());

    if let Err(e) = server.await {
        tracing::error!("Server error: {}", e);
    }

    info!("Gateway stopped");
    Ok(())
}

/// List all streams.
async fn list_streams(State(state): State<AppState>) -> Json<serde_json::Value> {
    let streams: Vec<Stream> = state.streams.iter().map(|s| s.value().clone()).collect();
    Json(json!({ "data": streams }))
}

/// Create a new stream.
async fn create_stream(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<CreateStreamRequest>,
) -> Result<Json<Stream>, StatusCode> {
    let stream = Stream {
        id: uuid::Uuid::new_v4().to_string(),
        name: payload.name,
        status: StreamStatus::Offline,
        health: StreamHealth::Healthy,
        ingest_protocol: Some(payload.ingest_protocol),
        egress_protocols: payload.egress_protocols,
        created_at: chrono::Utc::now(),
        started_at: None,
        ended_at: None,
        viewer_count: 0,
        bitrate_bps: 0,
        resolution: None,
        codec: None,
        metadata: payload.metadata.unwrap_or_default(),
    };

    state.streams.insert(stream.id.clone(), stream.clone());
    Ok(Json(stream))
}

/// Get a stream by ID.
async fn get_stream(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> Result<Json<Stream>, StatusCode> {
    state.streams
        .get(&stream_id)
        .map(|s| Json(s.value().clone()))
        .ok_or(StatusCode::NOT_FOUND)
}

/// Delete a stream.
async fn delete_stream(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    if state.streams.remove(&stream_id).is_some() {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Get ingest info for a stream.
async fn get_ingest_info(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> Result<Json<StreamIngestInfoResponse>, StatusCode> {
    let stream = state.streams.get(&stream_id).ok_or(StatusCode::NOT_FOUND)?;
    
    let ingest_info = match stream.ingest_protocol {
        Some(IngestProtocol::Rtmp) => StreamIngestInfoResponse::Rtmp {
            server_url: "rtmp://localhost:1935/live".to_string(),
            stream_key: stream_id,
            app_name: "live".to_string(),
        },
        Some(IngestProtocol::Srt) => StreamIngestInfoResponse::Srt {
            host: "localhost".to_string(),
            port: 8888,
            stream_id: stream_id.clone(),
            latency_ms: 120,
            passphrase: None,
        },
        Some(IngestProtocol::Whip) => StreamIngestInfoResponse::Whip {
            endpoint: "https://localhost:8080/whip".to_string(),
            stun_servers: vec!["stun:stun.l.google.com:19302".to_string()],
            turn_servers: vec![],
        },
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    Ok(Json(ingest_info))
}

/// Get egress info for a stream.
async fn get_egress_info(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
) -> Result<Json<Vec<StreamIngestInfoResponse>>, StatusCode> {
    let stream = state.streams.get(&stream_id).ok_or(StatusCode::NOT_FOUND)?;
    
    let mut egress_infos = Vec::new();
    for protocol in &stream.egress_protocols {
        match protocol {
            EgressProtocol::Hls => {
                egress_infos.push(StreamIngestInfoResponse::Hls(HlsPlaybackInfo {
                    master_playlist_url: format!("https://localhost:8080/api/v1/streams/{}/hls/master.m3u8", stream_id),
                    variant_playlists: vec![],
                }));
            }
            EgressProtocol::Dash => {
                egress_infos.push(StreamIngestInfoResponse::Dash(DashPlaybackInfo {
                    mpd_url: format!("https://localhost:8080/api/v1/streams/{}/dash/manifest.mpd", stream_id),
                    representations: vec![],
                }));
            }
            _ => {}
        }
    }

    Ok(Json(egress_infos))
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