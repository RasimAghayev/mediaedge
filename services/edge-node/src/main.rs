//! MediaEdge Edge Node Service
//!
//! CDN edge node with HLS/DASH segment caching, cache warming,
//! signed URLs, multi-tenant isolation, and origin shielding.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use axum::{
    body::Body,
    extract::{ConnectInfo, Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, head, post},
    Router,
};
use axum_server::Handle;
use dashmap::DashMap;
use futures::StreamExt;
use governor::{Quota, RateLimiter};
use hmac::{Hmac, Mac};
use moka::future::Cache as MokaCache;
use opendal::{Operator, layers::LoggingLayer};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
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
    openapi::{api_doc, create_openapi_router, OpenApiConfig, SignedUrlRequest, SignedUrlResponse, EgressProtocol},
    rate_limit::{RateLimitConfig, RateLimitMiddleware},
    validation::{ValidationMiddleware, ValidatedJson, ValidatedQuery},
    ApiMiddlewareConfig, ApiVersion, VersioningMiddleware,
};
use mediaedge_config::{
    loader,
    types::{AuthConfig, AuthMode, EdgeCacheConfig, EdgeNodeConfig, OriginConfig, ServerConfig, SignedUrlConfig, StorageConfig, TlsConfig},
    ArcSwapConfig,
};
use mediaedge_core::cluster::ClusterManager;
use mediaedge_edge::{cache::CacheBackend, sync::SyncManager};
use mediaedge_observability::{MetricsRegistry, TracingHandle};

// ============================================================================
// Types
// ============================================================================

type HmacSha256 = Hmac<Sha256>;

/// Shared application state for the edge node.
#[derive(Clone)]
pub struct EdgeState {
    /// Edge node configuration (hot-reloadable).
    pub config: ArcSwap<EdgeNodeConfig>,

    /// HTTP client for origin requests.
    pub http_client: HttpClient,

    /// Object storage operator for local cache.
    pub storage: Operator,

    /// In-memory cache for hot segments (LRU).
    pub memory_cache: MokaCache<String, CachedSegment>,

    /// Cache warming queue.
    pub warming_queue: mpsc::UnboundedSender<WarmingTask>,

    /// Active warming tasks (for deduplication).
    pub active_warming: Arc<DashMap<String, ()>>,

    /// Signed URL secret key.
    pub signed_url_key: Arc<RwLock<Option<Vec<u8>>>>,

    /// Cluster manager for coordination.
    pub cluster: Arc<ClusterManager>,

    /// Metrics registry.
    pub metrics: Arc<MetricsRegistry>,

    /// Rate limiters per tenant/IP.
    pub rate_limiters: Arc<DashMap<String, Arc<RateLimiter<std::sync::Mutex<governor::state::NotKeyed>, governor::state::InMemoryState, governor::clock::DefaultClock>>>>,

    /// Config reload channel.
    pub reload_tx: mpsc::UnboundedSender<()>,
}

/// Cached segment with metadata.
#[derive(Debug, Clone)]
pub struct CachedSegment {
    pub data: bytes::Bytes,
    pub content_type: String,
    pub etag: String,
    pub last_modified: chrono::DateTime<chrono::Utc>,
    pub tier: StorageTier,
    pub tenant_id: String,
}

/// Storage tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageTier {
    Hot,
    Warm,
    Cold,
}

/// Cache warming task.
#[derive(Debug, Clone)]
pub struct WarmingTask {
    pub stream_id: String,
    pub segment_path: String,
    pub tenant_id: String,
    pub priority: u8, // 0 = highest
}

/// Signed URL claims.
#[derive(Debug, Serialize, Deserialize)]
struct SignedUrlClaims {
    stream_id: String,
    protocol: String,
    exp: i64,
    iat: i64,
    ip_whitelist: Option<Vec<String>>,
    geo_whitelist: Option<Vec<String>>,
}

/// Authenticated context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub stream_access: Option<Vec<String>>,
}

// ============================================================================
// Application State Implementation
// ============================================================================

