//! Rate limiting middleware with token bucket algorithm.

use std::collections::HashMap;
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use axum::{
    Json,
    body::Body,
    extract::{ConnectInfo, Request},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use pin_project_lite::pin_project;
use serde_json::json;
use tokio::sync::RwLock;
use tower::Service;
use tracing::{debug, warn};

use crate::openapi::ErrorResponse;

/// Rate limiting configuration.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Enable rate limiting.
    pub enabled: bool,

    /// Requests per window.
    pub requests_per_window: u32,

    /// Window size.
    pub window: Duration,

    /// Key prefix for Redis.
    pub key_prefix: String,

    /// Redis URL (optional, for distributed rate limiting).
    pub redis_url: Option<String>,

    /// Redis connection pool size.
    pub redis_pool_size: u32,

    /// Custom key extractor.
    pub key_extractor: KeyExtractor,

    /// Skip paths (e.g., health checks).
    pub skip_paths: Vec<String>,

    /// Custom headers for rate limit info.
    pub headers: RateLimitHeaders,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_window: 1000,
            window: Duration::from_secs(60),
            key_prefix: "rl:".to_string(),
            redis_url: None,
            redis_pool_size: 10,
            key_extractor: KeyExtractor::Ip,
            skip_paths: vec!["/health".to_string(), "/api/health".to_string()],
            headers: RateLimitHeaders::default(),
        }
    }
}

/// Key extractor strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyExtractor {
    /// Use client IP address.
    Ip,
    /// Use authenticated user ID (from JWT).
    UserId,
    /// Use API key.
    ApiKey,
    /// Use tenant ID.
    TenantId,
    /// Custom header.
    CustomHeader(&'static str),
}

/// Rate limit headers configuration.
#[derive(Debug, Clone)]
pub struct RateLimitHeaders {
    /// Header for limit.
    pub limit: String,
    /// Header for remaining.
    pub remaining: String,
    /// Header for reset time.
    pub reset: String,
    /// Header for retry after.
    pub retry_after: String,
}

impl Default for RateLimitHeaders {
    fn default() -> Self {
        Self {
            limit: "X-RateLimit-Limit".to_string(),
            remaining: "X-RateLimit-Remaining".to_string(),
            reset: "X-RateLimit-Reset".to_string(),
            retry_after: "Retry-After".to_string(),
        }
    }
}

impl RateLimitConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_requests_per_window(mut self, requests: u32) -> Self {
        self.requests_per_window = requests;
        self
    }

    pub fn with_window(mut self, window: Duration) -> Self {
        self.window = window;
        self
    }

    pub fn with_key_extractor(mut self, extractor: KeyExtractor) -> Self {
        self.key_extractor = extractor;
        self
    }

    pub fn with_redis(mut self, url: String, pool_size: u32) -> Self {
        self.redis_url = Some(url);
        self.redis_pool_size = pool_size;
        self
    }

    pub fn with_skip_paths(mut self, paths: Vec<String>) -> Self {
        self.skip_paths = paths;
        self
    }
}

/// Token bucket for rate limiting.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    capacity: u32,
    tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(capacity: u32, window: Duration) -> Self {
        let refill_rate = capacity as f64 / window.as_secs_f64();
        Self {
            capacity,
            tokens: capacity as f64,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Try to consume tokens.
    pub fn try_consume(&mut self, tokens: u32) -> bool {
        self.refill();
        if self.tokens >= tokens as f64 {
            self.tokens -= tokens as f64;
            true
        } else {
            false
        }
    }

    /// Get remaining tokens.
    pub fn remaining(&mut self) -> u32 {
        self.refill();
        self.tokens.floor() as u32
    }

    /// Get reset time (when bucket will be full).
    pub fn reset_time(&mut self) -> Instant {
        self.refill();
        if self.tokens >= self.capacity as f64 {
            Instant::now()
        } else {
            let tokens_needed = self.capacity as f64 - self.tokens;
            let seconds_needed = tokens_needed / self.refill_rate;
            Instant::now() + Duration::from_secs_f64(seconds_needed)
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity as f64);
        self.last_refill = now;
    }
}

/// In-memory rate limiter.
#[derive(Debug)]
pub struct InMemoryRateLimiter {
    buckets: DashMap<String, TokenBucket>,
    config: RateLimitConfig,
}

impl InMemoryRateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: DashMap::new(),
            config,
        }
    }

    /// Check rate limit for a key.
    pub fn check(&self, key: &str) -> RateLimitResult {
        let mut bucket = self.buckets.entry(key.to_string()).or_insert_with(|| {
            TokenBucket::new(self.config.requests_per_window, self.config.window)
        });

        let allowed = bucket.try_consume(1);
        let remaining = bucket.remaining();
        let reset_time = bucket.reset_time();

        RateLimitResult {
            allowed,
            limit: self.config.requests_per_window,
            remaining,
            reset_time,
            retry_after: if allowed {
                None
            } else {
                Some(reset_time.saturating_duration_since(Instant::now()))
            },
        }
    }

    /// Clean up expired buckets.
    pub fn cleanup(&self) {
        let now = Instant::now();
        let window = self.config.window;
        self.buckets
            .retain(|_, bucket| now.duration_since(bucket.last_refill) < window * 2);
    }
}

