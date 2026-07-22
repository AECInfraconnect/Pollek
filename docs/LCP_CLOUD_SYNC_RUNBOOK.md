# LCP → Pollek Cloud sync client (the "Wallet") — runbook

This is the Local Control Plane / DEK side client that pushes **real** fleet,
telemetry, and cost/token data into Pollek Cloud so the console shows live data.
It follows the Cloud hand-off contract `2026.07.13`
(`pollek-cloud:docs/HANDOFF_LCP_SYNC.md`) in order and honors every gate — no
mock, no seed, no bypass.

## What ships

- **`crates/local-control-plane/src/cloud_sync_client.rs`** — the reusable,
  transport-only client library: `enroll`, `ingest_inventory`,
  `push_telemetry_batch`, `push_usage_ledger`, envelope builder, redaction
  guard (`contains_secret`), and `run_full_sync_once`. Unit tested.
- **`crates/local-control-plane/src/bin/cloud_sync_once.rs`** — a runnable CLI
  that performs one full ordered cycle and prints the real Cloud responses.
- **`crates/local-control-plane/src/cloud_sync.rs`** — the always-on background
  loop now **enrolls first every cycle** and applies the **redaction guard**
  before pushing telemetry (in addition to the existing registry/inventory and
  `ai_usage_event` telemetry sync).

## The ordered flow (and the gates it respects)

1. `POST /enroll` — **REQUIRED FIRST**. An unknown LCP is rejected and usage
   ledgers from it get `400 unknown_lcp:<id>`. The client always enrolls before
   anything else.
2. `POST /api/entities/ingest` — inventory snapshot (agents/tools/resources/relationships).
3. `POST /v1/telemetry/batches` — `telemetry-batch.v1` of `telemetry-envelope.v1`
   events (`ai_usage_event`, `decision_log`, `tool_invocation`, …). For cost/token,
   `ai_usage_event` carries `payload.tokens` + `payload.cost`.
4. `POST /v1/tenants/{tenant}/lcp/usage-ledgers` — billing-grade
   `pollek.lcp.usage-ledger.v1`.

Gates enforced by the client:

- **Idempotency** — telemetry is keyed by `tenant_id + event_id`; the client
  reuses the same `event_id` on retry, so replays return under `duplicates` and
  never double-count. Any 2xx is safe to clear the spool.
- **Redaction** — `redaction_applied` is set honestly and any event containing
  `authorization:`, `bearer` (with a trailing space), or `"password"` is dropped locally before send
  (the Cloud would otherwise quarantine it).
- **Consistent identity** — `tenant_id` / `device_id` / `lcp_id` are sent on
  every call (headers + body); never relying on Cloud defaults.
- **Production identity** — an OAuth/OIDC bearer is sent when available. Either
  supply a pre-fetched token in `DEK_CLOUD_API_KEY`, or set the `POLLEK_OIDC_*`
  variables and the client fetches one itself from Keycloak via the
  client-credentials grant. SPIFFE/mTLS is applied at the transport layer. Local
  dev may run auth-disabled over loopback.

## How to run

Configuration is by environment (same vars the LCP already uses):

| Var | Meaning | Default |
| --- | --- | --- |
| `DEK_CLOUD_URL` | Cloud base URL (local dev or Railway) | *required* |
| `DEK_CLOUD_API_KEY` | pre-fetched OAuth/OIDC bearer | *(empty ⇒ fetch via OIDC below, or omit for dev)* |
| `POLLEK_OIDC_ISSUER` | Keycloak realm issuer, e.g. `https://keycloak.../realms/pollek` (token endpoint derived) | *(unset ⇒ no fetch)* |
| `POLLEK_OIDC_TOKEN_URL` | explicit token endpoint (overrides issuer) | *(derived from issuer)* |
| `POLLEK_OIDC_CLIENT_ID` | e.g. `pollek-local-control-plane` | *(unset)* |
| `POLLEK_OIDC_CLIENT_SECRET` | client secret | *(unset)* |
| `POLLEK_OIDC_SCOPE` | optional scope | *(unset)* |
| `POLLEK_TENANT_ID` | tenant | `local` |
| `POLLEK_DEVICE_ID` | device id | `device_local` |
| `POLLEK_LCP_ID` | LCP id | `lcp_local` |
| `POLLEK_OS_FAMILY` / `POLLEK_OS_VERSION` / `POLLEK_HOSTNAME` / `POLLEK_ARCH` / `POLLEK_USER_SUBJECT` | device/user attributes | OS defaults |

When `DEK_CLOUD_API_KEY` is empty and the `POLLEK_OIDC_*` client-credentials are
set, the client fetches a bearer from Keycloak itself before syncing (prints
`auth=bearer (production)`); with neither, it runs unauthenticated (`auth=none`,
valid only against an auth-disabled dev Cloud).

On-demand full cycle with a built-in representative payload, then read back:

```bash
export DEK_CLOUD_URL=http://127.0.0.1:8790   # or your Railway URL
export POLLEK_DEVICE_ID=device_wallet_linux POLLEK_LCP_ID=lcp_wallet_linux
export POLLEK_OS_FAMILY=linux POLLEK_OS_VERSION="Ubuntu 24.04 LTS"

cargo run -p local-control-plane --bin cloud_sync_once -- --sample --replay --verify
```

Push your own real data instead of `--sample`:

```bash
cargo run -p local-control-plane --bin cloud_sync_once -- \
  --snapshot inventory.json \
  --telemetry events.json \
  --ledger usage-ledger.json \
  --verify
```