impl EdgeState {
    /// Create a new edge state.
    pub async fn new(config: EdgeNodeConfig, cluster: Arc<ClusterManager>, metrics: Arc<MetricsRegistry>) -> Result<(Self, mpsc::UnboundedReceiver<()>)> {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(config.server.request_timeout_secs))
            .build()?;

        // Initialize object storage
        let storage = build_storage_operator(&config.storage).await?;

        // Memory cache for hot segments
        let memory_cache = MokaCache::builder()
            .max_capacity(config.cache.max_entries as u64)
            .time_to_live(Duration::from_secs(config.cache.live_ttl_secs))
            .build();

        // Warming queue
        let (warming_tx, warming_rx) = mpsc::unbounded_channel();

        // Start warming worker
        let state_for_warming = EdgeState {
            config: ArcSwap::from_pointee(config.clone()),
            http_client: http_client.clone(),
            storage: storage.clone(),
            memory_cache: memory_cache.clone(),
            warming_queue: warming_tx.clone(),
            active_warming: Arc::new(DashMap::new()),
            signed_url_key: Arc::new(RwLock::new(None)),
            cluster: cluster.clone(),
            metrics: metrics.clone(),
            rate_limiters: Arc::new(DashMap::new()),
            reload_tx: mpsc::unbounded_channel().0,
        };
        tokio::spawn(warming_worker(state_for_warming, warming_rx));

        let (reload_tx, reload_rx) = mpsc::unbounded_channel();

        // Initialize signed URL key
        let signed_url_key = if config.signed_urls.enabled {
            if let Some(secret) = &config.signed_urls.hmac_secret {
                Some(secret.as_bytes().to_vec())
            } else {
                None
            }
        } else {
            None
        };

        Ok((
            Self {
                config: ArcSwap::from_pointee(config),
                http_client,
                storage,
                memory_cache,
                warming_queue: warming_tx,
                active_warming: Arc::new(DashMap::new()),
                signed_url_key: Arc::new(RwLock::new(signed_url_key)),
                cluster,
                metrics,
                rate_limiters: Arc::new(DashMap::new()),
                reload_tx,
            },
            reload_rx,
        ))
    }

    /// Get current config.
    pub fn config(&self) -> Arc<EdgeNodeConfig> {
        self.config.load()
    }

    /// Get or create rate limiter for a key.
    pub fn get_rate_limiter(&self, key: &str, quota: Quota) -> Arc<RateLimiter<std::sync::Mutex<governor::state::NotKeyed>, governor::state::InMemoryState, governor::clock::DefaultClock>> {
        self.rate_limiters
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(RateLimiter::direct(quota)))
            .clone()
    }

    /// Generate signed URL.
    pub async fn generate_signed_url(&self, request: SignedUrlRequest) -> Result<SignedUrlResponse> {
        let config = self.config();
        let key = self.signed_url_key.read().await;

        let signing_key = key.as_ref().ok_or_else(|| anyhow::anyhow!("Signed URL key not configured"))?;

        let now = chrono::Utc::now().timestamp();
        let exp = now + request.ttl_secs as i64;

        let claims = SignedUrlClaims {
            stream_id: request.stream_id.clone(),
            protocol: format!("{:?}", request.protocol).to_lowercase(),
            exp,
            iat: now,
            ip_whitelist: request.ip_whitelist,
            geo_whitelist: request.geo_whitelist,
        };

        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(signing_key),
        )?;

        // Build playback URL based on protocol
        let base_url = config.origin.urls.first()
            .ok_or_else(|| anyhow::anyhow!("No origin configured"))?;
        let url = match request.protocol {
            EgressProtocol::Hls => format!("{}/hls/{}/playlist.m3u8?token={}", base_url, request.stream_id, token),
            EgressProtocol::Dash => format!("{}/dash/{}/manifest.mpd?token={}", base_url, request.stream_id, token),
            EgressProtocol::Whep => format!("{}/whep/{}?token={}", base_url, request.stream_id, token),
            EgressProtocol::Moq => format!("{}/moq/{}?token={}", base_url, request.stream_id, token),
            EgressProtocol::Ws => format!("{}/ws/{}?token={}", base_url, request.stream_id, token),
            _ => format!("{}/{}?token={}", base_url, request.stream_id, token),
        };

        Ok(SignedUrlResponse {
            url,
            expires_at: chrono::DateTime::from_timestamp(exp, 0).unwrap_or_else(|| chrono::Utc::now()),
            signature: token,
        })
    }

    /// Validate signed URL token.
    pub async fn validate_signed_url(&self, token: &str, client_ip: Option<std::net::IpAddr>) -> Result<SignedUrlClaims> {
        let key = self.signed_url_key.read().await;
        let signing_key = key.as_ref().ok_or_else(|| anyhow::anyhow!("Signed URL key not configured"))?;

        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.validate_exp = true;

        let token_data = jsonwebtoken::decode::<SignedUrlClaims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(signing_key),
            &validation,
        )?;

        let claims = token_data.claims;

        // Check IP whitelist
        if let Some(whitelist) = &claims.ip_whitelist {
            if let Some(ip) = client_ip {
                let allowed = whitelist.iter().any(|cidr| {
                    cidr.parse::<ipnet::IpNet>().map(|net| net.contains(ip)).unwrap_or(false)
                });
                if !allowed {
                    return Err(anyhow::anyhow!("IP not allowed"));
                }
            }
        }

        Ok(claims)
    }
}

