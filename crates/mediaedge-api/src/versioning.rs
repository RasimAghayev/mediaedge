//! API versioning middleware.

use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::task::{Context, Poll};

use axum::{
    Json,
    body::Body,
    extract::{MatchedPath, Request},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use pin_project_lite::pin_project;
use serde_json::json;
use tower::Service;
use tracing::{debug, warn};

use crate::openapi::{ApiVersion, ErrorResponse};

/// Versioning configuration.
#[derive(Debug, Clone, Default)]
pub struct VersioningConfig {
    /// Default API version.
    pub default_version: ApiVersion,

    /// Supported versions.
    pub supported_versions: Vec<ApiVersion>,

    /// Header name for version.
    pub header_name: String,

    /// Query parameter name for version.
    pub query_param: String,

    /// URL prefix pattern (e.g., "/api/v1").
    pub url_prefix_pattern: String,

    /// Allow version negotiation via Accept header.
    pub accept_header_negotiation: bool,

    /// Deprecated versions (will warn but allow).
    pub deprecated_versions: Vec<ApiVersion>,
}

impl VersioningConfig {
    pub fn new() -> Self {
        Self {
            default_version: ApiVersion::V1,
            supported_versions: vec![ApiVersion::V1, ApiVersion::V2],
            header_name: "X-API-Version".to_string(),
            query_param: "api_version".to_string(),
            url_prefix_pattern: "/api/v".to_string(),
            accept_header_negotiation: true,
            deprecated_versions: vec![],
        }
    }

    pub fn with_default_version(mut self, version: ApiVersion) -> Self {
        self.default_version = version;
        self
    }

    pub fn with_supported_versions(mut self, versions: Vec<ApiVersion>) -> Self {
        self.supported_versions = versions;
        self
    }

    pub fn with_header_name(mut self, name: String) -> Self {
        self.header_name = name;
        self
    }

    pub fn with_query_param(mut self, param: String) -> Self {
        self.query_param = param;
        self
    }

    pub fn with_url_prefix(mut self, prefix: String) -> Self {
        self.url_prefix_pattern = prefix;
        self
    }

    pub fn with_accept_negotiation(mut self, enabled: bool) -> Self {
        self.accept_header_negotiation = enabled;
        self
    }

    pub fn with_deprecated_versions(mut self, versions: Vec<ApiVersion>) -> Self {
        self.deprecated_versions = versions;
        self
    }
}

/// Version info extracted from request.
#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub version: ApiVersion,
    pub source: VersionSource,
    pub deprecated: bool,
}

/// Source of version detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionSource {
    UrlPrefix,
    Header,
    QueryParam,
    AcceptHeader,
    Default,
}

/// Extract version from request.
pub fn extract_version(req: &Request, config: &VersioningConfig) -> VersionInfo {
    // 1. Check URL prefix
    if let Some(matched_path) = req.extensions().get::<MatchedPath>() {
        let path = matched_path.as_str();
        if let Some(version) = extract_version_from_path(path, config) {
            return VersionInfo {
                version,
                source: VersionSource::UrlPrefix,
                deprecated: config.deprecated_versions.contains(&version),
            };
        }
    }

    // 2. Check header
    if let Some(header_value) = req.headers().get(&config.header_name) {
        if let Ok(header_str) = header_value.to_str() {
            if let Some(version) = ApiVersion::from_header(header_str) {
                if config.supported_versions.contains(&version) {
                    return VersionInfo {
                        version,
                        source: VersionSource::Header,
                        deprecated: config.deprecated_versions.contains(&version),
                    };
                }
            }
        }
    }

    // 3. Check query parameter
    if let Some(query) = req.uri().query() {
        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                if key == config.query_param {
                    if let Some(version) = ApiVersion::from_header(value) {
                        if config.supported_versions.contains(&version) {
                            return VersionInfo {
                                version,
                                source: VersionSource::QueryParam,
                                deprecated: config.deprecated_versions.contains(&version),
                            };
                        }
                    }
                }
            }
        }
    }

    // 4. Check Accept header for version negotiation
    if config.accept_header_negotiation {
        if let Some(accept) = req.headers().get("accept") {
            if let Ok(accept_str) = accept.to_str() {
                if let Some(version) = negotiate_version_from_accept(accept_str, config) {
                    return VersionInfo {
                        version,
                        source: VersionSource::AcceptHeader,
                        deprecated: config.deprecated_versions.contains(&version),
                    };
                }
            }
        }
    }

    // 5. Default version
    VersionInfo {
        version: config.default_version,
        source: VersionSource::Default,
        deprecated: config.deprecated_versions.contains(&config.default_version),
    }
}

/// Extract version from URL path.
fn extract_version_from_path(path: &str, config: &VersioningConfig) -> Option<ApiVersion> {
    let prefix = &config.url_prefix_pattern;
    if let Some(rest) = path.strip_prefix(prefix) {
        if let Some(version_str) = rest.split('/').next() {
            return ApiVersion::from_header(version_str);
        }
    }
    None
}

/// Negotiate version from Accept header.
/// Supports: application/vnd.mediaedge.v1+json, application/json; version=1
fn negotiate_version_from_accept(accept: &str, config: &VersioningConfig) -> Option<ApiVersion> {
    for part in accept.split(',') {
        let part = part.trim();

        // Check for vendor-specific media type: application/vnd.mediaedge.v1+json
        if part.starts_with("application/vnd.mediaedge.v") {
            if let Some(version_part) = part.strip_prefix("application/vnd.mediaedge.v") {
                let version_str = version_part.split(['+', ';']).next().unwrap_or("");
                return ApiVersion::from_header(version_str);
            }
        }

        // Check for version parameter: application/json; version=1
        if part.starts_with("application/json") {
            for param in part.split(';').skip(1) {
                let param = param.trim();
                if let Some(value) = param.strip_prefix("version=") {
                    return ApiVersion::from_header(value);
                }
            }
        }
    }
    None
}

