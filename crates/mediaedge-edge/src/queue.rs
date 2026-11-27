//! Edge node offline queue.

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Queued operation.
#[derive(Debug, Clone)]
pub struct QueuedOp {
    pub id: String,
    pub op_type: OpType,
    pub payload: Vec<u8>,
    pub retries: u32,
    pub created_at: std::time::Instant,
}

/// Operation type.
#[derive(Debug, Clone)]
pub enum OpType {
    Read,
    Write,
    Delete,
    Sync,
}

/// Offline queue for edge nodes.
pub struct OfflineQueue {
    queue: Arc<Mutex<VecDeque<QueuedOp>>>,
    max_size: usize,
}

impl OfflineQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            max_size,
        }
    }

    pub async fn push(&self, op: QueuedOp) -> anyhow::Result<()> {
        let mut q = self.queue.lock().await;
        if q.len() >= self.max_size {
            return Err(anyhow::anyhow!("Queue full"));
        }
        q.push_back(op);
        Ok(())
    }

    pub async fn pop(&self) -> Option<QueuedOp> {
        self.queue.lock().await.pop_front()
    }

    pub async fn len(&self) -> usize {
        self.queue.lock().await.len()
    }
}

impl Default for OfflineQueue {
    fn default() -> Self {
        Self::new(10000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_queue() {
        let q = OfflineQueue::new(100);
        q.push(QueuedOp {
            id: "1".into(),
            op_type: OpType::Write,
            payload: vec![],
            retries: 0,
            created_at: std::time::Instant::now(),
        })
        .await
        .unwrap();
        assert_eq!(q.len().await, 1);
    }
}
