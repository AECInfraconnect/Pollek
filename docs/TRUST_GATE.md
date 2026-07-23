# Trust Policy Gate (`dek-trust-gate`)

The single activation choke point every Pollek bundle must pass before it can
take effect. It is the runtime half of the founding security principle:

> **Runtime trusts evidence, not location.** A bundle activates only after the
> full gate passes, regardless of whether it came from SaaS, a Relay, a local
> registry, `file://`, or an air-gapped import. The registry is storage, not the
> root of trust.

This document is the contract the Pollek Cloud team codes against when emitting
signed bundles (provenance, SBOM, attestation, signatures, revocation).

## Where it runs

- Crate: `crates/dek-trust-gate` — a pure library (`verify()` does no I/O).
- LCP surface: `crates/local-control-plane/src/trust_api.rs`
  - `POST /v1/tenants/:tenant/trust/verify` — submit a signed envelope (+ optional
    artifact bytes); the gate runs, the verdict is persisted, a tamper-evident
    audit entry is appended, and on `accept` the activated revision advances.
  - `GET  /v1/tenants/:tenant/trust` — the effective policy, key-provisioning
    status, and the latest verdict per bundle.
- Dashboard: **Trust & Provenance** page (`/trust-provenance`) renders, per
  bundle, exactly which checks passed or failed.

## What the gate verifies

Every field it checks lives **inside the signed content**, so tampering with any
of it breaks the ed25519 signature.

| Check | Source of truth | Fail class |
|---|---|---|
| signature | `dek-bundle-sync::keys::TrustedKeySet` (ed25519 over RFC-8785 canonical bytes) | `signature_failure` |
| signer allowlist | `key_id` ∈ policy allowlist | `signer_not_allowlisted` |
| revocation | `KeyStatus::Revoked` in the trusted key set | `signature_failure` |
| tenant / target match | `bundle.metadata.tenant` == expected tenant | `tenant_mismatch` |
| generation monotonicity | `bundle_revision` newer than last activated | `revision_mismatch` |
| artifact integrity | real bytes vs authenticated `sha256` | `activation_failure` |
| provenance | SLSA-style builder + compiler digest, level-gated | `provenance_*` |
| SBOM | CycloneDX component list + digest | `sbom_*` |
| test attestation | full pass + dual-control approvers | `attestation_*` / `insufficient_approvers` |

Any required-check failure yields `Quarantine`: the caller keeps the previous
artifact and appends a CRITICAL entry to the tamper-evident audit chain
(`dek-secure-spool::audit`). On success the verdict is INFO.

## Wire shape (what Cloud emits)

The envelope is a `signed` payload plus detached TUF-style signatures. The
`signed` bytes are canonicalized with RFC 8785 (`serde_jcs`), so DEK verification
is byte-identical to Cloud signing.

```json
{
  "signed": {
    "bundle": { "...": "PollekPolicyBundle (metadata.tenant, artifacts[].sha256, ...)" },
    "bundle_revision": "bundle-prod-2026.04.03.0012",
    "provenance": {
      "builder_id": "https://cloud.pollek.io/builder@v2",
      "build_type": "hermetic-wasm",
      "source_uri": "git+https://github.com/AECInfraconnect/pollek-policies",
      "source_revision": "abc123",
      "compiler_digest": "sha256:...",
      "slsa_level": 3,
      "materials": []
    },
    "sbom": {
      "format": "CycloneDX",
      "spec_version": "1.5",
      "components": [{ "name": "cedar-policy", "version": "4.0.0", "purl": "pkg:cargo/cedar-policy@4.0.0" }],
      "digest": "sha256:..."
    },
    "attestation": {
      "suite": "policy-conformance",
      "passed": 128,
      "total": 128,
      "attested_at": "2026-04-03T11:00:00Z",
      "attestor": "ci-runner",
      "approvers": ["alice", "bob"]
    }
  },
  "signatures": [{ "keyid": "release-key", "sig": "<base64 ed25519>" }]
}
```

`provenance`, `sbom`, and `attestation` are optional on the wire and only
required when the active `trust-policy.yaml` sets the matching `require_*` flag.

## Trust policy

Authored and distributed by Cloud as `trust-policy.yaml`; a DEK-local copy may
only make it **stricter** (effective = `max(cloud, local)`), never weaker. The
fail-closed baseline (used when no policy is present) requires signature and
generation monotonicity.

```json
{
  "require_signature": true,
  "require_provenance": false,
  "require_sbom": false,
  "require_test_attestation": false,
  "require_generation_monotonicity": true,
  "signer_allowlist": [],
  "expected_tenant": null,
  "min_slsa_level": 0,
  "min_approvers": 0
}
```

## Local trust material

Under `$DEK_LCP_DATA/trust/`:

- `trusted-keys.json` — a `dek-bundle-sync` `TrustedKeySet` (ed25519, rotation +
  revocation). If absent, the gate fails closed with `NoUsableKeys`.
- `trust-policy.json` — the local `TrustPolicy` (defaults to the fail-closed
  baseline).
- `verdicts.json` — latest verdict per bundle (read by `GET .../trust`).
- `activated.json` — last activated revision per bundle (the monotonicity guard).
- `audit.log` — hash-linked verdict chain (`GENESIS` → entry → entry).

## Cloud responsibilities (Phase A dependency)

To light up the richer checks, Cloud emits, alongside each bundle publication:
SLSA-style provenance, a CycloneDX SBOM, a test-pass attestation with approver
signatures, the detached ed25519 `signatures[]`, a signer allowlist, and a
revocation list — plus the `trust-policy.yaml` that turns the `require_*` flags
on. The bundle must be signed **including `data.json`**, not just `policy.wasm`.
