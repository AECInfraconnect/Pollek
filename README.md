# POLLEK.AI -- Open Source Local AI Policy Enforcement Kit

<img src="assets/POLLEK_LOGO.png" alt="POLLEK.AI Logo" width="250" />

[![CI](https://github.com/AECInfraconnect/Pollek/actions/workflows/ci.yml/badge.svg)](https://github.com/AECInfraconnect/Pollek/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/tag/AECInfraconnect/Pollek?include_prereleases&label=release)](https://github.com/AECInfraconnect/Pollek/releases)
[![Compatibility](https://img.shields.io/badge/Compatibility-Matrix-success.svg)](contracts/COMPATIBILITY.md)

**Pollek Local Enforcement Kit** is the local-first AI Agent Governance Runtime that discovers AI agents on a user's computer, deploys enforceable policies to the right PEP, evaluates decisions through local or cloud PDPs, records tamper-aware telemetry, and gives users a dashboard to observe, control, and prove what AI agents did.

It is an Apache-2.0 runtime that **enforces and observes AI-agent, MCP, API, and tool-call activity at the desktop/edge**. It runs **fully locally** with the built-in Local Admin Dashboard, or connects to **Pollek Cloud** (commercial) for managed multi-tenant policy, observability, and compliance. The Local Enforcement Kit speaks **one contract** to both — switching targets changes only the endpoint + trust store, never the enforcement code.

## Policy-First / PEP-Transparent Philosophy

Pollek is designed for humans. Users simply state their **Policy** (e.g., "Block PII to external agents"), select the **Agent**, and choose the **Control Level** (Observe, Enforce, etc.). Pollek automatically discovers the best underlying **PEP (Policy Enforcement Point)**—whether that's eBPF on Linux, WFP on Windows, or MCP Stdio wrapping—and applies the rule transparently. Users never have to manually configure complex routing or networking.

### 3 Operating Modes

1. **Simple Mode**: Focuses strictly on Data Protection and Agent Management. PEP configuration is fully hidden and auto-managed.
2. **Advance Mode**: Unlocks local power-user capabilities such as Simulator, detailed auditing, Policy Suggestions, Entities, Tools, Identities, and control-method diagnostics.
3. **Enterprise Cloud Mode**: Unlocks only after Pollek Cloud is configured and the connection probe succeeds. It enables centralized policy distribution, hot reload, telemetry sync, SPIFFE/OAuth-backed workload tracing, and compliance reporting across an organization.

---

## Features

### Enforcement & Policy

- **Enforce, don't just observe** — allow/deny/redact MCP tool calls and network
  egress against signed policy, fail-closed by default.
- **Parameter-level access control** — field-level restrictions on tool call
  parameters, enforced natively by the MCP proxy.
- **Policy your way** — Cedar (ABAC/RBAC), OPA/Rego (complex logic), OpenFGA
  (ReBAC); the router auto-selects the right engine per request.
- **Policy Presets V2** — pre-built Rego/Cedar/OpenFGA templates with dynamic PEP capability targeting; deploy common guardrails in one click.
- **Dry-run Simulation** — test draft policies with what-if scenarios from the
  dashboard without affecting live traffic.
- **Granular Control Modes** — define policies that simply `Observe`, `Warn`, require `Approval`, `Enforce`, or `StrictDeny` based on risk tolerance.

### AI Agent Observability & Fingerprinting

- **Shadow AI Discovery** — automatically detects unmanaged AI agents via eBPF/WFP network scanning and heuristic fingerprinting (Ollama, vLLM, Claude Desktop, GitHub Copilot, Cursor).
- **Secure Telemetry Spool** — telemetry events are seamlessly streamed via `dek-secure-spool`, providing a secure asynchronous feed for auto-discovery and offline processing without opening local IPC ports.
- **Agent Fingerprint Definitions** — natively supports Offline Baseline definitions with Cloud-pushed Delta updates over SSE. Definitions map agent binaries/processes to known identities securely with signature verification.
- **Agent Binding Governance** — Maps discovered agents to Runtime Capabilities (resolving HTTP/Stdio MCP surfaces dynamically) and enforces governance constraints throughout the agent lifecycle.
- **Token & Cost Ledger** - captures provider-reported token usage first from
  wrappers, proxies, browser events, and known agent logs, then labels any
  metadata-only fallback as estimated. Costs use provider-reported values first,
  then the configured price catalog when only exact token counts are available.
- **Data Resource Trace Depth** - records source-backed file, folder, database,
  table/collection, host, and query-fingerprint metadata where the local OS,
  wrapper, DB hook, or agent log can prove it. See
  [Resource Trace Depth](docs/RESOURCE_TRACE_DEPTH.md).
- **Telemetry-Driven Policy Suggestions** — auto-generates specific `DeployPreset` rules (like PII Redaction and Prompt Injection blocks) based on active observations from the secure spool.
- **Governance Loop** — fully integrated Observe → Suggest → Enforce cycle runs
  end-to-end.

### Security & Trust

- **Air-Gapped & Offline Support** — `dek-cli fingerprint import` natively supports injecting offline fingerprint definitions and rollbacks in completely air-gapped secure edge environments.
- **Trust Scoring** — calculates real-time Agent Trust Scores via `AgentBaseline`,
  enabling dynamic `KillSwitch` or `RequireApproval` obligations on anomaly.
- **Content Guard** — inspects payloads for prompt injection, PII leakage, and
  malicious content before policy evaluation triggers. The current local guard
  also normalizes encoded/obfuscated text, uses weighted scoring, and checks
  tool responses before returning them to an agent.
- **Rate Limiting** — token-bucket rate limiters per agent protect downstream
  endpoints from overuse and abuse.
- **Tamper-Evident Audit** — all decisions are securely queued locally with a
  SHA-256 hash chain before shipping, proving audit log integrity.
- **Kernel-grade network control** — eBPF on Linux (with DNS LRU caching and
  runtime modes); Windows WFP / macOS NetworkExtension are beta and report real
  `Enforce` only after the installed component and warm-check prove readiness.
- **Capability honesty** - Local capability snapshots separate real host
  readiness from opt-in demo fixtures, so production-like tests do not mix with
  demo output.
- **Isolated local demo profiles** - optional fixture snapshots can demonstrate
  Windows, Linux, and macOS readiness without changing the real host capability
  path. See [Local Demo Profiles](docs/local_demo_profiles.md).

### Platform

- **Local-first, Cloud-ready** — same schema, bundle format, and telemetry
  envelope in both modes. Built on OpenAPI, TypeSpec, and a shared Contract Hub
  for `/.well-known/pollek-contract` discovery.
- **A2A Mediator (Preview)** — Inter-Agent Trust Protocol mediator for Google A2A
  protocol communication between trusted agents.
- **Execution Sandbox (Preview)** — isolated, short-lived tool execution
  environments for untrusted code.
- **WASM Ext Authz** — natively integrates WASM-based External Authorization (`dek-ext-authz`) for fast, localized, custom edge authorization logic.
- **MCP Stdio Wrapper** — bridges legacy CLI-based agents (`dek-mcp-stdio-wrapper`), wrapping standard I/O to speak the Model Context Protocol securely.
- **Plugin / Adapter SDK** — `dek-pdp-sdk` for custom policy engines;
  `dek-plugin-sdk` for transform plugins (e.g. `pii-redactor`). Bundled adapters
  are feature-gated.
- **Internationalization** — Dashboard supports English and Thai natively.

---

## Dashboard

The **Local Admin Dashboard** (React/Vite) provides 20+ pages for full control:

| Category | Pages |
|----------|-------|
| **Overview** | Overview dashboard |
| **Registry** | Agents, MCP Servers, Tools, Resources, Entities, Relationships, Blackbox AI Providers |
| **Policy** | Policy Enforcer, Policy Presets, Simulator |
| **Observability** | Auto Discovery, Shadow AI Inbox, Policy Suggestions, Cost Ledger, Alerts |
| **Operations** | Bundles, Decision Logs (with CSV/JSON export), Settings (connectors, profiles) |

---

## Quickstart

### 3-Step Quickstart (Scan → Protect → Timeline)

The core workflow of Pollek is straightforward:

1. **Scan**: Run Auto-Discovery to automatically find hidden (Shadow) AI agents on your machine.
2. **Protect**: Deploy a Policy Preset (e.g., PII Redaction) to the discovered agent.
3. **Timeline**: View the agent's real-time activity and blocked events in its Timeline.

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

### Optional cross-OS demo profiles

The Local Dashboard can demonstrate Windows, Linux, and macOS readiness from one
development host without changing real host detection. Demo profiles are off by
default and must be explicitly enabled:

```bash
export POLLEK_ENABLE_DEMO_PROFILES=1
```

```powershell
$env:POLLEK_ENABLE_DEMO_PROFILES="1"
```

Then open **Capabilities** and choose a demo OS, or call
`/v1/tenants/local/devices/local/capability-snapshot-v2?demo_os=windows&demo_profile=ready`.
Demo snapshots are marked with `contract.reason_code=demo_fixture` and
`device_id=demo_*`; they do not replace the latest real capability snapshot.

### Enterprise Cloud mode

Enterprise Cloud appears in the dashboard only after Pollek Cloud is configured
and the local connection probe succeeds. The CLI `cloud` profile stores the
endpoint/trust configuration; the UI mode remains locked until contract
discovery proves that the endpoint is reachable.

```bash
dek-cli profile set cloud --url https://cloud.<your-cloud-domain> --tenant-id <tenant>
dek-cli enroll --cloud-url https://cloud.<your-cloud-domain>
dek-core &
```

## Download & verify

Binaries for Linux/macOS/Windows (both x86_64 and arm64/aarch64) are on **[GitHub Releases](https://github.com/AECInfraconnect/Pollek/releases)**.
Each asset ships with `SHA256SUMS`, GitHub Artifact Attestations (`actions/attest-build-provenance`), and a Sigstore cosign signature; verify before running:

```bash
# 1) Check SHA256SUMS
sha256sum -c SHA256SUMS

# 2) Verify Cosign Keyless Signature
cosign verify-blob --certificate <asset>.pem --signature <asset>.sig \
  --certificate-identity-regexp "https://github.com/AECInfraconnect/Pollek/.*" \
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
 Local Admin Dashboard            Pollek Cloud (commercial)
  SQLite · tenant=local            MySQL/TiDB · multi-tenant
  HTTP 127.0.0.1 · Bearer          mTLS + OAuth + SPIFFE/SPIRE
            \                         /
             \  same schema/bundle/  /
              \  telemetry + reload /
               ▼                   ▼
                ┌───────────────────┐
                │     Local Enforcement Kit (PEP)     │  profile: local | cloud
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
[Local Enforcement Kit↔Cloud contract](docs/contracts/pollek-cloud-dek-api.md).

## License

Local Enforcement Kit runtime, CLI, agent, SDK, adapters, and example policies are **Apache-2.0**.
**Pollek Cloud is commercial.** See [LICENSE](LICENSE) and [NOTICE](NOTICE).
