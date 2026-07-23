// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! The Trust Policy Gate — one `verify()` every activation routes through,
//! aligned to the real Cloud `bundle-manifest.v2` wire contract.
//!
//! Failure of any *required* check → `Quarantine`. Pure: no I/O, no globals.

use crate::model::*;
use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use serde_json::Value;
use sha2::Digest;
use std::collections::HashMap;

/// Fields the Cloud adds on top of the signed manifest
/// (`{ ...unsignedManifest, payload_hash, signatures, verification, signing_action }`).
/// The signed payload is the manifest with exactly these removed.
const ADDED_KEYS: &[&str] = &[
    "payload_hash",
    "signatures",
    "verification",
    "signing_action",
];

/// Canonical JSON identical to the Cloud's `stableJson` (server.mjs): recursive
/// sorted object keys, `JSON.stringify` scalars, no whitespace. Byte-for-byte
/// equal to what the Cloud signs, so verification matches across languages.
pub fn stable_json(v: &Value) -> String {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys
                .iter()
                .map(|k| {
                    let key_json = serde_json::to_string(k).unwrap_or_else(|_| "\"\"".to_string());
                    let val_json = map
                        .get(*k)
                        .map(stable_json)
                        .unwrap_or_else(|| "null".into());
                    format!("{key_json}:{val_json}")
                })
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        Value::Array(arr) => {
            let inner: Vec<String> = arr.iter().map(stable_json).collect();
            format!("[{}]", inner.join(","))
        }
        scalar => serde_json::to_string(scalar).unwrap_or_else(|_| "null".to_string()),
    }
}

/// The exact bytes the Cloud signed: the wire manifest minus the added fields,
/// canonicalized with `stable_json`.
pub fn unsigned_payload(manifest: &Value) -> String {
    match manifest {
        Value::Object(map) => {
            let mut obj = map.clone();
            for k in ADDED_KEYS {
                obj.remove(*k);
            }
            stable_json(&Value::Object(obj))
        }
        other => stable_json(other),
    }
}

fn sha256_hex(s: &str) -> String {
    hex::encode(sha2::Sha256::digest(s.as_bytes()))
}

impl TrustedSigner {
    /// Parse a signer from an SPKI PEM public key (Cloud `public_key_pem` form).
    pub fn from_pem(key_id: impl Into<String>, pem: &str) -> Option<Self> {
        let body: String = pem
            .lines()
            .filter(|l| !l.starts_with("-----"))
            .collect::<Vec<_>>()
            .join("");
        let der = base64::engine::general_purpose::STANDARD
            .decode(body.trim())
            .ok()?;
        if der.len() < 32 {
            return None;
        }
        let raw: [u8; 32] = der[der.len() - 32..].try_into().ok()?;
        Some(Self {
            key_id: key_id.into(),
            verifying_key: VerifyingKey::from_bytes(&raw).ok()?,
        })
    }

    /// Parse a signer from a base64 raw 32-byte ed25519 key (enrollment / `/v1/keys` form).
    pub fn from_base64(key_id: impl Into<String>, b64: &str) -> Option<Self> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
        let raw: [u8; 32] = bytes.as_slice().try_into().ok()?;
        Some(Self {
            key_id: key_id.into(),
            verifying_key: VerifyingKey::from_bytes(&raw).ok()?,
        })
    }
}

/// Everything the gate needs to reach a verdict.
pub struct VerifyInput<'a> {
    /// The full wire manifest (`bundle-manifest.v2`) including `signatures[]`.
    pub manifest: &'a Value,
    pub policy: &'a TrustPolicy,
    /// Trusted bundle-signing keys the DEK pins (from enrollment / rotation).
    pub trusted_signers: &'a [TrustedSigner],
    pub now_unix: i64,
    pub last_activated_revision: Option<&'a str>,
    /// Actual artifact bytes keyed by `artifact.name`. Empty ⇒ integrity skipped.
    pub artifact_bytes: &'a HashMap<String, Vec<u8>>,
}

fn str_field<'a>(m: &'a Value, k: &str) -> &'a str {
    m.get(k).and_then(|v| v.as_str()).unwrap_or("")
}

