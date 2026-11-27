//! Edge node logic: sync, offline queue, conflict resolution.
//!
//! Implements read-through cache, write-behind queue, and the sync protocol.
//! Called by `services/edge-node` (see `services/edge-node/src/main.rs`).

#![warn(missing_docs)]

pub mod cache;
pub mod conflict;
pub mod queue;
pub mod sync;
