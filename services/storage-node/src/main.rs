//! Storage node binary: object put/get/delete and health.
//!
//! Serves object operations backed by `mediaedge-storage`. Depends on
//! `mediaedge-storage`, `mediaedge-metadata`, and
//! `mediaedge-observability` (see `crates/mediaedge-storage/src/lib.rs`
//! and `crates/mediaedge-metadata/src/lib.rs`).

#![warn(missing_docs)]

use mediaedge_observability::tracing as obs_tracing;

#[tokio::main]
async fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;
    let _tracing = obs_tracing::init();
    tracing::info!("mediaedge-storage-node starting");
    Ok(())
}
