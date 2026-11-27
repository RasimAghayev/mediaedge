# MediaEdge Feature Implementation Plan

## Executive Summary

Based on comprehensive research of modern media infrastructure platforms (LVQR, SENCA, Flussonic Catena, Bilbycast, STUNner, atm0s-media-server, av-mesh) and streaming business cases (OTT monetization, FAST channels, hybrid models, AI-driven scheduling), this plan outlines the complete feature roadmap for MediaEdge.

**Current State**: Scaffolding complete (45 crates/services, Cargo workspace, CI/CD, Docker configs, Phase 1 docs). All crates have only module declarations - **zero implementation**.

**Target**: Production-ready media infrastructure platform supporting:
- Multi-protocol ingest/egress (RTMP, SRT, RIST, WHIP/WHEP, HLS, DASH, MoQ)
- Cluster orchestration with gossip-based ownership
- Hot config reload with atomic ArcSwap
- Full observability (Prometheus, OTLP, live logs, SLO tracking)
- Replication coordinator with erasure coding
- Edge mesh with WebRTC peer-to-peer
- Multi-tenant with OIDC/SSO, billing integration
- AI-assisted scheduling and monetization

---

## Phase 0: Foundation (Weeks 1-2) - CRITICAL PATH

### Issue #1: Core Configuration System Implementation
**Labels**: `phase-0`, `critical-path`, `config`, `backend`
**Business Value**: All services depend on config; enables hot reload, secrets management, validation

**Subtasks**:
- [ ] `mediaedge-config/src/types.rs` - Complete config structs for all services (Gateway, StorageNode, EdgeNode, ReplicationCoordinator, MediaProcessor)
- [ ] `mediaedge-config/src/loader.rs` - Figment-based layered config (TOML file → env vars → CLI args) with validation
- [ ] `mediaedge-config/src/secrets.rs` - Age-encrypted secrets, AWS KMS integration, runtime rotation via SIGHUP + `/api/v1/config-reload`
- [ ] ArcSwap atomic config hot-swap (single-digit-ns reads, in-flight request completion)
- [ ] Config schema generation (JSON Schema for validation, OpenAPI for docs)
- [ ] Unit tests: config merge precedence, validation errors, hot-reload atomicity

**Acceptance Criteria**:
- All 5 services start with `--config` flag
- `kill -HUP <pid>` reloads config without dropping connections
- `POST /api/v1/config-reload` returns `{status: "ok", version: N}`
- Secrets never appear in logs/metrics

---

### Issue #2: Observability Stack Implementation
**Labels**: `phase-0`, `critical-path`, `observability`, `backend`
**Business Value**: Production debugging, SLA compliance, capacity planning

**Subtasks**:
- [ ] `mediaedge-observability/src/tracing.rs` - OpenTelemetry setup (Jaeger + OTLP gRPC), trace context propagation
- [ ] `mediaedge-observability/src/metrics.rs` - Prometheus exporter with custom histograms (glass-to-glass latency, protocol-specific counters)
- [ ] `mediaedge-observability/src/logging.rs` - Structured JSON logs, live log tail via SSE (`GET /api/v1/logs`)
- [ ] `mediaedge-observability/src/slo.rs` - Latency SLO tracker with 5 transport buckets (hls, dash, ws, whep, moq)
- [ ] FanoutBuilder: single instrumentation → Prometheus + OTLP simultaneously
- [ ] Client-sample endpoint: `POST /api/v1/slo/client-sample` for browser/MoQ client latency
- [ ] Grafana dashboards: cluster overview, per-stream latency, error rates, config reload history

**Acceptance Criteria**:
- `/metrics` exposes 50+ custom metrics
- OTLP export to collector works
- SSE log tail filters by level
- SLO dashboard shows p50/p95/p99 per transport

---

### Issue #3: API Layer - OpenAPI + gRPC
**Labels**: `phase-0`, `critical-path`, `api`, `backend`
**Business Value**: Contract-first development, SDK generation, interoperability

**Subtasks**:
- [ ] `mediaedge-api/src/openapi.rs` - Utoipa-based OpenAPI 3.1 spec for all REST endpoints
- [ ] `mediaedge-api/src/grpc/` - Protobuf definitions for internal service communication
- [ ] Code generation: `mediaedge-api-gen` crate for TypeScript/Rust/Go clients
- [ ] Request/response validation middleware (validator crate)
- [ ] API versioning strategy (header-based, URL-based fallback)
- [ ] Rate limiting middleware (token bucket, Redis-backed for cluster)

**Acceptance Criteria**:
- `cargo doc` generates complete API docs
- Swagger UI at `/api/docs`
- gRPC reflection enabled
- All endpoints have request/response schemas

