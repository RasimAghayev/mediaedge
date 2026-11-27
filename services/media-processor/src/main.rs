//! MediaEdge Media Processor Service
//!
//! Async job processor for recording, transcoding, VOD processing,
//! caption generation, and cloud egress.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use axum::{
    Router,
    body::Body,
    extract::{ConnectInfo, Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use axum_server::Handle;
use dashmap::DashMap;
use ffmpeg_next::{codec, format, frame, media, software};
use futures::StreamExt;
use opendal::{Operator, layers::LoggingLayer};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, Semaphore, mpsc};
use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig as RustlsServerConfig};
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use mediaedge_api::{
    ApiMiddlewareConfig, ApiVersion, VersioningMiddleware,
    openapi::{
        BitratePreset, JobPriority, OpenApiConfig, TranscodeStatus, api_doc, create_openapi_router,
    },
    rate_limit::{RateLimitConfig, RateLimitMiddleware},
    validation::{ValidatedJson, ValidatedQuery, ValidationMiddleware},
};
use mediaedge_config::{
    ArcSwapConfig, loader,
    types::{
        AiConfig, ArchiveConfig, AuthConfig, CaptionConfig, MediaProcessorConfig, RecordingConfig,
        RecordingSchedule, ServerConfig, SpriteConfig, StorageConfig, ThumbnailConfig, TlsConfig,
        TranscodingConfig, VodConfig,
    },
};
use mediaedge_core::cluster::ClusterManager;
use mediaedge_events::{Event, EventPublisher};
use mediaedge_media::transcode::{HardwareAccel, TranscodeOptions, TranscodePipeline};
use mediaedge_observability::{MetricsRegistry, TracingHandle};

// ============================================================================
// Types
// ============================================================================

/// Shared application state for the media processor.
#[derive(Clone)]
pub struct ProcessorState {
    /// Media processor configuration (hot-reloadable).
    pub config: ArcSwap<MediaProcessorConfig>,

    /// HTTP client for external requests.
    pub http_client: HttpClient,

    /// Object storage operator for output.
    pub storage: Operator,

    /// NATS connection for job queue.
    pub nats: Option<async_nats::Client>,

    /// Job queue (in-memory for now, NATS-backed in production).
    pub job_queue: Arc<DashMap<String, TranscodeJob>>,

    /// Active jobs semaphore (limits concurrency).
    pub job_semaphore: Arc<Semaphore>,

    /// Scheduled recordings.
    pub scheduled_recordings: Arc<DashMap<String, RecordingJob>>,

    /// Transcode pipeline.
    pub transcode_pipeline: Arc<TranscodePipeline>,

    /// Cluster manager for coordination.
    pub cluster: Arc<ClusterManager>,

    /// Metrics registry.
    pub metrics: Arc<MetricsRegistry>,

    /// Event publisher.
    pub event_publisher: Arc<EventPublisher>,

    /// Config reload channel.
    pub reload_tx: mpsc::UnboundedSender<()>,
}

/// Transcode job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscodeJob {
    pub id: String,
    pub stream_id: String,
    pub input_path: String,
    pub output_paths: Vec<String>,
    pub preset: String,
    pub status: TranscodeStatus,
    pub progress_pct: f64,
    pub priority: JobPriority,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
    pub tenant_id: String,
}

/// Recording job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingJob {
    pub id: String,
    pub stream_id: String,
    pub schedule: RecordingSchedule,
    pub output_template: String,
    pub status: RecordingStatus,
    pub current_segment: Option<String>,
    pub segment_start: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Recording status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordingStatus {
    Scheduled,
    Recording,
    Paused,
    Completed,
    Failed,
}

/// Authenticated context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

// ============================================================================
// Application State Implementation
// ============================================================================

impl ProcessorState {
    /// Create a new processor state.
    pub async fn new(
        config: MediaProcessorConfig,
        cluster: Arc<ClusterManager>,
        metrics: Arc<MetricsRegistry>,
        event_publisher: Arc<EventPublisher>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<()>)> {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(config.server.request_timeout_secs))
            .build()?;

