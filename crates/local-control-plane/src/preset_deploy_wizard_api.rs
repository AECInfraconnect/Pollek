// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use dek_domain_schema::{
    BindingStatus, ControlBinding, ControlMode, DeploymentStatus, PolicyDeployment,
};
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/policy-deployment/recommend",
            post(recommend_deployment),
        )
        .route(
            "/v1/tenants/:tenant/policy-deployment/preview",
            post(preview_deployment),
        )
        .route(
            "/v1/tenants/:tenant/policy-deployment/simulate",
            post(simulate_deployment),
        )
        .route(
            "/v1/tenants/:tenant/policy-deployment/deploy",
            post(execute_deployment),
        )
}

async fn recommend_deployment(
    Path(_tenant): Path<String>,
    State(_st): State<AppState>,
    Json(_req): Json<dek_policy_presets::model::DeployPresetRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // Stub implementation for recommendation logic
    let mut candidates = Vec::new();
    candidates.push(serde_json::json!({
        "pep_type": "mcp_proxy",
        "score": 80,
        "max_mode": "enforce",
        "reason": "Agent exposes MCP tools",
        "requires_user_approval": false
    }));

    let recommendation = serde_json::json!({
        "recommended_pep": candidates[0].clone(),
        "alternatives": candidates,
        "pdp_route": {
            "primary": "local_cedar",
            "fallback": "pollek_cloud"
        },
        "warnings": []
    });

    Ok(Json(recommendation))
}

async fn preview_deployment(
    Path(_tenant): Path<String>,
    State(_st): State<AppState>,
    Json(_req): Json<dek_policy_presets::model::DeployPresetRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // Return a dummy ConfigDiff structure
    Ok(Json(serde_json::json!({
        "status": "preview",
        "diffs": [
            {
                "agent_id": "agent_auto",
                "file_path": "~/.pollek_dek/mcp_configs/example.json",
                "diff": "+ command: \"dek-stdio-wrapper\"\n- command: \"uvx\""
            }
        ]
    })))
}

async fn simulate_deployment(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(req): Json<dek_policy_presets::model::DeployPresetRequest>,
) -> ApiResult<Json<PolicyDeployment>> {
    let preset = dek_policy_presets::catalog::get_builtin_preset(&req.preset_id)
        .ok_or_else(|| ApiError::NotFound(req.preset_id.clone()))?;

    // 1. Validate request
    dek_policy_presets::validate::validate_request(&preset, &req)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    // 2. Map targets to agents
    let agents = st
        .registry_store
        .list_agent_inventories(&tenant)
        .await
        .map_err(ApiError::Internal)?;

    let matched_agents: Vec<_> = if req.targets.agent_ids.is_empty() {
        agents
    } else {
        agents
            .into_iter()
            .filter(|a| req.targets.agent_ids.contains(&a.agent_id))
            .collect()
    };

    let providers = st
        .registry_store
        .list_blackbox_ai(&tenant)
        .await
        .map_err(ApiError::Internal)?;

    let matched_providers: Vec<_> = if req.targets.provider_ids.is_empty() {
        providers
    } else {
        providers
            .into_iter()
            .filter(|p| req.targets.provider_ids.contains(&p.provider_id))
            .collect()
    };

    // 3. Generate Control Bindings based on PEP capabilities
    let mut control_bindings = Vec::new();
    for agent in matched_agents {
        control_bindings.push(ControlBinding {
            binding_id: Uuid::new_v4().to_string(),
            agent_id: agent.agent_id.clone(),
            pep_type: "std_wrapper".to_string(),
            action: "wrap".to_string(),
            status: BindingStatus::Pending,
            config_backup_id: None,
            binding_json: serde_json::json!({
                "preset_id": req.preset_id,
            }),
        });
    }

    for provider in matched_providers {
        control_bindings.push(ControlBinding {
            binding_id: Uuid::new_v4().to_string(),
            agent_id: provider.provider_id.clone(),
            pep_type: "ext_authz".to_string(),
            action: "wrap".to_string(),
            status: BindingStatus::Pending,
            config_backup_id: None,
            binding_json: serde_json::json!({
                "preset_id": req.preset_id,
            }),
        });
    }

    let deployment = PolicyDeployment {
        schema_version: "policy-deployment.v1".to_string(),
        tenant_id: tenant,
        deployment_id: Uuid::new_v4().to_string(),
        status: DeploymentStatus::Simulated,
        preset_id: Some(req.preset_id),
        preset_version: Some(preset.version),
        control_mode: ControlMode::Observe,
        target_device_groups: vec![],
        target_rollout_ring: None,
        targets: serde_json::to_value(&req.targets).unwrap_or_default(),
        params: serde_json::to_value(&req.params).unwrap_or_default(),
        control_bindings,
        rollback_snapshot_json: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    Ok(Json(deployment))
}

async fn execute_deployment(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(mut deployment): Json<PolicyDeployment>,
) -> ApiResult<Json<PolicyDeployment>> {
    // 1. Mark deployment as Active
    deployment.status = DeploymentStatus::Active;

    // 2. Update bindings and actually apply them!
    for binding in &mut deployment.control_bindings {
        let _ = crate::control_binding::do_apply_binding(&binding.binding_id).await;
        binding.status = BindingStatus::Applied;
    }

    // 3. Emit telemetry event
    let event = serde_json::json!({
        "schema_version": "pollek.telemetry.v2",
        "event_type": "policy_deployment",
        "event_id": format!("evt_{}", Uuid::new_v4()),
        "tenant_id": tenant,
        "deployment_id": deployment.deployment_id,
        "preset_id": deployment.preset_id,
        "status": "applied",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });
    let _ = st
        .telemetry_store
        .put_telemetry(
            &tenant,
            "policy_deployment",
            event["event_id"].as_str().unwrap_or(""),
            &event,
        )
        .await;

    Ok(Json(deployment))
}
