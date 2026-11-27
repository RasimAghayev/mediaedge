use reqwest::{Client, ClientBuilder, RequestBuilder, Method, header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE}};
use serde::{de::DeserializeOwned, Serialize};
use url::Url;
use std::time::Duration;
use governor::{RateLimiter, Quota, state::InMemoryState, clock::DefaultClock};
use std::num::NonZeroU32;
use std::sync::Arc;
use crate::error::{MediaEdgeError, Result};
use crate::models::*;

/// Main client for MediaEdge API
#[derive(Clone)]
pub struct MediaEdgeClient {
    inner: Arc<MediaEdgeClientInner>,
}

struct MediaEdgeClientInner {
    base_url: Url,
    http: Client,
    access_token: Option<String>,
    refresh_token: Option<String>,
    rate_limiter: Arc<RateLimiter<InMemoryState, DefaultClock>>,
}

impl MediaEdgeClient {
    /// Create a new client with base URL
    pub fn new(base_url: &str) -> Result<Self> {
        let base_url = Url::parse(base_url)
            .map_err(|e| MediaError::InvalidUrl(e.to_string()))?;
        
        let http = ClientBuilder::new()
            .timeout(Duration::from_secs(30))
            .gzip(true)
            .build()?;
        
        // Rate limiter: 100 requests per second
        let quota = Quota::per_second(NonZeroU32::new(100).unwrap());
        let rate_limiter = Arc::new(RateLimiter::direct(quota));
        
        Ok(Self {
            inner: Arc::new(MediaEdgeClientInner {
                base_url,
                http,
                access_token: None,
                refresh_token: None,
                rate_limiter,
            }),
        })
    }

    /// Create client with authentication tokens
    pub fn with_auth(base_url: &str, access_token: String, refresh_token: String) -> Result<Self> {
        let mut client = Self::new(base_url)?;
        client.inner.access_token = Some(access_token);
        client.inner.refresh_token = Some(refresh_token);
        Ok(client)
    }

    /// Get base URL
    pub fn base_url(&self) -> &str {
        self.inner.base_url.as_str()
    }

    /// Set access token
    pub fn set_access_token(&mut self, token: String) {
        Arc::get_mut(&mut self.inner).unwrap().access_token = Some(token);
    }

    /// Set refresh token
    pub fn set_refresh_token(&mut self, token: String) {
        Arc::get_mut(&mut self.inner).unwrap().refresh_token = Some(token);
    }

    /// Build request with auth and rate limiting
    async fn request(&self, method: Method, path: &str) -> Result<RequestBuilder> {
        // Rate limiting
        self.inner.rate_limiter.until_ready().await;
        
        let url = self.inner.base_url.join(path)
            .map_err(|e| MediaError::InvalidUrl(e.to_string()))?;
        
        let mut builder = self.inner.http.request(method, url);
        
        if let Some(token) = &self.inner.access_token {
            builder = builder.header(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", token))?);
        }
        
        builder = builder.header(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        
        Ok(builder)
    }

    /// GET request
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self.request(Method::GET, path).await?.send().await?;
        self.handle_response(resp).await
    }

    /// GET request with query parameters
    pub async fn get_with_query<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T> {
        // Rate limiting
        self.inner.rate_limiter.until_ready().await;
        
        let mut url = self.inner.base_url.join(path)
            .map_err(|e| MediaEdgeError::InvalidUrl(e.to_string()))?;
        
        url.query_pairs_mut().extend_pairs(query.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        
        let mut builder = self.inner.http.get(url);
        
        if let Some(token) = &self.inner.access_token {
            builder = builder.header(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", token))?);
        }
        
        builder = builder.header(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        
        let resp = builder.send().await?;
        self.handle_response(resp).await
    }

    /// POST request
    pub async fn post<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T> {
        let resp = self.request(Method::POST, path).await?
            .json(body)
            .send()
            .await?;
        self.handle_response(resp).await
    }

    /// PATCH request
    pub async fn patch<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T> {
        let resp = self.request(Method::PATCH, path).await?
            .json(body)
            .send()
            .await?;
        self.handle_response(resp).await
    }

    /// DELETE request
    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self.request(Method::DELETE, path).await?.send().await?;
        self.handle_response(resp).await
    }

    /// Handle response and deserialize
    async fn handle_response<T: DeserializeOwned>(&self, resp: reqwest::Response) -> Result<T> {
        let status = resp.status();
        let text = resp.text().await?;
        
        if status.is_success() {
            if text.is_empty() {
                return Err(MediaError::EmptyResponse.into());
            }
            serde_json::from_str(&text).map_err(|e| MediaError::Deserialization(e.to_string()).into())
        } else {
            let err: ApiError = serde_json::from_str(&text).unwrap_or_else(|_| ApiError {
                code: status.as_u16(),
                message: text,
                details: None,
            });
            Err(MediaError::Api(err).into())
        }
    }

    // API modules accessors
    pub fn streams(&self) -> streams::StreamsApi {
        streams::StreamsApi::new(self.clone())
    }

    pub fn cluster(&self) -> cluster::ClusterApi {
        cluster::ClusterApi::new(self.clone())
    }

    pub fn tenants(&self) -> tenants::TenantsApi {
        tenants::TenantsApi::new(self.clone())
    }

    pub fn monetization(&self) -> monetization::MonetizationApi {
        monetization::MonetizationApi::new(self.clone())
    }

    pub fn auth(&self) -> auth::AuthApi {
        auth::AuthApi::new(self.clone())
    }

    pub fn metrics(&self) -> metrics::MetricsApi {
        metrics::MetricsApi::new(self.clone())
    }
}