//! NATS JetStream consumer.

use async_nats::jetstream;
use futures::StreamExt;

/// Event consumer.
pub struct EventConsumer {
    js: jetstream::Context,
}

impl EventConsumer {
    /// Create a new consumer.
    pub fn new(js: jetstream::Context) -> Self {
        Self { js }
    }

    /// Subscribe to a subject.
    pub async fn subscribe(
        &self,
        subject: &str,
        durable_name: &str,
    ) -> anyhow::Result<jetstream::Consumer> {
        let consumer = self
            .js
            .create_consumer(
                "EVENTS",
                jetstream::consumer::Config {
                    durable_name: Some(durable_name.into()),
                    filter_subject: subject.into(),
                    ..Default::default()
                },
            )
            .await?;
        Ok(consumer)
    }

    /// Process messages from a consumer.
    pub async fn process<F, Fut>(
        &self,
        consumer: jetstream::Consumer,
        mut handler: F,
    ) -> anyhow::Result<()>
    where
        F: FnMut(async_nats::Message) -> Fut + Send,
        Fut: std::future::Future<Output = anyhow::Result<()>> + Send,
    {
        let mut messages = consumer.messages().await?;
        while let Some(msg) = messages.next().await {
            let msg = msg?;
            handler(msg).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_consumer() {
        // Requires NATS server
    }
}
