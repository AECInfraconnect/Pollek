# README Promise Task List

<!-- readme-promises-sha256: 141f86ebdc3b1105ced97cf4979975d0cd220318a6aabd15b884b0e6e3e3a218 -->
<!-- readme-promises-count: 56 -->

This is the living implementation checklist for the user-facing promises in
`README.md`. When the README changes, update this file in the same commit and
run `python3 scripts/check_readme_promises.py --write`.

Status values:

- `Done`: implemented and covered by current local checks or existing tests.
- `Active`: implemented enough for local demo/use, with follow-up hardening.
- `Host-dependent`: real behavior depends on OS privileges or installed PEPs.
- `Planned`: promised surface exists, but production-grade implementation is
  still tracked here.

## Product Modes

| Status | README Promise | Implementation Task |
| --- | --- | --- |
| Done | Simple Mode | Keep PEP details hidden and expose only protection, discovery, policy, timeline, cost, resource, agent, and settings workflows. |
| Done | Advance Mode | Expose local power-user pages for simulator, presets, tools, identities, capabilities, integrations, and diagnostics. |
| Done | Enterprise Cloud Mode | Keep the mode locked until Pollek Cloud is configured and contract discovery succeeds. |
| Done | Enterprise Cloud mode claim | Gate sidebar routes, dropdown selection, and direct URL access through the same cloud connection state. |
| Done | Local mode claim | Keep Local Dashboard and Local Control Plane usable without Pollek Cloud. |
| Active | Optional cross-OS demo profiles | Preserve isolated demo capability snapshots that never overwrite real host observations. |

## Quickstart Flow

| Status | README Promise | Implementation Task |
| --- | --- | --- |
| Active | 3-Step Quickstart | Keep Scan, Protect, and Timeline as the primary demo path. |
| Active | Scan | Auto Discovery must surface real local agents plus source-backed evidence. |
| Active | Protect | Policy Presets and feasibility planning must lead to a deployable local control method. |
| Active | Timeline | Activity Timeline must stream agent, resource, tool, policy, cost, and token evidence from the Local Control Plane. |
| Active | Policy-First / PEP-Transparent Philosophy | Keep policy authoring user-facing while resolving PEP/PDP details through capability planning. |

## Enforcement And Policy

| Status | README Promise | Implementation Task |
| --- | --- | --- |
| Host-dependent | Enforce, don't just observe | Enforce MCP/tool policy everywhere possible and mark OS network/file enforcement honestly by readiness probe. |
| Active | Parameter-level access control | Keep MCP proxy parameter enforcement and redact/block behavior wired into decisions. |
| Active | Policy your way | Route decisions through Cedar, OPA/Rego, or OpenFGA and record the real decision path as evidence. |
| Active | Policy Presets V2 | Keep presets generated from dynamic PEP capability targeting and deployable from the dashboard. |
| Done | Dry-run Simulation | Keep simulator usable without changing live policy or telemetry state. |
| Active | Granular Control Modes | Support Observe, Warn, Approval, Enforce, and StrictDeny in policy decisions and UI summaries. |
| Active | Governance Loop | Keep Observe -> Suggest -> Enforce loop connected through telemetry and preset deployment. |

## Observability

| Status | README Promise | Implementation Task |
| --- | --- | --- |
| Active | Shadow AI Discovery | Detect browser/process/network/local-model agents and attach evidence/source IDs. |
| Done | Secure Telemetry Spool | Continue routing local observations through the secure spool without requiring extra IPC ports. |
| Active | Agent Fingerprint Definitions | Keep local baseline import plus signed Cloud delta update path in the contract. |
| Active | Agent Binding Governance | Bind discovered agents to runtime capabilities, identity metadata, and enforceability state. |
| Active | Token & Cost Ledger | Use exact provider usage first from wrappers, proxies, browser events, and logs; label metadata-only fallback as estimated. |
| Active | Data Resource Trace Depth | Record file, folder, DB, table/collection, host, and query-fingerprint metadata where the host can prove it. |
| Active | Telemetry-Driven Policy Suggestions | Generate specific policy suggestions from observed activity and risk signals. |
| Active | Trust Scoring | Keep trust score and anomaly obligations available for kill-switch or approval policies. |
| Active | Tamper-Evident Audit | Preserve local hash-chain evidence before telemetry leaves the machine. |

