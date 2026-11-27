//! etcd integration, schema, and transactions for MediaEdge metadata.
//!
//! Provides strongly consistent metadata storage via etcd with transactional
//! operations and watches. Called by `mediaedge-core` services and
//! `services/storage-node` (see `services/storage-node/src/main.rs`).

#![warn(missing_docs)]

pub mod bucket;
pub mod client;
pub mod object;
pub mod schema;
pub mod transaction;
