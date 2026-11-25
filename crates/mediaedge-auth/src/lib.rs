//! Authentication and authorization (AuthN/AuthZ), IAM policies, tokens.
//!
//! Implements S3 Signature v4, OIDC/JWT, and IAM-style policy evaluation.
//! Called by `mediaedge-api` route handlers (see
//! `crates/mediaedge-api/src/routes/`).

#![warn(missing_docs)]

pub mod authn;
pub mod authz;
pub mod policy;
pub mod token;
pub mod signing;
