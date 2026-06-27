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
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(_req): Json<dek_policy_presets::model::DeployPresetRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let agents = st
        .registry_store
        .list_agent_inventories(&tenant)
        .await
        .map_err(ApiError::Internal)?;
    let providers = st
        .registry_store
        .list_blackbox_ai(&tenant)
        .await
        .map_err(ApiError::Internal)?;

    let mut candidates = Vec::new();
    for agent in agents {
        for binding in agent.supported_pep_bindings {
            let enforce_ready = binding.mode_supported.iter().any(|mode| {
                matches!(
                    mode,
                    dek_domain_schema::capability_inventory::ControlMode::Enforce
                        | dek_domain_schema::capability_inventory::ControlMode::StrictDeny
                )
            });
            candidates.push(serde_json::json!({
                "agent_id": agent.agent_id,
                "pep_type": binding.pep_type,
                "score": if enforce_ready { 90 } else { 60 },
                "max_mode": if enforce_ready { "enforce" } else { "observe" },
                "reason": binding.reason,
                "requires_user_approval": binding.requires_user_approval,
                "requires_admin": binding.requires_admin
            }));
        }
    }

    for provider in providers {
        candidates.push(serde_json::json!({
            "agent_id": provider.provider_id,
            "pep_type": "http_gateway",
            "score": 70,
            "max_mode": "observe",
            "reason": "Observed blackbox AI provider can be routed through an HTTP gateway when configured.",
            "requires_user_approval": true,
            "requires_admin": false
        }));
    }

    candidates.sort_by(|left, right| {
        right
            .get("score")
            .and_then(|value| value.as_i64())
            .cmp(&left.get("score").and_then(|value| value.as_i64()))
    });
    let recommended_pep = candidates.first().cloned();
    let warnings = if candidates.is_empty() {
        vec!["No discovered agent inventory is available yet. Run Auto Discovery first."]
    } else {
        Vec::<&str>::new()
    };

    let recommendation = serde_json::json!({
        "recommended_pep": recommended_pep,
        "alternatives": candidates,
        "pdp_route": {
            "primary": "local_cedar",
            "fallback": null
        },
        "warnings": warnings
    });

    Ok(Json(recommendation))
}

async fn preview_deployment(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(req): Json<dek_policy_presets::model::DeployPresetRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let agents = st
        .registry_store
        .list_agent_inventories(&tenant)
        .await
        .map_err(ApiError::Internal)?;
    let mut diffs = Vec::new();

    for agent in agents {
        if !req.targets.agent_ids.is_empty() && !req.targets.agent_ids.contains(&agent.agent_id) {
            continue;
        }
        for config in agent.config_surfaces {
            if config.editable {
                diffs.push(serde_json::json!({
                    "agent_id": agent.agent_id,
                    "file_path": config.path_redacted,
                    "diff": "+ command: \"dek-stdio-wrapper\"\n- command: <original MCP command>",
                    "backup_supported": config.backup_supported
                }));
            }
        }
    }
    let warnings = if diffs.is_empty() {
        vec!["No editable MCP config surfaces were found. Run Auto Discovery or use manual wrapper setup."]
    } else {
        Vec::<&str>::new()
    };

    Ok(Json(serde_json::json!({
        "status": "preview",
        "diffs": diffs,
        "warnings": warnings
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
    let mut failed_bindings = 0usize;
    for binding in &mut deployment.control_bindings {
        let config_path = binding
            .binding_json
            .get("config_path")
            .and_then(|value| value.as_str());

        let result = if let Some(config_path) = config_path {
            crate::control_binding::apply_binding_to_config(
                &binding.binding_id,
                std::path::Path::new(config_path),
            )
            .await
        } else {
            Err("binding has no config_path; cannot apply wrapper to a real MCP config".to_string())
        };

        match result {
            Ok(backup_path) => {
                binding.status = BindingStatus::Applied;
                binding.config_backup_id = Some(backup_path.to_string_lossy().to_string());
                if let Some(obj) = binding.binding_json.as_object_mut() {
                    obj.insert("applied_for_real".to_string(), serde_json::json!(true));
                    obj.insert(
                        "backup_path".to_string(),
                        serde_json::json!(backup_path.to_string_lossy()),
                    );
                }
            }
            Err(error) => {
                failed_bindings += 1;
                binding.status = BindingStatus::Failed;
                if let Some(obj) = binding.binding_json.as_object_mut() {
                    obj.insert("applied_for_real".to_string(), serde_json::json!(false));
                    obj.insert("error".to_string(), serde_json::json!(error));
                    obj.insert(
                        "required_action".to_string(),
                        serde_json::json!(
                            "Provide a real MCP config_path or use the manual wrapper instructions."
                        ),
                    );
                }
            }
        }
    }
    deployment.status = if failed_bindings == 0 {
        DeploymentStatus::Active
    } else {
        DeploymentStatus::Failed
    };

    let event = serde_json::json!({
        "schema_version": "pollek.telemetry.v2",
        "event_type": "policy_deployment",
        "event_id": format!("evt_{}", Uuid::new_v4()),
        "tenant_id": tenant,
        "deployment_id": deployment.deployment_id,
        "preset_id": deployment.preset_id,
        "status": if failed_bindings == 0 { "applied" } else { "failed" },
        "failed_bindings": failed_bindings,
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
