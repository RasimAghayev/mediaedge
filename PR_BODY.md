## Summary

This PR implements **MediaEdge Phase 0-6** - Complete infrastructure, core crates, services, SDKs, CLI, Terraform provider, and React Console with real-time SSE and WCAG 2.1 AA accessibility.

## Changes

### Phase 6: Console & Developer Experience (NEW)
- **React + TypeScript + Vite** console at `apps/console/`
- **11 pages**: Dashboard, Streams, StreamDetail, Cluster, Tenants, TenantDetail, Monetization (5 tabs), Users, Settings (5 tabs), Login
- **Real-time SSE integration**: `src/services/realtime.ts` with EventSource, auto-reconnect, hooks for streams/cluster/alerts
- **WCAG 2.1 AA accessibility**: skip links, focus trapping, keyboard navigation, ARIA live regions, color contrast utilities
- **i18n**: English + Azerbaijani translations

### Phase 0-5: Core Infrastructure (142 files, 50k+ lines)
- **Core crates** (13): config, observability, api, protocol, transcoding, cluster, replication, storage, edge, auth, monetization, AI, metadata
- **Services** (5): gateway, edge-node, media-processor, replication-coordinator, storage-node
- **Rust client** (`crates/mediaedge-client-rust`): 9 modules (auth, tenants, streams, cluster, metrics, monetization, etc.)
- **CLI** (`crates/mediaedge-cli`): `mediaedgectl` with stream/tenant/cluster/monetization commands
- **TypeScript SDK** (`sdks/mediaedge-client-ts`): full API coverage
- **Go SDK** (`sdks/mediaedge-client-go`): 7 modules
- **Terraform provider** (`terraform-provider-mediaedge`): 6 resources + data sources
- **Multi-stage Dockerfile.rust**: FFmpeg 6.1.1 from source, cargo-chef, xattr 0.2.3 ENOATTR fix, Rust 1.96
- **Config examples**: gateway, edge-node, media-processor, replication-coordinator, storage-node

## Testing
- All code compiles in Docker multi-stage build
- Console TypeScript passes `tsc --noEmit`
- Rust workspace Cargo.toml version conflicts resolved
- xattr ENOATTR patched for Rust 1.96 compatibility

## Breaking Changes
None - new feature branch, no existing API changes.

## Related Issues
Implements FEATURE_PLAN.md phases 0-6 (26 issues across 10 phases).