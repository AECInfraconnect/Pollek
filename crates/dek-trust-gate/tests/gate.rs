// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect
//
// End-to-end gate tests: sign a real bundle with a real ed25519 key, prove the
// gate accepts it, then prove each tamper vector (SRS §26) is quarantined with
// the specific failed check.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use base64::Engine;
use dek_bundle_format::{
    ActivationConfig, BundleArtifact, BundleCompatibility, BundleMetadata, OsModulesConfig,
    PollekPolicyBundle,
};
use dek_bundle_sync::keys::{KeyStatus, TrustedKey, TrustedKeySet};
use dek_trust_gate::{
    canonical_bytes, verify, CheckStatus, GateDecision, Provenance, Sbom, SbomComponent, Signature,
    SignedBundleEnvelope, SignedContent, TestAttestation, TrustPolicy, VerifyInput,
};
use ed25519_dalek::{Signer, SigningKey};
use std::collections::HashMap;

const NOW: i64 = 1_700_000_000;

fn sample_bundle() -> PollekPolicyBundle {
    PollekPolicyBundle {
        api_version: "pollek.io/v1".into(),
        kind: "PolicyBundle".into(),
        metadata: BundleMetadata {
            bundle_id: "payment-guard".into(),
            tenant: "tenant-abc".into(),
            version: "1.2.0".into(),
            created_at: "2026-04-03T12:00:00Z".into(),
            created_by: "cloud-compiler".into(),
        },
        compatibility: BundleCompatibility {
            min_dek_version: "1.0.0".into(),
            required_crates: vec![],
            required_pep_types: vec!["mcp_proxy".into()],
            required_os_modules: OsModulesConfig::default(),
        },
        artifacts: vec![BundleArtifact {
            r#type: "policy.wasm".into(),
            path: "policy.wasm".into(),
            sha256: hex::encode(<sha2::Sha256 as sha2::Digest>::digest(b"WASM-BYTES")),
        }],
        activation: ActivationConfig {
            strategy: "shadow_then_enforce".into(),
            rollback_on_failure: true,
            health_check_timeout_ms: 5000,
            shadow_before_enforce_seconds: 60,
        },
    }
}

fn signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn trusted_set(key: &SigningKey, key_id: &str, status: KeyStatus) -> TrustedKeySet {
    TrustedKeySet {
        keys: vec![TrustedKey {
            key_id: key_id.into(),
            public_b64: base64::prelude::BASE64_STANDARD.encode(key.verifying_key().to_bytes()),
            status,
            not_before_unix: 0,
            not_after_unix: 0,
        }],
    }
}

/// Sign a SignedContent with `key`/`key_id`, returning the wire envelope.
fn envelope_for(signed: SignedContent, key: &SigningKey, key_id: &str) -> SignedBundleEnvelope {
    let bytes = canonical_bytes(&signed).unwrap();
    let sig = key.sign(&bytes);
    SignedBundleEnvelope {
        signed,
        signatures: vec![Signature {
            keyid: Some(key_id.into()),
            sig: base64::prelude::BASE64_STANDARD.encode(sig.to_bytes()),
        }],
    }
}

fn base_signed() -> SignedContent {
    SignedContent {
        bundle: sample_bundle(),
        bundle_revision: "bundle-prod-2026.04.03.0012".into(),
        provenance: None,
        sbom: None,
        attestation: None,
    }
}

fn check<'a>(v: &'a dek_trust_gate::Verdict, name: &str) -> &'a dek_trust_gate::CheckResult {
    v.checks
        .iter()
        .find(|c| c.name == name)
        .expect("check present")
}

#[test]
fn valid_bundle_is_accepted() {
    let key = signing_key(1);
    let keys = trusted_set(&key, "k1", KeyStatus::Active);
    let env = envelope_for(base_signed(), &key, "k1");
    let no_bytes = HashMap::new();

    let v = verify(VerifyInput {
        envelope: &env,
        policy: &TrustPolicy::default(),
        trusted_keys: &keys,
        now_unix: NOW,
        last_activated_revision: None,
        artifact_bytes: &no_bytes,
    });

    assert_eq!(v.decision, GateDecision::Accept);
    assert!(v.accepted());
    assert_eq!(v.signer_key_id.as_deref(), Some("k1"));
    assert_eq!(check(&v, "signature").status, CheckStatus::Pass);
    assert!(v.failure_classes.is_empty());
}

#[test]
fn tampered_content_breaks_signature() {
    let key = signing_key(1);
    let keys = trusted_set(&key, "k1", KeyStatus::Active);
    let mut env = envelope_for(base_signed(), &key, "k1");
    // Tamper after signing: flip the tenant inside the signed content.
    env.signed.bundle.metadata.tenant = "tenant-evil".into();
    let no_bytes = HashMap::new();

    let v = verify(VerifyInput {
        envelope: &env,
        policy: &TrustPolicy::default(),
        trusted_keys: &keys,
        now_unix: NOW,
        last_activated_revision: None,
        artifact_bytes: &no_bytes,
    });

    assert_eq!(v.decision, GateDecision::Quarantine);
    assert_eq!(check(&v, "signature").status, CheckStatus::Fail);
    assert!(v.failure_classes.contains(&"signature_failure".to_string()));
}

