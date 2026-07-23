// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! The Trust Policy Gate — one `verify()` every activation path routes through.
//!
//! Failure of any *required* check → `Quarantine` (keep previous, raise CRITICAL
//! audit). The function is pure: no I/O, no globals; the caller wires activation
//! and audit. Signature + signer-allowlist + revocation come from the existing
//! `dek-bundle-sync::keys::TrustedKeySet`; provenance / SBOM / attestation /
//! generation-monotonicity / tenant-match are added here so a single gate
//! composes all of the SRS trust requirements.

use crate::model::*;
use dek_bundle_sync::keys::{SignatureEntry, TrustedKeySet, VerifyOutcome};
use sha2::Digest;
use std::collections::HashMap;

/// RFC-8785 canonical bytes of the signed content — the exact bytes ed25519
/// signs and verifies. Matches `serde_jcs::to_vec(&signed)` used across
/// `dek-bundle-sync`, so DEK verification is byte-identical to Cloud signing.
pub fn canonical_bytes(signed: &SignedContent) -> Result<Vec<u8>, serde_json::Error> {
    serde_jcs::to_vec(signed)
}

/// Everything the gate needs to reach a verdict. Borrowed — no ownership taken.
pub struct VerifyInput<'a> {
    pub envelope: &'a SignedBundleEnvelope,
    pub policy: &'a TrustPolicy,
    pub trusted_keys: &'a TrustedKeySet,
    pub now_unix: i64,
    /// Last activated revision for this `bundle_id` (monotonicity). `None` = first
    /// activation for this bundle.
    pub last_activated_revision: Option<&'a str>,
    /// Actual artifact bytes keyed by `BundleArtifact.path`. When empty, artifact
    /// integrity is `Skipped` (signature-only mode); when present, every declared
    /// artifact must be supplied and match its authenticated sha256.
    pub artifact_bytes: &'a HashMap<String, Vec<u8>>,
}