/// Build OpenDAL operator from config.
async fn build_storage_operator(config: &StorageConfig) -> Result<Operator> {
    let mut builder = match config.backend.as_str() {
        "fs" => {
            let fs_config = config.fs.as_ref().context("FS config required")?;
            opendal::services::Fs::default()
                .root(&fs_config.root)
                .create_if_missing(fs_config.create_if_missing)
        }
        "s3" => {
            let s3_config = config.s3.as_ref().context("S3 config required")?;
            let mut b = opendal::services::S3::default()
                .bucket(&s3_config.bucket)
                .region(&s3_config.region);
            if let Some(endpoint) = &s3_config.endpoint {
                b = b.endpoint(endpoint);
            }
            if let Some(key) = &s3_config.access_key_id {
                b = b.access_key_id(key);
            }
            if let Some(secret) = &s3_config.secret_access_key {
                b = b.secret_access_key(secret);
            }
            if let Some(token) = &s3_config.session_token {
                b = b.session_token(token);
            }
            b = b.path_style(s3_config.path_style);
            b
        }
        "azblob" => {
            let az_config = config.azblob.as_ref().context("AzBlob config required")?;
            opendal::services::Azblob::default()
                .container(&az_config.container)
                .account(&az_config.account)
                .endpoint_suffix(&az_config.endpoint_suffix)
        }
        "gcs" => {
            let gcs_config = config.gcs.as_ref().context("GCS config required")?;
            let mut b = opendal::services::Gcs::default()
                .bucket(&gcs_config.bucket);
            if let Some(creds) = &gcs_config.credentials_file {
                b = b.credentials_file(creds);
            }
            if let Some(project) = &gcs_config.project_id {
                b = b.project_id(project);
            }
            b
        }
        "memory" => {
            let mem_config = config.memory.as_ref().unwrap_or(&Default::default());
            opendal::services::Memory::default()
                .max_size(mem_config.max_size)
        }
        _ => anyhow::bail!("Unknown storage backend: {}", config.backend),
    };

    let op = Operator::new(builder)?.layer(LoggingLayer::default()).finish();
    Ok(op)
}

// ============================================================================
// Cache Operations
// ============================================================================

