//! Outbox pattern for reliable event publishing.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Outbox event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxEvent {
    pub id: String,
    pub subject: String,
    pub payload: Vec<u8>,
    pub headers: std::collections::HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub retries: u32,
}

/// Outbox for reliable event publishing.
pub struct Outbox {
    queue: Arc<Mutex<VecDeque<OutboxEvent>>>,
    max_size: usize,
}

impl Outbox {
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            max_size,
        }
    }

    pub async fn add(&self, event: OutboxEvent) -> anyhow::Result<()> {
        let mut q = self.queue.lock().await;
        if q.len() >= self.max_size {
            return Err(anyhow::anyhow!("Outbox full"));
        }
        q.push_back(event);
        Ok(())
    }

    pub async fn take_batch(&self, max_batch: usize) -> Vec<OutboxEvent> {
        let mut q = self.queue.lock().await;
        let mut batch = Vec::new();
        for _ in 0..max_batch {
            if let Some(event) = q.pop_front() {
                batch.push(event);
            } else {
                break;
            }
        }
        batch
    }

    pub async fn requeue(&self, event: OutboxEvent) {
        let mut q = self.queue.lock().await;
        q.push_front(event);
    }

    pub async fn len(&self) -> usize {
        self.queue.lock().await.len()
    }
}

impl Default for Outbox {
    fn default() -> Self {
        Self::new(10000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_outbox() {
        let outbox = Outbox::new(100);
        outbox
            .add(OutboxEvent {
                id: "1".into(),
                subject: "test".into(),
                payload: vec![],
                headers: Default::default(),
                created_at: chrono::Utc::now(),
                retries: 0,
            })
            .await
            .unwrap();
        assert_eq!(outbox.len().await, 1);
    }
}
