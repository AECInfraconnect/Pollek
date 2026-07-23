// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! `dek-trust-gate` — the single Trust Policy Gate for Pollek.
//!
//! One choke point every bundle activation routes through. It composes the SRS
//! trust requirements into a single verdict:
//!
//! * **signature** — ed25519 over RFC-8785 canonical bytes, via the fleet's
//!   `dek-bundle-sync::keys::TrustedKeySet` (which also gives **signer allowlist**
//!   by `key_id` and **revocation** by `KeyStatus`).
//! * **signer allowlist** — explicit `key_id` allowlist on top of the keyset.
//! * **tenant/target match** — bundle tenant must equal the expected tenant.
//! * **generation monotonicity** — `bundle_revision` must be newer than the last
//!   activated one (downgrade / replay guard).
//! * **artifact integrity** — real artifact bytes must match their authenticated
//!   sha256.
//! * **provenance** — SLSA-style build provenance, level-gated.
//! * **SBOM** — CycloneDX-style component list.
//! * **test-pass attestation** — full pass + dual-control approvers.
//!
//! Any required-check failure yields `GateDecision::Quarantine`: the caller keeps
//! the previous artifact and appends `Verdict::audit_payload()` (CRITICAL) to the
//! tamper-evident audit chain. Everything verified lives *inside* the signed
//! content, so tampering breaks the signature ("runtime trusts evidence, not
//! location").
//!
//! ```
//! use dek_trust_gate::{verify, VerifyInput, TrustPolicy};
//! use std::collections::HashMap;
//! # use dek_trust_gate::SignedBundleEnvelope;
//! # fn run(envelope: &SignedBundleEnvelope, keys: &dek_bundle_sync::keys::TrustedKeySet) {
//! let policy = TrustPolicy::default();
//! let no_bytes = HashMap::new();
//! let verdict = verify(VerifyInput {
//!     envelope,
//!     policy: &policy,
//!     trusted_keys: keys,
//!     now_unix: 1_700_000_000,
//!     last_activated_revision: None,
//!     artifact_bytes: &no_bytes,
//! });
//! if verdict.accepted() { /* activate */ } else { /* quarantine + keep previous */ }
//! # }
//! ```

mod gate;
mod model;

pub use gate::{canonical_bytes, verify, VerifyInput};
pub use model::{
    CheckResult, CheckStatus, GateDecision, Material, Provenance, Sbom, SbomComponent, Signature,
    SignedBundleEnvelope, SignedContent, TestAttestation, TrustPolicy, Verdict,
};
