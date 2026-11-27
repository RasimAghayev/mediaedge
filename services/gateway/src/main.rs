//! MediaEdge Gateway Service
//!
//! Unified entry point for all media protocols with TLS termination,
//! authentication, routing, rate limiting, and admin API.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use axum::{
    body::Body,
    extract::{ConnectInfo, MatchedPath, Request, State, WebSocketUpgrade},
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put, delete, any},
    Router,
};
use axum_server::Handle;
use dashmap::DashMap;
use futures::StreamExt;
use governor::{Quota, RateLimiter};
use jsonwebtoken::{decode, decode_header, jwks::Jwks, Algorithm, DecodingKey, Validation};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig as RustlsServerConfig};
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use mediaedge_api::{
    openapi::{api_doc, create_openapi_router, OpenApiConfig},
    rate_limit::{RateLimitConfig, RateLimitMiddleware},
    validation::{ValidationMiddleware, ValidatedJson, ValidatedQuery},
    ApiMiddlewareConfig, ApiVersion, VersioningMiddleware,
};
use mediaedge_config::{
    loader,
    types::{AuthConfig, AuthMode, GatewayConfig, GatewayProtocolsConfig, JwtConfig, RateLimitConfig as ConfigRateLimit, ServerConfig, TlsConfig, UpstreamConfig},
    ArcSwapConfig,
};
use mediaedge_core::cluster::ClusterManager;
use mediaedge_observability::{MetricsRegistry, TracingHandle};

// ============================================================================
// Application State
// ============================================================================

/// Shared application state for the gateway.
#[derive(Clone)]
pub struct GatewayState {
    /// Gateway configuration (hot-reloadable).
    pub config: ArcSwap<GatewayConfig>,

    /// HTTP client for upstream requests.
    pub http_client: HttpClient,

    /// Cluster manager for ownership/capacity/redirect.
    pub cluster: Arc<ClusterManager>,

    /// Metrics registry.
    pub metrics: Arc<MetricsRegistry>,

    /// Rate limiters per key (tenant/stream/IP).
    pub rate_limiters: Arc<DashMap<String, Arc<RateLimiter<std::sync::Mutex<governor::state::NotKeyed>, governor::state::InMemoryState, governor::clock::DefaultClock>>>>,

    /// JWT validation keys cache.
    pub jwks_cache: Arc<RwLock<Option<Jwks>>>,

    /// Active WebSocket connections for WHIP/WHEP/MoQ.
    pub ws_connections: Arc<DashMap<String, Vec<tokio::sync::mpsc::UnboundedSender<axum::extract::ws::Message>>>>,

    /// Config reload channel.
    pub reload_tx: mpsc::UnboundedSender<()>,
}

impl GatewayState {
    /// Create a new gateway state.
    pub async fn new(config: GatewayConfig, cluster: Arc<ClusterManager>, metrics: Arc<MetricsRegistry>) -> Result<(Self, mpsc::UnboundedReceiver<()>)> {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(config.server.request_timeout_secs))
            .build()?;

        let (reload_tx, reload_rx) = mpsc::unbounded_channel();

        Ok((
            Self {
                config: ArcSwap::from_pointee(config),
                http_client,
                cluster,
                metrics,
                rate_limiters: Arc::new(DashMap::new()),
                jwks_cache: Arc::new(RwLock::new(None)),
                ws_connections: Arc::new(DashMap::new()),
                reload_tx,
            },
            reload_rx,
        ))
    }

    /// Get current config.
    pub fn config(&self) -> Arc<GatewayConfig> {
        self.config.load()
    }

    /// Get or create rate limiter for a key.
    pub fn get_rate_limiter(&self, key: &str, quota: Quota) -> Arc<RateLimiter<std::sync::Mutex<governor::state::NotKeyed>, governor::state::InMemoryState, governor::clock::DefaultClock>> {
        self.rate_limiters
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(RateLimiter::direct(quota)))
            .clone()
    }
}

// ============================================================================
// Authentication
// ============================================================================

/// Authenticated user/tenant context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub stream_access: Option<Vec<String>>, // stream IDs this token can access
}

