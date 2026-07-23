// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! Types for the Trust Policy Gate, aligned to the **real** Pollek Cloud wire
//! contract `bundle-manifest.v2` (AECInfraconnect/Pollek-Cloud
//! `apps/api/server.mjs`): a policy-bundle manifest carrying a `signatures[]`
//! array, each an Ed25519 signature over the canonical (`stableJson`) bytes of
//! the manifest **minus** the Cloud-added fields (`payload_hash`, `signatures`,
//! `verification`, `signing_action`).
//!
//! The gate consumes the raw manifest as a `serde_json::Value` so it never drifts
//! when Cloud adds manifest fields: the signed payload is reconstructed by
//! *removing* the added keys, not by re-modelling every field.

use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// The `require_*` policy the gate enforces. Cloud-authored & distributed as
/// `trust-policy.yaml`; a DEK-local copy may only make it *stricter*.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct TrustPolicy {
    #[serde(default = "default_true")]
    pub require_signature: bool,
    #[serde(default = "default_true")]
    pub require_generation_monotonicity: bool,
    /// Optional supply-chain extensions — Cloud does not emit these in
    /// `bundle-manifest.v2` yet (tracked in the Cloud hand-off); when it does,
    /// flip these on and the gate enforces their presence inside the signed
    /// manifest.
    #[serde(default)]
    pub require_provenance: bool,
    #[serde(default)]
    pub require_sbom: bool,
    #[serde(default)]
    pub require_test_attestation: bool,
    /// If non-empty, the verifying signer `key_id` must be one of these.
    #[serde(default)]
    pub signer_allowlist: Vec<String>,
    /// If set, `manifest.tenant_id` must equal this.
    #[serde(default)]
    pub expected_tenant: Option<String>,
    #[serde(default)]
    pub min_slsa_level: u8,
    #[serde(default)]
    pub min_approvers: u8,
}

impl Default for TrustPolicy {
    /// Fail-closed baseline: signature + generation monotonicity required.
    fn default() -> Self {
        Self {
            require_signature: true,
            require_generation_monotonicity: true,
            require_provenance: false,
            require_sbom: false,
            require_test_attestation: false,
            signer_allowlist: Vec::new(),
            expected_tenant: None,
            min_slsa_level: 0,
            min_approvers: 0,
        }
    }
}

/// One entry of the manifest's `signatures[]` array (Cloud `bundle-manifest.v2`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestSignature {
    #[serde(default)]
    pub key_id: Option<String>,
    #[serde(default)]
    pub alg: Option<String>,
    /// base64url Ed25519 signature over the canonical unsigned-manifest bytes.
    pub sig: String,
    /// sha256 (hex) of the canonical unsigned-manifest payload.
    #[serde(default)]
    pub payload_hash: Option<String>,
    /// SPKI PEM of the signer (informational; the DEK verifies against its
    /// pinned trust anchor, not this embedded key).
    #[serde(default)]
    pub public_key_pem: Option<String>,
}

/// A trusted bundle-signing key the DEK pins (from enrollment / `/v1/keys`).
#[derive(Debug, Clone)]
pub struct TrustedSigner {
    pub key_id: String,
    pub verifying_key: VerifyingKey,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GateDecision {
    Accept,
    Quarantine,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Fail,
    Skipped,
}

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
    pub revision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_key_id: Option<String>,
    pub checks: Vec<CheckResult>,
    #[serde(default)]
    pub failure_classes: Vec<String>,
    pub evaluated_at_unix: i64,
}

impl Verdict {
    pub fn accepted(&self) -> bool {
        self.decision == GateDecision::Accept
    }

    /// Canonical JSON payload for the tamper-evident audit chain. CRITICAL on
    /// quarantine, INFO on accept.
    pub fn audit_payload(&self) -> String {
        let severity = if self.accepted() { "info" } else { "critical" };
        let value = serde_json::json!({
            "event": "trust_gate_verdict",
            "severity": severity,
            "decision": self.decision,
            "bundle_id": self.bundle_id,
            "tenant": self.tenant,
            "revision": self.revision,
            "signer_key_id": self.signer_key_id,
            "failure_classes": self.failure_classes,
            "checks": self.checks,
            "evaluated_at_unix": self.evaluated_at_unix,
        });
        value.to_string()
    }
}
