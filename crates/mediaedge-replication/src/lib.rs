//! Geo-replication, CRDT/last-writer-wins, consistency, failover.
//!
//! Orchestrates cross-region replication between buckets. Called by
//! `services/replication-coordinator` (see
//! `services/replication-coordinator/src/main.rs`).

#![warn(missing_docs)]

pub mod coordinator;
pub mod crdt;
pub mod failover;
pub mod rule;
