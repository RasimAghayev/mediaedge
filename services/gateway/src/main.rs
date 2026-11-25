//! API gateway binary: Axum HTTP server + S3 compatibility layer.
//!
//! Serves the `mediaedge-api` routes. Depends on `mediaedge-api`,
//! `mediaedge-core`, `mediaedge-auth`, `mediaedge-cache`,
//! `mediaedge-observability`, and `mediaedge-config` (see
//! `crates/mediaedge-api/src/lib.rs`).

#![warn(missing_docs)]

use mediaedge_observability::tracing as obs_tracing;
use mediaedge_config::loader;

#[tokio::main]
async fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;
    let _config = loader::load()?;
    let _tracing = obs_tracing::init();
    tracing::info!("mediaedge-gateway starting");
    Ok(())
}
