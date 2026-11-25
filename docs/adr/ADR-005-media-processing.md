# ADR-005: Media Processing

## Status
Proposed

## Context
Transcoding, thumbnails, HLS/DASH, AVIF/WebP. Options: ffmpeg, GStreamer,
custom Rust pipelines.

## Decision
Use ffmpeg-next (FFmpeg bindings) + image + ravif for media processing in an
async worker (`services/media-processor`).

## Consequences
- FFmpeg runtime dependency in media-processor image
- `ffmpeg-next` FFI unsafe boundaries