- `--snapshot` — the `snapshot` object (agents/tools/resources/relationships).
- `--telemetry` — a JSON array of `telemetry-envelope.v1` events.
- `--ledger` — a `pollek.lcp.usage-ledger.v1` document.
- `--replay` — send the telemetry batch twice to demonstrate idempotency.
- `--verify` — GET `/api/fleet`, `/api/telemetry/ingest-status`, and
  `/api/reports/cost-tokens/overview` afterwards.

### Against the Railway Cloud (production, Keycloak auth)

Point `DEK_CLOUD_URL` at the Railway URL and let the client authenticate itself
via Keycloak client-credentials:

```bash
export DEK_CLOUD_URL=https://pollek-cloud-production.up.railway.app
export POLLEK_OIDC_ISSUER=https://keycloak-production-a39c.up.railway.app/realms/pollek
export POLLEK_OIDC_CLIENT_ID=pollek-local-control-plane
export POLLEK_OIDC_CLIENT_SECRET=<client-secret>
export POLLEK_TENANT_ID=local POLLEK_LCP_ID=lcp_wallet POLLEK_DEVICE_ID=device_wallet

cargo run -p local-control-plane --bin cloud_sync_once -- --sample --replay --verify
```

Or pass a token you already hold in `DEK_CLOUD_API_KEY` instead of the
`POLLEK_OIDC_*` set. Nothing else in the flow changes.

> **Current auth state (as of Railway deploy):** the Cloud server does not yet
> *enforce* OAuth on the ingest endpoints (auth is provisioned but "planned"),
> so an unauthenticated run (`auth=none`) may still succeed. If a gateway/proxy
> in front of the Cloud returns **`401`**, that is the signal to supply the
> `POLLEK_OIDC_*` credentials (or `DEK_CLOUD_API_KEY`) so the client attaches a
> Keycloak bearer.
>
> **Fresh-deploy sanity:** a newly-deployed Cloud should report an empty
> `/api/fleet` (`lcps/entities/usage = 0`, tree = tenant root only). Run
> `cloud_sync_once --verify` (or just `curl .../api/fleet`) first; if it is not
> empty, stale persisted state is carried over — reset the Cloud state file.

The always-on background loop needs no flags: it enrolls, syncs the registry,
pushes spooled telemetry (redaction-guarded), and marks `ai_usage_event`s acked
each cycle whenever `DEK_CLOUD_URL` is set.

## Verified end-to-end (real Cloud, booted empty)

Run against a freshly-booted Pollek Cloud (`node apps/api/server.mjs`, boots
empty) with `cloud_sync_once --sample --replay --verify`:

| Step | Real result |
| --- | --- |
| Gate: usage-ledger **before** enroll | `400 unknown_lcp:<id>` (gate holds) |
| `enroll` | `200` + `join_token`, `spiffe_id`, trust bundle |
| `entities/ingest` | `202 accepted:true, entity_count:3, local_control_planes:1` |
| `telemetry/batches` | `accepted:2, rejected:0, duplicates:0` |
| telemetry **replay** (same `event_id`s) | `accepted:2, duplicates:2, stored:0` — no double-count |
| `lcp/usage-ledgers` (after enroll) | `202 accepted_count:1, total_tokens:2100, allocated_cost_cents:126` |
| redaction (event with a secret) | `rejected:1, reason:unredacted_secret_detected` |

Read-side verification (from empty → live):

- `GET /api/fleet` → `local_control_planes:1, telemetry_events:4, usage_records:2, local_entities:3`.
- `GET /api/telemetry/ingest-status` → `accepted:2, duplicates:…, rejected:0, quarantined_secrets:0, by_event_type:{ai_usage_event:1, decision_log:1}`.
- `GET /api/reports/cost-tokens/overview` → `total_tokens:2800, cost_cents:168` across `devices:1, users:1, agents:1, tenants:1, providers:1`.
- `GET /api/reports/cost-tokens?group_by=agent` → `agent_claude_code → total_tokens:2800, cost_cents:168, calls:4` (same by `device` / `user` / `tenant` / `provider`).

Re-running the same sample leaves the totals unchanged (`2800` / `168`),
confirming idempotency end to end.

## Known gaps (honest, not worked around)

- **`/enroll` returns `lcp_id: null`** in the response body and the derived
  `spiffe_id` uses the dev-default `lcp/lcp_local`, even though enrollment
  registers the sent `lcp_id` (the subsequent usage-ledger gate passes for that
  id). Per the hand-off §5, `/enroll` LCP registration is intentionally minimal;
  the client sends explicit ids so attribution is still correct.
- **Loop-side usage-ledger emission** is not yet wired: the background loop
  pushes `ai_usage_event` telemetry (which the Cloud bridges into cost/token),
  and the billing-grade `usage-ledger` path is delivered via the client library
  and `cloud_sync_once`. Sourcing ledgers from `ObservabilityStore::list_cost_ledger`
  inside the loop is the next hook.
- **Cloud persistence is dev-grade** (JSON snapshot); do not assume long-term
  durability until the PostgreSQL runtime store is enabled Cloud-side.
- **Railway production not yet exercised from this environment.** The
  end-to-end results above were captured against a freshly-booted **local**
  Pollek Cloud. The client now fetches a Keycloak bearer via client-credentials
  (`POLLEK_OIDC_*`), and the token request shape + issuer→token-endpoint
  derivation are unit tested, but a live run against
  `pollek-cloud-production.up.railway.app` + Keycloak is pending: this CI/agent
  environment's egress policy blocks `*.up.railway.app` (proxy `403` on
  CONNECT). Run the Railway command above from a network that can reach Railway
  to complete the production verification.
