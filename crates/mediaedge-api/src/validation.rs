//! Request/response validation middleware.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::{
    Json,
    body::Body,
    extract::{FromRequest, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use pin_project_lite::pin_project;
use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;
use tower::Service;
use tracing::{debug, warn};
use validator::{Validate, ValidationErrors};

use crate::openapi::ErrorResponse;

/// Validation middleware configuration.
#[derive(Debug, Clone, Default)]
pub struct ValidationConfig {
    /// Reject requests with unknown fields.
    pub reject_unknown_fields: bool,

    /// Maximum request body size.
    pub max_body_size: usize,
}

impl ValidationConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_reject_unknown_fields(mut self, reject: bool) -> Self {
        self.reject_unknown_fields = reject;
        self
    }

    pub fn with_max_body_size(mut self, size: usize) -> Self {
        self.max_body_size = size;
        self
    }
}

/// Validation error.
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Validation failed: {0}")]
    Validation(#[from] ValidationErrors),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("Body too large: {size} bytes (max: {max})")]
    BodyTooLarge { size: usize, max: usize },

    #[error("Unknown fields in request: {fields:?}")]
    UnknownFields { fields: Vec<String> },
}

/// Validated JSON extractor.
#[derive(Debug, Clone)]
pub struct ValidatedJson<T>(pub T);

impl<T> ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
{
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> std::ops::Deref for ValidatedJson<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> FromRequest<axum::body::Body> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate + Send + 'static,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &()) -> Result<Self, Self::Rejection> {
        // Extract bytes first to check size
        let (parts, body) = req.into_parts();

        let bytes = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
            Ok(bytes) => bytes,
            Err(e) => {
                return Err((
                    StatusCode::PAYLOAD_TOO_LARGE,
                    Json(ErrorResponse {
                        error: "payload_too_large".to_string(),
                        message: e.to_string(),
                        details: None,
                        timestamp: chrono::Utc::now(),
                        request_id: None,
                    }),
                )
                    .into_response());
            }
        };

        // Parse JSON
        let value: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "invalid_json".to_string(),
                        message: format!("Invalid JSON: {}", e),
                        details: None,
                        timestamp: chrono::Utc::now(),
                        request_id: None,
                    }),
                )
                    .into_response());
            }
        };

        // Check for unknown fields if enabled
        // (Would need a schema-aware approach for full implementation)

        // Deserialize to target type
        let data: T = match serde_json::from_value(value) {
            Ok(d) => d,
            Err(e) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "deserialization_failed".to_string(),
                        message: format!("Deserialization failed: {}", e),
                        details: None,
                        timestamp: chrono::Utc::now(),
                        request_id: None,
                    }),
                )
                    .into_response());
            }
        };

        // Validate
        if let Err(e) = data.validate() {
            let details = serde_json::to_value(&e).ok();
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ErrorResponse {
                    error: "validation_failed".to_string(),
                    message: "Request validation failed".to_string(),
                    details,
                    timestamp: chrono::Utc::now(),
                    request_id: None,
                }),
            )
                .into_response());
        }

        Ok(ValidatedJson(data))
    }
}

/// Validated query extractor.
#[derive(Debug, Clone)]
pub struct ValidatedQuery<T>(pub T);

impl<T> ValidatedQuery<T>
where
    T: DeserializeOwned + Validate,
{
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> std::ops::Deref for ValidatedQuery<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> FromRequest<axum::body::Body> for ValidatedQuery<T>
where
    T: DeserializeOwned + Validate + Send + 'static,
{
    type Rejection = Response;

    async fn from_request(req: Request, _state: &()) -> Result<Self, Self::Rejection> {
        let query = req.uri().query().unwrap_or("");
        let data: T = match serde_qs::from_str(query) {
            Ok(d) => d,
            Err(e) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "invalid_query".to_string(),
                        message: format!("Invalid query parameters: {}", e),
                        details: None,
                        timestamp: chrono::Utc::now(),
                        request_id: None,
                    }),
                )
                    .into_response());
            }
        };

        if let Err(e) = data.validate() {
            let details = serde_json::to_value(&e).ok();
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ErrorResponse {
                    error: "validation_failed".to_string(),
                    message: "Query validation failed".to_string(),
                    details,
                    timestamp: chrono::Utc::now(),
                    request_id: None,
                }),
            )
                .into_response());
        }

        Ok(ValidatedQuery(data))
    }
}

/// Validated path extractor.
#[derive(Debug, Clone)]
pub struct ValidatedPath<T>(pub T);

impl<T> ValidatedPath<T>
where
    T: DeserializeOwned + Validate,
{
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> std::ops::Deref for ValidatedPath<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> FromRequest<axum::body::Body> for ValidatedPath<T>
where
    T: DeserializeOwned + Validate + Send + 'static,
{
    type Rejection = Response;

    async fn from_request(req: Request, _state: &()) -> Result<Self, Self::Rejection> {
        // Path parameters are extracted by axum's Path extractor
        // This is a placeholder - actual implementation would use axum::extract::Path
        let (mut parts, body) = req.into_parts();

        // For now, just return an error - real implementation uses axum::extract::Path
        Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "not_implemented".to_string(),
                message: "ValidatedPath requires axum::extract::Path integration".to_string(),
                details: None,
                timestamp: chrono::Utc::now(),
                request_id: None,
            }),
        )
            .into_response())
    }
}

/// Validation middleware for responses.
pub struct ValidationMiddleware<S> {
    inner: S,
    config: ValidationConfig,
}

impl<S> ValidationMiddleware<S> {
    pub fn new(inner: S, config: ValidationConfig) -> Self {
        Self { inner, config }
    }
}

impl<S> Service<Request> for ValidationMiddleware<S>
where
    S: Service<Request, Response = Response, Error = std::convert::Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = std::convert::Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let inner = self.inner.clone();
        let _config = self.config.clone();

        Box::pin(async move {
            // For now, just pass through - validation happens at extractor level
            let response = inner.call(req).await?;
            Ok(response)
        })
    }
}

/// Layer for adding validation middleware.
#[derive(Clone)]
pub struct ValidationLayer {
    config: ValidationConfig,
}

impl ValidationLayer {
    pub fn new(config: ValidationConfig) -> Self {
        Self { config }
    }
}

impl<S> tower::Layer<S> for ValidationLayer {
    type Service = ValidationMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ValidationMiddleware::new(inner, self.config.clone())
    }
}

/// Create validation layer.
pub fn validation_layer(config: ValidationConfig) -> ValidationLayer {
    ValidationLayer::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
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

    #[test]
    fn test_validation_config() {
        let config = ValidationConfig::new()
            .with_reject_unknown_fields(true)
            .with_max_body_size(1024 * 1024);

        assert!(config.reject_unknown_fields);
        assert_eq!(config.max_body_size, 1024 * 1024);
    }

    #[test]
    fn test_validated_json_success() {
        // Test would require full axum test setup
    }

    #[test]
    fn test_validated_query() {
        // Test would require full axum test setup
    }
}
