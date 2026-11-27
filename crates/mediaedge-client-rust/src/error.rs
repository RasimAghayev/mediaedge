use thiserror::Error;
use serde::{Deserialize, Serialize};

#[derive(Error, Debug)]
pub enum MediaEdgeError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    
    #[error("API error: {0}")]
    Api(ApiError),
    
    #[error("Deserialization error: {0}")]
    Deserialization(String),
    
    #[error("Empty response")]
    EmptyResponse,
    
    #[error("Authentication required")]
    AuthRequired,
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Rate limited: retry after {0} seconds")]
    RateLimited(u64),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: u16,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "API Error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

pub type Result<T> = std::result::Result<T, MediaEdgeError>;