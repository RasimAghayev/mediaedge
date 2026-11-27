#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Creates GitHub issues for MediaEdge feature plan
.DESCRIPTION
    Since gh CLI is not available, this script outputs the JSON payloads
    that can be used with GitHub API or manually created via web UI.
#>

param(
    [string]$Owner = "RasimAghayev",
    [string]$Repo = "mediaedge",
    [string]$Token = $(Read-Host -AsSecureString "GitHub Personal Access Token" | ConvertFrom-SecureString -AsPlainText)
)

$headers = @{
    "Authorization" = "token $Token"
    "Accept" = "application/vnd.github.v3+json"
    "Content-Type" = "application/json"
}

$baseUrl = "https://api.github.com/repos/$Owner/$Repo/issues"

# Phase 0: Foundation (Critical Path)
$issues = @()

# Issue #1: Core Configuration System
$issues += @{
    title = "[Phase 0] Core Configuration System Implementation"
    body = @"
## Business Value
All services depend on config; enables hot reload, secrets management, validation. Unlocks all subsequent development.

## Description
Complete implementation of mediaedge-config crate with layered configuration (file → env → CLI), validation, secrets management, and atomic hot-reload via ArcSwap.

## Subtasks
- [ ] `mediaedge-config/src/types.rs` - Complete config structs for all 5 services
- [ ] `mediaedge-config/src/loader.rs` - Figment-based layered config with validation
- [ ] `mediaedge-config/src/secrets.rs` - Age-encrypted secrets, AWS KMS, runtime rotation
- [ ] ArcSwap atomic config hot-swap (single-digit-ns reads)
- [ ] Config schema generation (JSON Schema + OpenAPI)
- [ ] Unit tests: merge precedence, validation, hot-reload atomicity

## Acceptance Criteria
- [ ] All 5 services start with `--config` flag
- [ ] `kill -HUP <pid>` reloads config without dropping connections
- [ ] `POST /api/v1/config-reload` returns `{status: "ok", version: N}`
- [ ] Secrets never appear in logs/metrics

## Dependencies
None (critical path)

## Estimated Effort
2 weeks | 13 story points
"@
    labels = @("phase-0", "critical-path", "config", "backend")
}

# Issue #2: Observability Stack
$issues += @{
    title = "[Phase 0] Observability Stack Implementation"
    body = @"
## Business Value
Production debugging, SLA compliance, capacity planning. Required for any production deployment.

## Description
Complete observability stack: OpenTelemetry tracing, Prometheus metrics, structured logging, live log tail, SLO tracking with client-sample integration.

## Subtasks
- [ ] `mediaedge-observability/src/tracing.rs` - OpenTelemetry (Jaeger + OTLP gRPC)
- [ ] `mediaedge-observability/src/metrics.rs` - Prometheus with custom histograms
- [ ] `mediaedge-observability/src/logging.rs` - JSON logs, SSE live tail (`GET /api/v1/logs`)
- [ ] `mediaedge-observability/src/slo.rs` - Latency SLO tracker (5 transport buckets)
- [ ] FanoutBuilder: single instrumentation → Prometheus + OTLP
- [ ] Client-sample endpoint: `POST /api/v1/slo/client-sample`
- [ ] Grafana dashboards: cluster, per-stream latency, errors, config reload history

## Acceptance Criteria
- [ ] `/metrics` exposes 50+ custom metrics
- [ ] OTLP export to collector works
- [ ] SSE log tail filters by level
- [ ] SLO dashboard shows p50/p95/p99 per transport

## Dependencies
None (critical path)

## Estimated Effort
2 weeks | 13 story points
"@
    labels = @("phase-0", "critical-path", "observability", "backend")
}

# Issue #3: API Layer
$issues += @{
    title = "[Phase 0] API Layer - OpenAPI + gRPC"
    body = @"
## Business Value
Contract-first development, SDK generation, interoperability. Enables parallel frontend/backend work.

## Description
OpenAPI 3.1 spec via Utoipa, gRPC protobuf definitions, code generation for TypeScript/Rust/Go clients, validation middleware, API versioning, rate limiting.

## Subtasks
- [ ] `mediaedge-api/src/openapi.rs` - Utoipa OpenAPI 3.1 for all REST endpoints
- [ ] `mediaedge-api/src/grpc/` - Protobuf for internal service communication
- [ ] Code generation: `mediaedge-api-gen` crate for TS/Rust/Go clients
- [ ] Request/response validation middleware
- [ ] API versioning strategy (header + URL fallback)
- [ ] Rate limiting middleware (token bucket, Redis-backed)

## Acceptance Criteria
- [ ] `cargo doc` generates complete API docs
- [ ] Swagger UI at `/api/docs`
- [ ] gRPC reflection enabled
- [ ] All endpoints have request/response schemas

## Dependencies
None (critical path)

## Estimated Effort
1 week | 8 story points
"@
    labels = @("phase-0", "critical-path", "api", "backend")
}

