// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! Types for the Trust Policy Gate.
//!
//! The gate is the single activation choke point. Everything it verifies lives
//! **inside the signed content** (`SignedContent`), so tampering with any of it
//! breaks the ed25519 signature — the "runtime trusts evidence, not location"
//! principle. The detached `signatures[]` sit outside, TUF-style, matching the
//! `dek-bundle-sync::keys` verifier the fleet already uses.

use dek_bundle_format::PollekPolicyBundle;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// The `require_*` policy the gate enforces. Authored & distributed by Cloud as
/// `trust-policy.yaml`; a DEK-local copy may only make it *stricter*
/// (effective = `max(cloud, local)`), never weaker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct TrustPolicy {
    #[serde(default = "default_true")]
    pub require_signature: bool,
    #[serde(default)]
    pub require_provenance: bool,
    #[serde(default)]
    pub require_sbom: bool,
    #[serde(default)]
    pub require_test_attestation: bool,
    #[serde(default = "default_true")]
    pub require_generation_monotonicity: bool,
    /// If non-empty, the verifying signer `key_id` must be one of these.
    #[serde(default)]
    pub signer_allowlist: Vec<String>,
    /// If set, `bundle.metadata.tenant` must equal this.
    #[serde(default)]
    pub expected_tenant: Option<String>,
    /// Minimum acceptable SLSA build level for provenance (0 = any).
    #[serde(default)]
    pub min_slsa_level: u8,
    /// Dual-control: minimum distinct approver signatures required (0 = none).
    #[serde(default)]
    pub min_approvers: u8,
}

impl Default for TrustPolicy {
    /// Signature + generation-monotonicity are on by default (fail-closed baseline);
    /// the richer supply-chain checks are opt-in until Cloud emits them.
    fn default() -> Self {
        Self {
            require_signature: true,
            require_provenance: false,
            require_sbom: false,
            require_test_attestation: false,
            require_generation_monotonicity: true,
            signer_allowlist: Vec::new(),
            expected_tenant: None,
            min_slsa_level: 0,
            min_approvers: 0,
        }
    }
}

/// SLSA-style build provenance (inside the signed content).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Provenance {
    pub builder_id: String,
    pub build_type: String,
    pub source_uri: String,
    pub source_revision: String,
    /// Digest of the hermetic compiler/build image — the "compiler never holds
    /// signing keys" evidence: build identity is distinct from the release signer.
    pub compiler_digest: String,
    #[serde(default)]
    pub slsa_level: u8,
    #[serde(default)]
    pub materials: Vec<Material>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Material {
    pub uri: String,
    pub digest: String,
}

/// CycloneDX-style SBOM (inside the signed content).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sbom {
    /// "CycloneDX" | "SPDX".
    pub format: String,
    pub spec_version: String,
    pub components: Vec<SbomComponent>,
    /// sha256 hex over the canonical SBOM document (Cloud-computed).
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SbomComponent {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub purl: String,
}

/// Test-pass attestation + dual-control approvers (inside the signed content).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestAttestation {
    pub suite: String,
    pub passed: u32,
    pub total: u32,
    pub attested_at: String,
    pub attestor: String,
    /// Distinct approver identities that signed off (dual-control evidence).
    #[serde(default)]
    pub approvers: Vec<String>,
}

/// The canonical, signed payload. ed25519 signatures cover the RFC-8785
/// canonicalization (`serde_jcs`) of this struct.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignedContent {
    pub bundle: PollekPolicyBundle,
    /// Monotonic revision string, e.g. `bundle-prod-2026.04.03.0012`.
    pub bundle_revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sbom: Option<Sbom>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<TestAttestation>,
}

/// One TUF-style detached signature (matches `dek-bundle-sync` `parse_signatures`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Signature {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyid: Option<String>,
    /// base64 ed25519 signature.
    pub sig: String,
}

/// The wire envelope: signed content + detached signatures.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignedBundleEnvelope {
    pub signed: SignedContent,
    #[serde(default)]
    pub signatures: Vec<Signature>,
}

/// Terminal decision from the gate.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GateDecision {
    /// All required checks passed — safe to activate.
    Accept,
    /// A required check failed — quarantine, keep previous, raise CRITICAL audit.
    Quarantine,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Fail,
    /// Not required by the active policy (or not applicable) — recorded, not failing.
    Skipped,
}

/// Per-check result — the visible proof each gate step ran.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
}

impl CheckResult {
    pub fn pass(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Pass,
            detail: detail.into(),
        }
    }
    pub fn fail(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Fail,
            detail: detail.into(),
        }
    }
    pub fn skip(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Skipped,
            detail: detail.into(),
        }
    }
}

/// The gate's structured verdict. Pure data — the caller performs keep-previous
/// activation and appends `audit_payload()` to the tamper-evident audit chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Verdict {
    pub decision: GateDecision,
    pub bundle_id: String,
    pub tenant: String,
    pub bundle_revision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_key_id: Option<String>,
    pub checks: Vec<CheckResult>,
    /// Named failure classes for the audit event, e.g. `signature_failure`,
    /// `activation_failure`, `revision_mismatch` (SRS §26 taxonomy).
    #[serde(default)]
    pub failure_classes: Vec<String>,
    pub evaluated_at_unix: i64,
}

impl Verdict {
    pub fn accepted(&self) -> bool {
        self.decision == GateDecision::Accept
    }

    /// Canonical JSON payload for the tamper-evident audit chain. Severity is
    /// CRITICAL on quarantine, INFO on accept.
    pub fn audit_payload(&self) -> String {
        let severity = if self.accepted() { "info" } else { "critical" };
        let value = serde_json::json!({
            "event": "trust_gate_verdict",
            "severity": severity,
            "decision": self.decision,
            "bundle_id": self.bundle_id,
            "tenant": self.tenant,
            "bundle_revision": self.bundle_revision,
            "signer_key_id": self.signer_key_id,
            "failure_classes": self.failure_classes,
            "checks": self.checks,
            "evaluated_at_unix": self.evaluated_at_unix,
        });
        // Canonical form keeps the audit hash chain stable across serializations.
        serde_jcs::to_string(&value).unwrap_or_else(|_| value.to_string())
    }
}
