//! Domain models and core business logic for MediaEdge.
//!
//! Contains the core domain entities (Bucket, Object, ObjectVersion,
//! MultipartUpload, EdgeNode, ReplicationRule, MediaJob) and the services
//! that operate on them. Called by `mediaedge-api` route handlers and
//! `services/*` binaries.

#![warn(missing_docs)]

pub mod models;
pub mod services;