/// Extract auth context from request.
pub async fn extract_auth(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<AuthContext, StatusCode> {
    let config = state.config();

    // Try JWT first
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                if let Ok(ctx) = validate_jwt(&state, token, &config.auth).await {
                    return Ok(ctx);
                }
            }
        }
    }

    // Try API key
    if let Some(api_key) = headers.get("x-api-key") {
        if let Ok(key_str) = api_key.to_str() {
            if let Ok(ctx) = validate_static_token(&config.auth, key_str).await {
                return Ok(ctx);
            }
        }
    }

    // Try webhook auth
    if let Some(webhook_config) = &config.auth.webhook {
        if let Ok(ctx) = validate_webhook(&state, &headers, webhook_config).await {
            return Ok(ctx);
        }
    }

    // Allow unauthenticated for health/metrics
    Err(StatusCode::UNAUTHORIZED)
}

/// Validate JWT token.
async fn validate_jwt(
    state: &GatewayState,
    token: &str,
    auth_config: &AuthConfig,
) -> Result<AuthContext, StatusCode> {
    let jwt_config = auth_config.jwt.as_ref().ok_or(StatusCode::UNAUTHORIZED)?;

    // Get JWKS if using RS256
    let decoding_key = if jwt_config.algorithm == "RS256" {
        let mut cache = state.jwks_cache.write().await;
        if cache.is_none() {
            if let Some(jwks_url) = &jwt_config.jwks_url {
                let jwks = Jwks::fetch(jwks_url)
                    .await
                    .map_err(|_| StatusCode::UNAUTHORIZED)?;
                *cache = Some(jwks);
            }
        }
        if let Some(jwks) = cache.as_ref() {
            let header = decode_header(token).map_err(|_| StatusCode::UNAUTHORIZED)?;
            let kid = header.kid.ok_or(StatusCode::UNAUTHORIZED)?;
            let key = jwks.find(&kid).ok_or(StatusCode::UNAUTHORIZED)?;
            DecodingKey::from_rsa_components(&key.n, &key.e)
        } else {
            return Err(StatusCode::UNAUTHORIZED);
        }
    } else {
        let secret = jwt_config.secret.as_ref().ok_or(StatusCode::UNAUTHORIZED)?;
        DecodingKey::from_secret(secret.as_bytes())
    };

    let mut validation = Validation::new(Algorithm::from(jwt_config.algorithm.as_str()));
    validation.set_issuer(&[jwt_config.issuer.as_deref().unwrap_or("mediaedge")]);
    validation.set_audience(&[jwt_config.audience.as_deref().unwrap_or("api")]);
    validation.leeway = jwt_config.clock_skew_secs;

    let token_data = decode::<serde_json::Value>(token, &decoding_key, &validation)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let claims = token_data.claims;

    Ok(AuthContext {
        tenant_id: claims.get("tenant_id").and_then(|v| v.as_str()).unwrap_or("default").to_string(),
        user_id: claims.get("sub").and_then(|v| v.as_str()).map(|s| s.to_string()),
        roles: claims.get("roles").and_then(|v| v.as_array()).map(|a| {
            a.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect()
        }).unwrap_or_default(),
        permissions: claims.get("permissions").and_then(|v| v.as_array()).map(|a| {
            a.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect()
        }).unwrap_or_default(),
        stream_access: claims.get("stream_access").and_then(|v| v.as_array()).map(|a| {
            a.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect()
        }),
    })
}

/// Validate static token.
async fn validate_static_token(
    auth_config: &AuthConfig,
    token: &str,
) -> Result<AuthContext, StatusCode> {
    let static_config = auth_config.static_tokens.as_ref().ok_or(StatusCode::UNAUTHORIZED)?;
    let permissions = static_config.tokens.get(token).ok_or(StatusCode::UNAUTHORIZED)?;

    Ok(AuthContext {
        tenant_id: "default".to_string(),
        user_id: None,
        roles: permissions.split_whitespace().map(|s| s.to_string()).collect(),
        permissions: permissions.split_whitespace().map(|s| s.to_string()).collect(),
        stream_access: None,
    })
}

