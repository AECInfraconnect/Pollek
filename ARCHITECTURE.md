# Pollen DEK — Architecture

Pollen DEK is a Rust **Policy Enforcement Point (PEP)** with a local **Policy
Decision Point (PDP)**, built as a multi-crate workspace. It enforces signed
policy bundles produced by a control plane — either the local-first **Local
Control Plane** or **Pollen Cloud** — over one shared contract.

## Dual-mode design

| | Local (OSS) | Cloud (commercial) |
|---|---|---|
| Storage | SQLite | MySQL/TiDB |
| Tenancy | single-user (`tenant_id=local`) | multi-tenant, RBAC |
| Transport | HTTP `127.0.0.1` | mTLS over internet |
| Auth | Local Bearer token | X.509-SVID + OAuth + JWT-SVID (SPIFFE/SPIRE) |
| Trust root | pinned local signing key | SPIRE trust bundle (rotatable) |

**Invariants (same in both modes):**
- **I1** identical schema / bundle format / telemetry envelope (`dek-control-plane-api` is the single source of truth) — the DEK can't tell Local from Cloud.
- **I2** protocol/security may differ (Local = HTTP+Bearer, Cloud = mTLS+OAuth+SPIFFE).
- **I3** bundles are always signed; the DEK verifies identically and fail-closed.
- **I4** storage differs behind a trait; the DEK is unaffected.
- **I5** hot-reload behaves the same (polling + SSE push). Cutover = `dek-cli profile set`.

## Crate map

**Control / supervision**
- `dek-core` — supervisor: HTTP/IPC API (PEP on `:43890`), config load, SVID/mTLS lifecycle, hot-reload coordination, network enforcement loop, identity-health gate.
- `dek-config` — bootstrap config, profiles, paths.
- `dek-policy-syncer` — bundle sync (poll + SSE push), enforcement-state machine (Active / GracePeriod / StrictDeny), fail-closed freshness gate.
- `dek-bundle-sync` — TUF-lite fetch + signature verification (chain of trust).
- `dek-activation` — atomic bundle activation, hydration, LKG fallback.
- `dek-auth` — authentication and session handling primitives used by MCP proxy and activation.
- `dek-secure-spool` — durable disk-backed queueing for telemetry and audit events before shipping.

**Decision / PEP**
- `dek-mcp-proxy` — MCP authorize endpoint; emits decision telemetry; obligations (require_approval / step_up_mfa).
- `dek-policy-router` — route matching + **adaptive engine selection** (`engine_selector`), circuit breakers, per-tenant admission, failover, break-glass.
- `dek-policy-runtime` — `PolicyRuntime` trait + Wasmtime runtime.
- Adapters: `dek-cedar`, `dek-openfga`, OPA via Wasm — built on `dek-pdp-sdk`, feature-gated by `dek-router-builder`.
- `dek-plugin-sdk` / `dek-plugin-host` — transform plugins (e.g. `dek-pii-wasm`).
- `dek-resilience` — circuit breakers, admission control, and system overload protections.

**Network enforcement (OS)**
- `dek-ebpfd` (+ `dek-ebpf-prog`, `dek-ebpf-common`) — Linux eBPF cgroup enforcement (kernel).
- `dek-windows-wfp` — Windows Filtering Platform (user-mode today; kernel callout driver in progress).
- `dek-macos-nefilter` — macOS NetworkExtension / System Extension.
- **Kernel complexity guard** (`dek-core::kernel_guard`) — only simple, exact rules (CIDR/port/exact-domain, bounded count) go to the kernel; complex rules fall to the user-mode plane to avoid verifier rejection/instability.

**Identity (Cloud)**
- `dek-spire-node` — node attestation (join token → CSR → X.509-SVID), JWT-SVID cache, trust-bundle polling/rotation.
- `dek-enroll` — enrollment + OAuth device flow.

**Control planes**
- `dek-control-plane-api` — shared contract (bundle manifest, telemetry envelope, registry objects, policy drafts, identity modes).
- `local-control-plane` — Axum + SQLite + local signing; registry/policy/bundle/telemetry/push.
- `apps/local-admin-dashboard` — React/Vite UI (registry, policies, decision logs).
- `mock-cloud` — reference Cloud implementing the same contract for offline testing.

## Decision data flow

1. App sends a `DecisionRequest` to the DEK PEP on `127.0.0.1:43890`.
2. `dek-policy-router` matches a route; if no engine is pinned, `engine_selector` picks one (Cedar/OPA/OpenFGA/eBPF) by decision kind — choosing only engines compiled into this build.
3. The selected evaluator(s) run (behind circuit breakers + admission control).
4. Transform plugins (e.g. PII redaction) apply to obligations/effects.
5. The decision is enforced and emitted as a signed telemetry envelope; network rules are split across kernel and user-mode planes by the complexity guard.

## Failure posture (fail-closed everywhere)

- No bundle / stale bundle past `max_bundle_age` → strict-deny.
- PDP down / circuit open / admission exceeded → deny.
- Cloud unreachable → last-known-good, then strict-deny once stale.
- Identity SVID expired and un-renewable → deny (identity gate).
- Kernel rule apply fails → block-all at the kernel plane.

Authoring and compilation happen on the control plane (Local or Cloud) — **never
on the DEK**.
