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
`crates/dek-agent-discovery/src/capability_retrieval/`:

- MCP: initialize, `tools/list`, `resources/list`, and `prompts/list` only.
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
