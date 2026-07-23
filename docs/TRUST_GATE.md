# Trust Policy Gate (`dek-trust-gate`)

The single activation choke point every Pollek policy bundle must pass before it
can take effect. It is the runtime half of the founding security principle:

> **Runtime trusts evidence, not location.** A bundle activates only after the
> full gate passes, regardless of where it came from. The registry is storage,
> not the root of trust.

The gate is aligned to the **real Pollek Cloud wire contract `bundle-manifest.v2`**
(AECInfraconnect/Pollek-Cloud `apps/api/server.mjs`), so it verifies bundles
exactly as Cloud signs them.

## Where it runs

- Crate: `crates/dek-trust-gate` — a pure library (`verify()` does no I/O).
- LCP surface: `crates/local-control-plane/src/trust_api.rs`
  - `POST /v1/tenants/:tenant/trust/verify` — submit a `bundle-manifest.v2`
    (+ optional artifact bytes by name); the gate runs, the verdict is persisted,
    a tamper-evident audit entry is appended, and on `accept` the activated
    revision advances.
  - `GET  /v1/tenants/:tenant/trust` — the effective policy, trusted-signer
    status, and the latest verdict per bundle.
- Dashboard: **Trust & Provenance** page (`/trust-provenance`).

## The manifest and its signature

Cloud emits a `bundle-manifest.v2` (a JSON object) with a `signatures[]` array.
The **signed payload** is the manifest with the Cloud-added fields removed
(`payload_hash`, `signatures`, `verification`, `signing_action`), canonicalized
with `stable_json` — a recursive sorted-key, no-whitespace encoding **identical**
to the Cloud's `stableJson`. Each signature is:

```json
{
  "key_id": "local-dev-ed25519",
  "alg": "Ed25519",
  "sig": "<base64url Ed25519 over stable_json(unsigned manifest)>",
  "payload_hash": "<sha256 hex of that payload>",
  "public_key_pem": "<signer SPKI PEM (informational)>"
}
```

The gate recomputes the canonical payload, checks the declared `payload_hash`
matches, and verifies the Ed25519 signature against the DEK's **pinned** trusted
signer keys (never the embedded `public_key_pem`).

## What the gate verifies

| Check | Source of truth | Fail class |
|---|---|---|
| signature | Ed25519 (base64url) over `stable_json(unsigned manifest)`, against pinned signers; `payload_hash` must match | `signature_failure` |
| signer allowlist | verifying `key_id` ∈ policy allowlist | `signer_not_allowlisted` |
| tenant / target match | `manifest.tenant_id` == expected tenant | `tenant_mismatch` |
| generation monotonicity | `manifest.revision` newer than last activated | `revision_mismatch` |
| status (revocation) | `manifest.status` != `revoked` | `revoked` |
| artifact integrity | real bytes vs authenticated `sha256` (by `name`) | `activation_failure` |
| provenance / SBOM / attestation | optional signed-manifest extensions, level/approver-gated | `*_missing` |

Any required-check failure yields `GateDecision::Quarantine`: the caller keeps the
previous artifact and appends `Verdict::audit_payload()` (CRITICAL) to the
`dek-secure-spool::audit` hash chain.

## Trust anchor (single source of truth)

The gate verifies against the DEK's **pinned** bundle-signing keys, provided by
the caller. In the LCP those are the local control-plane signer (`state.signer`,
for Local-mode bundles) plus any Cloud signer public keys pinned at
`$DEK_LCP_DATA/trust/cloud-signers.json` (obtained at enrollment / `/v1/keys`
rotation). With no trusted signer the gate fails closed.

## Runtime state

Under `$DEK_LCP_DATA/trust/`:

- `cloud-signers.json` — pinned Cloud bundle-signing keys (`[{ key_id, public_key_pem }]`).
- `trust-policy.json` — optional operator override of the `TrustPolicy`; absent
  ⇒ the fail-closed default (signature + generation monotonicity required).
- `verdicts.json` — latest verdict per bundle (read by `GET .../trust`).
- `activated.json` — last activated revision per bundle (the monotonicity guard).
- `audit.log` — hash-linked verdict chain (`GENESIS` → entry → entry).

## Cross-repo verification

`tests/manifest_v2.rs` verifies a **ground-truth Cloud-signed manifest**
(`tests/fixtures/cloud_signed_manifest.json`) produced by
`tests/fixtures/gen_cloud_manifest.mjs`, which mirrors Pollek-Cloud
`apps/api/server.mjs` exactly (`stableJson` + Ed25519 base64url + SPKI PEM,
Node crypto builtins). This proves the Rust verifier interops byte-for-byte with
real Cloud signing; the same test proves each SRS §26 tamper vector is
quarantined.

## Cloud dependencies (hand-off)

Provenance / SBOM / test-attestation are **not yet in the signed
`bundle-manifest.v2`** — the gate treats them as optional extensions and enforces
them only when the policy requires them *and* Cloud emits them inside the signed
manifest. Adding them to the signed manifest (for poisoning resistance) is a
Cloud-side task, tracked in the DEK→Cloud hand-off, along with reconciling the
`bundle-manifest.schema.json` (`v1`) with what the server emits (`v2`).