---

## Phase 1: Data Plane - Ingest & Egress (Weeks 3-6)

### Issue #4: Protocol Ingest Implementations
**Labels**: `phase-1`, `data-plane`, `ingest`, `backend`
**Business Value**: Core revenue - accepting streams from broadcasters

**Subtasks**:
- [ ] **RTMP Ingest** (`mediaedge-media/src/ingest/rtmp.rs`):
  - Enhanced RTMP v2 (HEVC/H.265, AV1 via `hvc1`/`av01` codec IDs)
  - RTMPS (TLS) support
  - Stream key authentication via auth service
  - Metadata parsing (onMetaData, @setDataFrame)
- [ ] **SRT Ingest** (`mediaedge-media/src/ingest/srt.rs`):
  - Caller/Listener/Rendezvous modes
  - AES-128/192/256 encryption (CTR/GCM)
  - Stream ID access control
  - SRT FEC (row/col/staircase), ARQ tuning
  - SMPTE 2022-7 hitless redundancy (dual-network)
- [ ] **RIST Ingest** (`mediaedge-media/src/ingest/rist.rs`):
  - Simple Profile (VSF TR-06-1)
  - RTCP NACK retransmission
  - Pure-Rust `bilbycast-rist` integration
- [ ] **WHIP/WHEP Ingest** (`mediaedge-media/src/ingest/whip.rs`):
  - WebRTC ingest via WHIP (client + server modes)
  - Pure-Rust `str0m` integration
  - H.264 + Opus (HEVC auto-transcode to H.264 for browser compat)
- [ ] **RTSP Pull** (`mediaedge-media/src/ingest/rtsp.rs`):
  - TCP/UDP transport, auto-reconnect
  - H.264/H.265 from IP cameras
- [ ] Unified `FragmentBroadcasterRegistry` (like LVQR) - all ingests funnel to single fragment model

**Acceptance Criteria**:
- Each protocol: 10k+ concurrent streams per node
- < 50ms ingest-to-fragment latency
- Graceful degradation on protocol errors
- Metrics per protocol (ingest_errors_total, ingest_bitrate_bytes)

---

### Issue #5: Protocol Egress Implementations
**Labels**: `phase-1`, `data-plane`, `egress`, `backend`
**Business Value**: Delivering content to viewers - direct revenue impact

**Subtasks**:
- [ ] **LL-HLS Egress** (`mediaedge-media/src/egress/hls.rs`):
  - TS + fMP4 segments, LL-HLS (part-based)
  - HEVC/HDR support
  - In-process TS audio remuxing
  - Per-segment audio re-encode (AAC/MP2/AC-3)
- [ ] **MPEG-DASH Egress** (`mediaedge-media/src/egress/dash.rs`):
  - CMAF-LL, SegmentTemplate + SegmentTimeline
  - Multi-period for ad insertion (SCTE-35)
- [ ] **WHEP Egress** (`mediaedge-media/src/egress/whep.rs`):
  - WebRTC playback via WHEP
  - WHIP/WHEP signaling endpoints
- [ ] **MoQ over QUIC/WebTransport** (`mediaedge-media/src/egress/moq.rs`):
  - MoQ draft-22, native QUIC + WebTransport
  - Hang catalog + CMAF subscription
  - `0.timing` sidecar track for glass-to-glass
- [ ] **WebSocket fMP4** (`mediaedge-media/src/egress/ws.rs`):
  - Low-latency fragment delivery
- [ ] **RTMP/SRT/RTSP Play** (`mediaedge-media/src/egress/legacy.rs`):
  - RTMP/SRT/RTSP pull for legacy players

**Acceptance Criteria**:
- 100k+ concurrent viewers per node (HLS/DASH)
- < 100ms glass-to-glass for LL-HLS/MoQ
- SCTE-35 passthrough for SSAI
- CDN-friendly cache headers

---

### Issue #6: Transcoding Pipeline
**Labels**: `phase-1`, `data-plane`, `transcoding`, `backend`
**Business Value**: Multi-bitrate delivery, format normalization, cost optimization

**Subtasks**:
- [ ] `mediaedge-media/src/transcode/pipeline.rs` - FFmpeg-next based pipeline (decode → filter → encode)
- [ ] Hardware acceleration: NVENC (NVIDIA), VAAPI (Intel/AMD), VideoToolbox (Apple)
- [ ] Video: H.264, HEVC, AV1 (libx264, libx265, libaom, NVENC)
- [ ] Audio: AAC-LC, HE-AAC v1/v2, Opus, MP2, AC-3
- [ ] Per-stream adaptive bitrate ladder (configurable presets)
- [ ] Thumbnail generator (keyframe extraction, configurable interval)
- [ ] SCTE-35 ad marker preservation through transcode
- [ ] Transcode job queue with priority (live > VOD > archive)