/// Validate via webhook.
async fn validate_webhook(
    state: &GatewayState,
    headers: &HeaderMap,
    webhook_config: &mediaedge_config::types::WebhookAuthConfig,
) -> Result<AuthContext, StatusCode> {
    let mut request = state.http_client.post(&webhook_config.url);

    // Add custom headers
    for (key, value) in &webhook_config.headers {
        request = request.header(key, value);
    }

    // Forward auth headers
    for (key, value) in headers {
        if key.as_str().to_lowercase().starts_with("x-") || key == "authorization" {
            request = request.header(key, value);
        }
    }

    let response = request
        .timeout(Duration::from_secs(webhook_config.timeout_secs))
        .send()
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    if !response.status().is_success() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let body: serde_json::Value = response.json().await.map_err(|_| StatusCode::UNAUTHORIZED)?;

    Ok(AuthContext {
        tenant_id: body.get("tenant_id").and_then(|v| v.as_str()).unwrap_or("default").to_string(),
        user_id: body.get("user_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
        roles: body.get("roles").and_then(|v| v.as_array()).map(|a| {
            a.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect()
        }).unwrap_or_default(),
        permissions: body.get("permissions").and_then(|v| v.as_array()).map(|a| {
            a.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect()
        }).unwrap_or_default(),
        stream_access: body.get("stream_access").and_then(|v| v.as_array()).map(|a| {
            a.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect()
        }),
    })
}

// ============================================================================
// Rate Limiting
// ============================================================================

/// Rate limit key extractor.
fn rate_limit_key(auth: &Option<AuthContext>, headers: &HeaderMap, addr: SocketAddr, stream_id: Option<&str>) -> String {
    // Priority: tenant > stream > IP
    if let Some(auth) = auth {
        if let Some(stream) = stream_id {
            return format!("tenant:{}:stream:{}", auth.tenant_id, stream);
        }
        return format!("tenant:{}", auth.tenant_id);
    }

    // Check for API key
    if let Some(api_key) = headers.get("x-api-key") {
        if let Ok(key) = api_key.to_str() {
            return format!("apikey:{}", key);
        }
    }

    // Fallback to IP
    format!("ip:{}", addr.ip())
}

/// Rate limiting middleware.
async fn rate_limit_middleware(
    State(state): State<GatewayState>,
    auth: Option<AuthContext>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let config = state.config();
    let rl_config = &config.rate_limit;

    if !rl_config.enabled {
        return next.run(request).await;
    }

    // Extract stream ID from path if present
    let stream_id = request
        .uri()
        .path()
        .split('/')
        .nth(3) // /api/v1/streams/{stream_id}/...
        .filter(|s| !s.is_empty() && *s != "streams");

    let key = rate_limit_key(&auth, &headers, addr, stream_id);
    let quota = Quota::per_minute(rl_config.requests_per_window);
    let limiter = state.get_rate_limiter(&key, quota);

    match limiter.check() {
        Ok(_) => next.run(request).await,
        Err(_) => {
            let mut response = StatusCode::TOO_MANY_REQUESTS.into_response();
            response.headers_mut().insert(
                "Retry-After",
                HeaderValue::from_str(&rl_config.window_secs.to_string()).unwrap(),
            );
            response
        }
    }
}

// ============================================================================
// Request Routing (Redirect/Proxy)
// ============================================================================

/// Route request to owning node or proxy.
async fn route_request(
    State(state): State<GatewayState>,
    auth: Option<AuthContext>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let config = state.config();

    // Check if this is a stream-specific request
    let path = uri.path();
    let stream_id = extract_stream_id(path);

    if let Some(stream_id) = stream_id {
        // Get ownership info
        let ownership = state.cluster.ownership();
        if let Some(owner) = ownership.get_owner(stream_id).await {
            let owner_addr = owner.address.clone();

            // If we're the owner, handle locally
            if owner_addr == format!("{}:{}", config.server.host, config.server.port) {
                return handle_local_request(state, auth, method, uri, headers, body).await;
            }

            // Otherwise redirect or proxy
            if config.server.http2 {
                // Proxy for HTTP/2
                return proxy_request(state, &owner_addr, method, uri, headers, body).await;
            } else {
                // HTTP 302 redirect
                let redirect_url = format!("http://{}{}", owner_addr, uri);
                return Response::builder()
                    .status(StatusCode::FOUND)
                    .header("Location", redirect_url)
                    .body(Body::empty())
                    .unwrap();
            }
        }
    }

    // No stream ID or no owner - handle locally
    handle_local_request(state, auth, method, uri, headers, body).await
}

/// Extract stream ID from path.
fn extract_stream_id(path: &str) -> Option<String> {
    // Paths like /api/v1/streams/{stream_id}/... or /hls/{stream_id}/...
    let parts: Vec<&str> = path.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "streams" && i + 1 < parts.len() {
            return Some(parts[i + 1].to_string());
        }
        if *part == "hls" || *part == "dash" || *part == "whep" || *part == "moq" {
            if i + 1 < parts.len() {
                return Some(parts[i + 1].to_string());
            }
        }
    }
    None
}

