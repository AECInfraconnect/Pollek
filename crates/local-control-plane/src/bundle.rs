// SPDX-License-Identifier: Apache-2.0
//! bundle.rs — build signed policy bundles in the SAME format as Pollen Cloud

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json, Router,
};
use dek_control_plane_api::bundle::{
    ActivationStrategy, BundleArtifactV2, BundleSignature, PollenPolicyBundleManifestV2,
};
use serde_json::Value;

use crate::signing::LocalSigner;

pub struct CompiledArtifact {
    pub artifact_id: String,
    pub adapter_id: String,
    pub artifact_type: String,
    pub bytes: Vec<u8>,
}

pub struct SignedBundle {
    pub manifest: PollenPolicyBundleManifestV2,
    pub blobs: Vec<(String, Vec<u8>)>,
}

#[allow(clippy::too_many_arguments)]
pub async fn build_signed_bundle(
    signer: &LocalSigner,
    tenant_id: &str,
    workspace_id: &str,
    environment_id: &str,
    build_number: u64,
    compiled: Vec<CompiledArtifact>,
    registry_snap: &Value,
    router_config: &Value,
    rollback_from: Option<&str>,
) -> Result<SignedBundle> {
    let bundle_version = format!("v{}", build_number);
    let created_at = chrono::Utc::now().to_rfc3339();

    let mut artifacts = vec![];
    let mut blobs = vec![];

    use sha2::{Digest, Sha256};

    // Snapshot
    let snap_bytes = serde_json::to_vec(registry_snap)?;
    let snap_sha256 = hex::encode(Sha256::digest(&snap_bytes));
    blobs.push((format!("registry/{}", snap_sha256), snap_bytes));

    // Router
    let router_bytes = serde_json::to_vec(router_config)?;
    let router_sha256 = hex::encode(Sha256::digest(&router_bytes));
    blobs.push((format!("router/{}", router_sha256), router_bytes));

    for ca in compiled {
        let sha = hex::encode(Sha256::digest(&ca.bytes));
        let blob_path = format!("artifacts/{}", sha);
        blobs.push((blob_path.clone(), ca.bytes.clone()));

        artifacts.push(BundleArtifactV2 {
            artifact_id: ca.artifact_id,
            adapter_id: ca.adapter_id,
            artifact_type: ca.artifact_type,
            sha256: sha,
            size_bytes: ca.bytes.len() as u64,
            path: blob_path,
            entrypoint: None,
            data_path: None,
            schema_path: None,
            entities_path: None,
        });
    }

    let mut manifest = PollenPolicyBundleManifestV2 {
        schema_version: "2.0".to_string(),
        bundle_version: bundle_version.clone(),
        bundle_id: format!("bundle-local-{}", build_number),
        tenant_id: tenant_id.to_string(),
        workspace_id: workspace_id.to_string(),
        environment_id: environment_id.to_string(),
        build_number,
        created_at,
        expires_at: Some("2036-01-01T00:00:00Z".to_string()),
        created_by: "local-admin".to_string(),
        registry_snapshot_sha256: snap_sha256,
        router_config_sha256: router_sha256,
        artifacts,
        signatures: vec![],
        min_dek_version: "1.0.0-beta.1".to_string(),
        activation_strategy: ActivationStrategy::AtomicAllOrNothing,
        rollback_from: rollback_from.map(|s| s.to_string()),
    };

    manifest.signatures.clear(); // Ensure empty for signing
    let signed_bytes = serde_json::to_vec(&manifest).unwrap();

    let mut hasher = Sha256::new();
    hasher.update(&signed_bytes);
    let hash_bytes = hasher.finalize();
    let hash_hex = hash_bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    tracing::info!("LCP signed bytes SHA256: {}", hash_hex);

    let sig_b64 = signer.sign_b64(&signed_bytes);

    manifest.signatures.push(BundleSignature {
        signature_id: format!("sig-{}", bundle_version),
        signature_type: "ed25519".to_string(),
        payload: sig_b64,
        public_key_fingerprint: signer.key_id.clone(),
    });

    Ok(SignedBundle { manifest, blobs })
}