/// Run the full gate over a Cloud `bundle-manifest.v2` and return a verdict.
pub fn verify(input: VerifyInput<'_>) -> Verdict {
    let m = input.manifest;
    let mut checks: Vec<CheckResult> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    let mut signer_key_id: Option<String> = None;

    let tenant = str_field(m, "tenant_id").to_string();
    let revision = str_field(m, "revision").to_string();
    let bundle_id = str_field(m, "bundle_id").to_string();

    // ---- 1. Signature (Ed25519 over stable_json(unsigned manifest), base64url) ----
    let payload = unsigned_payload(m);
    let payload_hash = sha256_hex(&payload);
    let signatures: Vec<ManifestSignature> = m
        .get("signatures")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    if input.policy.require_signature {
        if signatures.is_empty() {
            checks.push(CheckResult::fail(
                "signature",
                "manifest carries no signatures",
            ));
            failures.push("signature_failure".into());
        } else if input.trusted_signers.is_empty() {
            checks.push(CheckResult::fail(
                "signature",
                "no trusted signer keys provisioned — fail closed",
            ));
            failures.push("signature_failure".into());
        } else {
            let mut verified: Option<String> = None;
            let mut hash_ok = true;
            'outer: for s in &signatures {
                // Signature must match the recomputed payload hash if it declares one.
                if let Some(ph) = &s.payload_hash {
                    if ph != &payload_hash {
                        hash_ok = false;
                        continue;
                    }
                }
                let Ok(sig_bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&s.sig)
                else {
                    continue;
                };
                let Ok(sig_arr): Result<[u8; 64], _> = sig_bytes.as_slice().try_into() else {
                    continue;
                };
                let signature = Signature::from_bytes(&sig_arr);
                for signer in input.trusted_signers {
                    if s.key_id.as_ref().is_some_and(|kid| kid != &signer.key_id) {
                        continue;
                    }
                    if signer
                        .verifying_key
                        .verify_strict(payload.as_bytes(), &signature)
                        .is_ok()
                    {
                        verified = Some(signer.key_id.clone());
                        break 'outer;
                    }
                }
            }
            match verified {
                Some(kid) => {
                    checks.push(CheckResult::pass(
                        "signature",
                        format!("Ed25519 verified by trusted signer '{kid}'"),
                    ));
                    signer_key_id = Some(kid);
                }
                None => {
                    let detail = if !hash_ok {
                        "signature payload_hash does not match recomputed manifest hash (tampered)"
                    } else {
                        "no signature verified against any pinned trusted signer (unsigned/forged/wrong-key)"
                    };
                    checks.push(CheckResult::fail("signature", detail));
                    failures.push("signature_failure".into());
                }
            }
        }
    } else {
        checks.push(CheckResult::skip("signature", "not required by policy"));
    }

    // ---- 2. Signer allowlist ----
    if input.policy.signer_allowlist.is_empty() {
        checks.push(CheckResult::skip(
            "signer_allowlist",
            "no allowlist configured",
        ));
    } else {
        match &signer_key_id {
            Some(kid) if input.policy.signer_allowlist.iter().any(|k| k == kid) => {
                checks.push(CheckResult::pass(
                    "signer_allowlist",
                    format!("signer '{kid}' allowlisted"),
                ));
            }
            Some(kid) => {
                checks.push(CheckResult::fail(
                    "signer_allowlist",
                    format!("signer '{kid}' not in allowlist"),
                ));
                failures.push("signer_not_allowlisted".into());
            }
            None => {
                checks.push(CheckResult::fail(
                    "signer_allowlist",
                    "no verified signer to check",
                ));
                failures.push("signer_not_allowlisted".into());
            }
        }
    }

    // ---- 3. Tenant / target match ----
    match &input.policy.expected_tenant {
        Some(expected) if expected == &tenant => {
            checks.push(CheckResult::pass(
                "tenant_match",
                format!("tenant '{expected}' matches"),
            ));
        }
        Some(expected) => {
            checks.push(CheckResult::fail(
                "tenant_match",
                format!("manifest tenant '{tenant}' != expected '{expected}'"),
            ));
            failures.push("tenant_mismatch".into());
        }
        None => checks.push(CheckResult::skip(
            "tenant_match",
            "no expected tenant configured",
        )),
    }

    // ---- 4. Generation monotonicity (revision string) ----
    if input.policy.require_generation_monotonicity {
        match input.last_activated_revision {
            None => checks.push(CheckResult::pass(
                "generation_monotonicity",
                format!("first activation at revision '{revision}'"),
            )),
            Some(last) if revision.as_str() > last => checks.push(CheckResult::pass(
                "generation_monotonicity",
                format!("revision '{revision}' > last '{last}'"),
            )),
            Some(last) => {
                checks.push(CheckResult::fail(
                    "generation_monotonicity",
                    format!(
                        "revision '{revision}' not newer than last '{last}' (downgrade/replay)"
                    ),
                ));
                failures.push("revision_mismatch".into());
            }
        }
    } else {
        checks.push(CheckResult::skip(
            "generation_monotonicity",
            "not required by policy",
        ));
    }

    // ---- 5. Status (revoked bundles rejected) ----
    match m.get("status").and_then(|v| v.as_str()) {
        Some("revoked") => {
            checks.push(CheckResult::fail("status", "bundle status is 'revoked'"));
            failures.push("revoked".into());
        }
        Some(s) => checks.push(CheckResult::pass("status", format!("status '{s}'"))),
        None => checks.push(CheckResult::skip("status", "no status field")),
    }

    // ---- 6. Artifact integrity (real bytes vs authenticated sha256, by name) ----
    let artifacts = m.get("artifacts").and_then(|v| v.as_array());
    if input.artifact_bytes.is_empty() {
        checks.push(CheckResult::skip(
            "artifact_integrity",
            "no artifact bytes provided (signature-only verification)",
        ));
    } else if let Some(arts) = artifacts {
        let mut bad: Vec<String> = Vec::new();
        for a in arts {
            let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let declared = a
                .get("sha256")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim_start_matches("sha256:");
            match input.artifact_bytes.get(name) {
                None => bad.push(format!("{name} (bytes not supplied)")),
                Some(bytes) => {
                    let got = hex::encode(sha2::Sha256::digest(bytes));
                    if got != declared {
                        bad.push(format!("{name} (sha256 {got} != declared {declared})"));
                    }
                }
            }
        }
        if bad.is_empty() {
            checks.push(CheckResult::pass(
                "artifact_integrity",
                format!(
                    "{} artifact(s) match their authenticated digest",
                    arts.len()
                ),
            ));
        } else {
            checks.push(CheckResult::fail(
                "artifact_integrity",
                format!("digest mismatch: {}", bad.join("; ")),
            ));
            failures.push("activation_failure".into());
        }
    } else {
        checks.push(CheckResult::skip(
            "artifact_integrity",
            "manifest declares no artifacts",
        ));
    }

    // ---- 7/8/9. Optional supply-chain extensions (Cloud not emitting yet) ----
    optional_extension(
        m,
        "provenance",
        input.policy.require_provenance,
        &mut checks,
        &mut failures,
    );
    optional_extension(
        m,
        "sbom",
        input.policy.require_sbom,
        &mut checks,
        &mut failures,
    );
    optional_extension(
        m,
        "attestation",
        input.policy.require_test_attestation,
        &mut checks,
        &mut failures,
    );

    let decision = if failures.is_empty() {
        GateDecision::Accept
    } else {
        GateDecision::Quarantine
    };
    Verdict {
        decision,
        bundle_id,
        tenant,
        revision,
        signer_key_id,
        checks,
        failure_classes: failures,
        evaluated_at_unix: input.now_unix,
    }
}

/// A supply-chain extension that must be present-and-non-empty in the signed
/// manifest when its `require_*` flag is on. Skipped when not required.
fn optional_extension(
    m: &Value,
    key: &str,
    required: bool,
    checks: &mut Vec<CheckResult>,
    failures: &mut Vec<String>,
) {
    if !required {
        checks.push(CheckResult::skip(key, "not required by policy"));
        return;
    }
    match m.get(key) {
        Some(v) if !v.is_null() => {
            checks.push(CheckResult::pass(key, "present in signed manifest"))
        }
        _ => {
            checks.push(CheckResult::fail(
                key,
                "required but absent from signed manifest",
            ));
            failures.push(format!("{key}_missing"));
        }
    }
}