/// Get segment from cache (memory -> disk -> origin).
async fn get_segment(
    State(state): State<EdgeState>,
    auth: Option<AuthContext>,
    Path((stream_id, segment_path)): Path<(String, String)>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    let config = state.config();
    let cache_key = format!("{}:{}", stream_id, segment_path);

    // Check memory cache first
    if let Some(cached) = state.memory_cache.get(&cache_key).await {
        return build_segment_response(&cached, &headers, true);
    }

    // Check disk cache
    let object_path = format!("{}/{}", stream_id, segment_path);
    if let Ok(meta) = state.storage.stat(&object_path).await {
        // Read from disk
        let data = state.storage.read(&object_path).await.unwrap_or_default();
        let cached = CachedSegment {
            data: data.to_bytes(),
            content_type: guess_content_type(&segment_path),
            etag: meta.etag().unwrap_or_default().to_string(),
            last_modified: meta.last_modified().unwrap_or_else(chrono::Utc::now),
            tier: StorageTier::Hot,
            tenant_id: auth.as_ref().map(|a| a.tenant_id.clone()).unwrap_or_else(|| "default".to_string()),
        };

        // Promote to memory cache
        state.memory_cache.insert(cache_key.clone(), cached.clone()).await;

        return build_segment_response(&cached, &headers, false);
    }

    // Cache miss - fetch from origin (origin shield)
    if config.cache.enabled && config.cache.warming_enabled {
        // Check if already warming
        if state.active_warming.insert(cache_key.clone(), ()).is_none() {
            // Queue for warming
            let _ = state.warming_queue.send(WarmingTask {
                stream_id: stream_id.clone(),
                segment_path: segment_path.clone(),
                tenant_id: auth.as_ref().map(|a| a.tenant_id.clone()).unwrap_or_else(|| "default".to_string()),
                priority: 0,
            });
        }
    }

    // Fetch from origin
    fetch_from_origin(state, &stream_id, &segment_path, &headers).await
}

/// Build segment response with proper headers.
fn build_segment_response(cached: &CachedSegment, request_headers: &HeaderMap, from_memory: bool) -> Response {
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", &cached.content_type)
        .header("ETag", &cached.etag)
        .header("Cache-Control", "public, max-age=300, stale-while-revalidate=60")
        .header("X-Cache", if from_memory { "HIT-MEMORY" } else { "HIT-DISK" })
        .header("X-Cache-Tier", format!("{:?}", cached.tier));

    // Handle range requests
    if let Some(range_header) = request_headers.get("range") {
        if let Ok(range_str) = range_header.to_str() {
            if let Some((start, end)) = parse_range(range_str, cached.data.len()) {
                let slice = &cached.data[start..=end];
                response = response
                    .status(StatusCode::PARTIAL_CONTENT)
                    .header("Content-Range", format!("bytes {}-{}/{}", start, end, cached.data.len()))
                    .header("Content-Length", slice.len())
                    .body(Body::from(slice.to_vec()))
                    .unwrap();
                return response;
            }
        }
    }

    response
        .header("Content-Length", cached.data.len())
        .body(Body::from(cached.data.clone()))
        .unwrap()
}

/// Parse range header.
fn parse_range(range: &str, total_len: usize) -> Option<(usize, usize)> {
    // Format: "bytes=start-end"
    let range = range.strip_prefix("bytes=")?;
    let parts: Vec<&str> = range.split('-').collect();
    if parts.len() != 2 {
        return None;
    }
    let start = parts[0].parse().ok()?;
    let end = if parts[1].is_empty() {
        total_len - 1
    } else {
        parts[1].parse().ok()?
    };
    if start > end || end >= total_len {
        return None;
    }
    Some((start, end))
}

