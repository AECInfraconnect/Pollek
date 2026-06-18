// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::state::AppState;
use crate::BUNDLE_SEED;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use dek_domain_schema::{ActivationMode, BundleArtifact, BundleManifest};
use ed25519_dalek::{Signer, SigningKey};
use serde::Deserialize;
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant_id/devices/:device_id/bundles/latest",
            post(get_latest_bundle),
        )
        .route(
            "/v1/tenants/:tenant_id/devices/:device_id/bundles/manifest",
            post(get_manifest),
        )
        .route(
            "/v1/devices/:device_id/desired-state",
            get(get_desired_state),
        )
        .route(
            "/v1/tenants/:tenant_id/bundles/publish",
            post(publish_bundle),
        )
        .route(
            "/v1/tenants/:tenant_id/bundles/rollback",
            post(rollback_bundle),
        )
        .route(
            "/v1/tenants/:tenant_id/bundles/invalid/signature",
            get(get_invalid_signature_bundle),
        )
        .route(
            "/v1/tenants/:tenant_id/bundles/invalid/rollback",
            get(get_invalid_rollback_bundle),
        )
        .route(
            "/v1/tenants/:tenant_id/bundles/invalid/malformed",
            get(get_malformed_bundle),
        )
        .route(
            "/v1/tenants/:tenant_id/devices/:device_id/bundles/artifacts/network_guardrails.json",
            get(get_network_guardrails_artifact),
        )
}

fn generate_bundle(tenant_id: &str, generation: u64, is_canary: bool) -> BundleManifest {
    let now = Utc::now();
    let expires = now + Duration::days(1);
    let mode = if is_canary {
        ActivationMode::Canary
    } else {
        ActivationMode::Full
    };

    BundleManifest {
        schema_version: "1.0.0".to_string(),
        bundle_id: format!("bnd-{}", generation),
        bundle_version: format!("v{}", generation),
        bundle_generation: generation,
        tenant_id: tenant_id.to_string(),
        created_at: now.to_rfc3339(),
        expires_at: expires.to_rfc3339(),
        activation_mode: mode,
        artifacts: vec![
            BundleArtifact {
                name: "policies.json".to_string(),
                artifact_type: "json".to_string(),
                sha256: "dummy_hash_for_policies".to_string(),
                url: Some(format!(
                    "https://mock-cloud/v1/tenants/{}/bundles/artifacts/policies.json",
                    tenant_id
                )),
            },
            BundleArtifact {
                name: "registry.json".to_string(),
                artifact_type: "json".to_string(),
                sha256: "dummy_hash_for_registry".to_string(),
                url: Some(format!(
                    "https://mock-cloud/v1/tenants/{}/bundles/artifacts/registry.json",
                    tenant_id
                )),
            },
            BundleArtifact {
                name: "network_guardrails.json".to_string(),
                artifact_type: "json".to_string(),
                sha256: "dummy_hash_for_network_guardrails".to_string(),
                url: Some(format!(
                    "https://mock-cloud/v1/tenants/{}/bundles/artifacts/network_guardrails.json",
                    tenant_id
                )),
            },
        ],
    }
}

fn sign_bundle(manifest: &BundleManifest) -> serde_json::Value {
    let signing_key = SigningKey::from_bytes(&BUNDLE_SEED);
    use base64::Engine;
    let public_key = signing_key.verifying_key();

    let payload_string = serde_json::to_string(manifest).unwrap();
    let signature = signing_key.sign(payload_string.as_bytes());

    json!({
        "schema_version": "bundle-envelope.v1",
        "manifest": manifest,
        "signatures": [{
            "signature_id": format!("sig-{}", manifest.bundle_version),
            "signature_type": "ed25519",
            "payload": base64::prelude::BASE64_STANDARD.encode(signature.to_bytes()),
            "public_key_fingerprint": base64::prelude::BASE64_STANDARD.encode(public_key.as_bytes()),
        }]
    })
}

async fn get_latest_bundle(
    Path((tenant_id, _device_id)): Path<(String, String)>,
    State(state): State<AppState>,
    body: Option<Json<serde_json::Value>>,
) -> impl IntoResponse {
    let current_generation = body
        .as_ref()
        .and_then(|b| b.get("current_generation"))
        .and_then(|v| v.as_i64())
        .unwrap_or(-1);

    let revision = state.revision.load(std::sync::atomic::Ordering::Relaxed) as u64;

    if current_generation == revision as i64 {
        return (
            StatusCode::OK,
            Json(json!({
                "schema_version": "bundle-fetch-response.v1",
                "status": "not_modified",
                "generation": revision
            })),
        );
    }

    let manifest = generate_bundle(&tenant_id, revision, false);
    (
        StatusCode::OK,
        Json(json!({
            "schema_version": "bundle-fetch-response.v1",
            "status": "bundle_ready",
            "generation": revision,
            "envelope": sign_bundle(&manifest)
        })),
    )
}

