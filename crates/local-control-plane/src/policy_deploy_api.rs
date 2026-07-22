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

/// Artifact identifiers of a bundle, used to compute a real preview diff.
fn artifact_ids(bundle: &dek_bundle_format::PollekPolicyBundle) -> Vec<String> {
    bundle
        .artifacts
        .iter()
        .map(|a| format!("{}:{}", a.r#type, a.path))
        .collect()
}

async fn preview_deploy(
    Path(tenant): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> crate::error::ApiResult<Json<Value>> {
    let incoming: dek_bundle_format::PollekPolicyBundle =
        serde_json::from_value(payload.clone())
            .map_err(|e| crate::error::ApiError::BadRequest(format!("Invalid bundle: {}", e)))?;

    // Real diff: compare the incoming bundle's artifacts against the currently
    // active bundle for this tenant (empty active set when none is deployed).
    let active_ids: Vec<String> = match state
        .policy_store
        .get_policy_raw(&tenant, "bundle:active")
        .await
        .map_err(crate::error::ApiError::Internal)?
        .and_then(|v| serde_json::from_value::<dek_bundle_format::PollekPolicyBundle>(v).ok())
    {
        Some(active) => artifact_ids(&active),
        None => Vec::new(),
    };
    let incoming_ids = artifact_ids(&incoming);

    let added: Vec<&String> = incoming_ids
        .iter()
        .filter(|id| !active_ids.contains(id))
        .collect();
    let removed: Vec<&String> = active_ids
        .iter()
        .filter(|id| !incoming_ids.contains(id))
        .collect();

    Ok(Json(json!({
        "status": "success",
        "message": "Preview generated",
        "diff": {
            "added": added,
            "removed": removed,
        },
        "incoming_version": incoming.metadata.version,
    })))
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

    let bundle_version = bundle.metadata.version.clone();
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
        "bundle_version": bundle_version,
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
    // Structural validation. The cryptographic (ed25519) signature lives on the
    // outer SignedBundle envelope and is verified by the DEK's activation path
    // (`dek-activation::signature::verify_bundle_signature`) against the pinned
    // key before a bundle is ever handed to a device — this function receives
    // the already-unwrapped manifest, so it validates what it actually can.
    if bundle.metadata.tenant.trim().is_empty() {
        anyhow::bail!("bundle metadata.tenant must not be empty");
    }
    if bundle.metadata.bundle_id.trim().is_empty() {
        anyhow::bail!("bundle metadata.bundle_id must not be empty");
    }

    // Compatibility validation against the real local PEP capabilities.
    assert_pep_supports(&bundle, state).await?;

    // Atomic promotion.
    let val = serde_json::to_value(&bundle)?;
    state
        .policy_store
        .upsert_policy_raw(&bundle.metadata.tenant, "bundle:active", &val)
        .await?;

    // Record activation (history used by rollback).
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
    let requested_version = payload.get("version").and_then(|v| v.as_str());

    // Real rollback: choose the target from recorded bundle activations —
    // the requested version if given, otherwise the newest activation that is
    // not the currently active bundle.
    let activations = state
        .registry_store
        .list_raw(&tenant, "bundle_activation")
        .await
        .map_err(crate::error::ApiError::Internal)?;

    let current_version = state
        .policy_store
        .get_policy_raw(&tenant, "bundle:active")
        .await
        .map_err(crate::error::ApiError::Internal)?
        .and_then(|v| {
            v.get("metadata")
                .and_then(|m| m.get("version"))
                .and_then(|s| s.as_str())
                .map(String::from)
        });

    let version_of = |v: &Value| -> Option<String> {
        v.get("metadata")
            .and_then(|m| m.get("version"))
            .and_then(|s| s.as_str())
            .map(String::from)
    };
    let created_of = |v: &Value| -> String {
        v.get("metadata")
            .and_then(|m| m.get("created_at"))
            .and_then(|s| s.as_str())
            .unwrap_or_default()
            .to_string()
    };

    let target = match requested_version {
        Some(want) => activations
            .iter()
            .find(|v| version_of(v).as_deref() == Some(want))
            .cloned(),
        None => {
            let mut prior: Vec<&Value> = activations
                .iter()
                .filter(|v| version_of(v) != current_version)
                .collect();
            prior.sort_by_key(|v| created_of(v));
            prior.last().cloned().cloned()
        }
    };

    let Some(target) = target else {
        return Err(crate::error::ApiError::NotFound(
            "no prior bundle activation to roll back to".into(),
        ));
    };

    let target_bundle: dek_bundle_format::PollekPolicyBundle =
        serde_json::from_value(target.clone()).map_err(|e| {
            crate::error::ApiError::Internal(anyhow::anyhow!("stored activation invalid: {e}"))
        })?;
    let rolled_back_to = target_bundle.metadata.version.clone();

    activate_bundle(target_bundle, &state)
        .await
        .map_err(crate::error::ApiError::Internal)?;

    Ok(Json(json!({
        "status": "success",
        "message": "Deployment rolled back to a previously activated bundle",
        "rolled_back_to": rolled_back_to,
    })))
}