#[test]
fn revoked_key_is_rejected() {
    let key = signing_key(1);
    let keys = trusted_set(&key, "k1", KeyStatus::Revoked);
    let env = envelope_for(base_signed(), &key, "k1");
    let no_bytes = HashMap::new();

    let v = verify(VerifyInput {
        envelope: &env,
        policy: &TrustPolicy::default(),
        trusted_keys: &keys,
        now_unix: NOW,
        last_activated_revision: None,
        artifact_bytes: &no_bytes,
    });

    assert_eq!(v.decision, GateDecision::Quarantine);
    assert!(v.failure_classes.contains(&"signature_failure".to_string()));
}

#[test]
fn wrong_tenant_is_rejected() {
    let key = signing_key(1);
    let keys = trusted_set(&key, "k1", KeyStatus::Active);
    let env = envelope_for(base_signed(), &key, "k1");
    let no_bytes = HashMap::new();
    let policy = TrustPolicy {
        expected_tenant: Some("tenant-other".into()),
        ..TrustPolicy::default()
    };

    let v = verify(VerifyInput {
        envelope: &env,
        policy: &policy,
        trusted_keys: &keys,
        now_unix: NOW,
        last_activated_revision: None,
        artifact_bytes: &no_bytes,
    });

    assert_eq!(v.decision, GateDecision::Quarantine);
    assert_eq!(check(&v, "tenant_match").status, CheckStatus::Fail);
    assert!(v.failure_classes.contains(&"tenant_mismatch".to_string()));
}

#[test]
fn downgrade_revision_is_rejected() {
    let key = signing_key(1);
    let keys = trusted_set(&key, "k1", KeyStatus::Active);
    let env = envelope_for(base_signed(), &key, "k1");
    let no_bytes = HashMap::new();

    // Already activated a NEWER revision than the incoming one.
    let v = verify(VerifyInput {
        envelope: &env,
        policy: &TrustPolicy::default(),
        trusted_keys: &keys,
        now_unix: NOW,
        last_activated_revision: Some("bundle-prod-2026.04.03.0099"),
        artifact_bytes: &no_bytes,
    });

    assert_eq!(v.decision, GateDecision::Quarantine);
    assert_eq!(
        check(&v, "generation_monotonicity").status,
        CheckStatus::Fail
    );
    assert!(v.failure_classes.contains(&"revision_mismatch".to_string()));
}

#[test]
fn newer_revision_passes_monotonicity() {
    let key = signing_key(1);
    let keys = trusted_set(&key, "k1", KeyStatus::Active);
    let env = envelope_for(base_signed(), &key, "k1");
    let no_bytes = HashMap::new();

    let v = verify(VerifyInput {
        envelope: &env,
        policy: &TrustPolicy::default(),
        trusted_keys: &keys,
        now_unix: NOW,
        last_activated_revision: Some("bundle-prod-2026.04.03.0001"),
        artifact_bytes: &no_bytes,
    });

    assert_eq!(v.decision, GateDecision::Accept);
    assert_eq!(
        check(&v, "generation_monotonicity").status,
        CheckStatus::Pass
    );
}

#[test]
fn artifact_bytes_matching_digest_pass_and_mismatch_quarantines() {
    let key = signing_key(1);
    let keys = trusted_set(&key, "k1", KeyStatus::Active);
    let env = envelope_for(base_signed(), &key, "k1");

    // Correct bytes -> accept.
    let mut good = HashMap::new();
    good.insert("policy.wasm".to_string(), b"WASM-BYTES".to_vec());
    let v = verify(VerifyInput {
        envelope: &env,
        policy: &TrustPolicy::default(),
        trusted_keys: &keys,
        now_unix: NOW,
        last_activated_revision: None,
        artifact_bytes: &good,
    });
    assert_eq!(v.decision, GateDecision::Accept);
    assert_eq!(check(&v, "artifact_integrity").status, CheckStatus::Pass);

    // Poisoned bytes -> quarantine.
    let mut bad = HashMap::new();
    bad.insert("policy.wasm".to_string(), b"POISONED".to_vec());
    let v = verify(VerifyInput {
        envelope: &env,
        policy: &TrustPolicy::default(),
        trusted_keys: &keys,
        now_unix: NOW,
        last_activated_revision: None,
        artifact_bytes: &bad,
    });
    assert_eq!(v.decision, GateDecision::Quarantine);
    assert_eq!(check(&v, "artifact_integrity").status, CheckStatus::Fail);
    assert!(v
        .failure_classes
        .contains(&"activation_failure".to_string()));
}

#[test]
fn required_provenance_absent_is_rejected() {
    let key = signing_key(1);
    let keys = trusted_set(&key, "k1", KeyStatus::Active);
    let env = envelope_for(base_signed(), &key, "k1");
    let no_bytes = HashMap::new();
    let policy = TrustPolicy {
        require_provenance: true,
        ..TrustPolicy::default()
    };

    let v = verify(VerifyInput {
        envelope: &env,
        policy: &policy,
        trusted_keys: &keys,
        now_unix: NOW,
        last_activated_revision: None,
        artifact_bytes: &no_bytes,
    });

    assert_eq!(v.decision, GateDecision::Quarantine);
    assert_eq!(check(&v, "provenance").status, CheckStatus::Fail);
    assert!(v
        .failure_classes
        .contains(&"provenance_missing".to_string()));
}

