# DEK/LCP → Pollek Cloud — trust-anchor asks (survey 2026-07-23)

**From:** DEK/LCP team. **Basis:** read-only survey of Pollek-Cloud `main` at
`7b87c95`. **We work only in `AECInfraconnect/Pollek`; we do not modify the
Cloud repo.** This supersedes the open items in
`docs/DEK_TO_CLOUD_RESPONSE_2026-07-24.md` — most are now closed on your side.

## Already aligned — thank you, nothing needed

- **`tenant_id` claim (was ask #2):** confirmed on your side. `evaluateMtls` /
  the JWT gate enforce `verified.tenant_id` and reject with
  `jwt_missing_tenant_claim` / `svid_tenant_mismatch`. Claim name `tenant_id` is
  the contract — locked.
- **`x-pollek-spiffe-id` (was ask #3):** confirmed. You read
  `POLLEK_MTLS_IDENTITY_HEADER` (default `x-pollek-spiffe-id`) *and* Envoy XFCC
  `URI=`, parse the DEK SAN scheme, and enforce `tenant/<id> == request tenant`
  in off/monitor/enforce. The DEK now sends exactly that header (PR #100).
- **Signed trust documents + bundle endpoints:** `/v1/trust/policy`,
  `/v1/trust/signer-allowlist`, `/v1/trust/revocations`, `/v1/trust/spiffe-bundle`
  exist and are signed; `/v1/tenants/{t}/bundles/latest`,
  `/v1/tenants/{t}/devices/{d}/bundles/latest`, `/v1/policy-bundles/{id}/manifest`
  emit `bundle-manifest.v2`. This matches what the DEK trust-gate verifies.

## Ask A (blocking end-to-end verify) — pin the bundle-signing key at `/enroll`

The DEK trust-gate verifies a bundle's Ed25519 signature against a **pinned**
signer key, not the manifest's embedded PEM (SSOT: "trust evidence, not
location"). Today `/enroll` returns the **transport** anchor (`spiffe_id`,
`trust_domain`, `spire_server_*`, `spiffe_bundle_url`, `trust_bundle_pem`) but
**not the bundle-signing signer**. `/v1/trust/signer-allowlist` publishes it
(`keyid` + `public_key.raw_base64url` + `public_key.pem`), but that document is
**itself signed by the same key** — so a DEK can't verify the allowlist without
already trusting the key. That's a bootstrap gap (TOFU-over-TLS only).

**Request:** include the active bundle-signing signer in the `/enroll` response
so the DEK pins the trust root at enrollment. Minimal shape:

```json
"bundle_signing_keys": [
  { "key_id": "<keyid>", "alg": "ed25519",
    "raw_base64url": "<32-byte pubkey b64url>",
    "public_key_pem": "<SPKI PEM>", "status": "active" }
]
```

Include retired-but-in-overlap and revoked keys too (same fields you already
build in `unsignedSignerAllowlist`). With this, the DEK pins at enroll, then
verifies the signed `/v1/trust/signer-allowlist` on every refresh and rotates
automatically. Until it ships, the DEK pins via a manually configured
`cloud-signers.json` (works, but not zero-touch).

## Ask B (contract drift) — reconcile the bundle-manifest schema to v2

`packages/contracts/bundle-manifest.schema.json` still declares
`schema_version` `const: "pollek.policy.bundle-manifest.v1"` and a single
required `signature`, but `server.mjs` emits `bundle-manifest.v2` with a
`signatures[]` array and the v2 field set. The DEK built to what the server
**emits** (v2). Please update the schema file to v2 (or state which is
authoritative). Your contract-first drift gate should be flagging this.

## Decision — ratify ADR 0001, but it does not block a secured link today

`docs/adr/0001-spire-topology.md` is **Proposed**, recommending **Option 2**
(DEK SPIRE upstream, Railway nested/federated). Two things from the DEK side:

1. **Operational reality check for Option 2:** `dek-spire-node` today is a node
   agent + SVID lifecycle (join-token attest, X.509/JWT-SVID issue+renew). It is
   **not** a production SPIRE *server* root with HA + PostgreSQL datastore + CA
   rotation + DR. Making the DEK the authoritative `pollek.io` root (Option 2) is
   a real commitment we are not resourced to operate as GA infrastructure yet.
   If the goal is "ship soon without an unsupported SPIRE plugin," Option 2 needs
   a named DEK operator for that server first; otherwise Option 1's Cosmian/KMIP
   plugin review is the path. We lean toward **deferring the root decision**
   (see #2) rather than forcing either now.
2. **The link is already securable without mTLS:** Cloud enforces the Keycloak
   JWT `tenant_id` claim and reads `x-pollek-spiffe-id`; the DEK presents both
   and fails closed on tenant mismatch. So keep `POLLEK_MTLS_MODE=off` (or
   `monitor`) and run the DEK↔Cloud path on **bearer + tenant binding now**,
   and treat ADR 0001 / mTLS enforce as GA hardening once a root operator and
   key custody are named. This unblocks you without waiting on the SPIRE root.

## Net

You are not blocked on us for the machine boundary: enable JWT enforcement with
the `tenant_id` mapper whenever you like — the DEK is ready. The one change that
lets the DEK verify **real signed bundles** end-to-end zero-touch is **Ask A**
(publish the bundle-signing key at `/enroll`); **Ask B** clears the schema
drift; ADR 0001 is a GA-hardening decision, not a today blocker.
