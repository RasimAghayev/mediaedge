# ADR-002: Metadata Consensus

## Status
Proposed

## Context
Strongly consistent metadata (buckets, objects, policies) requires a consensus
store with watches and leases. Options: etcd, Consul, custom Raft.

## Decision
Use etcd for metadata consensus. etcd provides serializable reads, watches,
lease-based locking, and multi-AZ deployment.

## Consequences
- Operational complexity of running etcd cluster
- Strong consistency guarantees
- `etcd-client` Rust dependency