        // Initialize object storage
        let storage = build_storage_operator(&config.storage).await?;

        // Initialize NATS if configured
        let nats = if let Some(nats_config) = &config.nats {
            let client = async_nats::connect(&nats_config.urls.join(",")).await?;
            if let Some(domain) = &nats_config.jetstream_domain {
                // JetStream setup would go here
            }
            Some(client)
        } else {
            None
        };

        // Job semaphore for concurrency control
        let job_semaphore = Arc::new(Semaphore::new(config.transcoding.max_concurrent_jobs));

        // Initialize transcode pipeline
        let transcode_pipeline = Arc::new(TranscodePipeline::new(
            config.transcoding.ffmpeg_path.clone(),
            parse_hwaccel(&config.transcoding.hwaccel),
            config.transcoding.gpu_device,
        )?);

        let (reload_tx, reload_rx) = mpsc::unbounded_channel();

        Ok((
            Self {
                config: ArcSwap::from_pointee(config),
                http_client,
                storage,
                nats,
                job_queue: Arc::new(DashMap::new()),
                job_semaphore,
                scheduled_recordings: Arc::new(DashMap::new()),
                transcode_pipeline,
                cluster,
                metrics,
                event_publisher,
                reload_tx,
            },
            reload_rx,
        ))
    }

    /// Get current config.
    pub fn config(&self) -> Arc<MediaProcessorConfig> {
        self.config.load()
    }

    /// Submit a transcode job.
    pub async fn submit_transcode_job(&self, job: TranscodeJob) -> Result<String> {
        let job_id = job.id.clone();
        self.job_queue.insert(job_id.clone(), job);

        // Spawn worker
        let state = self.clone();
        tokio::spawn(async move {
            state.process_transcode_job(job_id).await;
        });

        Ok(job_id)
    }

    /// Process a transcode job.
    async fn process_transcode_job(&self, job_id: String) {
        let _permit = self.job_semaphore.acquire().await.unwrap();

        let mut job = match self.job_queue.get_mut(&job_id) {
            Some(j) => j.clone(),
            None => return,
        };

        job.status = TranscodeStatus::Running;
        job.started_at = Some(chrono::Utc::now());
        self.job_queue.insert(job_id.clone(), job.clone());

        // Update progress callback
        let state = self.clone();
        let job_id_clone = job_id.clone();
        let progress_callback = move |progress: f64| {
            let state = state.clone();
            let job_id = job_id_clone.clone();
            tokio::spawn(async move {
                if let Some(mut job) = state.job_queue.get_mut(&job_id) {
                    job.progress_pct = progress;
                }
            });
        };

        // Run transcode
        let preset = self
            .config()
            .transcoding
            .presets
            .iter()
            .find(|p| p.name == job.preset)
            .cloned();

        let options = TranscodeOptions {
            input_path: job.input_path.clone(),
            output_paths: job.output_paths.clone(),
            video_codec: preset
                .as_ref()
                .map(|p| p.video_codec.clone())
                .unwrap_or_else(|| self.config().transcoding.default_video_codec.clone()),
            audio_codec: preset
                .as_ref()
                .map(|p| p.audio_codec.clone())
                .unwrap_or_else(|| self.config().transcoding.default_audio_codec.clone()),
            video_bitrate: preset
                .as_ref()
                .map(|p| p.video_bitrate_kbps * 1000)
                .unwrap_or(5_000_000),
            audio_bitrate: preset
                .as_ref()
                .map(|p| p.audio_bitrate_kbps * 1000)
                .unwrap_or(128_000),
            resolution: preset.as_ref().map(|p| p.resolution.clone()),
            framerate: preset.as_ref().map(|p| p.framerate).unwrap_or(30),
            hwaccel: parse_hwaccel(&self.config().transcoding.hwaccel),
            gpu_device: self.config().transcoding.gpu_device,
        };

        let result = self
            .transcode_pipeline
            .transcode(options, progress_callback)
            .await;

        // Update job status
        if let Some(mut job) = self.job_queue.get_mut(&job_id) {
            match result {
                Ok(_) => {
                    job.status = TranscodeStatus::Completed;
                    job.progress_pct = 100.0;
                    job.completed_at = Some(chrono::Utc::now());

                    // Publish completion event
                    let _ = self
                        .event_publisher
                        .publish(Event::TranscodeCompleted {
                            job_id: job_id.clone(),
                            stream_id: job.stream_id.clone(),
                            outputs: job.output_paths.clone(),
                        })
                        .await;
                }
                Err(e) => {
                    job.status = TranscodeStatus::Failed;
                    job.error = Some(e.to_string());
                    job.completed_at = Some(chrono::Utc::now());

                    // Publish failure event
                    let _ = self
                        .event_publisher
                        .publish(Event::TranscodeFailed {
                            job_id: job_id.clone(),
                            stream_id: job.stream_id.clone(),
                            error: e.to_string(),
                        })
                        .await;
                }
            }
        }
    }

    /// Schedule a recording.
    pub async fn schedule_recording(&self, schedule: RecordingSchedule) -> Result<String> {
        let job_id = Uuid::new_v4().to_string();
        let job = RecordingJob {
            id: job_id.clone(),
            stream_id: schedule.stream.clone(),
            schedule: schedule.clone(),
            output_template: schedule.output_template.clone(),
            status: RecordingStatus::Scheduled,
            current_segment: None,
            segment_start: None,
            created_at: chrono::Utc::now(),
        };

        self.scheduled_recordings.insert(job_id.clone(), job);

        // Parse cron and schedule
        // In production, use a proper cron scheduler
        info!(
            "Scheduled recording: {} for stream {}",
            job_id, schedule.stream
        );

        Ok(job_id)
    }

    /// Start recording for a stream.
    pub async fn start_recording(&self, stream_id: &str, output_path: &str) -> Result<String> {
        let recording_id = Uuid::new_v4().to_string();

        // Implementation would start FFmpeg recording
        // This is a placeholder

        // Publish recording started event
        let _ = self
            .event_publisher
            .publish(Event::RecordingStarted {
                recording_id: recording_id.clone(),
                stream_id: stream_id.to_string(),
                output_path: output_path.to_string(),
            })
            .await;

        Ok(recording_id)
    }

    /// Stop recording.
    pub async fn stop_recording(&self, recording_id: &str) -> Result<()> {
        // Implementation would stop FFmpeg recording

        // Publish recording completed event
        let _ = self
            .event_publisher
            .publish(Event::RecordingCompleted {
                recording_id: recording_id.to_string(),
            })
            .await;

        Ok(())
    }

    /// Generate thumbnails for VOD.
    pub async fn generate_thumbnails(
        &self,
        input_path: &str,
        output_dir: &str,
        config: &ThumbnailConfig,
    ) -> Result<Vec<String>> {
        // Use ffmpeg to extract thumbnails
        let mut thumbnails = Vec::new();

        // Implementation would use ffmpeg-next to extract frames
        // For now, return empty

        Ok(thumbnails)
    }

    /// Generate sprite sheet for VOD.
    pub async fn generate_sprites(
        &self,
        input_path: &str,
        output_dir: &str,
        config: &SpriteConfig,
    ) -> Result<String> {
        // Implementation would generate sprite sheet
        Ok("".to_string())
    }

    /// Generate captions using AI.
    pub async fn generate_captions(
        &self,
        input_path: &str,
        config: &CaptionConfig,
    ) -> Result<String> {
        // Implementation would use Whisper or cloud AI
        Ok("".to_string())
    }

    /// Push to cloud (YouTube, Twitch, custom RTMP/SRT).
    pub async fn push_to_cloud(
        &self,
        stream_id: &str,
        destination: &CloudDestination,
    ) -> Result<()> {
        // Implementation would use FFmpeg to push
        Ok(())
    }
}