## Security And Trust

| Status | README Promise | Implementation Task |
| --- | --- | --- |
| Active | Air-Gapped & Offline Support | Keep offline fingerprint import and rollback usable in local-only environments. |
| Active | Content Guard | Maintain normalization, weighted prompt-injection scoring, PII checks, and response-side scanning. |
| Active | Rate Limiting | Keep per-agent token buckets protecting downstream providers. |
| Host-dependent | Kernel-grade network control | Report Enforce only after Linux eBPF, Windows WFP, or macOS NetworkExtension warm-check proves readiness. |
| Done | Capability honesty | Never mix demo fixtures with real host capability snapshots. |
| Done | Isolated local demo profiles | Keep demo snapshots explicitly opt-in and marked with fixture reason codes. |

## Cloud And Contracts

| Status | README Promise | Implementation Task |
| --- | --- | --- |
| Done | Local-first, Cloud-ready | Keep Local Dashboard, Local Control Plane, and Pollek Cloud on the same Contract Hub interface. |
| Done | speaks one contract to both local and cloud | Keep `/.well-known/pollek-contract`, generated OpenAPI, schemas, and Rust/TS clients aligned. |
| Active | Pollek Cloud commercial boundary | Keep OSS local components independent while supporting secure cloud enrollment and telemetry sync. |
| Active | SPIFFE/OAuth workload tracing | Bind registered agents to SPIFFE-ready workload identity metadata and Cloud auth tokens when connected. |
| Done | Download & verify | Keep release URLs, cosign identity, and updater metadata pointed at `AECInfraconnect/Pollek`. |

## Extensions And Interop

| Status | README Promise | Implementation Task |
| --- | --- | --- |
| Active | MCP Stdio Wrapper | Keep legacy CLI agents bridgeable through secure MCP stdio wrapping. |
| Active | Plugin / Adapter SDK | Keep PDP/plugin SDK APIs stable and feature-gated where needed. |
| Active | WASM Ext Authz | Keep WASM external authorization available for low-latency local custom decisions. |
| Planned | A2A Mediator Preview | Expand inter-agent trust protocol mediation beyond preview coverage. |
| Planned | Execution Sandbox Preview | Harden short-lived untrusted tool execution environments. |

## Dashboard Surface

| Status | README Promise | Implementation Task |
| --- | --- | --- |
| Done | Overview dashboard | Keep the first screen operational and local-health focused. |
| Active | Observability pages | Keep Auto Discovery, Shadow AI Inbox, Policy Suggestions, Cost Ledger, and Alerts backed by real telemetry. |
| Active | Policy pages | Keep Policy Enforcer, Policy Presets, and Simulator wired to local policy deployment paths. |
| Active | Registry pages | Keep Agents, MCP servers, Tools, Resources, Entities, Relationships, and provider cards using friendly summaries. |
| Active | Operations pages | Keep Bundles, exportable decision/activity logs, and Settings aligned with the current mode. |
| Done | Internationalization | Keep English and Thai labels available for user-facing navigation and status. |

## Crate And Layer Coverage

| Status | README Promise | Implementation Task |
| --- | --- | --- |
| Active | Control crates | Keep `dek-core`, config, sync, activation, auth, and secure spool buildable together. |
| Active | Decision crates | Keep policy router/runtime, Cedar, OpenFGA, OPA/WASM, and resilience connected. |
| Active | Observability crates | Keep discovery, observer, suggester, and telemetry crates compiling and sharing event contracts. |
| Host-dependent | Network crates | Keep eBPF, WFP, and macOS NE capability probes honest across OSes. |
| Active | Identity crates | Keep SPIRE node and enrollment model aligned with Cloud trace identity. |
| Active | Interop crates | Keep A2A, sandbox, connector, normalizer, stdio wrapper, binding, and fingerprint crates buildable. |
| Active | SDK crates | Keep PDP/plugin host and policy preset APIs aligned with generated contracts. |
| Active | Control Plane crates | Keep local-control-plane, control-plane API, and mock-cloud using the same Pollek contracts. |
