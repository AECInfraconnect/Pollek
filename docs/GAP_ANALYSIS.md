# Gap Analysis — Auto-Discovery, Observation, Policy Enforcement & Cloud Telemetry Sync

Date: 2026-07-19
Method: deep-dive code review of the full workspace (`crates/`, `contracts/`, `apps/`, `docs/`) across three research tracks: (1) discovery & observability, (2) policy enforcement, (3) cloud / telemetry sync. Every claim below cites the code.

This document answers one question: **what stands between the current codebase and a system that auto-discovers AI agents, observes them, enforces policies on them, and syncs telemetry to Pollek Cloud through a data hub?**

---

## 1. Auto-Discovery of AI Agents

### What genuinely works

- 10 concurrent scan sources in `dek-agent-discovery/src/orchestrator.rs`: process (real, `sysinfo`), MCP config, local model probe (Ollama/LM Studio/vLLM/Jan/GPT4All/llamacpp/sglang/xinference/koboldcpp/TGI ports), IDE extension, CLI agent, container, browser extension, installed app, web-AI (browser history + SNI), python framework.
- Real evidence-merge pipeline in `aggregator.rs` (~500 lines of tests): merge-key bucketing, identity coalescing with deterministic candidate IDs, surface grouping with collapse rules.
- Fingerprint definitions: embedded `baseline.v4.json` in `dek-fingerprint-defs` (56 process signatures, 26 web-AI, 21 installed-app, 6 cloud-resource); `DefinitionStore` loads signed on-disk definitions with ed25519 verification and applies full/delta updates hot.
- Real browser extension app (`apps/prompt-guard-browser-extension`) posting to `browser_extension_api.rs`; browser-history scan gated on consent.
- `register_candidate` in `local-control-plane/src/agent_discovery_api.rs` is real: schema-validates, upserts the registry, creates an `AgentBinding` from discovery.

### Gaps

| # | Gap | Evidence |
|---|-----|----------|
| D1 | **Network-SNI discovery leg is dead end-to-end.** The SNI flow source reads `event == "network.flow.v1"` from the *SQLite* telemetry spool — but nothing in the workspace produces `network.flow.v1` events. Real producers write different formats to a *different* store: `dek-windows-wfp` emits `network_observation` and `dek-ebpfd` emits `decision_log`, both into the *segment* spool (`.pds` files). | `local-control-plane/src/agent_discovery_api.rs:19-57`; `dek-windows-wfp/src/sni_observe.rs:32`; `dek-ebpfd/src/lib.rs:288`; `dek-agent-discovery/src/sni_source.rs` (its `FlowStore` trait has **no implementation anywhere**) |
| D2 | **Wrong spool path + wrong key.** Even if a producer existed, `SpoolFlowSourceImpl` opens `dek_config::paths::get_data_dir()/telemetry_spool.db` (default `C:\ProgramData\PollekDEK\state`) while the LCP writes its `SqliteSpool` at `./pollek-local-data` — and the two spools use different encryption keys (keyring/file key vs DPAPI-protected master key), so cross-reading silently decrypts nothing. | `agent_discovery_api.rs:19-57` vs `local-control-plane/src/main.rs:107-110`, `config.rs` |
| D3 | **Cloud-pushed fingerprint delta updates are not wired.** `FingerprintService` (signed verify, delta base check, rollback fallback — unit-tested) has no caller; `dek-control-plane-api/src/agents.rs:44` says "Mocked response for now"; DEK IPC `FingerprintAction` handler (`dek-core/src/ipc_server.rs:186-190`) logs and returns version 0 (TODO). The LCP loads `DefinitionStore` with `None` pubkey → unsigned definitions accepted. | `dek-fingerprint-defs/src/store.rs`, `verify.rs`; `local-control-plane/src/main.rs:102-105` |
| D4 | `source_catalog.rs` is dead code: `verify_catalog_signature` accepts any non-empty signature (`load_default_catalog` passes the literal `"mock_valid_signature"`). Public module, zero consumers. | `dek-agent-discovery/src/source_catalog.rs:23-36` |
| D5 | PEP binaries construct `Spool::default()` — temp dir, 100-byte cap, **no key manager** — so every spool write fails silently. Only `dek-core` wires the segment spool properly (100 MB, OS key store). | `dek-mcp-proxy/src/main.rs:409`; `dek-secure-spool/src/lib.rs` (`Default` impl); `dek-core/src/supervisor.rs:173` |
| D6 | macOS keychain module is an unimplemented demo and is not even wired — macOS falls back to the Linux file key store. | `dek-secure-spool/src/os/macos_keychain.rs:14-24`, `os/mod.rs` |