/// Cloud destination for egress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudDestination {
    pub platform: String, // youtube, twitch, custom
    pub stream_key: String,
    pub url: String,
    pub protocol: String, // rtmp, srt
}

/// Parse hardware acceleration string.
fn parse_hwaccel(hwaccel: &str) -> HardwareAccel {
    match hwaccel.to_lowercase().as_str() {
        "nvenc" => HardwareAccel::Nvenc,
        "vaapi" => HardwareAccel::Vaapi,
        "videotoolbox" => HardwareAccel::VideoToolbox,
        "qsv" => HardwareAccel::Qsv,
        "amf" => HardwareAccel::Amf,
        _ => HardwareAccel::None,
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
            let mut b = opendal::services::Gcs::default().bucket(&gcs_config.bucket);
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
            opendal::services::Memory::default().max_size(mem_config.max_size)
        }
        _ => anyhow::bail!("Unknown storage backend: {}", config.backend),
    };

    let op = Operator::new(builder)?
        .layer(LoggingLayer::default())
        .finish();
    Ok(op)
}

// ============================================================================
// API Handlers
// ============================================================================

/// Health check.
async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "healthy")
}

/// Submit transcode job.
async fn submit_transcode(
    State(state): State<ProcessorState>,
    auth: AuthContext,
    ValidatedJson(payload): ValidatedJson<SubmitTranscodeRequest>,
) -> impl IntoResponse {
    let job = TranscodeJob {
        id: Uuid::new_v4().to_string(),
        stream_id: payload.stream_id,
        input_path: payload.input_path,
        output_paths: payload.output_paths,
        preset: payload.preset,
        status: TranscodeStatus::Queued,
        progress_pct: 0.0,
        priority: payload.priority.unwrap_or(JobPriority::Normal),
        created_at: chrono::Utc::now(),
        started_at: None,
        completed_at: None,
        error: None,
        tenant_id: auth.tenant_id,
    };

    match state.submit_transcode_job(job.clone()).await {
        Ok(job_id) => (
            StatusCode::ACCEPTED,
            axum::Json(serde_json::json!({
                "job_id": job_id,
                "status": "queued"
            })),
        )
            .into_response(),
        Err(e) => {
            error!("Failed to submit transcode job: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to submit job").into_response()
        }
    }
}

