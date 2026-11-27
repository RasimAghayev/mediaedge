//! NATS JetStream publisher.

use async_nats::jetstream;
use serde::Serialize;

/// Event publisher.
pub struct EventPublisher {
    js: jetstream::Context,
}

impl EventPublisher {
    /// Create a new publisher.
    pub fn new(js: jetstream::Context) -> Self {
        Self { js }
    }

    /// Publish an event.
    pub async fn publish<T>(&self, subject: &str, event: &T) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        let payload = serde_json::to_vec(event)?;
        self.js.publish(subject.into(), payload.into()).await?;
        Ok(())
    }

    /// Publish with headers.
    pub async fn publish_with_headers<T>(
        &self,
        subject: &str,
        event: &T,
        headers: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        let payload = serde_json::to_vec(event)?;
        let mut msg = async_nats::Message::new(payload.into());
        for (k, v) in headers {
            msg.headers.insert(k, v);
        }
        self.js.publish(subject.into(), msg).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_publisher() {
        // Requires NATS server
    }
}
