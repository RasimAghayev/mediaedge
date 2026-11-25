---
name: Bug Report
title: "[Bug] <short description>"
labels: ["type:bug", "triage"]
assignees: []
---

## Objective
<One-sentence description of the bug>

## Business Context
<Impact on users / media-CDN workloads>

## Technical Context
<Steps to reproduce, current behavior, expected behavior, relevant code>

## Architecture Requirements
- [ ] Requirement 1
- [ ] Requirement 2

## Implementation Prompt
<Detailed, executable instructions for the agent>

## Subtasks
- [ ] Subtask 1 → `subtask/ISSUE-<n>-1-<slug>`
- [ ] Subtask 2 → `subtask/ISSUE-<n>-2-<slug>`

## Files & Directories
- `crates/mediaedge-<component>/src/...`
- `services/<service>/src/...`

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
