# Auto Discovery Coverage Expansion

## Scope

Auto Discovery should remain local-first and privacy-preserving while expanding
coverage for AI agents, agentic host applications, MCP/A2A endpoints, local
model servers, containers, IDE extensions, frameworks, NVIDIA NIM deployments,
Hugging Face entities, tools, resources, models, and capabilities.

The current repo already has a strong modular foundation in
`crates/dek-agent-discovery`: process scanning, installed app scanning, MCP
config scanning, local model probing, IDE extension scanning, CLI agent scanning,
container scanning, browser/session/window scanning, web AI/SNI scanning, Python
framework scanning, signature matching, source cataloging, aggregation, and scan
orchestration.

The next expansion must extend that architecture. It should not replace it with
one large scanner.

## Current Implemented Slice

Pollek now exposes a first canonical discovery inventory slice:

- `dek-agent-discovery::capability_inventory` derives
  `DiscoveryEntityCandidate`, `CanonicalCapability`, and relationship records
  from existing source-backed discovery evidence.
- Local Control Plane exposes:
  - `GET /v1/tenants/{tenant}/discovery/entities`
  - `GET /v1/tenants/{tenant}/discovery/candidates/{candidate_id}/capabilities`
  - `POST /v1/tenants/{tenant}/discovery/candidates/{candidate_id}/retrieve-capabilities`
- `retrieve-capabilities` persists canonical entities, capabilities, and
  relationships as registry raw objects for local use and future Cloud sync.
- The Local Dashboard Auto Discovery detail pane shows a friendly Capabilities
  view with privacy profile, collection cost class, capability source, risk tags,
  and relationship links.
- Contract Hub TypeSpec/OpenAPI/TypeScript artifacts include the same
  `DiscoveryEntityCandidateV1` and `DiscoveryCapabilityInventoryResponse`
  shapes for Local Dashboard and Pollek Cloud.
- `dek-agent-discovery::capability_retrieval` performs a bounded, read-only MCP
  Streamable HTTP capability listing (`initialize`, `notifications/initialized`,
  `tools/list`, `resources/list`, `prompts/list` only) against loopback
  MCP-compatible ports found during the local model/MCP port probe. It never
  sends `tools/call`, `resources/read`, or `prompts/get`. Retrieved tool,
  resource, and prompt metadata (including declared `inputSchema`) flows through
  `capability_inventory` into per-item `CanonicalCapability` records
  (`mcp_tool`, `mcp_resource`, `mcp_prompt`) so `retrieve-capabilities` reflects
  real server-declared capabilities instead of only inferred placeholders.

Beyond that MCP listing vertical, this slice remains metadata-derived. It does
not read raw prompts/responses, call authenticated external provider APIs, or
download model weights.

## Design Principles

- Definition-driven product coverage: fast-changing agent signatures, endpoint
  profiles, model catalogs, risk rules, and parser hints should come from signed
  discovery definitions, not hardcoded Rust releases.
- Capability-aware output: discovery should identify not only an agent candidate,
  but also tools, resources, MCP servers, models, model providers, inference
  endpoints, frameworks, containers, and relationships.
- Metadata-only by default: no process memory reads, no chat scraping, no raw
  prompt/response logs, no arbitrary source execution, no model weight download,
  no MCP tool invocation, and no MCP resource content reads.
- Bounded probes: every file read, loopback request, provider metadata call, and
  container/Kubernetes query needs timeouts, byte limits, cancellation, and source
  metrics.
- Demo isolation: demo discovery fixtures must use `source: demo_fixture` or an
  equivalent marker and must never mix with real discovery results unless the user
  explicitly enables demo mode.

## Definition V4 Direction

Extend `dek-fingerprint-defs` with a future `pollek.discovery-def.v4` format that
can carry:

- `agent_signatures`
- `agentic_host_signatures`
- `config_parsers`
- `capability_retrievers`
- `endpoint_probe_profiles`
- `model_catalog_sources`
- `model_family_rules`
- `tool_capability_rules`
- `risk_rules`
- `performance_profiles`
- `privacy_profiles`
- existing web, browser, installed app, cloud resource, and AI process hints

Definitions must support signed full/delta updates, local last-known-good
rollback, size validation, removed IDs, and compatibility checks against the
local engine version.

## Capability Retrieval Layer

Add retrievers as small bounded modules under
`crates/dek-agent-discovery/src/capability_retrieval.rs` (and split into a
directory as more transports are added):

- MCP: initialize, `tools/list`, `resources/list`, and `prompts/list` only.
  **Implemented** for loopback Streamable HTTP MCP servers found by the local
  model/MCP port probe; still outstanding for stdio-launched and
  configured-remote MCP servers, which are not probed live for privacy/consent
  reasons (config discovery only stores a redacted domain, not a full URL).
- A2A: retrieve and validate `/.well-known/agent-card.json` when configured or
  discovered.
- OpenAI-compatible: loopback/configured `/v1/models`.
- Ollama: `/api/tags`.
- NVIDIA NIM: model/provider/entity metadata from configured endpoints,
  containers, or signed catalog definitions.
- Hugging Face: Hub/model-card/config/cache metadata only; never model weights.
- Container/Kubernetes: image labels, exposed ports, env key names only, and
  service metadata within consented bounds.
- Python/Node/workspace frameworks: package and manifest metadata only unless the
  user opts into a deeper workspace scan.

Each retriever should declare cost class, consent requirement, timeout, max
response bytes, host scope, redaction policy, and cache TTL.

## Canonical Entity Output

Current `DiscoveredAgentCandidateV2` remains compatible. New discovery output
should add canonical entities and relationships so the Dashboard can show the
same relationship-first view used by the rest of Local Pollek:

- Agent
- Agentic host
- Sub-agent
- MCP server
- Tool
- Resource
- Model provider
- Model
- Embedding model
- Reranker
- Safety/moderation model
- Vision/multimodal model
- Inference endpoint
- Container
- Framework
- IDE extension
- Browser extension

Relationship examples:

- agentic host contains sub-agent
- agent uses model
- agent uses tool
- tool accesses resource
- MCP server exposes tool/resource/prompt
- model provider serves model
- container hosts model server
- policy can target entity

## Source Matrix

Default-on sources should stay lightweight:

- process scan
- installed app scan
- MCP config scan
- configured or loopback MCP capability list
- local model probes on definition-declared ports
- container metadata when a local engine is available
- IDE and CLI agent signatures
- Python package/framework metadata
- browser window/session metadata
- SNI/web AI metadata when already available

Default-off or consent-gated sources:

- browser history
- authenticated provider metadata
- Kubernetes cluster discovery
- workspace source inspection beyond manifests
- any external provider API call that requires a credential reference

## Security Rules

Discovery must not:

- store API keys, OAuth tokens, cookies, bearer tokens, passwords, or raw header values
- invoke arbitrary tools or commands
- invoke MCP `tools/call`
- call MCP `resources/read` or `prompts/get` by default
- upload absolute local paths to Cloud
- download Hugging Face model weights
- scrape screens or browser content
- run untrusted source code

Evidence should classify privacy as `public_metadata`, `internal_metadata`,
`sensitive_metadata`, or `secret_redacted`.

## Implementation Phases

1. Definition V4 structs and validation in `dek-fingerprint-defs`.
2. `capability_retrieval` framework with budget, cache, and redaction helpers.
3. MCP and OpenAI-compatible retrievers as the first vertical slice.
4. Local Control Plane persistence/API for discovered capabilities and
   relationships.
5. Dashboard candidate details: capability groups, source health, privacy notes,
   model inventory, and relationship links.