**Acceptance Criteria**:
- 4K60 HEVC → 1080p30 H.264 in < 200ms latency
- GPU utilization > 80% under load
- Zero frame drops at configured bitrates
- Transcode metrics: queue_depth, active_jobs, gpu_utilization

---

## Phase 2: Cluster Plane (Weeks 7-10)

### Issue #7: Gossip-Based Cluster Membership
**Labels**: `phase-2`, `cluster`, `backend`
**Business Value**: Horizontal scaling, automatic failover, zero-config clustering

**Subtasks**:
- [ ] `mediaedge-core/src/cluster/gossip.rs` - Chitchat-style gossip (membership + ownership KV)
- [ ] `mediaedge-core/src/cluster/ownership.rs` - Broadcast ownership with lease renewal
- [ ] `mediaedge-core/src/cluster/capacity.rs` - Per-node capacity advertisement (CPU, mem, bandwidth, GPU)
- [ ] `mediaedge-core/src/cluster/redirect.rs` - HTTP 302 redirect-to-owner for HLS/DASH/RTSP
- [ ] `mediaedge-core/src/cluster/config.rs` - LWW (Last-Writer-Wins) config sync via gossip
- [ ] Admin API: `GET /api/v1/cluster/{nodes,broadcasts,config}`

**Acceptance Criteria**:
- 100-node cluster converges in < 5s
- Ownership failover < 2s on node loss
- Config changes propagate in < 1s
- No split-brain under network partition

---

### Issue #8: Cross-Cluster Federation
**Labels**: `phase-2`, `cluster`, `federation`, `backend`
**Business Value**: Multi-region, multi-provider, disaster recovery

**Subtasks**:
- [ ] `mediaedge-core/src/federation/link.rs` - FederationLink (authenticated one-way MoQ pulls)
- [ ] Exponential backoff reconnect (base 1s, cap 60s, ±10% jitter)
- [ ] `GET /api/v1/cluster/federation` - per-link state, last_connected, last_error, attempts
- [ ] mTLS between federated clusters
- [ ] Stream mirroring with offset compensation

**Acceptance Criteria**:
- Federation survives 5min network outage
- < 1s failover to federated peer
- Audit log of all cross-cluster pulls

---

### Issue #9: Browser Peer Mesh (WebRTC DataChannel)
**Labels**: `phase-2`, `cluster`, `mesh`, `frontend`, `backend`
**Business Value**: CDN cost reduction, edge offload, viewer-to-viewer relay

**Subtasks**:
- [ ] `mediaedge-edge/src/mesh/planner.rs` - Topology planner (capacity-aware)
- [ ] `mediaedge-edge/src/mesh/signaling.rs` - WebSocket signaling + server-side subscriber registration
- [ ] `mediaedge-edge/src/mesh/webrtc.rs` - WebRTC DataChannel parent/child relay
- [ ] CLI flags: `--mesh-enabled`, `--max-peers`, `--mesh-root-peer-count`, `--mesh-ice-servers`
- [ ] Client SDK: `@mediaedge/core` (TypeScript) for browser relay
- [ ] TURN deployment recipe + `coturn.conf`
- [ ] Playwright test: 3-peer chain signal-to-DataChannel delivery

**Acceptance Criteria**:
- 50%+ viewer traffic offloaded to mesh at scale
- Mesh self-heals on peer disconnect < 500ms
- Per-peer capacity influences planner (capacity: 1 forces descent)
- Zero mesh traffic through origin for relayed viewers

---

## Phase 3: Storage & Replication (Weeks 11-14)

### Issue #10: Replication Coordinator with Erasure Coding
**Labels**: `phase-3`, `storage`, `replication`, `backend`
**Business Value**: Durability, geo-redundancy, cost-efficient storage

**Subtasks**:
- [ ] `mediaedge-replication/src/coordinator.rs` - Raft-based coordinator (etcd-client)
- [ ] `mediaedge-replication/src/erasure.rs` - Reed-Solomon erasure coding (k=10, m=4 default)
- [ ] `mediaedge-replication/src/planner.rs` - Replication planner:
  - Continent/region staging
  - Demand-triggered local replicas
  - Storage capacity awareness
  - Anti-affinity (nearby nodes not redundant mirrors)
- [ ] `mediaedge-replication/src/mesh.rs` - RaptorQ-FEC datagram protocol for cache sync
- [ ] `/api/mesh` endpoint: planned replicas, stream IDs, edge services, FEC outcomes
- [ ] Baseline staging + demand-triggered replicas

