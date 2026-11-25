//! Cross-region replication orchestration binary.
//!
//! Coordinates replication rules between buckets across regions. Depends on
//! `mediaedge-replication`, `mediaedge-events`, and
//! `mediaedge-observability` (see `crates/mediaedge-replication/src/lib.rs`
//! and `crates/mediaedge-events/src/lib.rs`).

#![warn(missing_docs)]

use mediaedge_observability::tracing as obs_tracing;

#[tokio::main]
async fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;
    let _tracing = obs_tracing::init();
    tracing::info!("mediaedge-replication-coordinator starting");
    Ok(())
}