/// Handle request locally.
async fn handle_local_request(
    state: GatewayState,
    auth: Option<AuthContext>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> Response {
    // This would dispatch to the appropriate handler
    // For now, return 404
    StatusCode::NOT_FOUND.into_response()
}

/// Proxy request to upstream.
async fn proxy_request(
    state: GatewayState,
    upstream: &str,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let url = format!("http://{}{}", upstream, uri);

    let mut request = state.http_client.request(method, &url);

    // Forward headers (except host)
    for (key, value) in headers {
        if key != "host" {
            request = request.header(key, value);
        }
    }

    // Get body bytes
    let body_bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap_or_default();
    request = request.body(body_bytes);

    match request.send().await {
        Ok(response) => {
            let status = response.status();
            let mut resp_builder = Response::builder().status(status);

            // Forward response headers
            for (key, value) in response.headers() {
                if key != "content-length" && key != "transfer-encoding" {
                    resp_builder = resp_builder.header(key, value);
                }
            }

            let body = response.bytes().await.unwrap_or_default();
            resp_builder.body(Body::from(body)).unwrap()
        }
        Err(_) => StatusCode::BAD_GATEWAY.into_response(),
    }
}

// ============================================================================
// WebSocket Upgrade Handler
// ============================================================================

/// Handle WebSocket upgrade for WHIP/WHEP/MoQ.
async fn handle_websocket(
    ws: WebSocketUpgrade,
    State(state): State<GatewayState>,
    auth: Option<AuthContext>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    uri: Uri,
) -> Response {
    let path = uri.path();

    // Determine protocol from path
    let protocol = if path.contains("/whip") {
        "whip"
    } else if path.contains("/whep") {
        "whep"
    } else if path.contains("/moq") {
        "moq"
    } else {
        "ws"
    };

    ws.on_upgrade(move |socket| handle_ws_connection(socket, state, auth, protocol, addr))
}

async fn handle_ws_connection(
    mut socket: axum::extract::ws::WebSocket,
    state: GatewayState,
    auth: Option<AuthContext>,
    protocol: &str,
    addr: SocketAddr,
) {
    let connection_id = Uuid::new_v4().to_string();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Register connection
    state.ws_connections.entry(protocol.to_string()).or_default().push(tx.clone());

    info!("WebSocket connected: {} ({}) from {}", connection_id, protocol, addr);

    // Spawn sender task
    let sender = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if socket.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(axum::extract::ws::Message::Text(text)) => {
                // Handle signaling messages for WHIP/WHEP
                debug!("WS {}: {}", connection_id, text);
            }
            Ok(axum::extract::ws::Message::Binary(data)) => {
                // Handle MoQ data
                debug!("WS {}: binary {} bytes", connection_id, data.len());
            }
            Ok(axum::extract::ws::Message::Close(_)) => break,
            Err(e) => {
                error!("WS {} error: {}", connection_id, e);
                break;
            }
            _ => {}
        }
    }

    // Cleanup
    if let Some(mut conns) = state.ws_connections.get_mut(protocol) {
        conns.retain(|c| !c.same_channel(&tx));
    }
    sender.abort();
    info!("WebSocket disconnected: {}", connection_id);
}

// ============================================================================
// Admin API Handlers
// ============================================================================

/// Health check endpoint.
async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "healthy")
}

/// Get cluster nodes.
async fn get_cluster_nodes(State(state): State<GatewayState>) -> impl IntoResponse {
    let cluster = state.cluster.gossip().await;
    if let Some(gossip) = cluster {
        let members = gossip.members().await;
        axum::Json(serde_json::json!({ "nodes": members }))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "Cluster not available").into_response()
    }
}

/// Get broadcast ownership.
async fn get_broadcast_ownership(State(state): State<GatewayState>) -> impl IntoResponse {
    let ownership = state.cluster.ownership();
    let broadcasts = ownership.get_all_ownership().await;
    axum::Json(serde_json::json!({ "broadcasts": broadcasts }))
}

