# Pollek Local Enforcement Kit — Local Mode Quickstart

Run the **entire Pollek stack on one machine** — no Pollek Cloud required. The
**Local Control Plane** is a single-user, `tenant_id=local` stand-in for Cloud:
you author policies, publish signed bundles, and the Local Enforcement Kit enforces them and streams
decision logs back — all on `localhost`.

> Same schema, API contract, bundle format, and telemetry envelope as Cloud.
> Switching to Pollek Cloud later changes only the endpoint + trust store
> (`dek-cli profile set cloud ...`) — the Local Enforcement Kit's enforcement code is unchanged.

## Prerequisites

- Rust toolchain (stable) + Node 20+ (for the dashboard)
- Linux/macOS/Windows (network guardrails are kernel-enforced on Linux; Windows/macOS are redirect-advisory in beta)

## 1. Build

For Linux/macOS or PowerShell 7+:

```bash
cargo build --workspace
cd apps/local-admin-dashboard && npm install && npm run build && cd -
```

For Windows PowerShell (older versions):

```powershell
cargo build --workspace
cd apps/local-admin-dashboard; npm install; npm run build; cd ../..
```

## 2. Start the Local Control Plane

For Linux/macOS or bash/Zsh:

```bash
# data dir holds the local bundle-signing key (created 0600 on first run)
DEK_LCP_DATA=./Pollek-local-data \
DEK_LCP_DB="sqlite://./Pollek-local.db?mode=rwc" \
DEK_LCP_AUTH_DISABLE=1 \
  ./target/debug/local-control-plane
```

For Windows PowerShell:

```powershell
# data dir holds the local bundle-signing key (created 0600 on first run)
$env:DEK_LCP_DATA="./Pollek-local-data"
$env:DEK_LCP_DB="sqlite://./Pollek-local.db?mode=rwc"
$env:DEK_LCP_AUTH_DISABLE="1"
.\target\debug\local-control-plane.exe
```

It logs the bundle-signing public key on startup:

```
Local Control Plane listening on http://127.0.0.1:3000
local control-plane signing key: local-ab12cd34 (pub Base64EncodedKey==)
```

## 3. Point the Local Enforcement Kit at the Local Control Plane

> **Note for Windows Users:** Keep the terminal from Step 2 open, and open a **NEW terminal window or tab** for Step 3 and beyond.

For Linux/macOS or bash/Zsh:

```bash
# copy the trust key from the control plane log above (looks like 'pub Base64EncodedKey==')
# (Optional for bash/Zsh) you can fetch it with curl if auth is disabled:
# curl -s http://127.0.0.1:3000/v1/tenants/local/devices/_/trusted-keys

./target/debug/dek-cli profile set local --url http://127.0.0.1:3000 --trusted-key "Base64EncodedKey=="
./target/debug/dek-cli profile show     # confirm mode=local, tenant_id=local
```

For Windows PowerShell:

```powershell
# Copy the trust key from the control plane log above (looks like 'pub Base64EncodedKey==')
.\target\debug\dek-cli.exe profile set local --url http://127.0.0.1:3000 --trusted-key "Base64EncodedKey=="
.\target\debug\dek-cli.exe profile show     # confirm mode=local, tenant_id=local
```

## 4. Run the Local Enforcement Kit

_(Note: In local mode, `profile set local` already bootstraps the configuration, so we skip `dek-cli enroll`)_

For Linux/macOS or bash/Zsh:

```bash
./target/debug/dek-core &     # PEP on :43890 (runs in background)
./target/debug/dek-cli doctor                # checks certs / connectivity / permissions
./target/debug/dek-cli status                # enrollment + sync + enforcement state
```

For Windows PowerShell:

```powershell
# dek-core blocks the terminal, so we use Start-Process to run it in the background
Start-Process .\target\debug\dek-core.exe -NoNewWindow
# Or just run it in a 3rd terminal window.
.\target\debug\dek-cli.exe doctor
.\target\debug\dek-cli.exe status
```

## 5. Author → publish a policy

Use the dashboard (**Policy Enforcer** page) at <http://127.0.0.1:3000>, or the API:

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

The Local Enforcement Kit picks up the signed bundle on its next sync (a few seconds), verifies the
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

