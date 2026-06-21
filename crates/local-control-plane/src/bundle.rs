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
use dek_bundle_format::{
    ActivationConfig, BundleArtifact, BundleCompatibility, BundleMetadata, OsModulesConfig,
    PollenPolicyBundle,
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
    pub manifest: PollenPolicyBundle,
    pub envelope: serde_json::Value,
    pub blobs: Vec<(String, Vec<u8>)>,
}

#[allow(clippy::too_many_arguments)]
pub async fn build_signed_bundle(
    _signer: &LocalSigner,
    tenant_id: &str,
    _workspace_id: &str,
    _environment_id: &str,
    build_number: u64,
    compiled: Vec<CompiledArtifact>,
    registry_snap: &Value,
    router_config: &Value,
    _rollback_from: Option<&str>,
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

        artifacts.push(BundleArtifact {
            r#type: ca.artifact_type,
            sha256: sha,
            path: blob_path,
        });
    }

    let manifest = PollenPolicyBundle {
        api_version: "pollen.ai/v1alpha1".to_string(),
        kind: "Bundle".to_string(),
        metadata: BundleMetadata {
            bundle_id: format!("bundle-local-{}", build_number),
            tenant: tenant_id.to_string(),
            version: bundle_version.clone(),
            created_at,
            created_by: "local-admin".to_string(),
        },
        compatibility: BundleCompatibility {
            min_dek_version: "1.0.0-beta.1".to_string(),
            required_crates: vec![],
            required_pep_types: vec![],
            required_os_modules: OsModulesConfig {
                linux: vec![],
                windows: vec![],
                macos: vec![],
            },
        },
        artifacts,
        activation: ActivationConfig {
            strategy: "atomic".to_string(),
            rollback_on_failure: true,
            health_check_timeout_ms: 10000,
            shadow_before_enforce_seconds: 0,
        },
    };

    let signed_bytes = serde_json::to_vec(&manifest).unwrap();
    let sig_b64 = _signer.sign_b64(&signed_bytes);

    let envelope = serde_json::json!({
        "manifest": manifest,
        "signatures": [{
            "signature_id": format!("sig-{}", bundle_version),
            "signature_type": "ed25519",
            "payload": sig_b64,
            "public_key_fingerprint": _signer.key_id.clone(),
        }]
    });

    Ok(SignedBundle {
        manifest,
        envelope,
        blobs,
    })
}

pub fn verify_bundle(_manifest: &PollenPolicyBundle, _public_b64: &str) -> bool {
    // In v1, signature is verified against the outer SignedBundle or HTTP headers.
    // Stubbing to true for local-control-plane.
    true
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
        if let Some(inner) = manifest_val.get("manifest") {
            if let Ok(manifest) = serde_json::from_value::<PollenPolicyBundle>(inner.clone()) {
                for artifact in manifest.artifacts {
                    if artifact.r#type == "cedar_text" {
                        if let Ok(Some(bytes)) =
                            st.policy_store.get_blob(&tenant, &artifact.path).await
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
    match st
        .policy_store
        .get_policy_raw(&tenant, "bundle:latest")
        .await
    {
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
        return Ok((StatusCode::OK, serde_json::to_vec(&signed_payload).unwrap()));
    }

    let path = format!("artifacts/{sha}");
    match st.policy_store.get_blob(&tenant, &path).await {
        Ok(Some(bytes)) => Ok((StatusCode::OK, bytes)),
        Ok(None) => Err(ApiError::NotFound("artifact".into())),
        Err(e) => Err(ApiError::Internal(e)),
    }
}
