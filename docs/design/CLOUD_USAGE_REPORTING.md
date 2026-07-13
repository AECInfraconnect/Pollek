# Cloud Cost & Token Usage Reporting

Date: 2026-07-13

This note describes how AI cost and token usage flows from a Local Control Plane
(LCP) up to Pollek Cloud, and how Cloud rolls it up into reports broken down by
device, user, agent, and tenant.

See also `docs/COST_TOKEN_USAGE_RESEARCH.md` for what token/cost data can be
measured accurately versus estimated at the source.

## Data Flow

1. **Capture (LCP).** Observe ingestion produces canonical `AiUsageEventV1`
   records (from provider responses, SDK/proxy wrappers, browser extension, or
   local usage logs). Each event carries the dimensions Cloud reports on:
   `tenant_id`, `device_id`, `actor_id_hash` (a privacy-preserving hashed user
   id), `agent_id`/`agent_type`, `provider`, `model`, `tokens`, and `cost`.

2. **Spool + push (LCP → Cloud).** `usage_api::publish_ai_usage_event` wraps each
   event in a `PollekTelemetryEnvelopeV1` (`event_type = "ai_usage_event"`,
   envelope-level `tenant_id`/`device_id`) and pushes it into the secure spool.
   The `cloud_sync` loop drains the spool and POSTs a batch to
   `POST /v1/telemetry/batches`, then marks the AI usage events `acked`. Delivery
   is at-least-once, so the same `event_id` may arrive more than once.

3. **Ingest + flatten (Cloud).** `mock-cloud`'s telemetry ingest records every
   `ai_usage_event` envelope into a `UsageLedger` (see `mock-cloud/src/usage.rs`
   and `state.rs`). The ledger dedups by `event_id` so redelivery never
   double-counts cost or tokens. Only privacy-preserving identifiers are kept —
   the user dimension is the pre-hashed `actor_id_hash`.

4. **Report (Cloud).** Cloud aggregates the ledger on demand into grouped
   reports.

## Cloud Report Endpoints

- `GET /v1/usage/summary` — cross-tenant report, defaults to
  `group_by=tenant`.
- `GET /v1/tenants/{tenant}/usage/summary` — per-tenant report, defaults to
  `group_by=device`.
- `GET /v1/tenants/{tenant}/usage/records` — raw per-tenant records (newest
  first) for verification/debugging.

Query parameters:

- `group_by` — one of `device`, `user`, `agent`, `tenant`, `model`, `provider`.
- `from` / `to` — RFC3339 bounds (inclusive) on `occurred_at`.
- `limit` — cap for the raw-records endpoint.

A `usage-report.v1` response carries overall `totals`
(`request_count`, `input_tokens`, `output_tokens`, `total_tokens`,
`total_cost`, `currency`) and a `groups` array sorted by cost descending. Each
group row includes the same totals plus distinct counts of the other dimensions
(`devices`, `users`, `agents`, `models`), so a per-tenant row can still say
"3 devices / 5 users / 2 agents".

The `usage.reporting.v1` capability is advertised in
`/.well-known/pollek-contract`.

## Privacy & Security

- No raw prompts or responses leave the device; only aggregated token counts and
  costs plus the flattened dimensions above.
- The user dimension is a hash (`actor_id_hash`), never a plaintext user
  identity. When the source does not populate it, usage is grouped under
  `unknown`.
- Redaction validation on the telemetry ingest path still applies to usage
  envelopes.
- The ledger is memory-bounded; the oldest records are evicted past a fixed cap.
