# Pollen DEK — Local Mode Quickstart

Run the **entire Pollen stack on one machine** — no Pollen Cloud required. The
**Local Control Plane** is a single-user, `tenant_id=local` stand-in for Cloud:
you author policies, publish signed bundles, and the DEK enforces them and streams
decision logs back — all on `localhost`.

> Same schema, API contract, bundle format, and telemetry envelope as Cloud.
> Switching to Pollen Cloud later changes only the endpoint + trust store
> (`dek-cli profile set cloud ...`) — the DEK's enforcement code is unchanged.

## Prerequisites
- Rust toolchain (stable) + Node 20+ (for the dashboard)
- Linux/macOS/Windows (network guardrails are kernel-enforced on Linux; Windows/macOS are redirect-advisory in beta)

## 1. Build
```bash
cargo build --workspace
cd apps/local-admin-dashboard && npm install && npm run build && cd -
```

## 2. Start the Local Control Plane
```bash
# data dir holds the local bundle-signing key (created 0600 on first run)
DEK_LCP_DATA=./pollen-local-data \
DEK_LCP_DB="sqlite://./pollen-local.db?mode=rwc" \
  ./target/debug/local-control-plane
```
It logs the bundle-signing public key on startup:
```
Local Control Plane listening on http://127.0.0.1:3000
local control-plane signing key: local-ab12cd34 (pub Base64EncodedKey==)
```

## 3. Point the DEK at the Local Control Plane
```bash
# grab the trust key (or copy it from the log above)
curl -s http://127.0.0.1:3000/v1/tenants/local/devices/_/trusted-keys

dek-cli profile set local --url http://127.0.0.1:3000 --trusted-key "Base64EncodedKey=="
dek-cli profile show     # confirm mode=local, tenant_id=local
```

## 4. Enroll + run the DEK
```bash
dek-cli enroll --cloud-url http://127.0.0.1:3000
./target/debug/dek-core &     # PEP on :43890
dek-cli doctor                # checks certs / connectivity / permissions
dek-cli status                # enrollment + sync + enforcement state
```

## 5. Author → publish a policy
Use the dashboard (**Policy Enforcer** page) at http://127.0.0.1:3000, or the API:
```bash
curl -X POST http://127.0.0.1:3000/v1/tenants/local/policies \
  -H 'content-type: application/json' \
  -d '{"meta":{"schema_version":"1.0","tenant_id":"local","workspace_id":"default",
       "environment_id":"local","created_at":"2026-06-10T00:00:00Z",
       "updated_at":"2026-06-10T00:00:00Z","created_by":"local-admin",
       "updated_by":"local-admin","source":"manual","status":"draft","tags":[]},
       "policy_id":"pol-allow-echo","name":"allow echo","policy_type":"cedar",
       "targets":{"agent_ids":[],"tool_ids":[],"resource_ids":[],"entity_ids":[],"route_ids":[]},
       "source":{"kind":"raw_text","language":"cedar","text":"permit(principal, action, resource);"},
       "compile_options":{"fail_on_warnings":true}}'

# publish -> Local CP compiles + signs a bundle with the local key
curl -X POST http://127.0.0.1:3000/v1/tenants/local/policies/pol-allow-echo/publish
```
The DEK picks up the signed bundle on its next sync (a few seconds), verifies the
signature against the pinned local key, and hot-reloads it.

## 6. Enforce + view decision logs
```bash
curl -s -X POST http://127.0.0.1:43890/v1/authorize \
  -H 'content-type: application/json' \
  -d '{"mcp":{"method":"tools/call","params":{"name":"safe.echo"}},
       "principal":"me","tenant_id":"local","risk_tier":"low"}'
# -> { "allow": true, ... }

curl -s http://127.0.0.1:3000/v1/tenants/local/telemetry/decision-logs
# -> { "count": 1, "decisions": [ { ... "payload": { "decision": "allow" } } ] }
```
View them in the dashboard under **Audit & Decision Logs**.

## What just happened
1. The Local Control Plane **signed** the bundle with its own key.
2. The DEK **verified** it exactly as it verifies Pollen Cloud bundles — fail-closed if the signature doesn't match.
3. Decisions came back over the **same telemetry envelope** Cloud uses.

So the DEK never knows whether it's talking to Local or Cloud.

## Switching to Pollen Cloud (later)
```bash
dek-cli profile set cloud --url https://cloud.pollen.ai --tenant-id your-tenant
dek-cli enroll --cloud-url https://cloud.pollen.ai
# restart dek-core — same enforcement, multi-tenant control plane
```

## Guardrails (always on)
- The DEK never authors or compiles policy locally — that happens on the control plane.
- Bundles are always signed; unverifiable bundles are rejected (fail-closed).
- If the control plane is unreachable, the DEK serves the last-known-good bundle; once stale past `max_bundle_age`, enforcement defaults to deny.

## Troubleshooting
- `dek-cli doctor` reports cert/connectivity/permission problems and how to fix them.
- No decisions logged? Confirm `dek-core` is running and `dek-cli status` shows a recently synced bundle.
- Bundle rejected? The pinned trust key probably doesn't match the Local CP's key — re-run step 3 with the current `public_b64`.
