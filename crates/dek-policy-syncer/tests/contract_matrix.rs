//! Phase 5 — contract integration matrix (in-process, deterministic, CI-friendly).
//!
//! Exercises the policy-syncer contract end-to-end WITHOUT spawning binaries:
//! an in-process axum "mock cloud" serves a `/v1/keys` payload signed by the
//! bootstrap key, and the test drives verify (TrustedKeySet), rotation (merge),
//! the freshness state machine (evaluate_state), and the PEP gate (status file).
//!
//! Run: cargo test -p dek-policy-syncer --test contract_matrix
//!
//! The full process-level matrix (spawn mock-cloud + dek-core over mTLS) lives
//! in `crates/acceptance-tests` — see PHASE5_acceptance_matrix.md.

use dek_bundle_sync::keys::{
    parse_signatures, KeyStatus, TrustedKey, TrustedKeySet, VerifyOutcome,
};
use dek_policy_syncer::gate::{invalidate_cache, strict_deny_reason};
use dek_policy_syncer::state::{
    evaluate_state, write_status_atomic, EnforcementState, EnforcementStatus, FreshnessConfig,
};
use ed25519_dalek::{Signer, SigningKey};
use std::sync::Mutex;

static SERIAL: Mutex<()> = Mutex::new(());

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
fn b64(b: &[u8]) -> String {
    use base64::Engine;
    base64::prelude::BASE64_STANDARD.encode(b)
}
fn keypair(seed: u8) -> (SigningKey, String) {
    let sk = SigningKey::from_bytes(&[seed; 32]);
    (sk.clone(), b64(&sk.verifying_key().to_bytes()))
}
fn sign_sigs(sk: &SigningKey, kid: &str, signed: &serde_json::Value) -> serde_json::Value {
    let bytes = serde_json::to_vec(signed).unwrap();
    let sig = sk.sign(&bytes);
    serde_json::json!([{ "keyid": kid, "sig": b64(&sig.to_bytes()) }])
}

// ── Scenario 1: valid signed bundle metadata verifies; forged rejected ──────
#[test]
fn s1_verify_valid_and_reject_forged() {
    let (sk, pk) = keypair(1);
    let set = TrustedKeySet::from_single_pinned(&pk); // key_id = "bootstrap"

    let signed = serde_json::json!({ "version": 7, "expires_at": "unix:9999999999" });
    let sigs_json = sign_sigs(&sk, "bootstrap", &signed);
    let signed_bytes = serde_json::to_vec(&signed).unwrap();

    let out = set.verify(now(), &signed_bytes, &parse_signatures(&sigs_json));
    assert!(
        matches!(out, VerifyOutcome::Valid { .. }),
        "valid bundle must verify"
    );

    // forged: signed by a different key but claims keyid "bootstrap"
    let (sk_forged, _) = keypair(9);
    let forged = sign_sigs(&sk_forged, "bootstrap", &signed);
    assert_eq!(
        set.verify(now(), &signed_bytes, &parse_signatures(&forged)),
        VerifyOutcome::NoValidSignature,
        "forged signature must be rejected (unsigned-push guard)"
    );
}

// ── Scenario 2: key rotation merge + overlap + revoke ───────────────────────
#[test]
fn s2_key_rotation_overlap_then_revoke() {
    let (sk_old, pk_old) = keypair(1);
    let (sk_new, pk_new) = keypair(2);
    let mut set = TrustedKeySet::from_single_pinned(&pk_old);

    // introduce "next" key (overlap): both verify
    let delta = set.merge_rotation(vec![TrustedKey {
        key_id: "key-2".into(),
        public_b64: pk_new.clone(),
        status: KeyStatus::Next,
        not_before_unix: 0,
        not_after_unix: 0,
    }]);
    assert_eq!(delta.added, vec!["key-2".to_string()]);

    let signed = serde_json::json!({ "v": 1 });
    let sb = serde_json::to_vec(&signed).unwrap();
    assert!(matches!(
        set.verify(
            now(),
            &sb,
            &parse_signatures(&sign_sigs(&sk_old, "bootstrap", &signed))
        ),
        VerifyOutcome::Valid { .. }
    ));
    assert!(matches!(
        set.verify(
            now(),
            &sb,
            &parse_signatures(&sign_sigs(&sk_new, "key-2", &signed))
        ),
        VerifyOutcome::Valid { .. }
    ));

    // revoke old, promote new
    set.merge_rotation(vec![
        TrustedKey {
            key_id: "bootstrap".into(),
            public_b64: pk_old,
            status: KeyStatus::Revoked,
            not_before_unix: 0,
            not_after_unix: 0,
        },
        TrustedKey {
            key_id: "key-2".into(),
            public_b64: pk_new,
            status: KeyStatus::Active,
            not_before_unix: 0,
            not_after_unix: 0,
        },
    ]);
    // old key no longer usable
    assert_eq!(
        set.verify(
            now(),
            &sb,
            &parse_signatures(&sign_sigs(&sk_old, "bootstrap", &signed))
        ),
        VerifyOutcome::NoValidSignature
    );
    assert!(matches!(
        set.verify(
            now(),
            &sb,
            &parse_signatures(&sign_sigs(&sk_new, "key-2", &signed))
        ),
        VerifyOutcome::Valid { .. }
    ));
}