View them in the dashboard under **Audit & Decision Logs**. You can also explore:

- **Auto Discovery** — see locally running AI agents detected by process scanning
- **Shadow AI Inbox** — alerts for unrecognized/unmanaged AI activity
- **Policy Suggestions** — auto-generated policy recommendations based on observations
- **Cost Ledger** - exact-first token and cost usage, with estimates labeled only
  when POLLEK sees metadata but no provider usage payload
- **Policy Presets** — deploy common guardrails (Block Shadow AI, Cost Budget, etc.) in one click
- **Blackbox AI Providers** — manage registered external AI providers
- **Alerts** — system-wide security and compliance notifications

## Optional: cross-OS dashboard demo

Demo profiles are off by default and do not change real host capability
detection. They are useful when you want to demonstrate Windows, Linux, and
macOS readiness from one development machine.

```bash
export POLLEK_ENABLE_DEMO_PROFILES=1
```

```powershell
$env:POLLEK_ENABLE_DEMO_PROFILES="1"
```

Then open **Capabilities** and select `Windows`, `Linux`, or `macOS`, or call:

```bash
curl "http://127.0.0.1:3000/v1/tenants/local/devices/local/capability-snapshot-v2?mode=desktop_advanced&demo_os=windows&demo_profile=ready"
```

Demo snapshots are marked with `contract.reason_code=demo_fixture` and
`device_id=demo_*`; they do not replace the latest real capability snapshot.

## Optional: response-side output guard check

The MCP proxy can scan tool output before it returns to an agent:

```bash
curl -s -X POST http://127.0.0.1:43890/v1/filter/response \
  -H 'content-type: application/json' \
  -d '{"result":"tool returned sk-test and <script>alert(1)</script>"}'
```

Unsafe output is denied or redacted before the agent sees it.

## What just happened

1. The Local Control Plane **signed** the bundle with its own key.
2. The Local Enforcement Kit **verified** it exactly as it verifies Pollek Cloud bundles — fail-closed if the signature doesn't match.
3. Decisions came back over the **same telemetry envelope** Cloud uses.

So the Local Enforcement Kit never knows whether it's talking to Local or Cloud.

## Switching to Pollek Cloud (later)

For Linux/macOS or bash/Zsh:

```bash
./target/debug/dek-cli profile set cloud --url https://cloud.<your-cloud-domain> --tenant-id your-tenant
./target/debug/dek-cli enroll --cloud-url https://cloud.<your-cloud-domain>
# restart dek-core — same enforcement, multi-tenant control plane
```

For Windows PowerShell:

```powershell
.\target\debug\dek-cli.exe profile set cloud --url https://cloud.<your-cloud-domain> --tenant-id your-tenant
.\target\debug\dek-cli.exe enroll --cloud-url https://cloud.<your-cloud-domain>
# restart dek-core
```

## Guardrails (always on)

- The Local Enforcement Kit never authors or compiles policy locally — that happens on the control plane.
- Bundles are always signed; unverifiable bundles are rejected (fail-closed).
- If the control plane is unreachable, the Local Enforcement Kit serves the last-known-good bundle; once stale past `max_bundle_age`, enforcement defaults to deny.

## Troubleshooting

- **Dashboard shows HTTP 404:** The local control plane can't find the web UI files. Stop it (`Ctrl+C`), set `$env:DEK_DASHBOARD_DIR=".\apps\local-admin-dashboard\dist"` (Windows) or `export DEK_DASHBOARD_DIR="./apps/local-admin-dashboard/dist"` (Linux/mac), and restart `local-control-plane`.
- **`bootstrap already exists` error:** If you accidentally ran `dek-cli enroll` or have leftover configs from previous runs, stop `dek-core`, delete the config folder (`C:\ProgramData\PollekDEK` on Windows or `~/.Pollek-Local Enforcement Kit` / `/etc/Pollek-Local Enforcement Kit` on Linux), and repeat Step 3.
- **`dek-cli doctor`** reports cert/connectivity/permission problems and how to fix them.
- **No decisions logged?** Confirm `dek-core` is running and `dek-cli status` shows a recently synced bundle.
- **Bundle rejected?** The pinned trust key probably doesn't match the Local CP's key — re-run step 3 with the current `public_b64`.
