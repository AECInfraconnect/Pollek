# Token And Cost Usage Research

Date: 2026-06-26

This note documents what POLLEK can measure accurately today, what must be
estimated, and what enforcement plane is required before a cost/token policy can
be treated as hard enforcement.

## Practical Conclusion

Token and cost enforcement is accurate only when POLLEK observes the LLM request
and response body, or when the provider exposes an authenticated usage/billing
API that can be reconciled to a known agent, API key, project, or account.

Browser tab discovery alone can identify `ChatGPT (Chrome)`, `Claude (Edge)`,
`Gemini (Safari)`, and similar web AI surfaces, but it cannot reliably retrieve
per-message token counts or cost from a normal consumer web UI. Window titles,
browser sessions, history, and SNI are metadata. They do not expose provider
usage payloads over TLS. For hard enforcement on browser-hosted AI, POLLEK needs
one of these controlled planes:

- A user-approved browser extension that can observe supported AI web apps.
- A managed API/proxy path used by the agent instead of unmanaged web UI calls.
- Provider organization/admin usage or billing APIs mapped to API key, project,
  workspace, or user identity.

The policy target must be browser-scoped whenever the same web AI is open in
multiple browsers. `ChatGPT (Chrome)` and `ChatGPT (Edge)` are separate
observable/enforceable surfaces. Token, cost, data access, tool use, resource,
and decision events should use the browser-scoped `agent_id` or include enough
metadata for the local control plane to derive it.

## Provider Usage Shapes

| Provider or runtime | Response usage fields POLLEK should parse | Notes |
| --- | --- | --- |
| OpenAI Responses / OpenAI-compatible | `usage.input_tokens`, `usage.output_tokens`, `usage.total_tokens`; older compatible chat APIs often use `usage.prompt_tokens`, `usage.completion_tokens`, `usage.total_tokens` | OpenAI also exposes an input token count endpoint for preflight estimates. Source: https://platform.openai.com/docs/api-reference/responses/object |
| Anthropic Claude Messages | `usage.input_tokens`, `usage.output_tokens` | Total is derived as input plus output when no total field is returned. Source: https://docs.anthropic.com/en/api/messages-examples |
| Google Gemini | `usageMetadata.promptTokenCount`, `usageMetadata.candidatesTokenCount`, `usageMetadata.totalTokenCount`, `modelVersion` | Source: https://ai.google.dev/api/generate-content |
| DeepSeek | `usage.prompt_tokens`, `usage.completion_tokens`, `usage.total_tokens`, plus cache/reasoning detail fields | Streaming can include final usage in the last chunk when requested. Source: https://api-docs.deepseek.com/api/create-chat-completion |
| xAI | `usage.prompt_tokens`, `usage.completion_tokens`, `usage.total_tokens`, plus token detail fields | Source: https://docs.x.ai/developers/rest-api-reference/inference/chat |
| Mistral | `usage` object on chat completion responses | Mistral's public schema models usage as `UsageInfo`. Source: https://docs.mistral.ai/api |
| Cohere | `usage.tokens.input_tokens`, `usage.tokens.output_tokens`; `usage.billed_units` is also available | Prefer `tokens` for usage accounting; `billed_units` can support billing reconciliation. Source: https://docs.cohere.com/reference/chat |
| Ollama | `prompt_eval_count`, `eval_count` on final responses | Local runtime cost is usually zero unless mapped to custom infrastructure cost. Source: https://github.com/ollama/ollama/blob/main/docs/api.md |

## Enforcement Model

1. Preflight: estimate or count input tokens before the call when the PEP has
   request access. Use provider token-count endpoints when available, otherwise
   use a model-specific tokenizer or conservative estimate.
2. Decision: compare estimated input tokens, requested max output tokens, and
   daily ledger state with policy caps. A strict deny decision is safe only for
   controlled PEPs such as an SDK wrapper, MCP wrapper, local proxy, managed API
   gateway, or approved browser extension.
3. Postflight: parse provider usage from the response body, write the cost
   ledger, calculate cost using a versioned price catalog, and reconcile later
   with provider billing exports where possible.
4. Coverage gaps: if POLLEK only has browser metadata, mark token/cost coverage
   as unavailable or estimated. Do not present it as exact. Policies may still
   block or require approval for unmanaged web AI use, but exact per-message
   cost enforcement requires a stronger observation plane.

## Browser-Scoped Observation Contract

Browser extensions, local API proxies, SDK wrappers, or other PEPs should send
the exact `agent_id` when they know it. When they do not, they should include a
payload shape like this in every observation or policy decision event:

```json
{
  "browser_scope": {
    "base_name": "ChatGPT (Web)",
    "browser_id": "chrome",
    "browser_name": "Chrome"
  },
  "payload_json": "{}"
}
```

The local control plane derives the same candidate id that discovery uses for
`ChatGPT (Chrome)`. This keeps all policy categories separated by browser:

- Token and cost budgets.
- Prompt, response, and data-loss controls.
- Tool/resource access policies.
- Allow/deny decisions and audit trails.
- Alerts and policy suggestions.

If a user opens the same AI app in Chrome and Edge simultaneously, the extension
or proxy should emit `browser_id: "chrome"` for Chrome traffic and
`browser_id: "edge"` for Edge traffic. Those events remain separate in the
ledger and in policy enforcement. Older observers can still send the same shape
inside `payload_json.web_ai`; the local control plane accepts both forms.

## Current Code Hooks

- `crates/dek-agent-observer/src/usage_model.rs` defines the provider-neutral
  `AiUsageEventV1` model used by Local Dashboard and Pollek Cloud.
- `crates/dek-agent-observer/src/providers/*` normalizes initial OpenAI,
  Anthropic, Gemini, and Bedrock usage payloads into canonical token classes.
- `crates/dek-agent-observer/src/usage_cost.rs` implements `PriceCatalogV2` with
  per-token-class pricing, catalog versioning, effective dates, and optional
  provider-reported cost override.
- `crates/dek-agent-observer/src/usage_budget.rs` evaluates observe/warn,
  approval/throttle, and deny-style budget actions for rolling windows.
- `crates/local-control-plane/src/usage_api.rs` exposes
  `/v1/tenants/{tenant}/usage/events`, `/summary`, `/ledger`, `/budgets`, and
  `/stream` while keeping the older `/observations/costs` compatibility route.
- `crates/local-control-plane/migrations/20260626000000_ai_usage_cost_v2.sql`
  stores canonical usage events, rollups, budget limits, and budget events in
  SQLite with idempotency and cloud sync status columns.
- `contracts/spec/rest/usage.tsp` and `contracts/schemas/ai-*.schema.json`
  publish the shared Local/Cloud interface through the Contract Hub.
- `apps/local-admin-dashboard/src/pages/CostLedger.tsx` displays live total
  spend, token-class breakdown, per-agent/provider/model summaries, budget
  status, and cloud sync state using the V2 usage summary plus SSE.

Next recommended implementation steps are provider billing reconciliation
adapters, signed production price catalog distribution, and stronger collection
planes for browser-hosted AI where exact token counts are unavailable today.