async fn get_manifest(
    Path((tenant_id, _device_id)): Path<(String, String)>,
    State(state): State<AppState>,
    body: Option<Json<serde_json::Value>>,
) -> impl IntoResponse {
    let _ = body; // ignoring the device's current manifest
    let revision = state.revision.load(std::sync::atomic::Ordering::Relaxed) as u64;
    let manifest = generate_bundle(&tenant_id, revision, false);
    (StatusCode::OK, Json(sign_bundle(&manifest)))
}

#[derive(Deserialize)]
struct DesiredStateQuery {
    #[allow(dead_code)]
    current_bundle: Option<String>,
}

async fn get_desired_state(
    Path(device_id): Path<String>,
    Query(_query): Query<DesiredStateQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let revision = state.revision.load(std::sync::atomic::Ordering::Relaxed) as u64;
    let bundle_id = format!("bundle-{}", revision);

    let response = serde_json::json!({
        "device_id": device_id,
        "desired_bundle": {
            "bundle_id": bundle_id,
            "version": format!("1.0.{}", revision),
            "url": format!("https://mock-cloud.local/bundles/{}.tar.gz", bundle_id),
            "sha256": "dummy_sha256_hash",
            "signature": "dummy_signature"
        },
        "activation": {
            "mode": "hot_reload",
            "deadline": chrono::Utc::now().to_rfc3339()
        }
    });

    (StatusCode::OK, Json(response))
}

#[derive(Deserialize)]
struct PublishRequest {
    canary: Option<bool>,
}

async fn publish_bundle(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    Json(req): Json<PublishRequest>,
) -> impl IntoResponse {
    let new_revision = state
        .revision
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed) as u64
        + 1;
    let is_canary = req.canary.unwrap_or(false);
    let manifest = generate_bundle(&tenant_id, new_revision, is_canary);
    (StatusCode::OK, Json(sign_bundle(&manifest)))
}

async fn rollback_bundle(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // A real rollback might decrement or just publish the previous configuration with a higher generation.
    // To properly simulate rollback (anti-rollback test), we actually try to serve an OLD generation.
    // Here we will decrement revision to simulate a cloud rollback.
    // However, DEK devices should reject it due to anti-rollback protection!
    let mut current = state.revision.load(std::sync::atomic::Ordering::Relaxed);
    if current > 0 {
        current -= 1;
        state
            .revision
            .store(current, std::sync::atomic::Ordering::Relaxed);
    }
    let manifest = generate_bundle(&tenant_id, current as u64, false);
    (StatusCode::OK, Json(sign_bundle(&manifest)))
}

async fn get_invalid_signature_bundle(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let revision = state.revision.load(std::sync::atomic::Ordering::Relaxed) as u64;
    let manifest = generate_bundle(&tenant_id, revision, false);
    let mut signed = sign_bundle(&manifest);
    // Corrupt signature
    if let Some(obj) = signed.as_object_mut() {
        obj.insert(
            "signature".to_string(),
            json!("invalid_base64_signature!!!"),
        );
    }
    (StatusCode::OK, Json(signed))
}

async fn get_invalid_rollback_bundle(
    Path(tenant_id): Path<String>,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    // Serve generation 0 which is guaranteed to trigger anti-rollback if device is at > 0
    let manifest = generate_bundle(&tenant_id, 0, false);
    (StatusCode::OK, Json(sign_bundle(&manifest)))
}

async fn get_malformed_bundle(
    Path(_tenant_id): Path<String>,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    // Missing required fields
    let malformed = json!({
        "bundle_id": "bnd-malformed",
        "signature": "...",
        // Missing payload and public_key
    });
    (StatusCode::OK, Json(malformed))
}

async fn get_network_guardrails_artifact(
    Path((_tenant_id, _device_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let rules: Vec<serde_json::Value> = state.network_rules.lock().unwrap().clone();
    let signed = serde_json::Value::Array(rules);
    let signed_bytes = serde_jcs::to_vec(&signed).unwrap();
    let active_seed = state.active_seed.lock().unwrap();
    let sk = SigningKey::from_bytes(active_seed.as_slice().try_into().unwrap());
    let sig = sk.sign(&signed_bytes);

    use base64::Engine;
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "signed": signed,
            "signatures": [{
                "keyid": "bootstrap",
                "sig": base64::prelude::BASE64_STANDARD.encode(sig.to_bytes())
            }]
        })),
    )
}
