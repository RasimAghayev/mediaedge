//! MediaEdge API Layer
//!
//! Contract-first API development with:
//! - OpenAPI 3.1 specification via Utoipa
//! - gRPC protobuf definitions for internal service communication
//! - Code generation for TypeScript/Rust/Go clients
//! - Request/response validation middleware
//! - API versioning strategy (header + URL fallback)
//! - Rate limiting middleware (token bucket, Redis-backed)

#![warn(missing_docs)]

pub mod openapi;
pub mod grpc;
pub mod validation;
pub mod versioning;
pub mod rate_limit;
pub mod middleware;

// Re-export main types
pub use openapi::{
    ApiDoc, create_openapi_router, OpenApiConfig,
};
pub use grpc::{
    mediaedge, // generated gRPC module
};
pub use validation::{
    ValidationMiddleware, ValidationConfig, ValidatedJson, ValidatedQuery, ValidatedPath,
};
pub use versioning::{
    ApiVersion, VersioningMiddleware, VersioningConfig,
};
pub use rate_limit::{
    RateLimitMiddleware, RateLimitConfig, TokenBucket,
};
pub use middleware::{
    api_middleware_stack, ApiMiddlewareConfig,
};

/// Prelude for convenient imports.
pub mod prelude {
    pub use crate::{
        ApiDoc, create_openapi_router, OpenApiConfig,
        ValidationMiddleware, ValidatedJson, ValidatedQuery, ValidatedPath,
        ApiVersion, VersioningMiddleware,
        RateLimitMiddleware, RateLimitConfig,
        api_middleware_stack, ApiMiddlewareConfig,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prelude_imports() {
        // Just verify prelude compiles
    }
}