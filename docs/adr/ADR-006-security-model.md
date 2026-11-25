# ADR-006: Security Model

## Status
Proposed

## Context
Authentication and authorization for object and admin operations. Options:
IAM policies, ACLs, RBAC, S3 ACLs.

## Decision
IAM-style policy model with S3 Signature v4 for object ops, OIDC/JWT for admin
and edge registration, mTLS for service-to-service.

## Consequences
- Policy evaluation engine required (`mediaedge-auth`)
- S3 v4 signing implementation
