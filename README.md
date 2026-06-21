# Pollen DEK — Open-Source Device Enforcement Kit

[![CI](https://github.com/AECInfraconnect/AntiG_Pollen_DEK/actions/workflows/ci.yml/badge.svg)](https://github.com/AECInfraconnect/AntiG_Pollen_DEK/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/tag/AECInfraconnect/AntiG_Pollen_DEK?include_prereleases&label=release)](https://github.com/AECInfraconnect/AntiG_Pollen_DEK/releases)
[![Compatibility](https://img.shields.io/badge/Compatibility-Matrix-success.svg)](contracts/COMPATIBILITY.md)

**Pollen DEK** is an Apache-2.0 runtime that **enforces and observes AI-agent, MCP,
API, and tool-call activity at the desktop/edge** — a Policy Enforcement Point
(PEP) with a local Policy Decision Point (PDP).

It runs **fully locally** with the built-in Local Admin Dashboard, or connects to
**Pollen Cloud** (commercial) for managed multi-tenant policy, observability, and
compliance. The DEK speaks **one contract** to both — switching targets changes
only the endpoint + trust store, never the enforcement code.

---

## Features

### Enforcement & Policy

- **Enforce, don't just observe** — allow/deny/redact MCP tool calls and network
  egress against signed policy, fail-closed by default.
- **Parameter-level access control** — field-level restrictions on tool call
  parameters, enforced natively by the MCP proxy.
- **Policy your way** — Cedar (ABAC/RBAC), OPA/Rego (complex logic), OpenFGA
  (ReBAC); the router auto-selects the right engine per request.
- **Policy Presets** — pre-built Rego/Cedar/OpenFGA policy templates for
  zero-config quickstart; deploy common guardrails in one click.
- **Dry-run Simulation** — test draft policies with what-if scenarios from the
  dashboard without affecting live traffic.

### AI Agent Observability & Fingerprinting

- **Shadow AI Discovery** — automatically detects unmanaged AI agents via OS
  process scanning and heuristic fingerprinting (Ollama, vLLM, Claude Desktop,
  GitHub Copilot, Cursor, and more).
- **Agent Fingerprint Definitions** — natively supports Offline Baseline definitions with Cloud-pushed Delta updates over SSE. Definitions map agent binaries/processes to known identities securely with signature verification.
- **Agent Binding Governance** — Maps discovered agents to Runtime Capabilities (resolving HTTP/Stdio MCP surfaces dynamically) and enforces governance constraints throughout the agent lifecycle.
- **Token & Cost Ledger** — tracks estimated token costs across all observed AI
  APIs via a configurable price catalog, with per-agent breakdowns.
- **Policy Suggestion Engine** — auto-generates Rego/Cedar policies based on
  observed cost thresholds, Shadow AI detections, and agent behavior anomalies.
- **Governance Loop** — fully integrated Observe → Suggest → Enforce cycle runs
  end-to-end.

### Security & Trust

- **Air-Gapped & Offline Support** — `dek-cli fingerprint import` natively supports injecting offline fingerprint definitions and rollbacks in completely air-gapped secure edge environments.
- **Trust Scoring** — calculates real-time Agent Trust Scores via `AgentBaseline`,
  enabling dynamic `KillSwitch` or `RequireApproval` obligations on anomaly.
- **Content Guard** — inspects payloads for prompt injection, PII leakage, and
  malicious content before policy evaluation triggers.
- **Rate Limiting** — token-bucket rate limiters per agent protect downstream
  endpoints from overuse and abuse.
- **Tamper-Evident Audit** — all decisions are securely queued locally with a
  SHA-256 hash chain before shipping, proving audit log integrity.
- **Kernel-grade network control** — eBPF on Linux (with DNS LRU caching and
  runtime modes); Windows WFP / macOS System Extension in progress.

### Platform

- **Local-first, Cloud-ready** — same schema, bundle format, and telemetry
  envelope in both modes. Built on OpenAPI, TypeSpec, and a shared Contract Hub
  for `/.well-known/pollen-contract` discovery.
- **A2A Mediator (Preview)** — Inter-Agent Trust Protocol mediator for Google A2A
  protocol communication between trusted agents.
- **Execution Sandbox (Preview)** — isolated, short-lived tool execution
  environments for untrusted code.
- **Plugin / Adapter SDK** — `dek-pdp-sdk` for custom policy engines;
  `dek-plugin-sdk` for transform plugins (e.g. PII redaction). Bundled adapters
  are feature-gated.
- **Internationalization** — Dashboard supports English and Thai natively.

---

## Dashboard

The **Local Admin Dashboard** (React/Vite) provides 19 pages for full control:

| Category | Pages |
|----------|-------|
| **Overview** | Overview dashboard |
| **Registry** | Agents, MCP Servers, Tools, Resources, Entities, Relationships, Blackbox AI Providers |
| **Policy** | Policy Enforcer, Policy Presets, Simulator |
| **Observability** | Auto Discovery, Shadow AI Inbox, Policy Suggestions, Cost Ledger, Alerts |
| **Operations** | Bundles, Decision Logs (with CSV/JSON export), Settings (connectors, profiles) |

---

## Quickstart

### Local mode (single machine, no Cloud)

The easiest way to start the **Local Control Plane** and **Local Admin Dashboard** is using the included one-click scripts. This will automatically compile the backend, build the frontend, and run the server silently in the background.

**On Windows (PowerShell):**
```powershell
.\start-dek.ps1
```

**On macOS/Linux:**
```bash
./start-dek.sh
```

After running the script, the Local Admin Dashboard will automatically open in your browser at `http://127.0.0.1:43891`.

To stop the Local Control Plane:
- Windows: `.\stop-dek.ps1`
- macOS/Linux: `./stop-dek.sh`

*(Advanced)* You can also run the backend manually, but you will need to leave the terminal open:
```bash
# Start the Local Control Plane
cargo run -p local-control-plane &

# (Optional) If you want to develop the dashboard frontend:
cd apps/local-admin-dashboard && npm run dev
```

See **[docs/quickstart_local_en.md](docs/quickstart_local_en.md)** (TH: `_th`).

### Pollen Cloud mode

```bash
dek-cli profile set cloud --url https://cloud.<your-cloud-domain> --tenant-id <tenant>
dek-cli enroll --cloud-url https://cloud.<your-cloud-domain>
dek-core &
```

## Download & verify

Binaries for Linux/macOS/Windows (both x86_64 and arm64/aarch64) are on **[GitHub Releases](https://github.com/AECInfraconnect/AntiG_Pollen_DEK/releases)**.
Each asset ships with `SHA256SUMS`, GitHub Artifact Attestations (`actions/attest-build-provenance`), and a Sigstore cosign signature; verify before running:

```bash
# 1) Check SHA256SUMS
sha256sum -c SHA256SUMS

# 2) Verify Cosign Keyless Signature
cosign verify-blob --certificate <asset>.pem --signature <asset>.sig \
  --certificate-identity-regexp "https://github.com/AECInfraconnect/AntiG_Pollen_DEK/.*" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" <asset>

# 3) Verify GitHub Artifact Attestation
gh attestation verify <asset> -o AECInfraconnect
```

Update in place (verifies cosign before applying, with rollback):

```bash
dek-cli update --channel beta
```

## Architecture (at a glance)

```
 Local Admin Dashboard            Pollen Cloud (commercial)
  SQLite · tenant=local            MySQL/TiDB · multi-tenant
  HTTP 127.0.0.1 · Bearer          mTLS + OAuth + SPIFFE/SPIRE
            \                         /
             \  same schema/bundle/  /
              \  telemetry + reload /
               ▼                   ▼
                ┌───────────────────┐
                │     DEK (PEP)     │  profile: local | cloud
                │ enforce + observe │
                │ shadow AI scanner │
                │ cost/token ledger │
                │ rate + trust gate │
                │ content guard     │
                │ policy suggester  │
                │ policy presets    │
                │ A2A mediator      │
                │ exec sandbox      │
                └───────────────────┘
```

Full detail: **[ARCHITECTURE.md](ARCHITECTURE.md)**.

## Crate Landscape (56 crates)

| Layer | Key Crates |
|-------|-----------|
| **Control** | `dek-core`, `dek-config`, `dek-policy-syncer`, `dek-bundle-sync`, `dek-activation`, `dek-auth`, `dek-secure-spool` |
| **Decision** | `dek-mcp-proxy`, `dek-policy-router`, `dek-policy-runtime`, `dek-cedar`, `dek-openfga`, `dek-opa-wasm`, `dek-resilience` |
| **Observability** | `dek-agent-discovery`, `dek-agent-observer`, `dek-policy-suggester`, `dek-telemetry` |
| **Network** | `dek-ebpfd`, `dek-ebpf-common`, `dek-windows-wfp`, `dek-macos-nefilter` |
| **Identity** | `dek-spire-node`, `dek-enroll` |
| **Interop** | `dek-a2a-mediator`, `dek-execution-sandbox`, `dek-agent-connector`, `dek-mcp-normalizer`, `dek-mcp-stdio-wrapper`, `dek-agent-binding`, `dek-fingerprint-defs` |
| **SDK** | `dek-pdp-sdk`, `dek-plugin-sdk`, `dek-plugin-host`, `dek-policy-presets` |
| **Control Planes** | `dek-control-plane-api`, `local-control-plane`, `mock-cloud` |

## Documentation

Start at **[docs/README.md](docs/README.md)** — install guides, user/developer
guides, runbooks, security model, compliance mapping, and the
[DEK↔Cloud contract](docs/contracts/pollen-cloud-dek-api.md).

## License

DEK runtime, CLI, agent, SDK, adapters, and example policies are **Apache-2.0**.
**Pollen Cloud is commercial.** See [LICENSE](LICENSE) and [NOTICE](NOTICE).
