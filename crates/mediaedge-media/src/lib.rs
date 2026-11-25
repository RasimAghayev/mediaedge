//! Media processing: transcoding, thumbnails, HLS/DASH, signed URLs.
//!
//! Built on ffmpeg-next, image, and ravif. Called by
//! `services/media-processor` (see `services/media-processor/src/main.rs`).

#![warn(missing_docs)]

pub mod transcode;
pub mod thumbnail;
pub mod manifest;
pub mod signed_url;