**Acceptance Criteria**:
- 99.9999999% durability (10+4 erasure)
- < 1% storage overhead vs 3x replication
- Replica promotion < 5s on node loss
- FEC repair recovers 99% of losses at 3% packet loss

---

### Issue #11: Storage Node - Object + Archive
**Labels**: `phase-3`, `storage`, `backend`
**Business Value**: VOD, time-shift, archive, compliance

**Subtasks**:
- [ ] `services/storage-node/src/main.rs` - Opendal-based (FS, S3, AzBlob, GCS, memory)
- [ ] Cluster DVR: unified archive view across all nodes
- [ ] Edge caching: cache somebody else's archive without recording own
- [ ] VOD catalogue: single catalogue in central, upload from browser
- [ ] Archive tiering: hot (NVMe) → warm (SSD) → cold (S3/Glacier)
- [ ] Orphaned recording detection + cleanup
- [ ] Disk space breakdown: current archive, orphaned, free

**Acceptance Criteria**:
- Archive query latency < 10ms (hot tier)
- Automatic tiering based on access patterns
- Zero data loss on node failure (erasure coded)
- `GET /api/v1/storage/usage` shows per-tier breakdown

---

## Phase 4: Gateway & Edge Services (Weeks 15-18)

### Issue #12: Gateway Service - Unified Entry Point
**Labels**: `phase-4`, `gateway`, `backend`
**Business Value**: Single IP for all protocols, TLS termination, auth, routing

**Subtasks**:
- [ ] `services/gateway/src/main.rs` - Axum-based gateway
- [ ] Protocol multiplexing: single port for RTMP/SRT/RIST/WHIP/HTTP
- [ ] TLS termination (rustls) with ACME/Let's Encrypt + custom certs
- [ ] Auth integration: JWT (HS256/JWKS), webhook, static tokens
- [ ] Request routing to owning node (302 redirect or proxy)
- [ ] Rate limiting per tenant/stream/IP
- [ ] WebSocket upgrade for WHIP/WHEP/MoQ
- [ ] Admin API: stream CRUD, config, metrics, cluster views

**Acceptance Criteria**:
- 10k+ concurrent connections per gateway
- TLS handshake < 5ms (session resumption)
- Auth check < 1ms (ArcSwap config)
- Zero-downtime config reload

---

### Issue #13: Edge Node - CDN Integration
**Labels**: `phase-4`, `edge`, `cdn`, `backend`
**Business Value**: Global delivery, latency reduction, origin offload

**Subtasks**:
- [ ] `services/edge-node/src/main.rs` - Edge node service
- [ ] HLS/DASH segment caching with range request support
- [ ] Cache warming via replication coordinator
- [ ] Cache invalidation on stream end/purge
- [ ] Multi-tenant cache isolation
- [ ] CDN origin shield (single origin pull per segment)
- [ ] Signed playback URLs (HMAC, JWT, time-limited)
- [ ] Client IP geo-routing to nearest edge

**Acceptance Criteria**:
- Cache hit ratio > 95% for live
- < 5ms cache lookup latency
- Signed URLs unforgeable
- Edge scales to 1M concurrent viewers

---

### Issue #14: Media Processor - VOD/Transcode/Recording
**Labels**: `phase-4`, `media-processor`, `backend`
**Business Value**: VOD pipeline, recording, post-processing

**Subtasks**:
- [ ] `services/media-processor/src/main.rs` - Async job processor
- [ ] Recording: branched from live path, crash recovery, MP4 export
- [ ] VOD processing: transcode ladder, thumbnail sprites, HLS/DASH packaging
- [ ] Scheduled recording (cron-like), retention policies
- [ ] Caption/translation pipeline (WebVTT, AI-assisted)
- [ ] Egress: RTMP/SRT push to cloud (YouTube, Twitch, custom)
- [ ] Production controls: scenes, transitions, audio mixing
- [ ] Connector hooks: record/convert/upload events

**Acceptance Criteria**:
- Recording survives process crash (resume from last keyframe)
- VOD ready < 5min after live ends
- 100+ concurrent transcode jobs
- Webhook delivery with retry + dead letter queue

---

## Phase 5: Multi-Tenancy & Monetization (Weeks 19-22)

### Issue #15: Multi-Tenant Architecture
**Labels**: `phase-5`, `multi-tenant`, `auth`, `backend`
**Business Value**: SaaS model, isolation, compliance

