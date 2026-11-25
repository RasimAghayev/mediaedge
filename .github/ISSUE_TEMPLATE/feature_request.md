---
name: Feature Request
title: "[Feature] <short description>"
labels: ["type:feature", "triage"]
assignees: []
---

## Objective
<One-sentence description of the feature>

## Business Context
<Why this matters for media/CDN/edge use case>

## Technical Context
<Current state, relevant code, dependencies>

## Architecture Requirements
- [ ] Requirement 1 (e.g., "Must handle 10k req/s with p99 < 50ms")
- [ ] Requirement 2 (e.g., "Zero-downtime rolling upgrade")

## Implementation Prompt
<Detailed, executable instructions for the agent>

## Subtasks
- [ ] Subtask 1 → `subtask/ISSUE-<n>-1-<slug>`
- [ ] Subtask 2 → `subtask/ISSUE-<n>-2-<slug>`

## Files & Directories
- `crates/mediaedge-<component>/src/...`
- `services/<service>/src/...`
- `infrastructure/docker/compose/...`

## Test Requirements
- Unit: `cargo test -p mediaedge-<component>`
- Integration: `cargo test -p mediaedge-<component> --test integration`
- Contract: S3 API compliance for affected operations
- Chaos: Network partition during operation

## Security Requirements
- [ ] No secrets in code/config
- [ ] TLS/mTLS enforced
- [ ] Authorization checked on all endpoints
- [ ] Input validation / sanitization

## Acceptance Criteria
- [ ] AC1: Measurable, verifiable outcome
- [ ] AC2: ...

## Definition of Done
- [ ] All tests pass (unit, integration, contract)
- [ ] CI green (lint, typecheck, security, build)
- [ ] Documentation updated (API docs, runbooks, ADR if architectural)
- [ ] Observability: metrics, logs, traces for new code paths
- [ ] Deployed to `test` environment, smoke test passed

## PR Requirements
- Target branch: `test`
- Linked Issues: `Fixes #<epic>, #<task>, #<subtask...>`
- Conventional Commits: `feat(<component>): <description>`
- No AI signatures