/// Get cluster capacity.
async fn get_cluster_capacity(State(state): State<GatewayState>) -> impl IntoResponse {
    let capacity = state.cluster.capacity();
    let nodes = capacity.get_all_capacity().await;
    axum::Json(serde_json::json!({ "capacity": nodes }))
}

/// Config reload endpoint.
async fn config_reload(State(state): State<GatewayState>) -> impl IntoResponse {
    let _ = state.reload_tx.send(());
    let config = state.config();
    (StatusCode::OK, axum::Json(serde_json::json!({
        "status": "ok",
        "version": config.server.node_id.as_deref().unwrap_or("unknown")
    })))
}

/// Get config version.
async fn config_version(State(state): State<GatewayState>) -> impl IntoResponse {
    let config = state.config();
    axum::Json(serde_json::json!({
        "version": 1,
        "service": "gateway",
        "loaded_at": chrono::Utc::now()
    }))
}

/// Create stream.
async fn create_stream(
    State(state): State<GatewayState>,
    auth: AuthContext,
    ValidatedJson(payload): ValidatedJson<mediaedge_api::openapi::CreateStreamRequest>,
) -> impl IntoResponse {
    // Implementation would create stream in cluster
    let stream_id = Uuid::new_v4().to_string();
    let stream = mediaedge_api::openapi::Stream {
        id: stream_id.clone(),
        name: payload.name,
        status: mediaedge_api::openapi::StreamStatus::Starting,
        health: mediaedge_api::openapi::StreamHealth::Healthy,
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

    (StatusCode::CREATED, axum::Json(stream))
}

/// List streams.
async fn list_streams(
    State(state): State<GatewayState>,
    auth: AuthContext,
    ValidatedQuery(pagination): ValidatedQuery<mediaedge_api::openapi::PaginationParams>,
) -> impl IntoResponse {
    // Implementation would query cluster
    axum::Json(mediaedge_api::openapi::PaginatedResponse {
        data: vec![],
        pagination: mediaedge_api::openapi::PaginationInfo {
            page: pagination.page,
            per_page: pagination.per_page,
            total: 0,
            total_pages: 0,
        },
    })
}

/// Get stream.
async fn get_stream(
    State(state): State<GatewayState>,
    auth: AuthContext,
    axum::extract::Path(stream_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    // Implementation would query cluster
    (StatusCode::NOT_FOUND, "Stream not found").into_response()
}

/// Update stream.
async fn update_stream(
    State(state): State<GatewayState>,
    auth: AuthContext,
    axum::extract::Path(stream_id): axum::extract::Path<String>,
    ValidatedJson(payload): ValidatedJson<mediaedge_api::openapi::UpdateStreamRequest>,
) -> impl IntoResponse {
    // Implementation would update stream in cluster
    (StatusCode::OK, "Stream updated").into_response()
}

/// Delete stream.
async fn delete_stream(
    State(state): State<GatewayState>,
    auth: AuthContext,
    axum::extract::Path(stream_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    // Implementation would delete stream from cluster
    StatusCode::NO_CONTENT.into_response()
}

/// Get stream ingest info.
async fn get_ingest_info(
    State(state): State<GatewayState>,
    auth: AuthContext,
    axum::extract::Path(stream_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    // Implementation would return ingest URLs
    axum::Json(serde_json::json!({ "stream_id": stream_id }))
}

/// Get stream egress info.
async fn get_egress_info(
    State(state): State<GatewayState>,
    auth: AuthContext,
    axum::extract::Path(stream_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    // Implementation would return playback URLs
    axum::Json(serde_json::json!({ "stream_id": stream_id }))
}

/// Create signed URL.
async fn create_signed_url(
    State(state): State<GatewayState>,
    auth: AuthContext,
    ValidatedJson(payload): ValidatedJson<mediaedge_api::openapi::SignedUrlRequest>,
) -> impl IntoResponse {
    // Implementation would generate signed URL
    let url = format!("https://example.com/hls/{}/playlist.m3u8?token=signed", payload.stream_id);
    axum::Json(mediaedge_api::openapi::SignedUrlResponse {
        url,
        expires_at: chrono::Utc::now() + chrono::Duration::seconds(payload.ttl_secs as i64),
        signature: "signature".to_string(),
    })
}

// ============================================================================
// Protocol Handlers (RTMP, SRT, RIST, WHIP)
// ============================================================================

/// RTMP ingest handler - runs on separate port.
async fn rtmp_server(config: GatewayConfig, state: GatewayState) -> Result<()> {
    if !config.protocols.rtmp.as_ref().map(|c| c.enabled).unwrap_or(false) {
        return Ok(());
    }

    let port = config.protocols.rtmp.as_ref().map(|c| c.port).unwrap_or(1935);
    info!("Starting RTMP server on port {}", port);

    // RTMP server implementation would go here
    // Using the rtmp crate from workspace
    // For now, just log and return
    tokio::signal::ctrl_c().await?;
    Ok(())
}

/// SRT ingest handler - runs on separate port.
async fn srt_server(config: GatewayConfig, state: GatewayState) -> Result<()> {
    if !config.protocols.srt.as_ref().map(|c| c.enabled).unwrap_or(false) {
        return Ok(());
    }

    let port = config.protocols.srt.as_ref().map(|c| c.port).unwrap_or(8888);
    info!("Starting SRT server on port {}", port);

    // SRT server implementation would go here
    tokio::signal::ctrl_c().await?;
    Ok(())
}

/// RIST ingest handler - runs on separate port.
async fn rist_server(config: GatewayConfig, state: GatewayState) -> Result<()> {
    if !config.protocols.rist.as_ref().map(|c| c.enabled).unwrap_or(false) {
        return Ok(());
    }

    let port = config.protocols.rist.as_ref().map(|c| c.port).unwrap_or(5000);
    info!("Starting RIST server on port {}", port);

    // RIST server implementation would go here
    tokio::signal::ctrl_c().await?;
    Ok(())
}

/// WHIP ingest handler - runs on HTTP server path.
async fn whip_handler(
    State(state): State<GatewayState>,
    auth: Option<AuthContext>,
    request: Request,
) -> Response {
    // WHIP signaling endpoint
    // Implementation would handle WHIP OPTIONS/POST for WebRTC ingest
    StatusCode::NOT_IMPLEMENTED.into_response()
}

// ============================================================================
// Config Hot Reload
// ============================================================================

/// Config reload watcher.
async fn config_reload_watcher(
    mut reload_rx: mpsc::UnboundedReceiver<()>,
    state: GatewayState,
) {
    while reload_rx.recv().await.is_some() {
        info!("Config reload triggered");

        // Reload config from file
        match loader::load::<GatewayConfig>() {
            Ok(new_config) => {
                state.config.store(Arc::new(new_config));
                info!("Config reloaded successfully");
            }
            Err(e) => {
                error!("Config reload failed: {}", e);
            }
        }
    }
}

/// SIGHUP handler for config reload.
fn setup_signal_handlers(reload_tx: mpsc::UnboundedSender<()>) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sighup = signal(SignalKind::hangup()).expect("Failed to register SIGHUP handler");
        tokio::spawn(async move {
            while sighup.recv().await.is_some() {
                let _ = reload_tx.send(());
            }
        });
    }
}

// ============================================================================
// TLS Configuration
// ============================================================================

/// Build TLS config from gateway config.
fn build_tls_config(tls_config: &TlsConfig) -> Result<RustlsServerConfig> {
    let cert_file = std::fs::read(&tls_config.cert_file)
        .context("Failed to read cert file")?;
    let key_file = std::fs::read(&tls_config.key_file)
        .context("Failed to read key file")?;

    let certs = rustls_pemfile::certs(&mut std::io::BufReader::new(&cert_file[..]))
        .collect::<Result<Vec<_>, _>>()?;
    let mut keys = rustls_pemfile::pkcs8_private_keys(&mut std::io::BufReader::new(&key_file[..]))
        .collect::<Result<Vec<_>, _>>()?;

    if keys.is_empty() {
        keys = rustls_pemfile::rsa_private_keys(&mut std::io::BufReader::new(&key_file[..]))
            .collect::<Result<Vec<_>, _>>()?;
    }

    if keys.is_empty() {
        anyhow::bail!("No private key found");
    }

    let mut config = RustlsServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, keys.remove(0))?;

    config.alpn_protocols = tls_config
        .alpn_protocols
        .iter()
        .map(|p| p.as_bytes().to_vec())
        .collect();

    Ok(config)
}

/// Build ACME config for Let's Encrypt (placeholder - not implemented).
async fn build_acme_config(config: &GatewayConfig) -> Result<Option<()>> {
    if config.server.tls.is_none() {
        return Ok(None);
    }

    // ACME setup would go here for automatic cert management
    // For now, return None
    Ok(None)
}

// ============================================================================
// Main Application
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    // Load configuration
    let config: GatewayConfig = loader::load()?;

    // Initialize tracing
    let _tracing = mediaedge_observability::tracing::init();
    info!("Starting mediaedge-gateway");

    // Initialize metrics
    let metrics = Arc::new(MetricsRegistry::new());

    // Initialize cluster manager
    let core_config = mediaedge_core::CoreConfig {
        server: config.server.clone(),
    };
    let core_service = mediaedge_core::CoreService::new(core_config, metrics.clone());
    core_service.start().await?;
    let cluster = core_service.cluster();

    // Create gateway state
    let (state, reload_rx) = GatewayState::new(config.clone(), cluster, metrics.clone()).await?;

    // Start config reload watcher
    let state_clone = state.clone();
    tokio::spawn(config_reload_watcher(reload_rx, state_clone));

    // Setup signal handlers
    setup_signal_handlers(state.reload_tx.clone());

    // Build HTTP router
    let app = build_router(state.clone()).await;

    // Build TLS config if needed
    let tls_config = config.server.tls.as_ref().map(build_tls_config).transpose()?;

    // Start HTTP server
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    info!("HTTP server listening on {}", addr);

    let handle = Handle::new();
    let server = if let Some(tls_config) = tls_config {
        axum_server::bind_rustls(addr, tls_config)
            .handle(handle)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
    } else {
        axum_server::bind(addr)
            .handle(handle)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
    };

    // Start protocol servers in background
    let state_rtmp = state.clone();
    let config_rtmp = config.clone();
    tokio::spawn(async move {
        if let Err(e) = rtmp_server(config_rtmp, state_rtmp).await {
            error!("RTMP server error: {}", e);
        }
    });

    let state_srt = state.clone();
    let config_srt = config.clone();
    tokio::spawn(async move {
        if let Err(e) = srt_server(config_srt, state_srt).await {
            error!("SRT server error: {}", e);
        }
    });

    let state_rist = state.clone();
    let config_rist = config.clone();
    tokio::spawn(async move {
        if let Err(e) = rist_server(config_rist, state_rist).await {
            error!("RIST server error: {}", e);
        }
    });

    // Wait for server
    server.await?;

    // Shutdown
    info!("Shutting down gateway");
    core_service.stop().await?;

    Ok(())
}

/// Build the HTTP router with all routes and middleware.
async fn build_router(state: GatewayState) -> Router {
    let config = state.config();

    // OpenAPI config
    let openapi_config = OpenApiConfig {
        service_name: "MediaEdge Gateway".to_string(),
        version: "1.0.0".to_string(),
        description: "MediaEdge Gateway API".to_string(),
        servers: vec![],
        swagger_ui: true,
        redoc: true,
        rapidoc: true,
        swagger_path: "/api/docs/swagger".to_string(),
        redoc_path: "/api/docs/redoc".to_string(),
        rapidoc_path: "/api/docs/rapidoc".to_string(),
    };

    // Middleware stack
    let middleware_stack = ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any))
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit_middleware))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    // Admin routes
    let admin_routes = Router::new()
        .route("/health", get(health_check))
        .route("/cluster/nodes", get(get_cluster_nodes))
        .route("/cluster/ownership", get(get_broadcast_ownership))
        .route("/cluster/capacity", get(get_cluster_capacity))
        .route("/config/reload", post(config_reload))
        .route("/config/version", get(config_version))
        .layer(middleware::from_fn_with_state(state.clone(), admin_auth_middleware));

    // Stream management routes
    let stream_routes = Router::new()
        .route("/streams", post(create_stream).get(list_streams))
        .route("/streams/:stream_id", get(get_stream).put(update_stream).delete(delete_stream))
        .route("/streams/:stream_id/ingest", get(get_ingest_info))
        .route("/streams/:stream_id/egress", get(get_egress_info))
        .route("/streams/:stream_id/signed-url", post(create_signed_url));

    // WHIP endpoint
    let whip_path = config.protocols.whip.as_ref().map(|c| c.path.as_str()).unwrap_or("/whip");
    let whip_routes = Router::new()
        .route(whip_path, any(whip_handler));

    // WebSocket upgrade routes
    let ws_routes = Router::new()
        .route("/whip/:stream_id", get(handle_websocket))
        .route("/whep/:stream_id", get(handle_websocket))
        .route("/moq/:stream_id", get(handle_websocket));

    // OpenAPI docs
    let docs_router = create_openapi_router(&openapi_config);

    // Combine all routes
    let app = Router::new()
        .nest("/api/v1/admin", admin_routes)
        .nest("/api/v1", stream_routes)
        .merge(whip_routes)
        .merge(ws_routes)
        .merge(docs_router)
        .fallback(route_request)
        .layer(middleware_stack)
        .with_state(state);

    app
}

