# MediaEdge Architecture

## Overview

MediaEdge is a high-performance, S3-compatible object storage platform built in
Rust, optimized for media/CDN workloads with native edge/hybrid deployment.

## Bounded Contexts

- Object Storage: Put/Get, multipart, range requests, tiering, erasure coding
- Metadata Store: etcd transactions, schema versioning, indexing
- Media Processing: transcoding, thumbnails, HLS/DASH, signed media URLs
- Edge Sync: read-through cache, write-behind queue, conflict resolution
- Replication: geo-replication, CRDT/last-writer-wins, failover

## Event Bus

NATS JetStream with subjects under `mediaedge.>`. See `docs/architecture/events.md`.
