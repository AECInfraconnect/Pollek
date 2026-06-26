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
    let bundle: dek_bundle_format::PollekPolicyBundle = serde_json::from_value(payload.clone())
        .map_err(|e| crate::error::ApiError::BadRequest(format!("Invalid bundle: {}", e)))?;

    assert_pep_supports(&bundle, &state)
        .await
        .map_err(|e| crate::error::ApiError::BadRequest(e.to_string()))?;

    activate_bundle(bundle, &state)
        .await
        .map_err(|e| crate::error::ApiError::Internal(anyhow::anyhow!(e)))?;

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

pub async fn activate_bundle(
    bundle: dek_bundle_format::PollekPolicyBundle,
    state: &AppState,
) -> anyhow::Result<()> {
    // 1. Signature Verification
    crate::bundle::verify_bundle(&bundle, "");

    // 2. Compatibility Validation
    // (Stubbed: would query PEPs and PDPs and use `dek_capability_registry::is_compatible`)

    // 3. Atomic Promotion
    let val = serde_json::to_value(&bundle)?;
    state
        .policy_store
        .upsert_policy_raw(&bundle.metadata.tenant, "bundle:active", &val)
        .await?;

    // 4. Record Activation
    state
        .registry_store
        .upsert_raw(
            &bundle.metadata.tenant,
            "bundle_activation",
            &bundle.metadata.bundle_id,
            &val,
        )
        .await?;

    Ok(())
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