**Subtasks**:
- [ ] `mediaedge-auth/src/tenant.rs` - Tenant CRUD, resource quotas
- [ ] `mediaedge-auth/src/rbac.rs` - Roles: admin, operator, viewer, api-client
- [ ] `mediaedge-auth/src/oidc.rs` - OIDC SSO (Keycloak, Auth0, Azure AD, Google)
- [ ] `mediaedge-auth/src/mfa.rs` - TOTP, WebAuthn
- [ ] Encrypted backup + envelope-encrypted secrets at rest (age + KMS)
- [ ] Brand theming per tenant (console UI)
- [ ] Audit log (immutable, tamper-evident)

**Acceptance Criteria**:
- 10k+ tenants per cluster
- Zero cross-tenant data leakage
- SSO login < 2s
- Audit log query < 100ms

---

### Issue #16: Monetization Stack - Hybrid OTT Models
**Labels**: `phase-5`, `monetization`, `business`, `backend`
**Business Value**: Direct revenue - subscriptions, ads, FAST, TVOD, bundles

**Subtasks**:
- [ ] **Entitlement Service** (`mediaedge-core/src/monetization/entitlement.rs`):
  - Per-user, per-title access decisions
  - Partner-issued subscription acceptance
  - TTL caching (Redis)
- [ ] **SVOD Billing Integration** (`mediaedge-core/src/monetization/billing.rs`):
  - Cleeng, Evergent, Chargebee, RevenueCat webhooks
  - Churn recovery (dunning, grace periods)
- [ ] **SSAI + Ad Server** (`mediaedge-media/src/monetization/ssai.rs`):
  - AWS MediaTailor, Yospace, Broadpeak integration
  - SCTE-35 marker-based ad insertion
  - Prebid.js header bidding
  - Google Ad Manager integration
- [ ] **FAST Channel Scheduler** (`mediaedge-core/src/monetization/fast.rs`):
  - Linear scheduler, EPG generation
  - SCTE-35 for ad breaks
  - AI-assisted scheduling (Amazon Bedrock AgentCore)
- [ ] **TVOD Storefront** (`mediaedge-core/src/monetization/tvod.rs`):
  - Rental/purchase entitlement windows
  - Refund handling
- [ ] **Bundle Support** (`mediaedge-core/src/monetization/bundle.rs`):
  - Streamer-to-streamer, telco, aggregator bundles
  - Partner subscription validation

**Acceptance Criteria**:
- SSAI ad fill rate > 95%
- FAST channel creation < 1min
- Entitlement check < 5ms (cached)
- Supports 5+ simultaneous monetization models

---

### Issue #17: AI-Assisted Operations
**Labels**: `phase-5`, `ai`, `operations`, `backend`
**Business Value**: Cost reduction, revenue optimization, operational efficiency

**Subtasks**:
- [ ] **AI Scheduling** (`mediaedge-core/src/ai/scheduler.rs`):
  - Amazon Bedrock AgentCore + SageMaker integration
  - Content performance + audience behavior + ad revenue optimization
  - 60% schedule creation time reduction (per OTTera case study)
- [ ] **Anomaly Detection** (`mediaedge-observability/src/ai/anomaly.rs`):
  - Stream health anomalies (bitrate drops, error spikes)
  - Predictive failure (disk, GPU, network)
- [ ] **Capacity Planning** (`mediaedge-core/src/ai/capacity.rs`):
  - ML-based viewer prediction for pre-scaling
  - Cost optimization (spot vs on-demand)
- [ ] **Content Moderation** (`mediaedge-media/src/ai/moderation.rs`):
  - AI-based content analysis (NSFW, copyright, quality)

**Acceptance Criteria**:
- Scheduling AI improves engagement 15-25%
- Anomaly detection < 1min MTTR
- Capacity prediction MAE < 10%
- All AI features opt-in, auditable

---

## Phase 6: Console & Developer Experience (Weeks 23-26)

### Issue #18: Web Console (React/TypeScript)
**Labels**: `phase-6`, `console`, `frontend`
**Business Value**: Operator productivity, self-service, reduced support burden

**Subtasks**:
- [ ] `apps/console/` - React + TypeScript + Vite
- [ ] Dashboard: real-time viewer/traffic charts, error heatmap
- [ ] Stream management: create, monitor, debug per-stream
- [ ] Cluster view: node health, capacity, layouter decisions with reasons
- [ ] Source diagnostics: MPEG-TS continuity, bitrate floor, audio loss
- [ ] Layouter decision history (why stream placed on node X)
- [ ] Backup source health monitoring (probed/ready/never-probed)
- [ ] Tenant management: users, roles, quotas, billing
- [ ] Monetization: FAST channel builder, ad break preview, revenue reports
- [ ] Dark/light mode, i18n

