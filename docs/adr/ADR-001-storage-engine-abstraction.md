# ADR-001: Storage Engine Abstraction

## Status
Proposed

## Context
MediaEdge needs a pluggable storage backend supporting local FS, S3-compatible,
Azure Blob, and GCS. Options: OpenDAL, custom `object-store` trait, or SeaweedFS
/ Garage replication.

## Decision
Use OpenDAL as the backend abstraction with a custom `ObjectStore` trait layer
on top for MediaEdge-specific semantics (tiering, erasure coding, multipart).

## Consequences
- Pluggable backends via feature flags
- Additional dependency on OpenDAL
- Erasure coding implemented separately via reed-solomon-erasure
