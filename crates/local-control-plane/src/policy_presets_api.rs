// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};

use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tenants/:tenant/policy-presets", get(list_presets))
        .route(
            "/v1/tenants/:tenant/policy-presets/:preset_id",
            get(get_preset),
        )
        .route(
            "/v1/tenants/:tenant/policy-presets/:preset_id/preview",
            post(preview_preset),
        )
        .route(
            "/v1/tenants/:tenant/policy-presets/:preset_id/create-draft",
            post(create_draft),
        )
        .route(
            "/v1/tenants/:tenant/policy-presets/:preset_id/simulate",
            post(simulate_preset),
        )
}

async fn list_presets(Path(_tenant): Path<String>) -> ApiResult<Json<serde_json::Value>> {
    let items = dek_policy_presets::catalog::builtin_presets();
    Ok(Json(serde_json::json!({
        "schema_version": "policy-preset-list.v1",
        "items": items
    })))
}

async fn get_preset(
    Path((_tenant, preset_id)): Path<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let preset = dek_policy_presets::catalog::get_builtin_preset(&preset_id)
        .ok_or_else(|| ApiError::NotFound(preset_id.clone()))?;
    Ok(Json(serde_json::json!(preset)))
}

async fn preview_preset(
    Path((_tenant, preset_id)): Path<(String, String)>,
    Json(req): Json<dek_policy_presets::model::PresetApplyRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let preset = dek_policy_presets::catalog::get_builtin_preset(&preset_id)
        .ok_or_else(|| ApiError::NotFound(preset_id.clone()))?;

    let rendered = dek_policy_presets::render::render(&preset, &req).map_err(ApiError::Internal)?;

    Ok(Json(serde_json::json!({
        "schema_version": "policy-preset-preview.v1",
        "preset_id": preset_id,
        "policy_type": rendered.language,
        "recommended_pep_types": preset.recommended_pep_types,
        "generated_source": rendered.content,
        "warnings": rendered.warnings
    })))
}

async fn create_draft(
    Path((tenant, preset_id)): Path<(String, String)>,
    State(st): State<AppState>,
    Json(req): Json<dek_policy_presets::model::PresetApplyRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let preset = dek_policy_presets::catalog::get_builtin_preset(&preset_id)
        .ok_or_else(|| ApiError::NotFound(preset_id.clone()))?;

    let draft = dek_policy_presets::render::to_policy_draft(&tenant, &preset, &req)
        .map_err(ApiError::Internal)?;

    let saved = st
        .policy_store
        .upsert_policy(draft)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(serde_json::json!({
        "schema_version": "policy-preset-create-draft-response.v1",
        "policy_id": saved.policy_id,
        "status": "draft"
    })))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimulatePresetRequest {
    pub apply_request: dek_policy_presets::model::PresetApplyRequest,
    pub input: serde_json::Value,
}

async fn simulate_preset(
    Path((_tenant, preset_id)): Path<(String, String)>,
    Json(req): Json<SimulatePresetRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let preset = dek_policy_presets::catalog::get_builtin_preset(&preset_id)
        .ok_or_else(|| ApiError::NotFound(preset_id.clone()))?;

    let rendered = dek_policy_presets::render::render(&preset, &req.apply_request)
        .map_err(ApiError::Internal)?;

    let mut result = serde_json::json!({
        "schema_version": "policy-preset-simulation.v1",
        "preset_id": preset_id,
        "policy_type": rendered.language,
    });

    if rendered.language == "cedar" {
        match dek_cedar::CedarAdapter::new(&rendered.content) {
            Ok(adapter) => {
                use dek_plugin_sdk::PolicyEvaluator;
                let eval_req = dek_plugin_sdk::EvalRequest {
                    request_id: "sim-preset".into(),
                    tenant_id: None,
                    subject: None,
                    action: None,
                    resource: None,
                    payload: req.input,
                    context: std::collections::BTreeMap::new(),
                };
                match adapter.evaluate(eval_req).await {
                    Ok(r) => {
                        let effect_str = match r.decision {
                            dek_plugin_sdk::DecisionEffect::Allow => "allow",
                            _ => "deny",
                        };
                        result["result"] = serde_json::json!({
                            "allowed": effect_str == "allow",
                            "decision": effect_str,
                            "reason": r.reason,
                            "obligations": r.obligations,
                        });
                    }
                    Err(e) => {
                        result["result"] = serde_json::json!({
                            "allowed": false,
                            "decision": "error",
                            "reason": format!("Evaluation error: {}", e),
                        });
                    }
                }
            }
            Err(e) => {
                result["result"] = serde_json::json!({
                    "allowed": false,
                    "decision": "error",
                    "reason": format!("Cedar syntax error: {}", e),
                });
            }
        }
    } else {
        // Handle Error when PDP is missing/not found
        result["result"] = serde_json::json!({
            "allowed": false,
            "decision": "error",
            "reason": format!("Error: No active PDP found for language '{}'. Simulation not supported.", rendered.language),
        });
    }

    Ok(Json(result))
}