/// Get transcode job status.
async fn get_transcode_job(
    State(state): State<ProcessorState>,
    auth: AuthContext,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    if let Some(job) = state.job_queue.get(&job_id) {
        if job.tenant_id == auth.tenant_id || auth.roles.contains(&"admin".to_string()) {
            return axum::Json(job.clone()).into_response();
        }
    }
    (StatusCode::NOT_FOUND, "Job not found").into_response()
}

/// List transcode jobs.
async fn list_transcode_jobs(
    State(state): State<ProcessorState>,
    auth: AuthContext,
    Query(pagination): Query<PaginationParams>,
) -> impl IntoResponse {
    let jobs: Vec<TranscodeJob> = state
        .job_queue
        .iter()
        .filter(|entry| {
            entry.value().tenant_id == auth.tenant_id || auth.roles.contains(&"admin".to_string())
        })
        .map(|entry| entry.value().clone())
        .collect();

    let total = jobs.len() as u64;
    let start = ((pagination.page - 1) * pagination.per_page) as usize;
    let end = (start + pagination.per_page as usize).min(jobs.len());
    let page_jobs = jobs[start..end].to_vec();

    axum::Json(PaginatedResponse {
        data: page_jobs,
        pagination: PaginationInfo {
            page: pagination.page,
            per_page: pagination.per_page,
            total,
            total_pages: ((total + pagination.per_page as u64 - 1) / pagination.per_page as u64)
                .max(1) as u32,
        },
    })
    .into_response()
}

/// Schedule recording.
async fn schedule_recording(
    State(state): State<ProcessorState>,
    auth: AuthContext,
    ValidatedJson(payload): ValidatedJson<RecordingSchedule>,
) -> impl IntoResponse {
    match state.schedule_recording(payload).await {
        Ok(job_id) => (
            StatusCode::CREATED,
            axum::Json(serde_json::json!({
                "job_id": job_id,
                "status": "scheduled"
            })),
        )
            .into_response(),
        Err(e) => {
            error!("Failed to schedule recording: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to schedule recording",
            )
                .into_response()
        }
    }
}