pub fn verify_bundle(manifest: &PollenPolicyBundleManifestV2, public_b64: &str) -> bool {
    let mut copy = manifest.clone();
    let sigs = copy.signatures.clone();
    copy.signatures.clear();
    let Ok(signed_bytes) = serde_json::to_vec(&copy) else {
        return false;
    };

    use base64::Engine;
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    let Ok(pk_bytes) = base64::prelude::BASE64_STANDARD.decode(public_b64) else {
        return false;
    };
    let Ok(arr): Result<[u8; 32], _> = pk_bytes.try_into() else {
        return false;
    };
    let Ok(vk) = VerifyingKey::from_bytes(&arr) else {
        return false;
    };

    for s in sigs {
        let Ok(sig_bytes) = base64::prelude::BASE64_STANDARD.decode(&s.payload) else {
            continue;
        };
        let Ok(sig_arr): Result<[u8; 64], _> = sig_bytes.try_into() else {
            continue;
        };
        let sig = Signature::from_bytes(&sig_arr);
        if vk.verify(&signed_bytes, &sig).is_ok() {
            return true;
        }
    }
    false
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/devices/:device/bundles/manifest",
            axum::routing::get(get_manifest),
        )
        .route(
            "/v1/tenants/:tenant/devices/:device/bundles/artifacts/:sha",
            axum::routing::get(get_artifact),
        )
        .route(
            "/v1/tenants/:tenant/devices/:device/trusted-keys",
            axum::routing::get(get_trusted_keys),
        )
        .route(
            "/v1/tenants/:tenant/devices/:device/config",
            axum::routing::get(get_mock_config),
        )
}

async fn get_mock_config(
    Path((tenant, _device)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut combined_cedar = String::new();

    if let Ok(Some(manifest_val)) = st
        .policy_store
        .get_policy_raw(&tenant, "bundle:latest")
        .await
    {
        if let Ok(manifest) = serde_json::from_value::<PollenPolicyBundleManifestV2>(manifest_val) {
            for artifact in manifest.artifacts {
                if artifact.adapter_id == "cedar" {
                    if let Ok(Some(bytes)) = st.policy_store.get_blob(&tenant, &artifact.path).await
                    {
                        if let Ok(text) = String::from_utf8(bytes) {
                            combined_cedar.push_str(&text);
                            combined_cedar.push('\n');
                        }
                    }
                }
            }
        }
    }

    Ok(Json(serde_json::json!({
        "device_id": "device-001",
        "tenant_id": tenant,
        "mtls": {
            "root_ca_path": "certs/root_ca.crt",
            "client_cert_path": "certs/device.crt",
            "client_key_path": "certs/device.key"
        },
        "policy_config": {
            "mode": "strict_enforce",
            "fail_closed": true,
            "cedar": {
                "policy_src": combined_cedar
            },
            "routes": [
                {
                    "id": "route_default",
                    "priority": 10,
                    "match_rule": { "method": "*", "tool_category": null },
                    "pdp_required": ["cedar"],
                    "pdp_conditional": []
                }
            ]
        }
    })))
}

async fn get_trusted_keys(State(st): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({ "keys": [{
        "key_id": st.signer.key_id, "public_b64": st.signer.public_key_b64(),
        "status": "active", "not_before_unix": 0, "not_after_unix": 0
    }]})))
}

async fn get_manifest(
    Path((tenant, _device)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    match st.policy_store.get_policy_raw(&tenant, "bundle:latest").await {
        Ok(Some(val)) => Ok(Json(val)),
        Ok(None) => Err(ApiError::NotFound("bundle".into())),
        Err(e) => Err(ApiError::Internal(e)),
    }
}

async fn get_artifact(
    Path((tenant, _device, sha)): Path<(String, String, String)>,
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Vec<u8>)> {
    if sha == "network_guardrails.json" {
        let signed_bytes = serde_json::to_vec(&serde_json::json!([])).unwrap();
        let sig_b64 = st.signer.sign_b64(&signed_bytes);
        let signed_payload = serde_json::json!({
            "signed": [],
            "signatures": [{
                "signature_id": st.signer.key_id,
                "signature_type": "ed25519",
                "payload": sig_b64,
                "public_key_fingerprint": st.signer.public_key_b64(),
            }]
        });
        return Ok((
            StatusCode::OK,
            serde_json::to_vec(&signed_payload).unwrap(),
        ));
    }

    let path = format!("artifacts/{sha}");
    match st.policy_store.get_blob(&tenant, &path).await {
        Ok(Some(bytes)) => Ok((StatusCode::OK, bytes)),
        Ok(None) => Err(ApiError::NotFound("artifact".into())),
        Err(e) => Err(ApiError::Internal(e)),
    }
}