/// Versioning middleware.
pub struct VersioningMiddleware<S> {
    inner: S,
    config: VersioningConfig,
}

impl<S> VersioningMiddleware<S> {
    pub fn new(inner: S, config: VersioningConfig) -> Self {
        Self { inner, config }
    }
}

impl<S> Service<Request> for VersioningMiddleware<S>
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

    fn call(&mut self, mut req: Request) -> Self::Future {
        let inner = self.inner.clone();
        let config = self.config.clone();

        Box::pin(async move {
            // Extract version info
            let version_info = extract_version(&req, &config);

            // Add version info to request extensions
            req.extensions_mut().insert(version_info.clone());

            // Add version header to response
            let mut response = inner.call(req).await?;

            response.headers_mut().insert(
                HeaderName::from_static("x-api-version"),
                HeaderValue::from_str(version_info.version.as_str()).unwrap(),
            );

            // Add deprecation warning header if deprecated
            if version_info.deprecated {
                response.headers_mut().insert(
                    HeaderName::from_static("x-api-deprecated"),
                    HeaderValue::from_static("true"),
                );
                response.headers_mut().insert(
                    HeaderName::from_static("warning"),
                    HeaderValue::from_str(&format!(
                        "299 - \"API version {} is deprecated\"",
                        version_info.version.as_str()
                    ))
                    .unwrap(),
                );
            }

            Ok(response)
        })
    }
}

/// Layer for adding versioning middleware.
#[derive(Clone)]
pub struct VersioningLayer {
    config: VersioningConfig,
}

impl VersioningLayer {
    pub fn new(config: VersioningConfig) -> Self {
        Self { config }
    }
}

impl<S> tower::Layer<S> for VersioningLayer {
    type Service = VersioningMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        VersioningMiddleware::new(inner, self.config.clone())
    }
}

/// Create versioning layer.
pub fn versioning_layer(config: VersioningConfig) -> VersioningLayer {
    VersioningLayer::new(config)
}

/// Extract version info from request extensions.
pub fn get_version_info(req: &Request) -> Option<VersionInfo> {
    req.extensions().get::<VersionInfo>().cloned()
}

/// Require specific version (for route-level version enforcement).
pub fn require_version(version: ApiVersion) -> impl Fn(&Request) -> Result<(), Response> {
    move |req: &Request| {
        if let Some(version_info) = get_version_info(req) {
            if version_info.version == version {
                Ok(())
            } else {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "error": "version_mismatch",
                        "message": format!("This endpoint requires API version {}", version.as_str()),
                        "required_version": version.as_str(),
                        "current_version": version_info.version.as_str(),
                    })),
                ).into_response())
            }
        } else {
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "version_not_detected",
                    "message": "API version could not be determined",
                })),
            )
                .into_response())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openapi::ApiVersion;

    #[test]
    fn test_versioning_config() {
        let config = VersioningConfig::new()
            .with_default_version(ApiVersion::V2)
            .with_supported_versions(vec![ApiVersion::V1, ApiVersion::V2])
            .with_header_name("X-Custom-Version".to_string())
            .with_query_param("version".to_string());

        assert_eq!(config.default_version, ApiVersion::V2);
        assert_eq!(config.header_name, "X-Custom-Version");
        assert_eq!(config.query_param, "version");
    }

    #[test]
    fn test_api_version_from_header() {
        assert_eq!(ApiVersion::from_header("v1"), Some(ApiVersion::V1));
        assert_eq!(ApiVersion::from_header("V1"), Some(ApiVersion::V1));
        assert_eq!(ApiVersion::from_header("1"), Some(ApiVersion::V1));
        assert_eq!(ApiVersion::from_header("v2"), Some(ApiVersion::V2));
        assert_eq!(ApiVersion::from_header("2"), Some(ApiVersion::V2));
        assert_eq!(ApiVersion::from_header("v3"), None);
        assert_eq!(ApiVersion::from_header("invalid"), None);
    }

    #[test]
    fn test_extract_version_from_path() {
        let config = VersioningConfig::new();

        assert_eq!(
            extract_version_from_path("/api/v1/streams", &config),
            Some(ApiVersion::V1)
        );
        assert_eq!(
            extract_version_from_path("/api/v2/streams", &config),
            Some(ApiVersion::V2)
        );
        assert_eq!(extract_version_from_path("/api/v3/streams", &config), None);
        assert_eq!(extract_version_from_path("/streams", &config), None);
    }

    #[test]
    fn test_negotiate_version_from_accept() {
        let config = VersioningConfig::new();

        // Vendor-specific media type
        assert_eq!(
            negotiate_version_from_accept("application/vnd.mediaedge.v1+json", &config),
            Some(ApiVersion::V1)
        );
        assert_eq!(
            negotiate_version_from_accept("application/vnd.mediaedge.v2+json", &config),
            Some(ApiVersion::V2)
        );

        // Version parameter
        assert_eq!(
            negotiate_version_from_accept("application/json; version=1", &config),
            Some(ApiVersion::V1)
        );
        assert_eq!(
            negotiate_version_from_accept("application/json; version=2", &config),
            Some(ApiVersion::V2)
        );

        // Multiple types
        assert_eq!(
            negotiate_version_from_accept(
                "application/vnd.mediaedge.v2+json, application/json",
                &config
            ),
            Some(ApiVersion::V2)
        );

        // No version
        assert_eq!(
            negotiate_version_from_accept("application/json", &config),
            None
        );
    }
}
