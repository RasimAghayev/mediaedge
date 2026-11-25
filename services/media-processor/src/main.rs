//! Async media transcoding worker binary.
//!
//! Consumes `mediaedge.object.created` events and runs transcoding, thumbnails,
//! and HLS/DASH manifest generation. Depends on `mediaedge-media`,
//! `mediaedge-events`, `mediaedge-storage`, and
//! `mediaedge-observability` (see `crates/mediaedge-media/src/lib.rs`
//! and `crates/mediaedge-events/src/lib.rs`).

#![warn(missing_docs)]

use mediaedge_observability::tracing as obs_tracing;

#[tokio::main]
async fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;
    let _tracing = obs_tracing::init();
    tracing::info!("mediaedge-media-processor starting");
    Ok(())
}
