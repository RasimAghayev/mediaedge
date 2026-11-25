//! Redis integration and cache strategies for MediaEdge.
//!
//! Hot cache for signed URLs, rate limits, and session data. Called by
//! `mediaedge-api` and `services/gateway` (see
//! `services/gateway/src/main.rs`).

#![warn(missing_docs)]

pub mod client;
pub mod strategies;
