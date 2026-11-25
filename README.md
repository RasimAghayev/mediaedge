# MediaEdge

High-performance, S3-compatible object storage platform built in Rust, optimized for media/CDN workloads with native edge/hybrid deployment (central cluster + edge nodes, offline sync, geo-replication).

## Technology Stack

| Layer | Choice |
|-------|--------|
| Language | Rust (2024 edition) |
| Async Runtime | Tokio + Tower |
| HTTP/Gateway | Axum + Tower-HTTP |
| Metadata Store | etcd |
| Hot Cache | Redis |
| Event Bus | NATS JetStream |
| Storage Engine | OpenDAL (pluggable backends) |
| Media Processing | ffmpeg-next + image + ravif |
| Observability | tracing + OpenTelemetry + Prometheus |
| Container | Distroless + cargo-chef |
| IaC | Terraform + Helm |
| GitOps | ArgoCD / Flux |

## Repository Structure

```
mediaedge/
├── crates/            # Library crates (workspace members)
├── services/          # Application binaries
├── infrastructure/    # Docker, Kubernetes, Terraform, monitoring, nginx
├── docs/              # Architecture, ADRs, API, runbooks
├── scripts/           # Dev setup, benchmark, chaos
└── .github/           # CI/CD, issue templates, dependabot
```

## Getting Started

### Prerequisites

- Rust (2024 edition) — see https://rustup.rs/
- Docker & Docker Compose
- Just (optional, for Makefile alternatives)

### Local Development

```bash
# Start the full dev stack (etcd, Redis, NATS, MinIO mock, gateway, storage-node)
make up

# Run the test suite
make test

# Run linter
make lint

# Stop the stack
make down
```

### CI/CD

All changes flow through `feature/ISSUE-<n>-<slug>` → `test` → `main` pull requests. GitHub Actions runs lint, format, typecheck, unit/integration/contract tests, security audits, and multi-arch container builds on every push to `test` and `main`.

## License

MIT License. See [LICENSE](LICENSE).

## Security

See [SECURITY.md](SECURITY.md) for the security policy and vulnerability reporting process.