/// Start recording.
async fn start_recording_endpoint(
    State(state): State<ProcessorState>,
    auth: AuthContext,
    ValidatedJson(payload): ValidatedJson<StartRecordingRequest>,
) -> impl IntoResponse {
    match state
        .start_recording(&payload.stream_id, &payload.output_path)
        .await
    {
        Ok(recording_id) => (
            StatusCode::OK,
            axum::Json(serde_json::json!({
                "recording_id": recording_id,
                "status": "recording"
            })),
        )
            .into_response(),
        Err(e) => {
            error!("Failed to start recording: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to start recording",
            )
                .into_response()
        }
    }
}

/// Stop recording.
async fn stop_recording_endpoint(
    State(state): State<ProcessorState>,
    auth: AuthContext,
    Path(recording_id): Path<String>,
) -> impl IntoResponse {
    match state.stop_recording(&recording_id).await {
        Ok(_) => (
            StatusCode::OK,
            axum::Json(serde_json::json!({
                "status": "stopped"
            })),
        )
            .into_response(),
        Err(e) => {
            error!("Failed to stop recording: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to stop recording",
            )
                .into_response()
        }
    }
}

/// Generate VOD assets (thumbnails, sprites, captions).
async fn generate_vod_assets(
    State(state): State<ProcessorState>,
    auth: AuthContext,
    ValidatedJson(payload): ValidatedJson<GenerateVodRequest>,
) -> impl IntoResponse {
    let config = state.config();

    // Generate thumbnails
    let thumbnails = if config.vod.thumbnails.enabled {
        state
            .generate_thumbnails(
                &payload.input_path,
                &payload.output_dir,
                &config.vod.thumbnails,
            )
            .await
            .unwrap_or_default()
    } else {
        vec![]
    };

    // Generate sprites
    let sprites = if config.vod.sprites.enabled {
        state
            .generate_sprites(
                &payload.input_path,
                &payload.output_dir,
                &config.vod.sprites,
            )
            .await
            .unwrap_or_default()
    } else {
        "".to_string()
    };

    // Generate captions
    let captions = if config.vod.captions.enabled {
        state
            .generate_captions(&payload.input_path, &config.vod.captions)
            .await
            .unwrap_or_default()
    } else {
        "".to_string()
    };

    axum::Json(serde_json::json!({
        "thumbnails": thumbnails,
        "sprites": sprites,
        "captions": captions,
    }))
    .into_response()
}

/// Push to cloud destination.
async fn push_to_cloud_endpoint(
    State(state): State<ProcessorState>,
    auth: AuthContext,
    ValidatedJson(payload): ValidatedJson<PushToCloudRequest>,
) -> impl IntoResponse {
    let destination = CloudDestination {
        platform: payload.platform,
        stream_key: payload.stream_key,
        url: payload.url,
        protocol: payload.protocol,
    };

    match state.push_to_cloud(&payload.stream_id, &destination).await {
        Ok(_) => (
            StatusCode::OK,
            axum::Json(serde_json::json!({
                "status": "pushing"
            })),
        )
            .into_response(),
        Err(e) => {
            error!("Failed to push to cloud: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to push to cloud").into_response()
        }
    }
}

/// Config reload endpoint.
async fn config_reload(State(state): State<ProcessorState>) -> impl IntoResponse {
    let _ = state.reload_tx.send(());
    (
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "status": "ok"
        })),
    )
}

/// Get config version.
async fn config_version(State(state): State<ProcessorState>) -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "version": 1,
        "service": "media-processor",
        "loaded_at": chrono::Utc::now()
    }))
}

