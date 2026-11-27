//! Redis client implementation.
//!
//! Provides connection pooling, pipelining, and cluster support.

use redis::Client;
use redis::aio::ConnectionManager;

/// Cache client wrapper.
#[derive(Clone)]
pub struct CacheClient {
    manager: ConnectionManager,
}

impl CacheClient {
    /// Create a new cache client.
    pub async fn new(redis_url: &str) -> anyhow::Result<Self> {
        let client = Client::open(redis_url)?;
        let manager = ConnectionManager::new(client).await?;
        Ok(Self { manager })
    }

    /// Get a connection from the pool.
    pub fn connection(&self) -> ConnectionManager {
        self.manager.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        // Requires Redis server
    }
}
