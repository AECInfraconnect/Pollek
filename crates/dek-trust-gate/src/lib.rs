// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! `dek-trust-gate` — the single Trust Policy Gate for Pollek, aligned to the
//! real Cloud wire contract **`bundle-manifest.v2`**.
//!
//! One choke point every bundle activation routes through. It verifies a policy
//! bundle manifest exactly as Pollek Cloud signs it
//! (AECInfraconnect/Pollek-Cloud `apps/api/server.mjs`):
//!
//! * **signature** — Ed25519 (base64url) over the canonical (`stable_json`)
//!   bytes of the manifest **minus** the Cloud-added fields, verified against the
//!   DEK's pinned trusted signer keys; the declared `payload_hash` must match.
//! * **signer allowlist** — the verifying `key_id` must be allowlisted.
//! * **tenant/target match** — `manifest.tenant_id` must equal the expected tenant.
//! * **generation monotonicity** — `manifest.revision` must be newer than the
//!   last activated one.
//! * **status** — a `revoked` bundle is rejected.
//! * **artifact integrity** — real artifact bytes must match their authenticated
//!   `sha256` (by `name`).
//! * **provenance / SBOM / attestation** — optional supply-chain extensions,
//!   enforced only when the policy requires them and Cloud emits them in the
//!   signed manifest.
//!
//! Any required-check failure yields `GateDecision::Quarantine`: the caller keeps
//! the previous artifact and appends `Verdict::audit_payload()` (CRITICAL) to the
//! tamper-evident audit chain. Everything verified lives *inside the signed
//! manifest*, so tampering breaks the signature.

mod gate;
mod model;

pub use gate::{stable_json, unsigned_payload, verify, VerifyInput};
pub use model::{
    CheckResult, CheckStatus, GateDecision, ManifestSignature, TrustPolicy, TrustedSigner, Verdict,
};