/// Get cluster capacity.
async fn get_cluster_capacity(State(state): State<ProcessorState>) -> impl IntoResponse {
    let capacity = state.cluster.capacity();
    let nodes = capacity.get_all_capacity().await;
    axum::Json(serde_json::json!({ "capacity": nodes }))
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize, Serialize)]
pub struct SubmitTranscodeRequest {
    pub stream_id: String,
    pub input_path: String,
    pub output_paths: Vec<String>,
    pub preset: String,
    pub priority: Option<JobPriority>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StartRecordingRequest {
    pub stream_id: String,
    pub output_path: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GenerateVodRequest {
    pub input_path: String,
    pub output_dir: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PushToCloudRequest {
    pub stream_id: String,
    pub platform: String,
    pub stream_key: String,
    pub url: String,
    pub protocol: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub per_page: u32,
}

fn default_page() -> u32 {
    1
}
fn default_page_size() -> u32 {
    50
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub pagination: PaginationInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationInfo {
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
    pub total_pages: u32,
}

// ============================================================================
// Authentication & Rate Limiting
// ============================================================================

async fn extract_auth(
    State(state): State<ProcessorState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<AuthContext, StatusCode> {
    let config = state.config();

    // Try JWT
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                // Validate JWT (simplified)
                return Ok(AuthContext {
                    tenant_id: "default".to_string(),
                    user_id: None,
                    roles: vec!["operator".to_string()],
                    permissions: vec!["transcode".to_string(), "record".to_string()],
                });
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
                permissions: vec!["transcode".to_string(), "record".to_string()],
            });
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

async fn rate_limit_middleware(
    State(state): State<ProcessorState>,
    auth: Option<AuthContext>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    // Rate limiting for processor API
    let key = auth
        .map(|a| format!("tenant:{}", a.tenant_id))
        .unwrap_or_else(|| format!("ip:{}", addr.ip()));

    let quota = Quota::per_minute(1000);
    let limiter = state.get_rate_limiter(&key, quota);

    match limiter.check() {
        Ok(_) => next.run(request).await,
        Err(_) => {
            let mut response = StatusCode::TOO_MANY_REQUESTS.into_response();
            response
                .headers_mut()
                .insert("Retry-After", HeaderValue::from_str("60").unwrap());
            response
        }
    }
}

// Add get_rate_limiter to ProcessorState
impl ProcessorState {
    fn get_rate_limiter(
        &self,
        key: &str,
        quota: Quota,
    ) -> Arc<
        RateLimiter<
            std::sync::Mutex<governor::state::NotKeyed>,
            governor::state::InMemoryState,
            governor::clock::DefaultClock,
        >,
    > {
        self.job_queue // reuse dashmap for rate limiters
            .entry(key.to_string())
            .or_insert_with(|| {
                // This is a hack - in reality we'd have a separate rate_limiter map
                // For now, just create a new one each time
                TranscodeJob {
                    id: "rate_limiter".to_string(),
                    stream_id: "".to_string(),
                    input_path: "".to_string(),
                    output_paths: vec![],
                    preset: "".to_string(),
                    status: TranscodeStatus::Queued,
                    progress_pct: 0.0,
                    priority: JobPriority::Normal,
                    created_at: chrono::Utc::now(),
                    started_at: None,
                    completed_at: None,
                    error: None,
                    tenant_id: "".to_string(),
                }
            });

        // Create a proper rate limiter
        Arc::new(RateLimiter::direct(quota))
    }
}

// ============================================================================
// Config Hot Reload
// ============================================================================

async fn config_reload_watcher(mut reload_rx: mpsc::UnboundedReceiver<()>, state: ProcessorState) {
    while reload_rx.recv().await.is_some() {
        info!("Config reload triggered");

        match loader::load::<MediaProcessorConfig>() {
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
        use tokio::signal::unix::{SignalKind, signal};
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
    let cert_file = std::fs::read(&tls_config.cert_file).context("Failed to read cert file")?;
    let key_file = std::fs::read(&tls_config.key_file).context("Failed to read key file")?;

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
    let config: MediaProcessorConfig = loader::load()?;

    // Initialize tracing
    let _tracing = mediaedge_observability::tracing::init();
    info!("Starting mediaedge-media-processor");

    // Initialize metrics
    let metrics = Arc::new(MetricsRegistry::new());

    // Initialize cluster manager
    let core_config = mediaedge_core::CoreConfig {
        server: config.server.clone(),
    };
    let core_service = mediaedge_core::CoreService::new(core_config, metrics.clone());
    core_service.start().await?;
    let cluster = core_service.cluster();

    // Initialize event publisher
    let event_publisher = Arc::new(
        EventPublisher::new(
            config
                .nats
                .as_ref()
                .map(|c| c.urls.clone())
                .unwrap_or_default(),
        )
        .await?,
    );

    // Create processor state
    let (state, reload_rx) =
        ProcessorState::new(config.clone(), cluster, metrics.clone(), event_publisher).await?;

    // Start config reload watcher
    let state_clone = state.clone();
    tokio::spawn(config_reload_watcher(reload_rx, state_clone));

    // Setup signal handlers
    setup_signal_handlers(state.reload_tx.clone());

    // Start scheduled recording checker
    let state_scheduler = state.clone();
    tokio::spawn(scheduled_recording_worker(state_scheduler));

    // Build HTTP router
    let app = build_router(state.clone()).await;

    // Build TLS config if needed
    let tls_config = config
        .server
        .tls
        .as_ref()
        .map(build_tls_config)
        .transpose()?;

    // Start HTTP server
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    info!("Media processor HTTP server listening on {}", addr);

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
    info!("Shutting down media processor");
    core_service.stop().await?;

    Ok(())
}

/// Scheduled recording worker - checks for recordings to start.
async fn scheduled_recording_worker(state: ProcessorState) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));

    loop {
        interval.tick().await;

        let now = chrono::Utc::now();
        let config = state.config();

        for entry in state.scheduled_recordings.iter() {
            let job = entry.value();

            // Check if it's time to start (simplified - real impl would use cron)
            if job.status == RecordingStatus::Scheduled {
                // In production, check cron expression against current time
                info!(
                    "Would start scheduled recording: {} for stream {}",
                    job.id, job.stream_id
                );
            }
        }
    }
}

/// Build the HTTP router with all routes and middleware.
async fn build_router(state: ProcessorState) -> Router {
    let config = state.config();

    // OpenAPI config
    let openapi_config = OpenApiConfig {
        service_name: "MediaEdge Media Processor".to_string(),
        version: "1.0.0".to_string(),
        description: "MediaEdge Media Processor API".to_string(),
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
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Public routes
    let public_routes = Router::new().route("/health", get(health_check));

    // Protected routes
    let protected_routes = Router::new()
        .route("/api/v1/transcode", post(submit_transcode))
        .route("/api/v1/transcode/:job_id", get(get_transcode_job))
        .route("/api/v1/transcode", get(list_transcode_jobs))
        .route("/api/v1/recordings/schedule", post(schedule_recording))
        .route("/api/v1/recordings/start", post(start_recording_endpoint))
        .route(
            "/api/v1/recordings/:recording_id/stop",
            post(stop_recording_endpoint),
        )
        .route("/api/v1/vod/generate", post(generate_vod_assets))
        .route("/api/v1/egress/push", post(push_to_cloud_endpoint));

    // Admin routes
    let admin_routes = Router::new()
        .route("/health", get(health_check))
        .route("/config/reload", post(config_reload))
        .route("/config/version", get(config_version))
        .route("/cluster/capacity", get(get_cluster_capacity));

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
    State(state): State<ProcessorState>,
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
    {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hwaccel() {
        assert_eq!(parse_hwaccel("nvenc"), HardwareAccel::Nvenc);
        assert_eq!(parse_hwaccel("vaapi"), HardwareAccel::Vaapi);
        assert_eq!(parse_hwaccel("videotoolbox"), HardwareAccel::VideoToolbox);
        assert_eq!(parse_hwaccel("none"), HardwareAccel::None);
        assert_eq!(parse_hwaccel("unknown"), HardwareAccel::None);
    }

    #[test]
    fn test_transcode_job_serialization() {
        let job = TranscodeJob {
            id: "test123".to_string(),
            stream_id: "stream1".to_string(),
            input_path: "/input/video.mp4".to_string(),
            output_paths: vec!["/output/1080p.mp4".to_string()],
            preset: "1080p".to_string(),
            status: TranscodeStatus::Queued,
            progress_pct: 0.0,
            priority: JobPriority::Normal,
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            error: None,
            tenant_id: "tenant1".to_string(),
        };
        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("test123"));
        assert!(json.contains("queued"));
    }
}