/// Run the full gate and return a structured verdict.
pub fn verify(input: VerifyInput<'_>) -> Verdict {
    let signed = &input.envelope.signed;
    let bundle = &signed.bundle;
    let mut checks: Vec<CheckResult> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    let mut signer_key_id: Option<String> = None;

    // ---- 1. Signature (also covers signer-allowlist + revocation via keyset) ----
    if input.policy.require_signature {
        match canonical_bytes(signed) {
            Ok(bytes) => {
                let sigs: Vec<SignatureEntry> = input
                    .envelope
                    .signatures
                    .iter()
                    .map(|s| SignatureEntry {
                        key_id: s.keyid.clone(),
                        sig_b64: s.sig.clone(),
                    })
                    .collect();
                match input.trusted_keys.verify(input.now_unix, &bytes, &sigs) {
                    VerifyOutcome::Valid { key_id } => {
                        checks.push(CheckResult::pass(
                            "signature",
                            format!("verified by trusted key '{key_id}'"),
                        ));
                        signer_key_id = Some(key_id);
                    }
                    VerifyOutcome::NoValidSignature => {
                        checks.push(CheckResult::fail(
                            "signature",
                            "no signature verified against any usable trusted key (unsigned/forged/revoked-key)",
                        ));
                        failures.push("signature_failure".into());
                    }
                    VerifyOutcome::NoUsableKeys => {
                        checks.push(CheckResult::fail(
                            "signature",
                            "no usable trusted keys (all revoked or misconfigured) — fail closed",
                        ));
                        failures.push("signature_failure".into());
                    }
                }
            }
            Err(e) => {
                checks.push(CheckResult::fail(
                    "signature",
                    format!("could not canonicalize signed content: {e}"),
                ));
                failures.push("signature_failure".into());
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
                    format!("signer '{kid}' is allowlisted"),
                ));
            }
            Some(kid) => {
                checks.push(CheckResult::fail(
                    "signer_allowlist",
                    format!("signer '{kid}' is not in the allowlist"),
                ));
                failures.push("signer_not_allowlisted".into());
            }
            None => {
                checks.push(CheckResult::fail(
                    "signer_allowlist",
                    "no verified signer to check against the allowlist",
                ));
                failures.push("signer_not_allowlisted".into());
            }
        }
    }

    // ---- 3. Tenant / target match ----
    match &input.policy.expected_tenant {
        Some(expected) if expected == &bundle.metadata.tenant => {
            checks.push(CheckResult::pass(
                "tenant_match",
                format!("bundle tenant '{expected}' matches expected"),
            ));
        }
        Some(expected) => {
            checks.push(CheckResult::fail(
                "tenant_match",
                format!(
                    "bundle tenant '{}' != expected '{}'",
                    bundle.metadata.tenant, expected
                ),
            ));
            failures.push("tenant_mismatch".into());
        }
        None => {
            checks.push(CheckResult::skip(
                "tenant_match",
                "no expected tenant configured",
            ));
        }
    }

    // ---- 4. Generation monotonicity (downgrade / replay guard) ----
    if input.policy.require_generation_monotonicity {
        match input.last_activated_revision {
            None => checks.push(CheckResult::pass(
                "generation_monotonicity",
                format!("first activation at revision '{}'", signed.bundle_revision),
            )),
            Some(last) if signed.bundle_revision.as_str() > last => {
                checks.push(CheckResult::pass(
                    "generation_monotonicity",
                    format!("revision '{}' > last '{}'", signed.bundle_revision, last),
                ));
            }
            Some(last) => {
                checks.push(CheckResult::fail(
                    "generation_monotonicity",
                    format!(
                        "revision '{}' is not newer than last activated '{}' (downgrade/replay)",
                        signed.bundle_revision, last
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

    // ---- 5. Artifact integrity (real bytes vs authenticated sha256) ----
    if input.artifact_bytes.is_empty() {
        checks.push(CheckResult::skip(
            "artifact_integrity",
            "no artifact bytes provided (signature-only verification)",
        ));
    } else {
        let mut bad: Vec<String> = Vec::new();
        for artifact in &bundle.artifacts {
            match input.artifact_bytes.get(&artifact.path) {
                None => bad.push(format!("{} (bytes not supplied)", artifact.path)),
                Some(bytes) => {
                    let digest = hex::encode(sha2::Sha256::digest(bytes));
                    let expected = artifact.sha256.trim_start_matches("sha256:");
                    if digest != expected {
                        bad.push(format!(
                            "{} (sha256 {digest} != declared {expected})",
                            artifact.path
                        ));
                    }
                }
            }
        }
        if bad.is_empty() {
            checks.push(CheckResult::pass(
                "artifact_integrity",
                format!(
                    "{} artifact(s) match their authenticated digest",
                    bundle.artifacts.len()
                ),
            ));
        } else {
            checks.push(CheckResult::fail(
                "artifact_integrity",
                format!("artifact digest mismatch: {}", bad.join("; ")),
            ));
            failures.push("activation_failure".into());
        }
    }

    // ---- 6. Provenance ----
    if input.policy.require_provenance {
        match &signed.provenance {
            None => {
                checks.push(CheckResult::fail("provenance", "required but absent"));
                failures.push("provenance_missing".into());
            }
            Some(p) if p.builder_id.is_empty() || p.compiler_digest.is_empty() => {
                checks.push(CheckResult::fail(
                    "provenance",
                    "present but missing builder_id or compiler_digest",
                ));
                failures.push("provenance_incomplete".into());
            }
            Some(p) if p.slsa_level < input.policy.min_slsa_level => {
                checks.push(CheckResult::fail(
                    "provenance",
                    format!(
                        "SLSA level {} below required {}",
                        p.slsa_level, input.policy.min_slsa_level
                    ),
                ));
                failures.push("provenance_insufficient".into());
            }
            Some(p) => {
                checks.push(CheckResult::pass(
                    "provenance",
                    format!(
                        "builder '{}', SLSA L{}, source {}@{}",
                        p.builder_id, p.slsa_level, p.source_uri, p.source_revision
                    ),
                ));
            }
        }
    } else {
        checks.push(CheckResult::skip("provenance", "not required by policy"));
    }

    // ---- 7. SBOM ----
    if input.policy.require_sbom {
        match &signed.sbom {
            None => {
                checks.push(CheckResult::fail("sbom", "required but absent"));
                failures.push("sbom_missing".into());
            }
            Some(s) if s.components.is_empty() || s.digest.is_empty() => {
                checks.push(CheckResult::fail(
                    "sbom",
                    "present but empty components or missing digest",
                ));
                failures.push("sbom_incomplete".into());
            }
            Some(s) => {
                checks.push(CheckResult::pass(
                    "sbom",
                    format!("{} ({} components)", s.format, s.components.len()),
                ));
            }
        }
    } else {
        checks.push(CheckResult::skip("sbom", "not required by policy"));
    }

    // ---- 8. Test-pass attestation (+ dual-control approvers) ----
    if input.policy.require_test_attestation {
        match &signed.attestation {
            None => {
                checks.push(CheckResult::fail("test_attestation", "required but absent"));
                failures.push("attestation_missing".into());
            }
            Some(a) if a.total == 0 || a.passed != a.total => {
                checks.push(CheckResult::fail(
                    "test_attestation",
                    format!(
                        "suite '{}' not fully passing ({}/{})",
                        a.suite, a.passed, a.total
                    ),
                ));
                failures.push("attestation_failed".into());
            }
            Some(a) => {
                let distinct: std::collections::BTreeSet<&String> = a.approvers.iter().collect();
                if (distinct.len() as u8) < input.policy.min_approvers {
                    checks.push(CheckResult::fail(
                        "test_attestation",
                        format!(
                            "tests pass but {} distinct approver(s) < required {}",
                            distinct.len(),
                            input.policy.min_approvers
                        ),
                    ));
                    failures.push("insufficient_approvers".into());
                } else {
                    checks.push(CheckResult::pass(
                        "test_attestation",
                        format!(
                            "suite '{}' {}/{} passing, {} approver(s)",
                            a.suite,
                            a.passed,
                            a.total,
                            distinct.len()
                        ),
                    ));
                }
            }
        }
    } else {
        checks.push(CheckResult::skip(
            "test_attestation",
            "not required by policy",
        ));
    }

    let decision = if failures.is_empty() {
        GateDecision::Accept
    } else {
        GateDecision::Quarantine
    };

    Verdict {
        decision,
        bundle_id: bundle.metadata.bundle_id.clone(),
        tenant: bundle.metadata.tenant.clone(),
        bundle_revision: signed.bundle_revision.clone(),
        signer_key_id,
        checks,
        failure_classes: failures,
        evaluated_at_unix: input.now_unix,
    }
}