/// Authentication middleware - extracts auth context and adds to request extensions.
async fn auth_middleware(
    State(state): State<GatewayState>,
    mut request: Request,
    next: Next,
) -> Response {
    let headers = request.headers().clone();
    let addr = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0)
        .unwrap_or_else(|| "0.0.0.0:0".parse().unwrap());

    // Skip auth for health, metrics, docs
    let path = request.uri().path();
    if path == "/health" || path == "/metrics" || path.starts_with("/api/docs") || path == "/api/openapi.json" {
        return next.run(request).await;
    }

    match extract_auth(state, headers, ConnectInfo(addr)).await {
        Ok(auth) => {
            request.extensions_mut().insert(auth);
            next.run(request).await
        }
        Err(status) => status.into_response(),
    }
}

/// Admin authentication middleware - requires admin role.
async fn admin_auth_middleware(
    State(state): State<GatewayState>,
    mut request: Request,
    next: Next,
) -> Response {
    let headers = request.headers().clone();
    let addr = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0)
        .unwrap_or_else(|| "0.0.0.0:0".parse().unwrap());

    let config = state.config();

    // Check admin token
    if let Some(admin_token) = &config.admin.token {
        if let Some(auth_header) = headers.get("authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str == format!("Bearer {}", admin_token) {
                    return next.run(request).await;
                }
            }
        }
    }

    // Check allowed IPs
    if let Some(connect_info) = request.extensions().get::<ConnectInfo<SocketAddr>>() {
        let ip = connect_info.0.ip();
        for cidr in &config.admin.allowed_ips {
            if cidr_contains(cidr, ip) {
                return next.run(request).await;
            }
        }
    }

    // Fall back to regular auth with admin role check
    match extract_auth(state, headers, ConnectInfo(addr)).await {
        Ok(auth) => {
            if auth.roles.contains(&"admin".to_string()) {
                request.extensions_mut().insert(auth);
                next.run(request).await
            } else {
                StatusCode::FORBIDDEN.into_response()
            }
        }
        Err(status) => status.into_response(),
    }
}

