# DEK/LCP → Pollek Cloud — response to hand-off 2026-07-24

**From:** DEK/LCP team. **Re:** `HANDOFF_FOR_DEK_20260724.md` (your three asks).
**We work only in `AECInfraconnect/Pollek` and do not modify the Pollek-Cloud repo.**

## Ask #1 — Ratify SPIRE topology ADR 0001 → **ratified (with a drift flag)**

Ratified on the DEK side in `docs/adr/ADR-0002-spire-topology-and-tenant-binding.md`:
trust domain `pollek.io`, SAN scheme
`spiffe://pollek.io/tenant/<tenant_id>/device/<device_id>` (agents
`.../agent/<agent_id>`), mTLS gated until the bundle + ingress matrix passes.

**Blocker to close on your side:** we surveyed Pollek-Cloud `main` (`ca2e015`)
and **`docs/adr/0001` is not there** — nor is
`docs/HANDOFF_TO_DEK_AND_CODEX_2026-07-24.md` or the trust-spine additions the
hand-off describes as "origin/main". Only `docs/architecture/SECURE_CONTROL_CHANNEL.md`
is pushed. Please **push ADR 0001** so we can confirm our ratification matches it
byte-for-byte; if it differs from the scheme above, tell us and we revise ADR-0002.

## Ask #2 — Confirm the LCP token can carry a `tenant_id` claim → **confirmed**

Yes. The `pollek-local-control-plane` client-credentials token **can** carry
`tenant_id`; we request it via a Keycloak client-scope / protocol-mapper
(`POLLEK_OIDC_SCOPE`). When your JWT enforcement is on and the mapper is live,
the bearer's `tenant_id` will equal the request tenant (`POLLEK_TENANT_ID`).

Until the mapper is enabled, the **agreed alternative tenant binding** is the
SPIFFE SAN `tenant/<id>` segment + the `x-pollek-tenant-id` header. The DEK
**fails closed**: it aborts sync before any request leaves the device if the
SVID's tenant segment ≠ request tenant, so enforcement will never reject *valid*
LCP traffic and never see an unprovable tenant.

**What we need from you:** confirm the exact mapper/claim name is `tenant_id`
(not `tenant`/`tid`) and whether you read it from the access token or an
introspection response, so both sides agree on one key.

## Ask #3 — Present the verified SPIFFE ID at ingress → **done**

The DEK now sends `x-pollek-spiffe-id: spiffe://pollek.io/tenant/<t>/device/<d>`
on **every** Cloud request (value from the X.509-SVID URI SAN; omitted only in
bearer/dev mode, never fabricated). Enforce `tenant/<id> == request tenant` from
this header, or from Envoy XFCC `URI=` once mTLS is on — both carry the same ID.

**What we need from you:** confirm your trusted ingress strips any
client-supplied `x-pollek-spiffe-id` and re-injects it only from the verified
mTLS client cert (so the header cannot be spoofed before mTLS is enforced), and
tell us the header name if it is not exactly `x-pollek-spiffe-id`.

## Status of our runbook vs your corrections (acknowledged)

- Persistence is Postgres + forced RLS (not dev JSON) — noted; our sync client is
  storage-agnostic and unaffected.
- `/enroll` echoes `lcp_id` and returns real SPIRE bootstrap with
  `trust_bundle_status=pending_spire_provisioning` — our client already treats a
  null SPIRE address / pending bundle as "mTLS not yet available" and stays on
  bearer transport (no fabricated identity).
- Contract `2026.07.23` (adds `GET /v1/trust/spiffe-bundle`) is additive; our
  `2026.07.13` client keeps working and we will pin the bundle from that endpoint
  once SPIRE is provisioned.
- Keycloak RS256/JWKS verification exists but is OFF — we are ready for it via
  the `tenant_id` binding above; flip it on when you confirm the claim name.

## Verified on our side

- `cloud_sync_client` unit tests: SPIFFE tenant parsing, fail-closed tenant
  binding, and `x-pollek-spiffe-id` presence/omission.
- End-to-end against a real X.509-SVID: `GET /v1/tenants/local/identity` returns
  the parsed SPIFFE ID and a `tenant_binding` verdict; the **Workload Identity**
  page renders the *Tenant binding* card (`consistent`).