## 2. Observation & Telemetry Pipeline

### What genuinely works

- `dek-agent-observer`: real `SqliteObservationStore`, provider usage normalizers (OpenAI/Anthropic/Gemini/Bedrock response JSON → canonical `AiUsageEventV1`), two-generation cost ledger (`PriceCatalogV2` with tiers and regex model matching), OTel spans, anomaly detector (deny-rate → StrictDeny mutation + security event).
- LCP usage API: single/batch/provider-response ingestion, cost catalog with 3-tier fallback (v2 file → v1 file → embedded default), budget alerts, SSE stream. Harness-based e2e tests exist and are meaningful.
- `local_observe.rs` bridges *exact* usage from local agent logs (e.g. Codex token-count events). Estimated presence usage is honestly marked `estimated: true` / `capture_quality: "estimated_metadata_only"`.
- `dek-detection`: real detection-as-code engine (YAML packs, glob/sequence/anomaly), 5 core packs in `contracts/detections/packs/core-v1`, per-rule SHA-256 verification.
- `dek-secure-spool`: real hash-chained AES-256-GCM segment spool with tamper quarantine; `dek-telemetry` spooler: SQLite, AES-256-GCM at rest, 10k-row cap with priority eviction.

### Gaps

| # | Gap | Evidence |
|---|-----|----------|
| O1 | **Policy suggestions come from mock rules.** `generate_suggestions` uses 4 `Mock*Rule` structs returning canned suggestions (one always fires `total_cost=30.0`). The real rules (`ShadowAgentDetectionRule`, `HighRiskResourceRule`) exist in `rules.rs` but are never wired in. `simulate` in the LCP returns a hardcoded `{blocked: 5, allowed: 95}`. | `dek-policy-suggester/src/api.rs`; `local-control-plane/src/policy_suggestions_api.rs` |
| O2 | **Observe/Warn findings are dropped.** Guard pipeline computes findings with action forced to `Allow`, and the proxy silently discards `GuardAction::Allow` outcomes — no telemetry event, no response header. Warn mode is indistinguishable from Observe ("silent observe"). | `dek-mcp-proxy/src/main.rs:789`; `dek-guard-pipeline/src/lib.rs:305-322` |
| O3 | Windows WFP observation loop emits **fabricated** demo events (hardcoded 8.8.8.8/svchost every 60s). Capability detection is env-var spoofable. | `dek-windows-wfp/src/lib.rs:194-214`; `dek-capability-registry/src/detect.rs:140-167` |
| O4 | Detection-pack signature verification is a no-op callback (`Ok(())`), yet the API reports `manifest_integrity: "verified"`. Per-rule SHA-256 is still checked. | `local-control-plane/src/detection_api.rs:161,325` |
| O5 | `dek-agent-observer/src/coverage.rs` `update_coverage` is an empty no-op; `dek-risk-score` is fully orphaned (defined, never called, no tests); `dek-control-plane-api/src/agents.rs` router is entirely mocked (and unmounted — dead code). | respective files |
| O6 | Suggestion→artifact renderers are near-toy: Cedar emits a blanket `permit(...)`; OpenFGA ignores the suggestion entirely; only the Rego budget template has real content. | `dek-policy-suggester/src/render_cedar.rs`, `render_openfga.rs` |

## 3. Policy Enforcement

### What genuinely works

- MCP HTTP PEP end-to-end: JWT auth → admission backpressure → normalization → guard scan → multi-PDP `PolicyRouter` (Cedar / OPA-WASM / OpenFGA all real; 500 ms per-PDP timeout, circuit breakers, EWMA stats, deny/permit-overrides merge, shadow evaluation, break-glass) → obligations → response filtering with PII redaction.
- Guard pipeline is the strongest component: injection (weighted multilingual signatures + normalization evidence), PII (Luhn, Thai national-ID checksum, context boosting), output guard (secret-echo, system-prompt leak, canary), spotlighting. 46 tests + golden corpora.
- Fail-closed freshness gate (`dek-policy-syncer/src/gate.rs`): strict-deny on stale/absent bundle, enforced in the proxy.
- Linux eBPF egress blocking is real and can block (cgroup_sock_addr verdict maps + DNS cache). MCP stdio wrapper really evaluates every line (fail-closed, but unparseable lines pass through).
- Capability broker state machine (probe→consent→install→rollback with consent ledger) and feasibility planner with per-domain gaps.