/// Check if CIDR contains IP.
fn cidr_contains(cidr: &str, ip: std::net::IpAddr) -> bool {
    if let Ok(network) = cidr.parse::<ipnet::IpNet>() {
        network.contains(ip)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_stream_id() {
        assert_eq!(extract_stream_id("/api/v1/streams/abc123/egress"), Some("abc123".to_string()));
        assert_eq!(extract_stream_id("/hls/stream1/playlist.m3u8"), Some("stream1".to_string()));
        assert_eq!(extract_stream_id("/dash/live/manifest.mpd"), Some("live".to_string()));
        assert_eq!(extract_stream_id("/health"), None);
    }

    #[test]
    fn test_rate_limit_key() {
        let auth = Some(AuthContext {
            tenant_id: "tenant1".to_string(),
            user_id: None,
            roles: vec![],
            permissions: vec![],
            stream_access: None,
        });
        let headers = HeaderMap::new();
        let addr: SocketAddr = "192.168.1.1:8080".parse().unwrap();

        assert_eq!(rate_limit_key(&auth, &headers, addr, Some("stream1")), "tenant:tenant1:stream:stream1");
        assert_eq!(rate_limit_key(&auth, &headers, addr, None), "tenant:tenant1");

        let auth_none: Option<AuthContext> = None;
        assert_eq!(rate_limit_key(&auth_none, &headers, addr, None), "ip:192.168.1.1");
    }
}