# Issue #4: Protocol Ingest
$issues += @{
    title = "[Phase 1] Protocol Ingest Implementations (RTMP, SRT, RIST, WHIP, RTSP)"
    body = @"
## Business Value
Core revenue - accepting streams from broadcasters. Supports all major contribution protocols.

## Description
Unified fragment model (like LVQR) with FragmentBroadcasterRegistry. All ingests funnel to single fragment model.

## Subtasks
- [ ] **RTMP Ingest**: Enhanced RTMP v2 (HEVC/H.265, AV1), RTMPS, stream key auth, metadata parsing
- [ ] **SRT Ingest**: Caller/Listener/Rendezvous, AES encryption, Stream ID auth, SRT FEC, SMPTE 2022-7
- [ ] **RIST Ingest**: Simple Profile (VSF TR-06-1), RTCP NACK, pure-Rust bilbycast-rist
- [ ] **WHIP/WHEP Ingest**: WebRTC via WHIP, str0m integration, HEVC→H.264 transcode
- [ ] **RTSP Pull**: TCP/UDP, auto-reconnect, H.264/H.265 from IP cameras
- [ ] Unified `FragmentBroadcasterRegistry`

## Acceptance Criteria
- [ ] Each protocol: 10k+ concurrent streams per node
- [ ] < 50ms ingest-to-fragment latency
- [ ] Graceful degradation on protocol errors
- [ ] Metrics per protocol (ingest_errors_total, ingest_bitrate_bytes)

## Dependencies
#1 Config, #2 Observability, #3 API

## Estimated Effort
4 weeks | 21 story points
"@
    labels = @("phase-1", "data-plane", "ingest", "backend")
}

# Issue #5: Protocol Egress
$issues += @{
    title = "[Phase 1] Protocol Egress Implementations (LL-HLS, DASH, WHEP, MoQ, WS)"
    body = @"
## Business Value
Delivering content to viewers - direct revenue impact. Supports all major playback protocols.

## Description
Low-latency delivery for all major protocols with CDN-friendly caching, SCTE-35 passthrough for SSAI.

## Subtasks
- [ ] **LL-HLS Egress**: TS + fMP4, LL-HLS parts, HEVC/HDR, audio remuxing, per-segment re-encode
- [ ] **MPEG-DASH Egress**: CMAF-LL, SegmentTemplate/Timeline, multi-period for ads (SCTE-35)
- [ ] **WHEP Egress**: WebRTC playback via WHEP, WHIP/WHEP signaling
- [ ] **MoQ over QUIC/WebTransport**: draft-22, hang catalog + CMAF, 0.timing track
- [ ] **WebSocket fMP4**: Low-latency fragment delivery
- [ ] **Legacy Play**: RTMP/SRT/RTSP pull for legacy players

## Acceptance Criteria
- [ ] 100k+ concurrent viewers per node (HLS/DASH)
- [ ] < 100ms glass-to-glass for LL-HLS/MoQ
- [ ] SCTE-35 passthrough for SSAI
- [ ] CDN-friendly cache headers

## Dependencies
#1 Config, #2 Observability, #3 API

## Estimated Effort
4 weeks | 21 story points
"@
    labels = @("phase-1", "data-plane", "egress", "backend")
}

# Issue #6: Transcoding Pipeline
$issues += @{
    title = "[Phase 1] Transcoding Pipeline (FFmpeg-next + HW Accel)"
    body = @"
## Business Value
Multi-bitrate delivery, format normalization, cost optimization. Essential for adaptive streaming.

## Description
FFmpeg-next based pipeline with hardware acceleration (NVENC, VAAPI, VideoToolbox), adaptive bitrate ladder, thumbnail generation, SCTE-35 preservation.

## Subtasks
- [ ] `mediaedge-media/src/transcode/pipeline.rs` - FFmpeg-next pipeline
- [ ] Hardware acceleration: NVENC, VAAPI, VideoToolbox
- [ ] Video: H.264, HEVC, AV1 (libx264, libx265, libaom, NVENC)
- [ ] Audio: AAC-LC, HE-AAC v1/v2, Opus, MP2, AC-3
- [ ] Per-stream adaptive bitrate ladder
- [ ] Thumbnail generator (keyframe extraction)
- [ ] SCTE-35 ad marker preservation
- [ ] Transcode job queue with priority

## Acceptance Criteria
- [ ] 4K60 HEVC → 1080p30 H.264 in < 200ms latency
- [ ] GPU utilization > 80% under load
- [ ] Zero frame drops at configured bitrates
- [ ] Metrics: queue_depth, active_jobs, gpu_utilization

## Dependencies
#4 Ingest, #5 Egress

## Estimated Effort
3 weeks | 13 story points
"@
    labels = @("phase-1", "data-plane", "transcoding", "backend")
}

# Output JSON for manual creation
$issues | ForEach-Object {
    $json = $_ | ConvertTo-Json -Depth 10 -Compress
    Write-Host "---"
    Write-Host "curl -X POST -H \"Authorization: token \$TOKEN\" -H \"Accept: application/vnd.github.v3+json\" -d '$json' https://api.github.com/repos/$Owner/$Repo/issues"
    Write-Host ""
}