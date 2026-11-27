//! Erasure coding for durability.

use reed_solomon_erasure::galois_8::ReedSolomon;

/// Erasure coding configuration.
#[derive(Debug, Clone, Copy)]
pub struct ErasureConfig {
    pub data_shards: usize,
    pub parity_shards: usize,
}

impl ErasureConfig {
    pub fn new(data_shards: usize, parity_shards: usize) -> Self {
        Self {
            data_shards,
            parity_shards,
        }
    }

    pub fn total_shards(&self) -> usize {
        self.data_shards + self.parity_shards
    }

    pub fn can_recover(&self, lost: usize) -> bool {
        lost <= self.parity_shards
    }
}

/// Erasure coder.
pub struct ErasureCoder {
    rs: ReedSolomon,
    config: ErasureConfig,
}

impl ErasureCoder {
    pub fn new(config: ErasureConfig) -> anyhow::Result<Self> {
        let rs = ReedSolomon::new(config.data_shards, config.parity_shards)?;
        Ok(Self { rs, config })
    }

    /// Encode data into shards.
    pub fn encode(&self, data: &[u8]) -> anyhow::Result<Vec<Vec<u8>>> {
        let mut shards =
            vec![vec![0u8; data.len() / self.config.data_shards + 1]; self.config.total_shards()];
        self.rs.encode(&mut shards)?;
        Ok(shards)
    }

    /// Decode shards back to data.
    pub fn decode(&self, shards: &mut [Option<Vec<u8>>]) -> anyhow::Result<Vec<u8>> {
        self.rs.decode(shards)?;
        let data_len = shards
            .iter()
            .filter_map(|s| s.as_ref())
            .map(|s| s.len())
            .sum();
        let mut data = Vec::with_capacity(data_len);
        for shard in shards.iter().take(self.config.data_shards) {
            if let Some(s) = shard {
                data.extend_from_slice(s);
            }
        }
        Ok(data)
    }

    /// Recover missing shards.
    pub fn recover(&self, shards: &mut [Option<Vec<u8>>]) -> anyhow::Result<()> {
        self.rs.decode(shards)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_erasure() {
        let config = ErasureConfig::new(4, 2);
        let coder = ErasureCoder::new(config).unwrap();
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let mut shards = coder.encode(&data).unwrap();
        let mut opt_shards: Vec<Option<Vec<u8>>> = shards.drain(..).map(Some).collect();
        // Simulate losing 2 shards
        opt_shards[0] = None;
        opt_shards[5] = None;
        let recovered = coder.decode(&mut opt_shards).unwrap();
        assert_eq!(recovered, data);
    }
}
