#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::duplicated_attributes
)]
//! E4: the LCP activation path (`POST /v1/tenants/:tenant/policies/deploy/commit`)
//! must enforce bundle signature verification: valid signed envelopes activate,
//! tampered envelopes and unsigned bare manifests are rejected with a 4xx.

use reqwest::Client;
use serde_json::json;

mod common;

async fn build_envelope(harness: &common::LocalControlPlaneHarness) -> serde_json::Value {
    let built = local_control_plane::bundle::build_signed_bundle(
        &harness.signer,
        "local",
        "default",
        "local",
        1,
        vec![],
        &json!({}),
        &json!({}),
        None,
    )
    .await
    .unwrap();
    built.envelope
}

#[tokio::test]
async fn valid_signed_envelope_activates() {
    let harness = common::LocalControlPlaneHarness::start().await;
    let client = Client::new();
    let envelope = build_envelope(&harness).await;

    let resp = client
        .post(format!(
            "{}/v1/tenants/local/policies/deploy/commit",
            harness.base_url
        ))
        .json(&envelope)
        .send()
        .await
        .unwrap();

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap();
        panic!("valid signed bundle must activate: {}", text);
    }
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "success");
}

#[tokio::test]
async fn tampered_envelope_is_rejected() {
    let harness = common::LocalControlPlaneHarness::start().await;
    let client = Client::new();
    let mut envelope = build_envelope(&harness).await;
    // Tamper with the manifest after signing; the signature no longer matches.
    envelope["manifest"]["metadata"]["bundle_id"] = json!("bundle-evil-tampered");

    let resp = client
        .post(format!(
            "{}/v1/tenants/local/policies/deploy/commit",
            harness.base_url
        ))
        .json(&envelope)
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        400,
        "tampered bundle must be rejected with a 4xx"
    );
}

#[tokio::test]
async fn envelope_signed_by_wrong_key_is_rejected() {
    let harness = common::LocalControlPlaneHarness::start().await;
    let client = Client::new();

    // Sign with a DIFFERENT key than the control plane's trusted signer.
    let rogue_dir = tempfile::tempdir().unwrap();
    let rogue_signer =
        local_control_plane::signing::LocalSigner::load_or_create(rogue_dir.path()).unwrap();
    let built = local_control_plane::bundle::build_signed_bundle(
        &rogue_signer,
        "local",
        "default",
        "local",
        1,
        vec![],
        &json!({}),
        &json!({}),
        None,
    )
    .await
    .unwrap();

    let resp = client
        .post(format!(
            "{}/v1/tenants/local/policies/deploy/commit",
            harness.base_url
        ))
        .json(&built.envelope)
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        400,
        "bundle signed by an untrusted key must be rejected with a 4xx"
    );
}

#[tokio::test]
async fn unsigned_bare_manifest_is_rejected_by_default() {
    let harness = common::LocalControlPlaneHarness::start().await;
    let client = Client::new();
    let envelope = build_envelope(&harness).await;
    // Post ONLY the manifest (no signatures) — the pre-E4 contract.
    let manifest = envelope["manifest"].clone();

    let resp = client
        .post(format!(
            "{}/v1/tenants/local/policies/deploy/commit",
            harness.base_url
        ))
        .json(&manifest)
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        400,
        "unsigned bare manifest must be rejected unless DEK_LCP_ALLOW_UNSIGNED_ACTIVATION=1"
    );
}