#[test]
fn signer_not_in_allowlist_is_rejected() {
    let key = signing_key(1);
    let keys = trusted_set(&key, "k1", KeyStatus::Active);
    let env = envelope_for(base_signed(), &key, "k1");
    let no_bytes = HashMap::new();
    let policy = TrustPolicy {
        signer_allowlist: vec!["some-other-key".into()],
        ..TrustPolicy::default()
    };

    let v = verify(VerifyInput {
        envelope: &env,
        policy: &policy,
        trusted_keys: &keys,
        now_unix: NOW,
        last_activated_revision: None,
        artifact_bytes: &no_bytes,
    });

    assert_eq!(v.decision, GateDecision::Quarantine);
    assert_eq!(check(&v, "signer_allowlist").status, CheckStatus::Fail);
    assert!(v
        .failure_classes
        .contains(&"signer_not_allowlisted".to_string()));
}

#[test]
fn full_strict_policy_accepts_complete_bundle() {
    let key = signing_key(7);
    let keys = trusted_set(&key, "release-key", KeyStatus::Active);
    let signed = SignedContent {
        bundle: sample_bundle(),
        bundle_revision: "bundle-prod-2026.04.03.0012".into(),
        provenance: Some(Provenance {
            builder_id: "https://cloud.pollek.io/builder@v2".into(),
            build_type: "hermetic-wasm".into(),
            source_uri: "git+https://github.com/AECInfraconnect/pollek-policies".into(),
            source_revision: "abc123".into(),
            compiler_digest: "sha256:deadbeef".into(),
            slsa_level: 3,
            materials: vec![],
        }),
        sbom: Some(Sbom {
            format: "CycloneDX".into(),
            spec_version: "1.5".into(),
            components: vec![SbomComponent {
                name: "cedar-policy".into(),
                version: "4.0.0".into(),
                purl: "pkg:cargo/cedar-policy@4.0.0".into(),
            }],
            digest: "sha256:cafe".into(),
        }),
        attestation: Some(TestAttestation {
            suite: "policy-conformance".into(),
            passed: 128,
            total: 128,
            attested_at: "2026-04-03T11:00:00Z".into(),
            attestor: "ci-runner".into(),
            approvers: vec!["alice".into(), "bob".into()],
        }),
    };
    let env = envelope_for(signed, &key, "release-key");
    let mut bytes = HashMap::new();
    bytes.insert("policy.wasm".to_string(), b"WASM-BYTES".to_vec());

    let policy = TrustPolicy {
        require_signature: true,
        require_provenance: true,
        require_sbom: true,
        require_test_attestation: true,
        require_generation_monotonicity: true,
        signer_allowlist: vec!["release-key".into()],
        expected_tenant: Some("tenant-abc".into()),
        min_slsa_level: 3,
        min_approvers: 2,
    };

    let v = verify(VerifyInput {
        envelope: &env,
        policy: &policy,
        trusted_keys: &keys,
        now_unix: NOW,
        last_activated_revision: Some("bundle-prod-2026.04.03.0001"),
        artifact_bytes: &bytes,
    });

    assert_eq!(v.decision, GateDecision::Accept, "checks: {:?}", v.checks);
    assert!(v.checks.iter().all(|c| c.status != CheckStatus::Fail));
    // The audit payload for an accept is INFO severity.
    assert!(v.audit_payload().contains("\"severity\":\"info\""));
}

#[test]
fn insufficient_approvers_is_rejected() {
    let key = signing_key(7);
    let keys = trusted_set(&key, "release-key", KeyStatus::Active);
    let signed = SignedContent {
        bundle: sample_bundle(),
        bundle_revision: "bundle-prod-2026.04.03.0012".into(),
        provenance: None,
        sbom: None,
        attestation: Some(TestAttestation {
            suite: "policy-conformance".into(),
            passed: 10,
            total: 10,
            attested_at: "2026-04-03T11:00:00Z".into(),
            attestor: "ci-runner".into(),
            approvers: vec!["alice".into()],
        }),
    };
    let env = envelope_for(signed, &key, "release-key");
    let no_bytes = HashMap::new();
    let policy = TrustPolicy {
        require_test_attestation: true,
        min_approvers: 2,
        ..TrustPolicy::default()
    };

    let v = verify(VerifyInput {
        envelope: &env,
        policy: &policy,
        trusted_keys: &keys,
        now_unix: NOW,
        last_activated_revision: None,
        artifact_bytes: &no_bytes,
    });

    assert_eq!(v.decision, GateDecision::Quarantine);
    assert!(v
        .failure_classes
        .contains(&"insufficient_approvers".to_string()));
    assert!(v.audit_payload().contains("\"severity\":\"critical\""));
}
