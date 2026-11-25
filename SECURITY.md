# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 1.0.x   | :white_check_mark: |
| < 1.0   | :x: |

## Reporting a Vulnerability

Please disclose security vulnerabilities responsibly. Email
security@mediaedge.example with a description of the issue, steps to reproduce,
and any affected versions. Do not open public issues for security vulnerabilities.

## Security Features

* S3 Signature v4 authentication for object operations
* OIDC/JWT for admin API and edge node registration
* mTLS for service-to-service communication
* AES-256-GCM at-rest encryption, TLS 1.3 in-transit
* IAM-style policy authorization with conditions
* Presigned URLs with short-lived, bounded scope

## Dependency Scanning

Dependencies are scanned in CI via `cargo-audit`, `cargo-deny`, Trivy, and
CodeQL. Dependabot and Renovate keep dependencies up to date.
