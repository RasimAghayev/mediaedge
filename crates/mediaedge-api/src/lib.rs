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

pub mod grpc;
pub mod middleware;
pub mod openapi;
pub mod rate_limit;
pub mod validation;
pub mod versioning;

// Re-export main types
pub use grpc::{
    mediaedge, // generated gRPC module
};
pub use middleware::{ApiMiddlewareConfig, api_middleware_stack};
pub use openapi::{ApiDoc, OpenApiConfig, create_openapi_router};
pub use rate_limit::{RateLimitConfig, RateLimitMiddleware, TokenBucket};
pub use validation::{
    ValidatedJson, ValidatedPath, ValidatedQuery, ValidationConfig, ValidationMiddleware,
};
pub use versioning::{ApiVersion, VersioningConfig, VersioningMiddleware};

/// Prelude for convenient imports.
pub mod prelude {
    pub use crate::{
        ApiDoc, ApiMiddlewareConfig, ApiVersion, OpenApiConfig, RateLimitConfig,
        RateLimitMiddleware, ValidatedJson, ValidatedPath, ValidatedQuery, ValidationMiddleware,
        VersioningMiddleware, api_middleware_stack, create_openapi_router,
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
