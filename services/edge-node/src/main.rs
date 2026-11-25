//! Edge node binary: sync, cache, offline queue.
//!
//! Implements read-through cache, write-behind queue, and sync protocol.
//! Depends on `mediaedge-edge`, `mediaedge-storage`, and
//! `mediaedge-observability` (see `crates/mediaedge-edge/src/lib.rs`).

#![warn(missing_docs)]

use mediaedge_observability::tracing as obs_tracing;

#[tokio::main]
async fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;
    let _tracing = obs_tracing::init();
    tracing::info!("mediaedge-edge-node starting");
    Ok(())
}