/// Rate limit check result.
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub limit: u32,
    pub remaining: u32,
    pub reset_time: Instant,
    pub retry_after: Option<Duration>,
}

/// Rate limiter trait for different backends.
#[async_trait::async_trait]
pub trait RateLimiter: Send + Sync {
    async fn check(&self, key: &str) -> RateLimitResult;
    async fn cleanup(&self);
}

#[async_trait::async_trait]
impl RateLimiter for InMemoryRateLimiter {
    async fn check(&self, key: &str) -> RateLimitResult {
        self.check(key)
    }

    async fn cleanup(&self) {
        self.cleanup()
    }
}

/// Extract rate limit key from request.
fn extract_key(req: &Request, config: &RateLimitConfig) -> String {
    match config.key_extractor {
        KeyExtractor::Ip => {
            if let Some(connect_info) = req.extensions().get::<ConnectInfo<IpAddr>>() {
                format!("{}{}", config.key_prefix, connect_info.0)
            } else if let Some(forwarded) = req.headers().get("x-forwarded-for") {
                if let Ok(ip_str) = forwarded.to_str() {
                    let ip = ip_str.split(',').next().unwrap_or("unknown").trim();
                    format!("{}{}", config.key_prefix, ip)
                } else {
                    format!("{}unknown", config.key_prefix)
                }
            } else {
                format!("{}unknown", config.key_prefix)
            }
        }
        KeyExtractor::UserId => {
            // Would extract from JWT claims in extensions
            req.extensions()
                .get::<String>()
                .map(|id| format!("{}{}", config.key_prefix, id))
                .unwrap_or_else(|| format!("{}anonymous", config.key_prefix))
        }
        KeyExtractor::ApiKey => req
            .headers()
            .get("x-api-key")
            .and_then(|v| v.to_str().ok())
            .map(|k| format!("{}{}", config.key_prefix, k))
            .unwrap_or_else(|| format!("{}no-api-key", config.key_prefix)),
        KeyExtractor::TenantId => req
            .extensions()
            .get::<String>()
            .map(|id| format!("{}tenant:{}", config.key_prefix, id))
            .unwrap_or_else(|| format!("{}no-tenant", config.key_prefix)),
        KeyExtractor::CustomHeader(header) => req
            .headers()
            .get(header)
            .and_then(|v| v.to_str().ok())
            .map(|v| format!("{}{}", config.key_prefix, v))
            .unwrap_or_else(|| format!("{}missing-header", config.key_prefix)),
    }
}

/// Rate limiting middleware.
pub struct RateLimitMiddleware<S> {
    inner: S,
    limiter: Arc<dyn RateLimiter>,
    config: RateLimitConfig,
}

impl<S> RateLimitMiddleware<S> {
    pub fn new(inner: S, limiter: Arc<dyn RateLimiter>, config: RateLimitConfig) -> Self {
        Self {
            inner,
            limiter,
            config,
        }
    }
}

impl<S> Service<Request> for RateLimitMiddleware<S>
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
        let limiter = self.limiter.clone();
        let config = self.config.clone();

        Box::pin(async move {
            // Check if rate limiting is enabled
            if !config.enabled {
                return inner.call(req).await;
            }

            // Check if path should be skipped
            let path = req.uri().path();
            if config.skip_paths.iter().any(|p| path.starts_with(p)) {
                return inner.call(req).await;
            }

            // Extract key
            let key = extract_key(&req, &config);

            // Check rate limit
            let result = limiter.check(&key).await;

            // Add rate limit headers
            let mut response = if result.allowed {
                inner.call(req).await?
            } else {
                // Rate limited
                let mut resp = (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(ErrorResponse {
                        error: "rate_limited".to_string(),
                        message: "Rate limit exceeded".to_string(),
                        details: Some(json!({
                            "limit": result.limit,
                            "remaining": result.remaining,
                            "reset_at": result.reset_time.elapsed().as_secs(),
                        })),
                        timestamp: chrono::Utc::now(),
                        request_id: None,
                    }),
                )
                    .into_response();

                // Add headers to error response too
                add_rate_limit_headers(resp.headers_mut(), &result, &config.headers);
                return Ok(resp);
            };

            // Add headers to successful response
            add_rate_limit_headers(response.headers_mut(), &result, &config.headers);

            Ok(response)
        })
    }
}

