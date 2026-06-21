# Changelog

All notable changes to Pollen DEK are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/); the project uses semantic-ish
versioning with pre-release tags (e.g. `1.0.0-beta.1`).

## [1.0.0-beta.1] — 2026-06

First public beta. Pollen DEK can be downloaded, installed on Linux/macOS/Windows,
and run fully locally with the Local Admin Dashboard, or pointed at a Cloud-style
control plane (exercised via `mock-cloud` until Pollen Cloud is GA).

### Added
- **Official release pipeline** — signed binaries for Linux/macOS/Windows on
  GitHub Releases, each with `SHA256SUMS`, cosign signatures, and SBOM.
- **End-to-end auto-update** — `dek-cli update --channel beta` checks GitHub
  Releases, verifies SHA-256 **and** cosign (identity bound to this repo) before
  applying, with atomic swap and rollback on health-check failure.
- **Local Control Plane + Admin Dashboard** — single-user (`tenant_id=local`)
  control plane on SQLite with a React dashboard (registry, policies, decision
  logs); authors and publishes locally-signed policy bundles.
- **Dual-mode** — Local and Cloud share one schema, bundle format, and telemetry
  envelope; switching targets is `dek-cli profile set local|cloud` (endpoint +
  trust store only — enforcement code is identical).
- **Adaptive policy routing** — the router auto-selects the engine (Cedar /
  OpenFGA / OPA-Rego / eBPF) by decision kind when a route doesn't pin one,
  choosing only engines compiled into the build.
- **Kernel complexity guard** — only simple, exact network rules (CIDR / port /
  exact-domain, capped at 1024 entries) are pushed to the kernel (eBPF); complex
  or overflow rules fall back to the user-mode plane to avoid verifier
  limits/instability.
- **Plugin/Adapter SDK** — `dek-pdp-sdk` for custom policy engines and
  `dek-plugin-sdk` for transform plugins (e.g. `dek-pii-wasm` PII redaction);
  bundled adapters are feature-gated.
- **Cloud identity (preview)** — OAuth device-flow enrollment, node attestation
  (join-token → CSR → X.509-SVID), JWT-SVID issuance/caching, proactive SVID
  renewal, and trust-bundle rotation — all exercised against `mock-cloud`.
- **Compliance evidence** — control mapping (NIST/PDPA/HIPAA/SOC2/ISO27001),
  tamper-evident audit hash chain, and evidence-export guidance.

### Changed
- Telemetry is split by type to typed endpoints (decision-logs, security-events,
  traces, ebpf-events, metrics) matching the Cloud contract.
- Network capability is reported honestly per OS: kernel-enforced on Linux,
  redirect-advisory on Windows/macOS in beta.
- All workspace crates are Apache-2.0 with SPDX headers; `NOTICE` added.

### Security
- Fail-closed everywhere: no/stale bundle → strict-deny; PDP down or circuit
  open → deny; Cloud unreachable → last-known-good then strict-deny; expired,
  un-renewable identity → deny; kernel apply failure → block-all.
- Bundles are always signed; the DEK verifies identically regardless of source.
- Update artifacts must pass SHA-256 **and** cosign verification before apply.

### Known limitations
- Windows/macOS network enforcement is redirect-advisory in beta; kernel-grade
  enforcement (WFP callout driver / macOS System Extension) is in progress.
- Pollen Cloud is not yet public; the Cloud path is validated against
  `mock-cloud`, and SPIRE-server integration testing is pending.
- See [ARCHITECTURE.md](ARCHITECTURE.md) and the security model for the full
  capability matrix.

[1.0.0-beta.1]: https://github.com/AECInfraconnect/AntiG_Pollen_DEK/releases/tag/v1.0.0-beta.1
