# Security Policy

## Supported Versions

Currently, the `main` branch and the latest `beta` release receive security updates.

## Reporting a Vulnerability

Please do NOT report security vulnerabilities via public GitHub issues.

Instead, please email security@aecinfraconnect.com with the details of the vulnerability. We will aim to respond within 48 hours.

## Threat Model & Scope

Pollen DEK treats the local environment with a bounded trust model. Any bypass of the application-level proxy on Windows/macOS is currently out of scope for security bounties until the OS-level enforcement (WFP/NetworkExtension) becomes stable.

Bypassing the proxy on Linux when eBPF guardrails are enabled IS considered a vulnerability.
