//! Storage abstraction, backends, tiering, and erasure coding.
//!
//! Implements the `ObjectStore` trait backed by OpenDAL. Called by
//! `services/storage-node` and `services/edge-node` (see
//! `services/storage-node/src/main.rs` and
//! `services/edge-node/src/main.rs`).

#![warn(missing_docs)]

pub mod backends;
pub mod erasure;
pub mod multipart;
pub mod store;
pub mod tiering;