6. NVIDIA NIM and Hugging Face metadata enrichment.
7. A2A Agent Card retrieval and agentic host definitions.
8. Signed definition delta lifecycle and rollback.

## Acceptance Criteria

- Broad default scan completes within the configured deadline and returns partial
  results when a source is slow.
- MCP capability retrieval lists tools/resources/prompts without invoking tools
  or reading resource content.
- OpenAI-compatible local model servers and Ollama model IDs can be discovered
  from bounded loopback probes.
- New product coverage can be delivered by a signed definition update.
- Demo fixtures are visibly marked and never overwrite real discovery/capability
  evidence.
- Dashboard cards show friendly summary details for discovered agents, tools,
  resources, models, and related entities.

## Coverage Expansion: Black-Box Browser Agents, Claw Family, Third-Party Engines (2026-07-14)

Definition `20260714001` expands the signature/footprint catalog and fixes the
grouping defects that made one real agent show up as several duplicate
candidates.

### Black-box agents in browsers

- New process signatures: `comet_browser` (Perplexity Comet), `dia_browser`
  (The Browser Company Dia), `chatgpt_atlas_browser` (OpenAI's agentic
  browser), plus matching `browser_processes` entries so tab/window scanning
  works inside those browsers.
- `headless_browser_automation` detects CDP/headless-driven black-box
  automation (Playwright/Puppeteer/Selenium/browser-use style) **only** from
  automation-specific evidence — `--remote-debugging-port=`, `--headless`,
  driver binaries (`chromedriver`, `geckodriver`, `headless_shell`), or
  playwright/puppeteer module paths — never from a normal interactive browser
  process.
- `browser_use_agent` fingerprints the Python `browser-use` library.
- New web-AI domain signatures: Qwen Chat, Kimi, Z.ai GLM, NotebookLM,
  Genspark, and `operator.chatgpt.com` (ChatGPT agent surface, excluded from
  the plain `chatgpt.com` signature via `not_alias_domains` so it is counted
  once, as the agentic surface).

### Claw-family agents

- `openclaw` now carries the full footprint: gateway port `18789`, legacy
  Clawdbot/Moltbot config dirs (`~/.clawdbot`, `~/.moltbot`), legacy npm
  package/cli/env markers, and dual-form cmd patterns. Legacy installs that
  never migrated still resolve to the same canonical agent.
- `hiclaw` / `claw_orchestrator` gained glob-form cmd patterns that work in
  the glob-based matchers (regex-only patterns were dead there).

### Local agents in third-party engines

- New engine signatures: `sglang` (30000), `tgi`
  (text-generation-launcher/router, 3000), `xinference` (9997), `llamafile`
  (8080), `mlx_lm` (8080), `anythingllm` (3001), `msty`; `vllm` now matches
  real launch forms (`vllm serve`, `python -m vllm.entrypoints...`).
- The local model probe now scans SGLang/Xinference/KoboldCpp/TGI ports and
  labels each probe hit with the engine's signature id.

### Duplicate-candidate prevention (grouping)

One real agent discovered through several sources now coalesces into one
candidate:

- Probe evidence without an explicit `port` field previously fell back to
  `listening_ports = 80`, breaking port-based signature attribution; the port
  is now parsed from the probed endpoint URL.
- A local-model probe candidate adopts the signature id carried in its
  provider label, so it lands in the same identity bucket as the process-scan
  candidate for the same engine (Ollama process + `:11434` endpoint = one
  agent).
- Near-duplicate catalog ids (`openclaw_agent` vs `openclaw`, `cursor_desktop`
  vs `cursor`, `aider_cli` vs `aider`, …) normalize to one canonical id during
  identity bucketing.
- Generic interpreter process names (`node`, `python`, `bun`, …) no longer
  match a signature on the bare process name alone — previously every `node`
  process on the machine scored 0.9 as OpenClaw, creating both false positives
  and duplicates. Cmd patterns, exe paths, markers, and ports still match.
