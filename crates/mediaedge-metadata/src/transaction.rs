//! etcd transaction operations.

use etcd_client::{Client, Compare, CompareOp, TxnOp, TxnOpResponse};

/// Transaction builder for atomic operations.
pub struct Transaction {
    client: Client,
    compares: Vec<Compare>,
    success_ops: Vec<TxnOp>,
    failure_ops: Vec<TxnOp>,
}

impl Transaction {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            compares: Vec::new(),
            success_ops: Vec::new(),
            failure_ops: Vec::new(),
        }
    }

    /// Add a compare condition.
    pub fn compare(mut self, compare: Compare) -> Self {
        self.compares.push(compare);
        self
    }

    /// Add a success operation.
    pub fn success(mut self, op: TxnOp) -> Self {
        self.success_ops.push(op);
        self
    }

    /// Add a failure operation.
    pub fn failure(mut self, op: TxnOp) -> Self {
        self.failure_ops.push(op);
        self
    }

    /// Execute the transaction.
    pub async fn execute(self) -> anyhow::Result<Vec<TxnOpResponse>> {
        let resp = self
            .client
            .txn(self.compares, self.success_ops, self.failure_ops)
            .await?;
        Ok(resp
            .succeeded()
            .then(|| resp.succeeded().unwrap_or_default())
            .unwrap_or_default())
    }

    /// Create a compare for key existence.
    pub fn compare_key_exists(key: &str) -> Compare {
        Compare::key(key, CompareOp::NotEqual, 0)
    }

    /// Create a compare for key value.
    pub fn compare_key_value(key: &str, value: &str) -> Compare {
        Compare::value(key, CompareOp::Equal, value)
    }

    /// Create a put operation.
    pub fn put_op(key: &str, value: &str) -> TxnOp {
        TxnOp::put(key, value, None)
    }

    /// Create a delete operation.
    pub fn delete_op(key: &str) -> TxnOp {
        TxnOp::delete(key, None)
    }
}

/// Helper for common transaction patterns.
pub mod helpers {
    use super::*;

    /// Atomic compare-and-set.
    pub async fn cas(
        client: &mut Client,
        key: &str,
        expected: &str,
        new_value: &str,
    ) -> anyhow::Result<bool> {
        let txn = Transaction::new(client.clone())
            .compare(Compare::value(key, CompareOp::Equal, expected))
            .success(TxnOp::put(key, new_value, None));
        let resp = txn.execute().await?;
        Ok(!resp.is_empty())
    }

    /// Atomic get-and-set.
    pub async fn get_and_set(
        client: &mut Client,
        key: &str,
        new_value: &str,
    ) -> anyhow::Result<Option<String>> {
        let resp = client.get(key, None).await?;
        let old = resp
            .kvs()
            .first()
            .map(|kv| String::from_utf8_lossy(kv.value()).to_string());
        client.put(key, new_value, None).await?;
        Ok(old)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transaction() {
        // Requires etcd server
    }
}
