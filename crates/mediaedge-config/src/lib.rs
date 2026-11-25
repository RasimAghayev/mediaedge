//! Configuration types, validation, and secrets management.
//!
//! Layered configuration (file, env, CLI) via figment. Called by all
//! services at startup (see `services/gateway/src/main.rs`).

#![warn(missing_docs)]

pub mod types;
pub mod loader;
pub mod secrets;
