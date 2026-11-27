//! NATS JetStream event definitions, publishers, and consumers.
//!
//! Event subjects follow `mediaedge.<domain>.<action>.<entity>.<id>`
//! structure. Consumers: replication-coordinator, media-processor,
//! edge-sync, analytics, monitoring. Called by `services/*` binaries and
//! `mediaedge-core` (see `services/media-processor/src/main.rs`).

#![warn(missing_docs)]

pub mod consumer;
pub mod outbox;
pub mod publisher;
pub mod subjects;
