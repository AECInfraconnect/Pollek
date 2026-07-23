// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect
//
// Verifies the gate against a GROUND-TRUTH Cloud-signed bundle-manifest.v2
// (tests/fixtures/cloud_signed_manifest.json), produced by the exact Cloud
// signing algorithm (tests/fixtures/gen_cloud_manifest.mjs mirrors
// Pollek-Cloud apps/api/server.mjs: stableJson + Ed25519 base64url). This proves
// the Rust verifier interops byte-for-byte with real Cloud signing, then proves
// each SRS §26 tamper vector is quarantined.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use dek_trust_gate::{verify, CheckStatus, GateDecision, TrustPolicy, TrustedSigner, VerifyInput};
use serde_json::Value;
use std::collections::HashMap;

const NOW: i64 = 1_800_000_000;

fn fixture() -> Value {
    let raw = include_str!("fixtures/cloud_signed_manifest.json");
    serde_json::from_str(raw).expect("fixture parses")
}

/// The DEK pins the Cloud bundle-signing key (here read from the fixture's
/// embedded SPKI PEM — in production it is pinned at enrollment).
fn pinned_signers(manifest: &Value) -> Vec<TrustedSigner> {
    let sig = &manifest["signatures"][0];
    let key_id = sig["key_id"].as_str().unwrap();
    let pem = sig["public_key_pem"].as_str().unwrap();
    vec![TrustedSigner::from_pem(key_id, pem).expect("pins the Cloud signer")]
}

fn check<'a>(v: &'a dek_trust_gate::Verdict, name: &str) -> &'a dek_trust_gate::CheckResult {
    v.checks
        .iter()
        .find(|c| c.name == name)
        .expect("check present")
}

#[test]
fn real_cloud_signed_manifest_is_accepted() {
    let m = fixture();
    let signers = pinned_signers(&m);
    let policy = TrustPolicy {
        expected_tenant: Some("local".into()),
        ..TrustPolicy::default()
    };
    // Provide the real artifact bytes (the fixture's sha256 is over "WASM-POLICY-BYTES").
    let mut bytes = HashMap::new();
    bytes.insert("policy.wasm".to_string(), b"WASM-POLICY-BYTES".to_vec());

    let v = verify(VerifyInput {
        manifest: &m,
        policy: &policy,
        trusted_signers: &signers,
        now_unix: NOW,
        last_activated_revision: Some("2026.07.22.999"),
        artifact_bytes: &bytes,
    });

    assert_eq!(v.decision, GateDecision::Accept, "checks: {:?}", v.checks);
    assert_eq!(v.signer_key_id.as_deref(), Some("local-dev-ed25519"));
    assert_eq!(v.bundle_id, "bnd_eu_ai_act_high_risk_ab12cd34");
    assert_eq!(v.revision, "2026.07.23.001");
    assert_eq!(check(&v, "signature").status, CheckStatus::Pass);
    assert_eq!(check(&v, "artifact_integrity").status, CheckStatus::Pass);
    assert!(v.failure_classes.is_empty());
}

#[test]
fn tampered_manifest_field_breaks_signature() {
    let mut m = fixture();
    let signers = pinned_signers(&m);
    // Flip a signed field after signing.
    m["tenant_id"] = Value::String("tenant-evil".into());
    let v = verify(VerifyInput {
        manifest: &m,
        policy: &TrustPolicy::default(),
        trusted_signers: &signers,
        now_unix: NOW,
        last_activated_revision: None,
        artifact_bytes: &HashMap::new(),
    });
    assert_eq!(v.decision, GateDecision::Quarantine);
    assert_eq!(check(&v, "signature").status, CheckStatus::Fail);
    assert!(v.failure_classes.contains(&"signature_failure".to_string()));
}