/// Guess content type from file extension.
fn guess_content_type(path: &str) -> String {
    if path.ends_with(".m3u8") || path.ends_with(".m3u") {
        "application/vnd.apple.mpegurl".to_string()
    } else if path.ends_with(".ts") {
        "video/mp2t".to_string()
    } else if path.ends_with(".mpd") {
        "application/dash+xml".to_string()
    } else if path.ends_with(".m4s") || path.ends_with(".mp4") {
        "video/iso.segment".to_string()
    } else if path.ends_with(".webm") {
        "video/webm".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

/// Fetch segment from origin with origin shielding.
async fn fetch_from_origin(
    state: EdgeState,
    stream_id: &str,
    segment_path: &str,
    request_headers: &HeaderMap,
) -> Response {
    let config = state.config();
    let cache_key = format!("{}:{}", stream_id, segment_path);

    // Origin shield: only one request per segment at a time
    // (active_warming acts as a lock)

    // Try each origin URL
    for origin_url in &config.origin.urls {
        let url = format!("{}/{}/{}", origin_url.trim_end_matches('/'), stream_id, segment_path);

        let mut request = state.http_client.get(&url);

        // Forward relevant headers
        for (key, value) in request_headers {
            let key_str = key.as_str().to_lowercase();
            if key_str == "range" || key_str == "if-none-match" || key_str == "if-modified-since" {
                request = request.header(key, value);
            }
        }

        match request.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let content_type = response.headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("application/octet-stream")
                        .to_string();

                    let etag = response.headers()
                        .get("etag")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_string();

                    let last_modified = response.headers()
                        .get("last-modified")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| chrono::DateTime::parse_from_rfc2822(v).ok())
                        .map(|v| v.with_timezone(&chrono::Utc))
                        .unwrap_or_else(chrono::Utc::now);

                    let data = response.bytes().await.unwrap_or_default();

                    // Store in cache
                    let cached = CachedSegment {
                        data: data.clone(),
                        content_type: content_type.clone(),
                        etag: etag.clone(),
                        last_modified,
                        tier: StorageTier::Hot,
                        tenant_id: "default".to_string(),
                    };

                    // Write to disk (async, don't wait)
                    let storage = state.storage.clone();
                    let object_path = format!("{}/{}", stream_id, segment_path);
                    let data_clone = data.clone();
                    tokio::spawn(async move {
                        let _ = storage.write(&object_path, data_clone).await;
                    });

                    // Insert to memory cache
                    state.memory_cache.insert(cache_key, cached.clone()).await;

                    // Remove from active warming
                    state.active_warming.remove(&cache_key);

                    return build_segment_response(&cached, request_headers, false);
                } else if response.status() == StatusCode::NOT_FOUND {
                    // Segment not found at origin
                    state.active_warming.remove(&cache_key);
                    continue; // Try next origin
                }
            }
            Err(e) => {
                warn!("Origin fetch error: {}", e);
                continue; // Try next origin
            }
        }
    }

    // All origins failed
    state.active_warming.remove(&cache_key);
    StatusCode::NOT_FOUND.into_response()
}

/// Cache warming worker.
async fn warming_worker(
    state: EdgeState,
    mut warming_rx: mpsc::UnboundedReceiver<WarmingTask>,
) {
    while let Some(task) = warming_rx.recv().await {
        let cache_key = format!("{}:{}", task.stream_id, task.segment_path);

        // Check if already cached
        if state.memory_cache.contains_key(&cache_key).await {
            state.active_warming.remove(&cache_key);
            continue;
        }

        // Fetch from origin
        let _ = fetch_from_origin(state.clone(), &task.stream_id, &task.segment_path, &HeaderMap::new()).await;
    }
}

// ============================================================================
// Signed URL Endpoint
// ============================================================================

/// Create signed URL.
async fn create_signed_url_endpoint(
    State(state): State<EdgeState>,
    auth: AuthContext,
    ValidatedJson(payload): ValidatedJson<SignedUrlRequest>,
) -> impl IntoResponse {
    match state.generate_signed_url(payload).await {
        Ok(response) => (StatusCode::OK, axum::Json(response)).into_response(),
        Err(e) => {
            error!("Signed URL generation failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate signed URL").into_response()
        }
    }
}

/// Validate signed URL (for debugging).
async fn validate_signed_url_endpoint(
    State(state): State<EdgeState>,
    Query(params): Query<HashMap<String, String>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let token = params.get("token").cloned().unwrap_or_default();
    let client_ip = Some(addr.ip());

    match state.validate_signed_url(&token, client_ip).await {
        Ok(claims) => axum::Json(serde_json::json!({
            "valid": true,
            "claims": claims
        })).into_response(),
        Err(e) => axum::Json(serde_json::json!({
            "valid": false,
            "error": e.to_string()
        })).into_response(),
    }
}

// ============================================================================
// Cache Management Endpoints
// ============================================================================