### Gaps

| # | Gap | Evidence |
|---|-----|----------|
| E1 | **Approval mode has no backend.** `require_approval` → request is *denied* with `pending_approval` + a telemetry event. There is no approval queue, no resolution endpoint, no path to release a held call. Approval = "deny + notify". | `dek-mcp-proxy/src/main.rs:1143-1161`; `dek-core/src/api.rs:147-186` |
| E2 | **Preset deployments never reach the data plane.** `deploy_preset` only persists bindings into `policy_store`; nothing renders them into `active_bundle.json`, and the proxy hardcodes `GuardPipeline::default()` — deployed guard configs are never loaded. Rollback admits it should "also disable the PEP bindings here". | `local-control-plane/src/preset_deploy_api.rs:60-94,122`; `dek-mcp-proxy/src/state.rs:61` |
| E3 | **9 of 14 V2 presets render placeholder policies** with embedded `# TODO` text (budget caps, fs guards, approval presets, shadow-AI network block, tool allowlist). Only 5 presets render meaningful artifacts. | `dek-policy-presets/src/render.rs:42,51,57,84,90,96,102,108,114` |
| E4 | **Bundle signature check bypassed**: `verify_bundle(&bundle, "")` called with an empty key and the result discarded. OPA-WASM artifact signature verification is commented out. | `local-control-plane/src/policy_deploy_api.rs:121`; `dek-opa-wasm/src/lib.rs:36` |
| E5 | **macOS NetworkExtension sends empty rules** — `NeRuleMessage::from_compiled` discards all compiled rules and always sends empty block lists. | `dek-macos-nefilter/src/lib.rs:111-120` |
| E6 | Rego/OpenFGA dry-run is theater: the LCP endpoint evaluates them with `MockPolicyRuntime`; the decision has no relation to the submitted policy text. Wizard `simulate_deployment` is planning metadata, not evaluation (no historical replay). | `local-control-plane/src/policy.rs:573-637`; `preset_deploy_wizard_api.rs:155-247` |
| E7 | `dek-enforcement-api` is a facade: 8 macro-generated stub backends reporting hardcoded success, bailing egress observers, empty `CompiledRules`, unused `EnforcementRouter`. Dead code with honest-looking names; also dead `dek-pep-router`, and `dek-decision`'s alternate router ignores its routes. | `dek-enforcement-api/src/backends/mod.rs:10-91`, `egress_observer.rs:18-60`; `dek-decision/src/lib.rs:111-159` |
| E8 | Forward proxy tunnels CONNECT with zero policy evaluation while being presented as an enforcement surface. | `dek-mcp-proxy/src/main.rs:590-623` |

## 4. Telemetry Sync to Pollek Cloud ("data hub")

There is **no "data hub" concept anywhere in the repo** (`grep -ri "data[-_ ]?hub"` → zero matches). The foundation that exists for it: the TypeSpec contract (`contracts/spec/rest/telemetry.tsp`), `mock-cloud` (the de-facto reference implementation of a cloud ingest hub), and two uploader paths.

### What genuinely works

- **Policy/bundle sync (cloud → LEK) is the strongest part**: TUF-Lite metadata with ed25519 role verification, anti-rollback, sha256 artifact checks, atomic stage→rename, hybrid polling + SSE-triggered sync, 1s freshness watchdog with fail-closed enforcement states, key rotation. Contract-matrix tests cover forged signatures, key rotation, strict-deny FSM, and the PEP gate.
- Enrollment + identity: real RFC 8628 device flow, SPIFFE-style X.509-SVID lifecycle with fail-closed renewal health, trust-bundle hot poller, e2e test.
- Both offline queues are real (see §2). LCP uploader posts proper `telemetry-batch.v1` payloads to `/v1/telemetry/batches` and deletes on 2xx.
- `mock-cloud`: strict mTLS listener + enrollment listener, full device flow, SSE, TUF endpoints, registry CRUD, chaos middleware — a solid reference for a real data hub.

