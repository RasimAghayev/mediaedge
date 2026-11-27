//! Integration tests for mediaedge-api.

use axum::{
    Json, Router,
    body::Body,
    http::{HeaderMap, HeaderValue, Request, StatusCode},
    middleware::Next,
    routing::{get, post},
};
use mediaedge_api::{
    middleware::{
        ApiMiddlewareConfig, CorsConfig, cors_layer, create_api_router, create_full_api_router,
    },
    openapi::{ApiVersion, ErrorResponse, OpenApiConfig, create_openapi_router},
    rate_limit::{KeyExtractor, RateLimitConfig, RateLimitLayer, TokenBucket, rate_limit_layer},
    validation::{ValidatedJson, ValidatedQuery, ValidationConfig},
    versioning::{
        VersionSource, VersioningConfig, VersioningLayer, extract_version, get_version_info,
        versioning_layer,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tower::ServiceExt;
use validator::Validate;

#[derive(Debug, Deserialize, Serialize, Validate)]
struct TestRequest {
    #[validate(length(min = 1, max = 100))]
    name: String,

    #[validate(range(min = 1, max = 100))]
    age: u32,

    #[validate(email)]
    email: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
struct TestQuery {
    #[validate(range(min = 1))]
    page: u32,

    #[validate(range(min = 1, max = 100))]
    per_page: u32,
}

#[tokio::test]
async fn test_openapi_router() {
    let config = OpenApiConfig::default();
    let router = create_openapi_router(&config);

    // Test OpenAPI JSON endpoint
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(doc["info"]["title"], "MediaEdge API");
    assert_eq!(doc["info"]["version"], "1.0.0");
}

#[tokio::test]
async fn test_swagger_ui() {
    let config = OpenApiConfig::default();
    let router = create_openapi_router(&config);

    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/docs/swagger")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers().get("content-type").unwrap();
    assert!(content_type.to_str().unwrap().contains("text/html"));
}

#[tokio::test]
async fn test_api_version_from_header() {
    assert_eq!(ApiVersion::from_header("v1"), Some(ApiVersion::V1));
    assert_eq!(ApiVersion::from_header("V1"), Some(ApiVersion::V1));
    assert_eq!(ApiVersion::from_header("1"), Some(ApiVersion::V1));
    assert_eq!(ApiVersion::from_header("v2"), Some(ApiVersion::V2));
    assert_eq!(ApiVersion::from_header("2"), Some(ApiVersion::V2));
    assert_eq!(ApiVersion::from_header("v3"), None);
}

#[tokio::test]
async fn test_validation_config() {
    let config = ValidationConfig::new()
        .with_reject_unknown_fields(true)
        .with_max_body_size(1024 * 1024);

    assert!(config.reject_unknown_fields);
    assert_eq!(config.max_body_size, 1024 * 1024);
}

#[tokio::test]
async fn test_validated_json_success() {
    let config = ValidationConfig::default();
    let app = Router::new()
        .route("/test", post(handler_validated_json))
        .layer(mediaedge_api::validation::validation_layer(config));

    let request_body = json!({
        "name": "John Doe",
        "age": 30,
        "email": "john@example.com"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

async fn handler_validated_json(
    ValidatedJson(payload): ValidatedJson<TestRequest>,
) -> Json<serde_json::Value> {
    Json(json!({ "received": payload }))
}

#[tokio::test]
async fn test_validated_json_failure() {
    let config = ValidationConfig::default();
    let app = Router::new()
        .route("/test", post(handler_validated_json))
        .layer(mediaedge_api::validation::validation_layer(config));

    let request_body = json!({
        "name": "",
        "age": 150,
        "email": "invalid-email"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_validated_query() {
    let config = ValidationConfig::default();
    let app = Router::new()
        .route("/test", get(handler_validated_query))
        .layer(mediaedge_api::validation::validation_layer(config));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/test?page=1&per_page=20")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

async fn handler_validated_query(
    ValidatedQuery(query): ValidatedQuery<TestQuery>,
) -> Json<serde_json::Value> {
    Json(json!({ "query": query }))
}

#[tokio::test]
async fn test_versioning_config() {
    let config = VersioningConfig::new()
        .with_default_version(ApiVersion::V2)
        .with_supported_versions(vec![ApiVersion::V1, ApiVersion::V2])
        .with_header_name("X-Custom-Version".to_string())
        .with_query_param("version".to_string());

    assert_eq!(config.default_version, ApiVersion::V2);
    assert_eq!(config.header_name, "X-Custom-Version");
    assert_eq!(config.query_param, "version");
}

#[tokio::test]
async fn test_extract_version_from_path() {
    let config = VersioningConfig::new();

    let req = Request::builder()
        .uri("/api/v1/streams")
        .body(Body::empty())
        .unwrap();

    let version_info = extract_version(&req, &config);
    assert_eq!(version_info.version, ApiVersion::V1);
    assert_eq!(version_info.source, VersionSource::UrlPrefix);
}

#[tokio::test]
async fn test_extract_version_from_header() {
    let config = VersioningConfig::new();

    let mut headers = HeaderMap::new();
    headers.insert("X-API-Version", HeaderValue::from_static("v2"));

    let req = Request::builder()
        .uri("/streams")
        .headers(headers)
        .body(Body::empty())
        .unwrap();

    let version_info = extract_version(&req, &config);
    assert_eq!(version_info.version, ApiVersion::V2);
    assert_eq!(version_info.source, VersionSource::Header);
}

#[tokio::test]
async fn test_extract_version_from_query() {
    let config = VersioningConfig::new();

    let req = Request::builder()
        .uri("/streams?api_version=v1")
        .body(Body::empty())
        .unwrap();

    let version_info = extract_version(&req, &config);
    assert_eq!(version_info.version, ApiVersion::V1);
    assert_eq!(version_info.source, VersionSource::QueryParam);
}

#[tokio::test]
async fn test_extract_version_from_accept() {
    let config = VersioningConfig::new().with_accept_negotiation(true);

    let mut headers = HeaderMap::new();
    headers.insert(
        "accept",
        HeaderValue::from_static("application/vnd.mediaedge.v2+json"),
    );

    let req = Request::builder()
        .uri("/streams")
        .headers(headers)
        .body(Body::empty())
        .unwrap();

    let version_info = extract_version(&req, &config);
    assert_eq!(version_info.version, ApiVersion::V2);
    assert_eq!(version_info.source, VersionSource::AcceptHeader);
}

#[tokio::test]
async fn test_versioning_middleware() {
    let config = VersioningConfig::new();
    let app = Router::new()
        .route("/test", get(handler_version))
        .layer(versioning_layer(config));

    let mut headers = HeaderMap::new();
    headers.insert("X-API-Version", HeaderValue::from_static("v1"));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/test")
                .headers(headers)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get("x-api-version").unwrap(), "v1");
}

async fn handler_version() -> Json<serde_json::Value> {
    Json(json!({ "ok": true }))
}

#[tokio::test]
async fn test_rate_limit_config() {
    let config = RateLimitConfig::new()
        .with_requests_per_window(100)
        .with_window(Duration::from_secs(30))
        .with_key_extractor(KeyExtractor::ApiKey);

    assert_eq!(config.requests_per_window, 100);
    assert_eq!(config.window, Duration::from_secs(30));
    assert_eq!(config.key_extractor, KeyExtractor::ApiKey);
}

#[tokio::test]
async fn test_token_bucket() {
    let mut bucket = TokenBucket::new(10, Duration::from_secs(60));

    // Should allow 10 requests
    for _ in 0..10 {
        assert!(bucket.try_consume(1));
    }

    // 11th should be denied
    assert!(!bucket.try_consume(1));
    assert_eq!(bucket.remaining(), 0);
}

#[tokio::test]
async fn test_rate_limit_middleware() {
    let config = RateLimitConfig::new()
        .with_requests_per_window(5)
        .with_window(Duration::from_secs(60))
        .with_skip_paths(vec!["/health".to_string()]);

    let app = Router::new()
        .route("/test", get(handler_rate_limit))
        .route("/health", get(|| async { "ok" }))
        .layer(rate_limit_layer(config));

    // Allow 5 requests
    for i in 0..5 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Request {} should succeed",
            i + 1
        );
        assert!(response.headers().contains_key("x-ratelimit-limit"));
        assert!(response.headers().contains_key("x-ratelimit-remaining"));
    }

    // 6th should be rate limited
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    // Health check should still work
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

async fn handler_rate_limit() -> Json<serde_json::Value> {
    Json(json!({ "ok": true }))
}

#[tokio::test]
async fn test_api_middleware_config() {
    let config = ApiMiddlewareConfig::new()
        .with_service_name("Test Service".to_string())
        .with_version("2.0.0".to_string());

    assert_eq!(config.openapi.service_name, "Test Service");
    assert_eq!(config.openapi.version, "2.0.0");
}

#[tokio::test]
async fn test_create_api_router() {
    let config = ApiMiddlewareConfig::default();
    let app = create_api_router(config);

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
async fn test_create_full_api_router() {
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
    assert!(response.headers().contains_key("x-request-id"));
    assert_eq!(
        response.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
}

#[tokio::test]
async fn test_cors_layer() {
    let config = CorsConfig::default();
    let _layer = cors_layer(config);
    // Layer creation should succeed
}

#[tokio::test]
async fn test_error_response_format() {
    let config = ValidationConfig::default();
    let app = Router::new()
        .route("/test", post(handler_validated_json))
        .layer(mediaedge_api::validation::validation_layer(config));

    let request_body = json!({
        "name": "",
        "age": 150,
        "email": "invalid-email"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let error: ErrorResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(error.error, "validation_failed");
    assert!(error.details.is_some());
}

#[tokio::test]
async fn test_version_deprecation_warning() {
    let config = VersioningConfig::new().with_deprecated_versions(vec![ApiVersion::V1]);

    let app = Router::new()
        .route("/test", get(handler_version))
        .layer(versioning_layer(config));

    let mut headers = HeaderMap::new();
    headers.insert("X-API-Version", HeaderValue::from_static("v1"));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/test")
                .headers(headers)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get("x-api-deprecated").unwrap(), "true");
    assert!(response.headers().contains_key("warning"));
}

#[tokio::test]
async fn test_rate_limit_headers() {
    let config = RateLimitConfig::new()
        .with_requests_per_window(2)
        .with_window(Duration::from_secs(60));

    let app = Router::new()
        .route("/test", get(handler_rate_limit))
        .layer(rate_limit_layer(config));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get("x-ratelimit-limit").unwrap(), "2");
    assert_eq!(
        response.headers().get("x-ratelimit-remaining").unwrap(),
        "1"
    );
    assert!(response.headers().contains_key("x-ratelimit-reset"));
}