#[test]
fn untrusted_signer_is_rejected() {
    let m = fixture();
    // A different (untrusted) key.
    let other_pem = {
        // Deterministic throwaway ed25519 SPKI PEM via a fixed 32-byte key.
        use base64::Engine;
        let mut der = vec![
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
        ];
        der.extend_from_slice(&[9u8; 32]);
        let b64 = base64::engine::general_purpose::STANDARD.encode(&der);
        format!("-----BEGIN PUBLIC KEY-----\n{b64}\n-----END PUBLIC KEY-----\n")
    };
    let signers = vec![TrustedSigner::from_pem("some-other-key", &other_pem).unwrap()];
    let v = verify(VerifyInput {
        manifest: &m,
        policy: &TrustPolicy::default(),
        trusted_signers: &signers,
        now_unix: NOW,
        last_activated_revision: None,
        artifact_bytes: &HashMap::new(),
    });
    assert_eq!(v.decision, GateDecision::Quarantine);
    assert!(v.failure_classes.contains(&"signature_failure".to_string()));
}

#[test]
fn wrong_tenant_is_rejected() {
    let m = fixture();
    let signers = pinned_signers(&m);
    let policy = TrustPolicy {
        expected_tenant: Some("tenant-other".into()),
        ..TrustPolicy::default()
    };
    let v = verify(VerifyInput {
        manifest: &m,
        policy: &policy,
        trusted_signers: &signers,
        now_unix: NOW,
        last_activated_revision: None,
        artifact_bytes: &HashMap::new(),
    });
    assert_eq!(v.decision, GateDecision::Quarantine);
    assert!(v.failure_classes.contains(&"tenant_mismatch".to_string()));
}

#[test]
fn downgrade_revision_is_rejected() {
    let m = fixture();
    let signers = pinned_signers(&m);
    let v = verify(VerifyInput {
        manifest: &m,
        policy: &TrustPolicy::default(),
        trusted_signers: &signers,
        now_unix: NOW,
        last_activated_revision: Some("2026.07.23.999"), // newer already active
        artifact_bytes: &HashMap::new(),
    });
    assert_eq!(v.decision, GateDecision::Quarantine);
    assert!(v.failure_classes.contains(&"revision_mismatch".to_string()));
}

#[test]
fn poisoned_artifact_bytes_are_rejected() {
    let m = fixture();
    let signers = pinned_signers(&m);
    let mut bytes = HashMap::new();
    bytes.insert("policy.wasm".to_string(), b"POISONED".to_vec());
    let v = verify(VerifyInput {
        manifest: &m,
        policy: &TrustPolicy {
            expected_tenant: Some("local".into()),
            ..TrustPolicy::default()
        },
        trusted_signers: &signers,
        now_unix: NOW,
        last_activated_revision: None,
        artifact_bytes: &bytes,
    });
    assert_eq!(v.decision, GateDecision::Quarantine);
    assert_eq!(check(&v, "artifact_integrity").status, CheckStatus::Fail);
    assert!(v
        .failure_classes
        .contains(&"activation_failure".to_string()));
}

#[test]
fn required_provenance_absent_is_rejected() {
    let m = fixture();
    let signers = pinned_signers(&m);
    let policy = TrustPolicy {
        require_provenance: true,
        ..TrustPolicy::default()
    };
    let v = verify(VerifyInput {
        manifest: &m,
        policy: &policy,
        trusted_signers: &signers,
        now_unix: NOW,
        last_activated_revision: None,
        artifact_bytes: &HashMap::new(),
    });
    assert_eq!(v.decision, GateDecision::Quarantine);
    assert!(v
        .failure_classes
        .contains(&"provenance_missing".to_string()));
    assert!(v.audit_payload().contains("\"severity\":\"critical\""));
}

/// stable_json must match the Cloud `stableJson`: sorted keys, no whitespace.
#[test]
fn stable_json_matches_cloud_canonicalization() {
    let v = serde_json::json!({ "b": 2, "a": [1, { "y": true, "x": null }], "c": "s" });
    assert_eq!(
        dek_trust_gate::stable_json(&v),
        r#"{"a":[1,{"x":null,"y":true}],"b":2,"c":"s"}"#
    );
}