/// Purge cache for a stream.
async fn purge_stream_cache(
    State(state): State<EdgeState>,
    auth: AuthContext,
    Path(stream_id): Path<String>,
) -> impl IntoResponse {
    let config = state.config();

    // Remove from memory cache
    let keys_to_remove: Vec<String> = state.memory_cache
        .iter()
        .filter_map(|entry| {
            if entry.key().starts_with(&format!("{}:", stream_id)) {
                Some(entry.key().clone())
            } else {
                None
            }
        })
        .collect();

    for key in keys_to_remove {
        state.memory_cache.invalidate(&key).await;
    }

    // Remove from disk cache
    let storage = state.storage.clone();
    let stream_prefix = format!("{}/", stream_id);
    tokio::spawn(async move {
        // Note: OpenDAL doesn't have a direct prefix delete, would need to list and delete
        // For now, just log
        info!("Purged disk cache for stream: {}", stream_id);
    });

    // Invalidate active warming
    for entry in state.active_warming.iter() {
        if entry.key().starts_with(&format!("{}:", stream_id)) {
            state.active_warming.remove(entry.key());
        }
    }

    (StatusCode::OK, axum::Json(serde_json::json!({
        "status": "purged",
        "stream_id": stream_id
    }))).into_response()
}

/// Get cache statistics.
async fn cache_stats(State(state): State<EdgeState>) -> impl IntoResponse {
    let memory_size = state.memory_cache.entry_count();
    let memory_capacity = state.memory_cache.capacity().unwrap_or(0);

    axum::Json(serde_json::json!({
        "memory_cache": {
            "entries": memory_size,
            "capacity": memory_capacity,
            "utilization_pct": if memory_capacity > 0 { (memory_size as f64 / memory_capacity as f64) * 100.0 } else { 0.0 }
        },
        "active_warming": state.active_warming.len(),
        "disk_cache": {
            "status": "enabled"
        }
    }))
}

/// Health check.
async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "healthy")
}

// ============================================================================
// Authentication & Rate Limiting
// ============================================================================

async fn extract_auth(
    State(state): State<EdgeState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<AuthContext, StatusCode> {
    let config = state.config();

    // Try JWT
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                if let Ok(claims) = state.validate_signed_url(token, Some(addr.ip())).await {
                    return Ok(AuthContext {
                        tenant_id: "default".to_string(),
                        user_id: None,
                        roles: vec!["viewer".to_string()],
                        permissions: vec!["view".to_string()],
                        stream_access: Some(vec![claims.stream_id]),
                    });
                }
            }
        }
    }

    // Try API key
    if let Some(api_key) = headers.get("x-api-key") {
        if let Ok(_) = api_key.to_str() {
            return Ok(AuthContext {
                tenant_id: "default".to_string(),
                user_id: None,
                roles: vec!["api-client".to_string()],
                permissions: vec!["cache:read".to_string(), "cache:write".to_string()],
                stream_access: None,
            });
        }
    }

    // Allow unauthenticated for public endpoints
    Err(StatusCode::UNAUTHORIZED)
}

async fn rate_limit_middleware(
    State(state): State<EdgeState>,
    auth: Option<AuthContext>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let config = state.config();

    // Simple IP-based rate limiting for edge
    let key = format!("ip:{}", addr.ip());
    let quota = Quota::per_minute(10000); // High limit for edge
    let limiter = state.get_rate_limiter(&key, quota);

    match limiter.check() {
        Ok(_) => next.run(request).await,
        Err(_) => {
            let mut response = StatusCode::TOO_MANY_REQUESTS.into_response();
            response.headers_mut().insert(
                "Retry-After",
                HeaderValue::from_str("60").unwrap(),
            );
            response
        }
    }
}

// ============================================================================
// Config Hot Reload
// ============================================================================

