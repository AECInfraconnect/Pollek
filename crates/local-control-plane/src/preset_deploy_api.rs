// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/policy-presets/deploy",
            post(deploy_preset),
        )
        .route(
            "/v1/tenants/:tenant/policy-presets/deployments/:deployment_id/rollback",
            post(rollback_deployment),
        )
        .route(
            "/v1/tenants/:tenant/policy-presets/deployments",
            get(list_deployments),
        )
}

async fn deploy_preset(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(req): Json<dek_policy_presets::model::DeployPresetRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let preset = dek_policy_presets::catalog::get_builtin_preset(&req.preset_id)
        .ok_or_else(|| ApiError::NotFound(req.preset_id.clone()))?;

    // 1. Validate request
    dek_policy_presets::validate::validate_request(&preset, &req)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let deployment_id = Uuid::new_v4().to_string();

    // 2. Render artifacts
    let rendered_artifacts =
        dek_policy_presets::render::render(&preset, &req).map_err(ApiError::Internal)?;

    // 3. Save to Store
    let deployment_data = serde_json::json!({
        "preset_id": req.preset_id,
        "preset_version": preset.version,
        "control_mode": req.control_mode,
        "status": "active",
        "targets": req.targets,
        "params": req.params,
    });

    st.policy_store
        .upsert_preset_deployment(&tenant, &deployment_id, &deployment_data)
        .await
        .map_err(ApiError::Internal)?;

    let mut bindings = Vec::new();
    for artifact in rendered_artifacts {
        // A budget_limit artifact binds to the REAL budget engine: upsert it
        // into the budget store the usage pipeline evaluates against, so the
        // cap is enforced rather than merely described.
        if artifact.language == "budget_limit" {
            let mut budget: dek_agent_observer::usage_budget::AiBudgetLimit =
                serde_json::from_str(&artifact.content).map_err(|e| {
                    ApiError::Internal(anyhow::anyhow!("invalid budget_limit artifact: {e}"))
                })?;
            budget.tenant_id = tenant.clone();
            let now = chrono::Utc::now().to_rfc3339();
            if budget.created_at.is_empty() {
                budget.created_at = now.clone();
            }
            budget.updated_at = now;
            st.observability_store
                .upsert_ai_budget(&budget)
                .await
                .map_err(ApiError::Internal)?;
            bindings.push(format!("budget:{}", budget.budget_id));
            continue;
        }

        // Save PEP bindings based on language
        let pep_type = match artifact.language.as_str() {
            "rego" => "opa_rego",
            "cedar" => "aws_cedar",
            "openfga" => "openfga",
            "json" => "dek_router_rule",
            _ => "unknown",
        };

        let binding_id = Uuid::new_v4().to_string();
        let binding_data = serde_json::json!({
            "status": "active",
            "content": artifact.content,
        });

        st.policy_store
            .upsert_pep_binding(
                &tenant,
                &binding_id,
                &deployment_id,
                pep_type,
                &binding_data,
            )
            .await
            .map_err(ApiError::Internal)?;

        bindings.push(binding_id);
    }

    Ok(Json(serde_json::json!({
        "schema_version": "policy-preset-deploy-response.v1",
        "deployment_id": deployment_id,
        "status": "active",
        "bindings_created": bindings.len(),
    })))
}

async fn rollback_deployment(
    Path((tenant, deployment_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut deployment = st
        .policy_store
        .get_preset_deployment(&tenant, &deployment_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(deployment_id.clone()))?;

    // Mark deployment as rolled_back
    deployment["status"] = serde_json::json!("rolled_back");
    st.policy_store
        .upsert_preset_deployment(&tenant, &deployment_id, &deployment)
        .await
        .map_err(ApiError::Internal)?;

    // Ideally, we would also disable the PEP bindings here.

    Ok(Json(serde_json::json!({
        "status": "rolled_back",
        "deployment_id": deployment_id
    })))
}

async fn list_deployments(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let deployments = st
        .policy_store
        .list_preset_deployments(&tenant)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(serde_json::json!({
        "deployments": deployments
    })))
}