**Acceptance Criteria**:
- Console loads < 2s
- Real-time updates via SSE/WebSocket
- Mobile-responsive
- Accessibility WCAG 2.1 AA

---

### Issue #19: SDKs & CLI
**Labels**: `phase-6`, `sdk`, `cli`, `developer-experience`
**Business Value**: Ecosystem, integration, adoption

**Subtasks**:
- [ ] `crates/mediaedge-client-rust/` - Rust async client
- [ ] `apps/sdk-js/` - TypeScript client (browser + Node)
- [ ] `apps/sdk-go/` - Go client
- [ ] `crates/mediaedge-cli/` - `mediaedgectl` (like `stunnerctl`/`mediactl`)
  - Stream CRUD, config, metrics, cluster ops
  - `mediaedgectl test` - end-to-end media flow test (like `mediactl`)
- [ ] OpenAPI-generated clients for Python, Java, .NET
- [ ] Terraform provider for infrastructure-as-code

**Acceptance Criteria**:
- All SDKs publish to package registries (crates.io, npm, pkg.go.dev)
- CLI completes common tasks in < 5 commands
- 100% API coverage in SDKs

---

## Phase 7: Standards & Interoperability (Weeks 27-30)

### Issue #20: Broadcast Standards Compliance
**Labels**: `phase-7`, `standards`, `broadcast`, `backend`
**Business Value**: Enterprise/broadcast market, professional workflows

**Subtasks**:
- [ ] **NMOS IS-04/05/08** (`mediaedge-core/src/standards/nmos.rs`):
  - Discovery & Registration, Connection Management, Audio Channel Mapping
  - mDNS-SD `_nmos-node._tcp` registration
  - Optional auth protection
- [ ] **SMPTE ST 2110** (`mediaedge-media/src/standards/st2110.rs`):
  - 2110-20 (video), -23 (audio), -30/-31 (ancillary), -40 (metadata)
  - PTP synchronization, 2022-7 Red/Blue redundancy
  - BCP-004 receiver capability sets
- [ ] **SMPTE 2022-1 FEC** (`mediaedge-media/src/standards/smpte2022.rs`):
  - Forward error correction for RTP/UDP
- [ ] **TR-101290** monitoring (MPEG-TS quality metrics)
- [ ] **SRT/RIST** interop testing with librist, SRT reference impl

**Acceptance Criteria**:
- NMOS IS-04/05/08 certified (JT-NM Tested)
- ST 2110 interop with Lawo, Imagine, Evertz
- FEC recovers 100% single-packet loss
- TR-101290 alerts on continuity errors

---

### Issue #21: MoQ (Media over QUIC) Full Implementation
**Labels**: `phase-7`, `moq`, `quic`, `backend`
**Business Value**: Next-gen low-latency delivery, IETF standard track

**Subtasks**:
- [ ] `mediaedge-media/src/moq/` - Full MoQ draft-22 implementation
- [ ] Native QUIC + WebTransport support
- [ ] Hang catalog + CMAF subscription model
- [ ] `0.timing` track for glass-to-glass
- [ ] MoQ relay (cache + forward)
- [ ] Interop with `moq-whep`, `moq-transport` reference impls

**Acceptance Criteria**:
- < 50ms glass-to-glass MoQ
- MoQ relay scales to 10k subscribers
- Passes MoQ interop test suite

---

## Phase 8: Advanced Features (Weeks 31-36)

### Issue #22: Real-Time Rooms (SFU/Simulcast)
**Labels**: `phase-8`, `realtime`, `webrtc`, `backend`
**Business Value**: Video conferencing, interactive live, virtual events

**Subtasks**:
- [ ] `mediaedge-edge/src/rooms/sfu.rs` - Selective Forwarding Unit
- [ ] Simulcast (multiple quality layers)
- [ ] Spotlight (active speaker highlighting)
- [ ] Forwarding filters (bandwidth adaptation)
- [ ] DataChannels (chat, screenshare, whiteboard)
- [ ] Join tokens (Ed25519 signed)
- [ ] Events on SSE (join/leave, quality change)
- [ ] E2EE pass-through (insertable streams)
- [ ] Browser SDK integration

**Acceptance Criteria**:
- 1000+ participants per room
- Simulcast layer switching < 100ms
- E2EE zero server-side decryption
- Room state recovers on SFU restart

---

### Issue #23: Media Pipelines (Compose/Record/Transcode)
**Labels**: `phase-8`, `pipelines`, `backend`
**Business Value**: Production workflows, virtual studios, automated highlights