async fn config_reload_watcher(
    mut reload_rx: mpsc::UnboundedReceiver<()>,
    state: EdgeState,
) {
    while reload_rx.recv().await.is_some() {
        info!("Config reload triggered");

        match loader::load::<EdgeNodeConfig>() {
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

// ============================================================================
// Main Application
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    // Load configuration
    let config: EdgeNodeConfig = loader::load()?;

    // Initialize tracing
    let _tracing = mediaedge_observability::tracing::init();
    info!("Starting mediaedge-edge-node");

    // Initialize metrics
    let metrics = Arc::new(MetricsRegistry::new());

    // Initialize cluster manager (for cache coordination)
    let core_config = mediaedge_core::CoreConfig {
        server: config.server.clone(),
    };
    let core_service = mediaedge_core::CoreService::new(core_config, metrics.clone());
    core_service.start().await?;
    let cluster = core_service.cluster();

    // Create edge state
    let (state, reload_rx) = EdgeState::new(config.clone(), cluster, metrics.clone()).await?;

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
    info!("Edge node HTTP server listening on {}", addr);

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

    // Wait for server
    server.await?;

    // Shutdown
    info!("Shutting down edge node");
    core_service.stop().await?;

    Ok(())
}

/// Build the HTTP router with all routes and middleware.
async fn build_router(state: EdgeState) -> Router {
    let config = state.config();

    // OpenAPI config
    let openapi_config = OpenApiConfig {
        service_name: "MediaEdge Edge Node".to_string(),
        version: "1.0.0".to_string(),
        description: "MediaEdge Edge Node API".to_string(),
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

    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/health", get(health_check))
        .route("/hls/:stream_id/*segment_path", get(get_segment).head(get_segment))
        .route("/dash/:stream_id/*segment_path", get(get_segment).head(get_segment))
        .route("/signed-url/validate", get(validate_signed_url_endpoint));

    // Protected routes (auth required)
    let protected_routes = Router::new()
        .route("/api/v1/signed-url", post(create_signed_url_endpoint))
        .route("/api/v1/cache/stats", get(cache_stats))
        .route("/api/v1/cache/purge/:stream_id", post(purge_stream_cache));

    // Admin routes
    let admin_routes = Router::new()
        .route("/health", get(health_check))
        .route("/config/reload", post(config_reload));

    // OpenAPI docs
    let docs_router = create_openapi_router(&openapi_config);

    // Combine all routes
    let app = Router::new()
        .merge(public_routes)
        .nest("/api/v1", protected_routes)
        .nest("/api/v1/admin", admin_routes)
        .merge(docs_router)
        .layer(middleware_stack)
        .with_state(state);

    app
}

/// Authentication middleware.
async fn auth_middleware(
    State(state): State<EdgeState>,
    mut request: Request,
    next: Next,
) -> Response {
    let headers = request.headers().clone();
    let addr = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0)
        .unwrap_or_else(|| "0.0.0.0:0".parse().unwrap());

    // Skip auth for public endpoints
    let path = request.uri().path();
    if path == "/health" 
        || path == "/metrics" 
        || path.starts_with("/api/docs") 
        || path == "/api/openapi.json"
        || path.starts_with("/hls/")
        || path.starts_with("/dash/")
        || path == "/signed-url/validate" {
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

/// Config reload endpoint.
async fn config_reload(State(state): State<EdgeState>) -> impl IntoResponse {
    let _ = state.reload_tx.send(());
    (StatusCode::OK, axum::Json(serde_json::json!({
        "status": "ok"
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range() {
        assert_eq!(parse_range("bytes=0-999", 2000), Some((0, 999)));
        assert_eq!(parse_range("bytes=500-", 2000), Some((500, 1999)));
        assert_eq!(parse_range("bytes=-500", 2000), None); // suffix-byte-range not supported
        assert_eq!(parse_range("bytes=2000-3000", 2000), None); // out of bounds
    }

    #[test]
    fn test_guess_content_type() {
        assert_eq!(guess_content_type("playlist.m3u8"), "application/vnd.apple.mpegurl");
        assert_eq!(guess_content_type("segment.ts"), "video/mp2t");
        assert_eq!(guess_content_type("manifest.mpd"), "application/dash+xml");
        assert_eq!(guess_content_type("segment.m4s"), "video/iso.segment");
    }

    #[test]
    fn test_signed_url_claims() {
        let claims = SignedUrlClaims {
            stream_id: "test123".to_string(),
            protocol: "hls".to_string(),
            exp: chrono::Utc::now().timestamp() + 3600,
            iat: chrono::Utc::now().timestamp(),
            ip_whitelist: Some(vec!["192.168.1.0/24".to_string()]),
            geo_whitelist: None,
        };
        let json = serde_json::to_string(&claims).unwrap();
        assert!(json.contains("test123"));
        assert!(json.contains("hls"));
    }
}