/// Add rate limit headers to response.
fn add_rate_limit_headers(
    headers: &mut HeaderMap,
    result: &RateLimitResult,
    header_config: &RateLimitHeaders,
) {
    let _ = headers.insert(
        HeaderName::from_bytes(header_config.limit.as_bytes()).unwrap(),
        HeaderValue::from_str(&result.limit.to_string()).unwrap(),
    );
    let _ = headers.insert(
        HeaderName::from_bytes(header_config.remaining.as_bytes()).unwrap(),
        HeaderValue::from_str(&result.remaining.to_string()).unwrap(),
    );
    let _ = headers.insert(
        HeaderName::from_bytes(header_config.reset.as_bytes()).unwrap(),
        HeaderValue::from_str(&result.reset_time.elapsed().as_secs().to_string()).unwrap(),
    );
    if let Some(retry_after) = result.retry_after {
        let _ = headers.insert(
            HeaderName::from_bytes(header_config.retry_after.as_bytes()).unwrap(),
            HeaderValue::from_str(&retry_after.as_secs().to_string()).unwrap(),
        );
    }
}

/// Layer for adding rate limiting middleware.
#[derive(Clone)]
pub struct RateLimitLayer {
    limiter: Arc<dyn RateLimiter>,
    config: RateLimitConfig,
}

impl RateLimitLayer {
    pub fn new(limiter: Arc<dyn RateLimiter>, config: RateLimitConfig) -> Self {
        Self { limiter, config }
    }
}

impl<S> tower::Layer<S> for RateLimitLayer {
    type Service = RateLimitMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitMiddleware::new(inner, self.limiter.clone(), self.config.clone())
    }
}

/// Create rate limit layer with in-memory limiter.
pub fn rate_limit_layer(config: RateLimitConfig) -> RateLimitLayer {
    let limiter = Arc::new(InMemoryRateLimiter::new(config.clone()));
    RateLimitLayer::new(limiter, config)
}

/// Create rate limit layer with custom limiter.
pub fn rate_limit_layer_with_limiter(
    limiter: Arc<dyn RateLimiter>,
    config: RateLimitConfig,
) -> RateLimitLayer {
    RateLimitLayer::new(limiter, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_token_bucket() {
        let mut bucket = TokenBucket::new(10, Duration::from_secs(60));

        // Should allow 10 requests
        for _ in 0..10 {
            assert!(bucket.try_consume(1));
        }

        // 11th should be denied
        assert!(!bucket.try_consume(1));
        assert_eq!(bucket.remaining(), 0);
    }

    #[test]
    fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(10, Duration::from_secs(1));

        // Consume all
        for _ in 0..10 {
            assert!(bucket.try_consume(1));
        }
        assert!(!bucket.try_consume(1));

        // Wait for refill
        std::thread::sleep(Duration::from_secs(2));

        // Should be refilled
        assert!(bucket.try_consume(1));
    }

    #[test]
    fn test_in_memory_rate_limiter() {
        let config = RateLimitConfig::new()
            .with_requests_per_window(5)
            .with_window(Duration::from_secs(60));

        let limiter = InMemoryRateLimiter::new(config);

        // Allow 5 requests
        for _ in 0..5 {
            let result = limiter.check("test-key");
            assert!(result.allowed);
        }

        // 6th denied
        let result = limiter.check("test-key");
        assert!(!result.allowed);
        assert_eq!(result.remaining, 0);
    }

    #[test]
    fn test_rate_limit_config() {
        let config = RateLimitConfig::new()
            .with_requests_per_window(100)
            .with_window(Duration::from_secs(30))
            .with_key_extractor(KeyExtractor::ApiKey);

        assert_eq!(config.requests_per_window, 100);
        assert_eq!(config.window, Duration::from_secs(30));
        assert_eq!(config.key_extractor, KeyExtractor::ApiKey);
    }

    #[test]
    fn test_extract_key_ip() {
        use axum::http::Request;
        use std::net::IpAddr;

        let config = RateLimitConfig::new().with_key_extractor(KeyExtractor::Ip);
        let req = Request::builder()
            .extension(ConnectInfo(IpAddr::from([192, 168, 1, 1])))
            .body(Body::empty())
            .unwrap();

        let key = extract_key(&req, &config);
        assert_eq!(key, "rl:192.168.1.1");
    }

    #[test]
    fn test_extract_key_forwarded() {
        use axum::http::Request;

        let config = RateLimitConfig::new().with_key_extractor(KeyExtractor::Ip);
        let req = Request::builder()
            .header("x-forwarded-for", "10.0.0.1, 192.168.1.1")
            .body(Body::empty())
            .unwrap();

        let key = extract_key(&req, &config);
        assert_eq!(key, "rl:10.0.0.1");
    }
}