**Subtasks**:
- [ ] `mediaedge-media/src/pipelines/` - Pipeline DSL (decode → compose → encode)
- [ ] Export room participants as streams
- [ ] Composite layouts (grid, picture-in-picture, custom)
- [ ] Record composite output
- [ ] Push composite over RTMP/SRT
- [ ] Transcribe (ASR integration: Whisper, cloud APIs)
- [ ] AI highlight detection (scene change, audio energy, speech)

**Acceptance Criteria**:
- Pipeline definition in YAML/JSON
- 4K composite at 60fps
- Transcription accuracy > 95%
- Highlight detection F1 > 0.9

---

### Issue #24: Grid Orchestration (Resource-Aware Scheduling)
**Labels**: `phase-8`, `grid`, `orchestration`, `backend`
**Business Value**: Multi-node efficiency, GPU utilization, cost optimization

**Subtasks**:
- [ ] `mediaedge-core/src/grid/layouter.rs` - 129-rule layouter (per Flussonic Catena)
- [ ] Resource registry: CPU, memory, bandwidth, GPUs, capture devices
- [ ] Placement requirements: labels, namespaces, GPU affinity, archive storage
- [ ] Preferences for tie-breaking
- [ ] Reassignment on node offline/drain/leave (frozen nodes keep assignments)
- [ ] Overload relief order: network → archive copies → ingest → heaviest stream first
- [ ] Roles beyond ingest: archive copies, leftover archive, edge delivery, timeshift
- [ ] Decision audit trail (why each assignment made)

**Acceptance Criteria**:
- Layouter handles 1000+ streams, 100+ nodes
- 96% of placement issues are config errors (visible in console)
- Overload relief completes < 30s
- Zero manual intervention for routine scaling

---

## Phase 9: Security & Compliance (Weeks 37-40)

### Issue #25: Zero-Trust Security
**Labels**: `phase-9`, `security`, `compliance`, `backend`
**Business Value**: Enterprise sales, regulatory compliance, trust

**Subtasks**:
- [ ] mTLS everywhere (service-to-service, cluster, federation)
- [ ] SPIFFE/SPIRE integration for identity
- [ ] Network policies (Cilium/Calico)
- [ ] Secrets: HashiCorp Vault, AWS Secrets Manager, Azure Key Vault
- [ ] Runtime security: Falco, eBPF-based syscall monitoring
- [ ] SBOM generation (Syft) + vulnerability scanning (Grype) in CI
- [ ] FIPS 140-2 mode (rustls fips)
- [ ] GDPR/CCPA data handling (right to deletion, data portability)

**Acceptance Criteria**:
- Zero critical CVEs in dependencies
- mTLS handshake < 10ms
- SOC 2 Type II ready
- GDPR deletion request < 24h

---

## Phase 10: Business Case Validation (Ongoing)

### Issue #26: Reference Deployments & Case Studies
**Labels**: `phase-10`, `business`, `documentation`, `marketing`
**Business Value**: Sales enablement, credibility, adoption

**Subtasks**:
- [ ] **Self-hosted OTT** (like Tivify/TVUP): 1M+ users, ad-funded platform
- [ ] **FAST Channel Platform** (like OTTera): 2000+ channels, AI scheduling
- [ ] **Broadcast Contribution** (like Bilbycast): SRT/RIST/ST2110, NMOS
- [ ] **Enterprise Video** (like YuppTV/YVS): 100M+ users, multi-tenant
- [ ] **Low-Latency Interactive** (like LVQR): MoQ, WebRTC mesh, <100ms
- [ ] **Cost Analysis**: Infrastructure cost per 1M viewer-hours vs competitors
- [ ] **Migration Guides**: From Wowza, Flussonic, Ant Media, custom stacks

**Acceptance Criteria**:
- 5+ production reference architectures
- Benchmark results published
- Migration docs for top 5 competitors
- TCO calculator spreadsheet

---

## Implementation Priority Matrix