// ── Scenario 3: freshness state machine (partition -> grace -> strict deny) ─
#[test]
fn s3_freshness_partition_to_strict_deny() {
    let cfg = FreshnessConfig {
        max_bundle_age_secs: 1000,
        grace_secs: 10,
    };
    // fresh
    assert!(matches!(
        evaluate_state(100, Some(200), Some(90), &cfg),
        EnforcementState::Active { .. }
    ));
    // expired within grace -> still enforcing (LKG)
    assert!(matches!(
        evaluate_state(205, Some(200), Some(150), &cfg),
        EnforcementState::GracePeriod { .. }
    ));
    // beyond grace -> deny
    assert!(evaluate_state(215, Some(200), Some(150), &cfg).is_strict_deny());
    // max_bundle_age exceeded (partition) even if bundle not expired -> deny
    assert!(evaluate_state(2000, Some(99999), Some(500), &cfg).is_strict_deny());
    // cold start -> deny
    assert!(evaluate_state(100, None, None, &cfg).is_strict_deny());
}

// ── Scenario 4: PEP gate reads status file and fails closed ─────────────────
#[test]
fn s4_pep_gate_failsafe() {
    let _g = SERIAL.lock().unwrap();

    // strict deny published -> gate denies
    write_status_atomic(&EnforcementStatus {
        state: EnforcementState::StrictDeny {
            since_unix: now(),
            reason: "bundle_expired_beyond_grace".into(),
        },
        updated_unix: now(),
        bundle_version: None,
    })
    .unwrap();
    invalidate_cache();
    assert!(strict_deny_reason().is_some());

    // active -> gate allows
    write_status_atomic(&EnforcementStatus {
        state: EnforcementState::Active {
            expires_at_unix: now() + 3600,
        },
        updated_unix: now(),
        bundle_version: Some("9_9_9".into()),
    })
    .unwrap();
    invalidate_cache();
    assert_eq!(strict_deny_reason(), None);

    // absent status -> fail closed
    let _ = std::fs::remove_file(dek_policy_syncer::state::status_path());
    invalidate_cache();
    assert!(
        strict_deny_reason().is_some(),
        "no status => deny (fail-closed)"
    );
}

// ── Scenario 5: in-process mock /v1/keys is verified before merge (chain) ───
#[tokio::test]
async fn s5_v1_keys_chain_of_trust() {
    use axum::{routing::get, Json, Router};
    use serde_json::json;

    let (sk_boot, pk_boot) = keypair(1);
    let (_sk_rogue, _pk_rogue) = keypair(7);
    let current = TrustedKeySet::from_single_pinned(&pk_boot);

    // mock cloud serves /v1/keys signed by bootstrap (legit) on an ephemeral port
    let (_sk2, pk2) = keypair(2);
    let signed = json!({ "version": 1, "keys": [
        { "key_id": "bootstrap", "public_b64": pk_boot, "status": "active", "not_before_unix": 0, "not_after_unix": 0 },
        { "key_id": "key-2", "public_b64": pk2, "status": "next", "not_before_unix": 0, "not_after_unix": 0 }
    ]});
    let sigs = sign_sigs(&sk_boot, "bootstrap", &signed);
    let body = json!({ "signed": signed, "signatures": sigs });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/v1/keys",
        get(move || {
            let b = body.clone();
            async move { Json(b) }
        }),
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let url = format!("http://{addr}/v1/keys");
    let (merged, delta) = dek_policy_syncer::keys::fetch_and_merge(&client, &url, None, &current)
        .await
        .unwrap();
    assert!(
        delta.added.contains(&"key-2".to_string()),
        "next key merged after chain-of-trust verify"
    );
    assert_eq!(merged.usable_keys(now()).count(), 2);
}
