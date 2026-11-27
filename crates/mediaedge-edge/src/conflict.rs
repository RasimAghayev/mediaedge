//! Conflict resolution for edge sync.

use std::time::Instant;

/// Conflict resolution strategy.
#[derive(Debug, Clone, Copy)]
pub enum ConflictStrategy {
    /// Last write wins (based on timestamp).
    LastWriteWins,
    /// Server wins.
    ServerWins,
    /// Client wins.
    ClientWins,
    /// Custom merge function.
    Custom,
}

/// Conflict resolution result.
#[derive(Debug, Clone)]
pub struct Resolution<T> {
    pub winner: T,
    pub strategy: ConflictStrategy,
    pub resolved_at: Instant,
}

/// Resolve a conflict between two values.
pub fn resolve<T>(local: T, remote: T, strategy: ConflictStrategy) -> Resolution<T>
where
    T: Clone,
{
    let winner = match strategy {
        ConflictStrategy::LastWriteWins => remote, // Assume remote is newer
        ConflictStrategy::ServerWins => remote,
        ConflictStrategy::ClientWins => local,
        ConflictStrategy::Custom => remote,
    };

    Resolution {
        winner,
        strategy,
        resolved_at: Instant::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve() {
        let r = resolve("local", "remote", ConflictStrategy::ServerWins);
        assert_eq!(r.winner, "remote");
    }
}
