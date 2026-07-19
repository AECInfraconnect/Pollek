use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use serde::Serialize;
use serde_json::{json, Value};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/policies/deploy/preview",
            post(preview_deploy),
        )
        .route(
            "/v1/tenants/:tenant/policies/deploy/commit",
            post(commit_deploy),
        )
        .route(
            "/v1/tenants/:tenant/policies/deploy/rollback",
            post(rollback_deploy),
        )
}

async fn preview_deploy(
    Path(_tenant): Path<String>,
    State(_state): State<AppState>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    Json(json!({
        "status": "success",
        "message": "Preview generated",
        "diff": {
            "added": ["new_policy.cedar"],
            "removed": []
        },
        "payload": payload
    }))
}

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct DeployReport {
    pub deploy_status: String,
    pub targets: std::collections::HashMap<String, DeployTarget>,
}

#[derive(Debug, Serialize, serde::Deserialize)]
pub struct DeployTarget {
    pub os: String,
    pub target_pep: String,
    pub capability: String,
    pub fallback_pdp: Option<String>,
}

async fn commit_deploy(
    Path(_tenant): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> crate::error::ApiResult<Json<Value>> {
    // Verification + compatibility + atomic promotion; signature failures
    // surface as 4xx (BadRequest), store failures as 500.
    activate_bundle(&payload, &state).await?;

    let mut targets = std::collections::HashMap::new();
    let local_caps = dek_capability_registry::detect::detect_pep_capabilities();
    let has_enforce = local_caps.iter().any(|c| c.control_level.may_block());

    targets.insert(
        "localhost".into(),
        DeployTarget {
            os: std::env::consts::OS.into(),
            target_pep: "auto".into(),
            capability: if has_enforce {
                "enforce".into()
            } else {
                "observe".into()
            },
            fallback_pdp: None,
        },
    );

    Ok(Json(json!({
        "status": "success",
        "message": "Deployment committed and activated successfully",
        "bundle_version": "v1.0.1",
        "report": DeployReport {
            deploy_status: "active".into(),
            targets,
        }
    })))
}

pub async fn assert_pep_supports(
    bundle: &dek_bundle_format::PollekPolicyBundle,
    _state: &AppState,
) -> anyhow::Result<()> {
    let local_caps = dek_capability_registry::detect::detect_pep_capabilities();
    for req_pep in &bundle.compatibility.required_pep_types {
        let cap = local_caps.iter().find(|c| &c.r#type == req_pep);
        if let Some(c) = cap {
            if bundle.activation.strategy == "enforce" && !c.control_level.may_block() {
                return Err(anyhow::anyhow!(
                    "PEP {} does not support enforce mode on this OS",
                    req_pep
                ));
            }
        } else {
            return Err(anyhow::anyhow!("Required PEP {} is not available", req_pep));
        }
    }
    Ok(())
}

/// Activate a policy bundle submitted to `deploy/commit`.
///
/// Step 1 enforces the chain of trust (invariant I3: bundles are always
/// signed): the payload MUST be a signed bundle envelope (`manifest` +
/// `signatures`, as built by [`crate::bundle::build_signed_bundle`]) whose
/// ed25519 signature verifies against this control plane's trusted key — the
/// same key the DEK seeds its local trust store with. Unsigned bare manifests
/// are rejected, unless the explicit local-dev bypass
/// `DEK_LCP_ALLOW_UNSIGNED_ACTIVATION=1` is set (warn-logged, never silent).
pub async fn activate_bundle(
    payload: &Value,
    state: &AppState,
) -> crate::error::ApiResult<dek_bundle_format::PollekPolicyBundle> {
    // 1. Signature Verification
    let bundle = extract_verified_manifest(
        payload,
        &state.signer.public_key_b64(),
        unsigned_activation_allowed(),
    )?;

    // 2. Compatibility Validation
    assert_pep_supports(&bundle, state)
        .await
        .map_err(|e| crate::error::ApiError::BadRequest(e.to_string()))?;

    // 3. Atomic Promotion
    let val =
        serde_json::to_value(&bundle).map_err(|e| crate::error::ApiError::Internal(e.into()))?;
    state
        .policy_store
        .upsert_policy_raw(&bundle.metadata.tenant, "bundle:active", &val)
        .await
        .map_err(crate::error::ApiError::Internal)?;

    // 4. Record Activation
    state
        .registry_store
        .upsert_raw(
            &bundle.metadata.tenant,
            "bundle_activation",
            &bundle.metadata.bundle_id,
            &val,
        )
        .await
        .map_err(crate::error::ApiError::Internal)?;

    Ok(bundle)
}

/// Extract the manifest from a deploy/commit payload, enforcing signature policy.
///
/// Envelope payloads (anything carrying `manifest`/`signatures` members) must
/// carry a signature that verifies against `public_key_b64`; bare manifests
/// are accepted only when `allow_unsigned` is set.
fn extract_verified_manifest(
    payload: &Value,
    public_key_b64: &str,
    allow_unsigned: bool,
) -> crate::error::ApiResult<dek_bundle_format::PollekPolicyBundle> {
    let is_envelope = payload.get("manifest").is_some() || payload.get("signatures").is_some();

    let manifest_val = if is_envelope {
        crate::bundle::verify_bundle_signature(payload, public_key_b64)
            .map_err(crate::error::ApiError::BadRequest)?;
        payload
            .get("manifest")
            .cloned()
            .ok_or_else(|| {
                crate::error::ApiError::BadRequest("bundle envelope is missing manifest".into())
            })?
    } else {
        if !allow_unsigned {
            return Err(crate::error::ApiError::BadRequest(
                "unsigned bundle rejected: deploy/commit requires a signed bundle envelope \
                 (manifest + signatures); set DEK_LCP_ALLOW_UNSIGNED_ACTIVATION=1 to bypass \
                 for local development"
                    .into(),
            ));
        }
        tracing::warn!(
            "DEK_LCP_ALLOW_UNSIGNED_ACTIVATION=1: activating UNSIGNED bundle manifest \
             (local development bypass, do not use in production)"
        );
        payload.clone()
    };

    serde_json::from_value(manifest_val)
        .map_err(|e| crate::error::ApiError::BadRequest(format!("Invalid bundle manifest: {e}")))
}

/// Explicit opt-in for activating unsigned manifests during local development.
fn unsigned_activation_allowed() -> bool {
    std::env::var("DEK_LCP_ALLOW_UNSIGNED_ACTIVATION").unwrap_or_default() == "1"
}

async fn rollback_deploy(
    Path(tenant): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> crate::error::ApiResult<Json<Value>> {
    let version = payload
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("v1.0.0");
    // In a real rollback, we would fetch the old bundle by version and activate it.
    // For now, we just clear the active bundle or mock it.
    state
        .policy_store
        .delete_policy(&tenant, "bundle:active")
        .await
        .map_err(crate::error::ApiError::Internal)?;

    Ok(Json(json!({
        "status": "success",
        "message": "Deployment rolled back",
        "rolled_back_to": version
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ApiError;
    use crate::signing::LocalSigner;

    fn test_signer(tag: &str) -> LocalSigner {
        let dir = std::env::temp_dir().join(format!("lcp-extract-{tag}-{}", std::process::id()));
        #[allow(clippy::unwrap_used)]
        LocalSigner::load_or_create(&dir).unwrap()
    }

    fn sample_manifest_value() -> Value {
        let manifest = dek_bundle_format::PollekPolicyBundle {
            api_version: "local/v1alpha1".into(),
            kind: "Bundle".into(),
            metadata: dek_bundle_format::BundleMetadata {
                bundle_id: "bundle-test-1".into(),
                tenant: "local".into(),
                version: "v1".into(),
                created_at: "2026-07-19T00:00:00Z".into(),
                created_by: "local-admin".into(),
            },
            compatibility: dek_bundle_format::BundleCompatibility {
                min_dek_version: "1.0.0-beta.1".into(),
                required_crates: vec![],
                required_pep_types: vec![],
                required_os_modules: dek_bundle_format::OsModulesConfig {
                    linux: vec![],
                    windows: vec![],
                    macos: vec![],
                },
            },
            artifacts: vec![],
            activation: dek_bundle_format::ActivationConfig {
                strategy: "atomic".into(),
                rollback_on_failure: true,
                health_check_timeout_ms: 10000,
                shadow_before_enforce_seconds: 0,
            },
        };
        #[allow(clippy::unwrap_used)]
        serde_json::to_value(&manifest).unwrap()
    }

    fn signed_envelope(signer: &LocalSigner, manifest: &Value) -> Value {
        #[allow(clippy::unwrap_used)]
        let canonical = serde_jcs::to_vec(manifest).unwrap();
        let sig = signer.sign_b64(&canonical);
        json!({
            "schema_version": "bundle-envelope.v1",
            "manifest": manifest,
            "signatures": [{
                "signature_id": "sig-test",
                "signature_type": "ed25519",
                "payload": sig,
                "public_key_fingerprint": signer.key_id,
            }]
        })
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn signed_envelope_yields_manifest() {
        let signer = test_signer("ok");
        let envelope = signed_envelope(&signer, &sample_manifest_value());
        let manifest = extract_verified_manifest(&envelope, &signer.public_key_b64(), false)
            .unwrap();
        assert_eq!(manifest.metadata.bundle_id, "bundle-test-1");
        assert_eq!(manifest.metadata.tenant, "local");
    }

    #[test]
    fn tampered_envelope_is_bad_request() {
        let signer = test_signer("tampered");
        let mut envelope = signed_envelope(&signer, &sample_manifest_value());
        envelope["manifest"]["metadata"]["bundle_id"] = json!("bundle-evil-tampered");
        let res = extract_verified_manifest(&envelope, &signer.public_key_b64(), false);
        assert!(
            matches!(&res, Err(ApiError::BadRequest(_))),
            "expected BadRequest, got ok={}",
            res.is_ok()
        );
    }

    #[test]
    fn envelope_without_signatures_is_bad_request() {
        let signer = test_signer("nosig");
        let envelope = json!({ "manifest": sample_manifest_value() });
        let res = extract_verified_manifest(&envelope, &signer.public_key_b64(), false);
        assert!(
            matches!(&res, Err(ApiError::BadRequest(_))),
            "expected BadRequest, got ok={}",
            res.is_ok()
        );
    }

    #[test]
    fn envelope_from_untrusted_key_is_bad_request() {
        let trusted = test_signer("trusted");
        let rogue = test_signer("rogue");
        let envelope = signed_envelope(&rogue, &sample_manifest_value());
        let res = extract_verified_manifest(&envelope, &trusted.public_key_b64(), false);
        assert!(
            matches!(&res, Err(ApiError::BadRequest(_))),
            "expected BadRequest, got ok={}",
            res.is_ok()
        );
    }

    #[test]
    fn bare_manifest_rejected_without_bypass() {
        let signer = test_signer("bare-deny");
        let manifest = sample_manifest_value();
        let res = extract_verified_manifest(&manifest, &signer.public_key_b64(), false);
        assert!(
            matches!(&res, Err(ApiError::BadRequest(_))),
            "expected BadRequest, got ok={}",
            res.is_ok()
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn bare_manifest_allowed_with_explicit_bypass() {
        let signer = test_signer("bare-allow");
        let manifest = sample_manifest_value();
        let extracted =
            extract_verified_manifest(&manifest, &signer.public_key_b64(), true).unwrap();
        assert_eq!(extracted.metadata.bundle_id, "bundle-test-1");
    }
}
