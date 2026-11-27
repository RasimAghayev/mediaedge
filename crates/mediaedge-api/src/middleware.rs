//! Combined API middleware stack.

use axum::{
    body::Body,
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceBuilder;
use tracing::{info, warn};

use crate::{
    rate_limit::{RateLimitConfig, RateLimitLayer, rate_limit_layer},
    validation::{ValidationConfig, ValidationLayer, validation_layer},
    versioning::{VersioningConfig, VersioningLayer, versioning_layer},
    openapi::{OpenApiConfig, create_openapi_router, ApiVersion},
};

/// Combined API middleware configuration.
#[derive(Debug, Clone)]
pub struct ApiMiddlewareConfig {
    pub openapi: OpenApiConfig,
    pub validation: ValidationConfig,
    pub versioning: VersioningConfig,
    pub rate_limit: RateLimitConfig,
}

impl Default for ApiMiddlewareConfig {
    fn default() -> Self {
        Self {
            openapi: OpenApiConfig::default(),
            validation: ValidationConfig::default(),
            versioning: VersioningConfig::default(),
            rate_limit: RateLimitConfig::default(),
        }
    }
}

impl ApiMiddlewareConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_service_name(mut self, name: String) -> Self {
        self.openapi.service_name = name;
        self
    }

    pub fn with_version(mut self, version: String) -> Self {
        self.openapi.version = version;
        self
    }

    pub fn with_validation(mut self, config: ValidationConfig) -> Self {
        self.validation = config;
        self
    }

    pub fn with_versioning(mut self, config: VersioningConfig) -> Self {
        self.versioning = config;
        self
    }

    pub fn with_rate_limit(mut self, config: RateLimitConfig) -> Self {
        self.rate_limit = config;
        self
    }
}

/// Create the complete API middleware stack.
pub fn api_middleware_stack(config: ApiMiddlewareConfig) -> impl tower::Layer<axum::Router> + Clone {
    ServiceBuilder::new()
        .layer(versioning_layer(config.versioning))
        .layer(validation_layer(config.validation))
        .layer(rate_limit_layer(config.rate_limit))
}

/// Create API router with all middleware and OpenAPI docs.
pub fn create_api_router(config: ApiMiddlewareConfig) -> Router {
    let openapi_router = create_openapi_router(&config.openapi);

    Router::new()
        .merge(openapi_router)
        .route("/health", get(health_check))
        .route("/api/health", get(health_check))
        .layer(api_middleware_stack(config))
}

/// Health check endpoint.
async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "mediaedge-api",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Error handling middleware.
pub async fn error_handling_middleware(
    req: axum::http::Request<Body>,
    next: Next,
) -> Response {
    let response = next.run(req).await;
    
    // Add security headers
    let mut response = response;
    let headers = response.headers_mut();
    
    // Security headers
    let _ = headers.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        axum::http::HeaderValue::from_static("nosniff"),
    );
    let _ = headers.insert(
        axum::http::header::X_FRAME_OPTIONS,
        axum::http::HeaderValue::from_static("DENY"),
    );
    let _ = headers.insert(
        axum::http::header::REFERRER_POLICY,
        axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    
    response
}

/// CORS middleware configuration.
#[derive(Debug, Clone)]
pub struct CorsConfig {
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<String>,
    pub allowed_headers: Vec<String>,
    pub allow_credentials: bool,
    pub max_age: u32,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec!["GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS"]
                .into_iter()
                .map(String::from)
                .collect(),
            allowed_headers: vec!["*".to_string()],
            allow_credentials: true,
            max_age: 3600,
        }
    }
}

/// Create CORS layer.
pub fn cors_layer(config: CorsConfig) -> tower_http::cors::CorsLayer {
    use tower_http::cors::{Any, CorsLayer};
    
    let mut cors = CorsLayer::new()
        .allow_methods(config.allowed_methods.parse::<axum::http::Method>().unwrap_or_else(|_| Any))
        .allow_headers(config.allowed_headers.parse::<axum::http::HeaderName>().unwrap_or_else(|_| Any))
        .max_age(std::time::Duration::from_secs(config.max_age as u64));
    
    if config.allow_credentials {
        cors = cors.allow_credentials(true);
    }
    
    if config.allowed_origins.contains(&"*".to_string()) {
        cors = cors.allow_origin(Any);
    } else {
        let origins: Vec<axum::http::HeaderValue> = config.allowed_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        cors = cors.allow_origin(origins);
    }
    
    cors
}

/// Request ID middleware.
pub async fn request_id_middleware(
    mut req: axum::http::Request<Body>,
    next: Next,
) -> Response {
    let request_id = uuid::Uuid::new_v4().to_string();
    
    // Add to request extensions
    req.extensions_mut().insert(request_id.clone());
    
    let mut response = next.run(req).await;
    
    // Add to response headers
    response.headers_mut().insert(
        axum::http::header::HeaderName::from_static("x-request-id"),
        request_id.parse().unwrap(),
    );
    
    response
}

/// Tracing middleware for request/response logging.
pub async fn tracing_middleware(
    req: axum::http::Request<Body>,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let version = req.version();
    let request_id = req.extensions().get::<String>().cloned().unwrap_or_default();
    
    let start = std::time::Instant::now();
    
    info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        version = ?version,
        "Request started"
    );
    
    let response = next.run(req).await;
    
    let latency = start.elapsed();
    let status = response.status();
    
    info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        status = %status,
        latency_ms = %latency.as_millis(),
        "Request completed"
    );
    
    response
}

/// Combined middleware stack with all common middleware.
pub fn full_middleware_stack(config: ApiMiddlewareConfig) -> impl tower::Layer<Router> + Clone {
    ServiceBuilder::new()
        .layer(middleware::from_fn(request_id_middleware))
        .layer(middleware::from_fn(tracing_middleware))
        .layer(middleware::from_fn(error_handling_middleware))
        .layer(api_middleware_stack(config))
}

/// Create full API router with all middleware.
pub fn create_full_api_router(config: ApiMiddlewareConfig) -> Router {
    let openapi_router = create_openapi_router(&config.openapi);

    Router::new()
        .merge(openapi_router)
        .route("/health", get(health_check))
        .route("/api/health", get(health_check))
        .layer(full_middleware_stack(config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        let config = ApiMiddlewareConfig::default();
        let app = create_full_api_router(config);
        
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_request_id_header() {
        let config = ApiMiddlewareConfig::default();
        let app = create_full_api_router(config);
        
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert!(response.headers().contains_key("x-request-id"));
    }

    #[tokio::test]
    async fn test_security_headers() {
        let config = ApiMiddlewareConfig::default();
        let app = create_full_api_router(config);
        
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(
            response.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(
            response.headers().get("x-frame-options").unwrap(),
            "DENY"
        );
    }
}