### Gaps

| # | Gap | Evidence |
|---|-----|----------|
| S1 | **Two divergent telemetry contracts.** DEK `CloudTelemetrySink` posts `{"events":[...]}` to legacy typed endpoints (`/v1/telemetry/decision-logs`, … from the older markdown contract) with no `X-Pollek-*` contract headers, while the LCP posts `telemetry-batch.v1` to `/v1/telemetry/batches` (the TypeSpec contract). mock-cloud serves both. | `dek-telemetry/src/lib.rs:224-322`, `routing.rs:22-31`; `local-control-plane/src/cloud_sync.rs:255-331` |
| S2 | **Fallback replay never acks.** `SecureFallback` replays the segment spool to `{cloud}/fallback` — a route that does not exist in mock-cloud — and never deletes/acks spooled records ("For now we just log success"). Records would be re-sent forever. | `dek-telemetry/src/fallback_spool.rs:43-56` |
| S3 | LCP cloud sync: Bearer-only auth (no mTLS/SPIFFE), tenant hardcoded `"local"`, pulled bundles discarded ("If we had a real cloud…"), pulled route suggestions discarded. | `cloud_sync.rs:47,229,349` |
| S4 | No server-side schema validation of ingested telemetry anywhere (mock-cloud accepts raw `serde_json::Value`); `contract-conformance` has a single bundle-envelope test. | `mock-cloud/src/telemetry.rs:61-69` |
| S5 | Zero end-to-end tests of the telemetry upload path (spool→flush→ingest→ack→delete); `dek-core/tests/e2e_mock_cloud.rs` is a 20-line stub. | respective files |
| S6 | Enrollment API undefined in the TypeSpec contract (`enrollment.tsp` is a 3-line stub); mock JWKS is a stub; SPIRE transport is HTTP-mock-shaped, not gRPC. | `contracts/spec/rest/enrollment.tsp`; `mock-cloud/src/main.rs:550-563` |

---

## 5. Fix plan

This PR fixes a bounded, verifiable subset — chosen for impact on the discover→observe→enforce→sync loop and reviewability:

1. **O1 — real policy suggestions.** Wire the real `ShadowAgentDetectionRule` / `HighRiskResourceRule` (and a real cost-spike rule) into `dek-policy-suggester::api::generate_suggestions`; remove the Mock rules from production paths. *(observe → enforce loop)*
2. **O2 — stop dropping Observe/Warn findings.** The MCP proxy emits a telemetry/observation event for guard findings even when the action is `Allow` (Observe/Warn), so observe-mode signal reaches the telemetry pipeline. *(observe)*
3. **E5 — macOS NE encodes real rules.** `NeRuleMessage::from_compiled` serializes the compiled block rules instead of sending empty lists. *(enforce)*
4. **E4 — bundle signature enforcement.** `policy_deploy_api.rs` fails the deploy when bundle verification fails, using the configured verify key instead of `""`. *(enforce)*
5. **S2 — fallback replay acks.** `fallback_spool.rs` deletes acknowledged records after a successful replay, and mock-cloud gains a fallback ingest route so the path is exercisable end-to-end. *(sync)*

### Deliberately out of scope for this PR (documented next steps)

- **Approval queue + resolution API** (E1) — needs a new stateful component and proxy hold/resume semantics; design first.
- **Preset → data-plane delivery** (E2) and the **9 placeholder preset renderers** (E3) — requires a bundle-compilation step from bindings; design first.
- **Network-SNI discovery leg unification** (D1/D2) — requires one canonical flow-event schema + one spool; touches ebpfd, WFP, dek-telemetry, discovery.
- **DEK uploader contract unification** (S1) — migrate `CloudTelemetrySink` to `telemetry-batch.v1`; coordinate with cloud side.
- **Windows WFP real observation** (O3), **Rego/FGA dry-run** (E6), **forward-proxy policy gate** (E8), **`dek-enforcement-api` facade** (E7) — each is its own PR.
- **"Data hub" naming** — if that is the intended product name for the ingest layer, build it on `mock-cloud` + `contracts/spec/rest/telemetry.tsp`; no existing code uses the term.
