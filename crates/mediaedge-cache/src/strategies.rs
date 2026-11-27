//! Cache strategies for different data types.

use std::time::Duration;

/// Cache strategy trait.
pub trait CacheStrategy {
    /// Get the TTL for this strategy.
    fn ttl(&self) -> Duration;

    /// Get the cache key prefix.
    fn key_prefix(&self) -> &'static str;
}

/// Signed URL cache strategy (short TTL).
pub struct SignedUrlStrategy;

impl CacheStrategy for SignedUrlStrategy {
    fn ttl(&self) -> Duration {
        Duration::from_secs(300) // 5 minutes
    }

    fn key_prefix(&self) -> &'static str {
        "signed_url"
    }
}

/// Rate limit cache strategy (medium TTL).
pub struct RateLimitStrategy;

impl CacheStrategy for RateLimitStrategy {
    fn ttl(&self) -> Duration {
        Duration::from_secs(60) // 1 minute
    }

    fn key_prefix(&self) -> &'static str {
        "ratelimit"
    }
}

/// Session cache strategy (long TTL).
pub struct SessionStrategy;

impl CacheStrategy for SessionStrategy {
    fn ttl(&self) -> Duration {
        Duration::from_secs(3600) // 1 hour
    }

    fn key_prefix(&self) -> &'static str {
        "session"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategies() {
        assert_eq!(SignedUrlStrategy.key_prefix(), "signed_url");
        assert_eq!(RateLimitStrategy.key_prefix(), "ratelimit");
        assert_eq!(SessionStrategy.key_prefix(), "session");
    }
}
