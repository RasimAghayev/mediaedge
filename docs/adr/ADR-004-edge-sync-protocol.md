# ADR-004: Edge Sync Protocol

## Status
Proposed

## Context
Edge nodes operate offline and sync on reconnect. Conflict resolution needed
for concurrent mutations. Options: operational transform, CRDT, last-writer-wins.

## Decision
Last-writer-wins by default for objects; configurable CRDT for metadata.
Sync protocol uses `sync_cursor` (last known global sequence) for incremental
replication.

## Consequences
- Potential data loss on concurrent object writes (documented)
- Simpler than OT/CRDT for most media workloads
