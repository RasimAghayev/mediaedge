---
name: Pull Request
about: Standard PR template for MediaEdge
title: "[<type>(<component>)] <short description>"
labels: []
assignees: []
---

## Summary
<One-sentence summary of the change>

## Changes
- <bullet 1>
- <bullet 2>

## Linked Issues
Fixes #<issue>
Refs: #<issue>

## Testing
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Contract tests pass (S3 API compliance where applicable)
- [ ] `cargo fmt --all --check` clean
- [ ] `cargo clippy --all-targets --all-features -D warnings` clean
- [ ] `cargo check --all-targets --all-features` clean

## Security
- [ ] No secrets introduced
- [ ] TLS/mTLS enforced where applicable
- [ ] Authorization checked on all endpoints
- [ ] Input validation / sanitization

## Documentation
- [ ] API docs updated (OpenAPI)
- [ ] Runbooks updated if new failure mode
- [ ] ADR added if architectural decision

## Checklist
- [ ] Target branch is `test`
- [ ] Conventional Commits used
- [ ] No AI signatures
- [ ] Linked to epic/task/subtask issues