| Priority | Issue | Dependencies | Est. Weeks | Business Impact |
|----------|-------|--------------|------------|-----------------|
| P0 | #1 Config | - | 2 | Unlocks all services |
| P0 | #2 Observability | - | 2 | Production readiness |
| P0 | #3 API | - | 1 | Contract-first dev |
| P1 | #4 Ingest | #1, #2, #3 | 4 | Revenue intake |
| P1 | #5 Egress | #1, #2, #3 | 4 | Viewer delivery |
| P1 | #6 Transcode | #4, #5 | 3 | Quality/bandwidth |
| P2 | #7 Cluster | #1 | 4 | Horizontal scale |
| P2 | #8 Federation | #7 | 3 | Multi-region |
| P2 | #9 Mesh | #7 | 4 | CDN cost reduction |
| P3 | #10 Replication | #1, #7 | 4 | Durability |
| P3 | #11 Storage | #10 | 3 | VOD/Archive |
| P3 | #12 Gateway | #4, #5, #7 | 3 | Unified entry |
| P3 | #13 Edge | #12 | 3 | Global delivery |
| P3 | #14 Media Processor | #6, #11 | 3 | VOD pipeline |
| P4 | #15 Multi-tenant | #12, #3 | 4 | SaaS model |
| P4 | #16 Monetization | #15 | 4 | Direct revenue |
| P4 | #17 AI Ops | #2, #16 | 3 | Optimization |
| P5 | #18 Console | #15 | 4 | Operator UX |
| P5 | #19 SDK/CLI | #3 | 3 | Ecosystem |
| P6 | #20 Standards | #4, #5 | 4 | Enterprise sales |
| P6 | #21 MoQ | #5 | 3 | Next-gen delivery |
| P7 | #22 Rooms | #9, #5 | 4 | Interactive |
| P7 | #23 Pipelines | #6, #22 | 3 | Production |
| P7 | #24 Grid | #7, #10 | 4 | Efficiency |
| P8 | #25 Security | All | 4 | Compliance |
| P9 | #26 Business Cases | All | Ongoing | Sales |

---

## GitHub Issue Creation Commands

Since `gh` CLI is unavailable, create issues manually at:
- `https://github.com/RasimAghayev/mediaedge/issues/new`

Use these templates for each issue:

### Template: Feature Issue
```markdown
## Title: [Phase X] Short Description

**Labels**: `phase-X`, `component`, `backend|frontend`

### Business Value
[1-2 sentences on revenue/cost/strategic value]

### Description
[Detailed technical specification]

### Subtasks
- [ ] Subtask 1
- [ ] Subtask 2
...

### Acceptance Criteria
- [ ] Measurable criterion 1
- [ ] Measurable criterion 2
...

### Dependencies
- Issue #X (link)

### Estimated Effort
[Weeks] | [Story Points]
```

### Template: Subtask Issue (for large features)
```markdown
## Title: [Phase X] Component: Specific Subtask

**Labels**: `phase-X`, `component`, `subtask`

### Parent Issue
#X

### Description
[Focused technical detail]

### Acceptance Criteria
- [ ] Specific measurable outcome

### Estimated Effort
[Days]
```

---

## Continuous Research Loop

### Weekly Research Tasks (Automated via Cron)
1. **Monday**: Search GitHub trending for `media-server`, `streaming`, `webrtc`, `moq`, `srt`, `rist`
2. **Wednesday**: Check competitor releases (Flussonic, Wowza, Ant Media, LVQR, SENCA, Bilbycast, STUNner)
3. **Friday**: Review IETF drafts (MoQ, WHIP/WHEP, SRT, RIST, NMOS)

### Monthly Business Case Review
- Analyze OTT market reports (Frost & Sullivan, Deloitte, Forrester)
- Track FAST channel growth, AVOD/SSAI adoption
- Monitor AI in media (scheduling, moderation, compression)
- Update monetization models based on Netflix/Disney/Warner earnings

### Quarterly Architecture Review
- Benchmark against latest LVQR/SENCA/Catena releases
- Evaluate new Rust crates (async, media, observability)
- Assess cloud provider media services (AWS Elemental, Azure Media, GCP Transcoder)
- Update tech stack decisions

---

## Success Metrics (KPIs)

### Technical
- [ ] 100k concurrent viewers per node (HLS/DASH)
- [ ] 10k concurrent WebRTC viewers per node
- [ ] < 50ms glass-to-glass (MoQ/LL-HLS)
- [ ] 99.999% uptime (SLA)
- [ ] < 1s failover time
- [ ] Zero data loss (erasure coded)

### Business
- [ ] 5+ production deployments
- [ ] TCO 40% lower than commercial alternatives
- [ ] 10+ enterprise customers
- [ ] $1M+ ARR within 18 months
- [ ] 90%+ customer retention

### Operational
- [ ] < 1hr MTTR for critical incidents
- [ ] < 5min deploy time (CI/CD)
- [ ] 100% API coverage in tests
- [ ] Zero critical CVEs

---

## Next Immediate Actions

1. **Create GitHub Issues** for Phase 0 (Issues #1, #2, #3) - CRITICAL PATH
2. **Assign owners** for each issue
3. **Set up project board** with phases as columns
4. **Configure CI** to run on every PR (already done in bb7a498)
5. **Start implementation** with Issue #1 (Config System)

---

*Document version: 1.0*
*Last updated: 2026-11-27*
*Next review: 2026-12-04 (weekly)*