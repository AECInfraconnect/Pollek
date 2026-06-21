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
        .route(
            "/v1/tenants/:tenant/discovery/scans",
            post(start_scan).get(list_scans),
        )
        .route(
            "/v1/tenants/:tenant/discovery/scans/:scan_id",
            get(get_scan_status),
        )
        .route(
            "/v1/tenants/:tenant/discovery/scans/:scan_id/cancel",
            post(cancel_scan),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates",
            get(list_candidates),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate/register",
            post(register_candidate),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate_id/control-plan",
            post(generate_control_plan),
        )
        .route(
            "/v1/tenants/:tenant/discovery/control-bindings/:binding_id/apply",
            post(apply_control_binding),
        )
        .route(
            "/v1/tenants/:tenant/discovery/control-bindings/:binding_id/rollback",
            post(rollback_control_binding),
        )
}

async fn start_scan(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let scan_id = format!("scan_{}", uuid::Uuid::new_v4());
    let st2 = st.clone();
    let tenant2 = tenant.clone();
    let scan_id2 = scan_id.clone();

    tokio::spawn(async move {
        match dek_agent_discovery::run_scan_v2(&tenant2, &req).await {
            Ok((job, candidates)) => {
                let job_val = serde_json::to_value(&job).unwrap_or_default();
                let _ = st2
                    .registry_store
                    .upsert_raw(&tenant2, "discovery_scan", &job.scan_id, &job_val)
                    .await;

                for c in candidates {
                    let val = serde_json::to_value(&c).unwrap_or_default();
                    let _ = st2
                        .registry_store
                        .upsert_raw(&tenant2, "discovery_candidate", &c.candidate_id, &val)
                        .await;
                }
            }
            Err(e) => {
                tracing::warn!(error=%e, scan_id=%scan_id2, "agent discovery scan failed");
                let job = serde_json::json!({
                    "scan_id": scan_id2,
                    "tenant_id": tenant2,
                    "status": "failed",
                    "error": e.to_string(),
                });
                let _ = st2
                    .registry_store
                    .upsert_raw(&tenant2, "discovery_scan", &scan_id2, &job)
                    .await;
            }
        }
    });

    Ok(Json(serde_json::json!({
        "schema_version": "agent-discovery-scan-response.v1",
        "scan_id": scan_id,
        "status": "queued"
    })))
}

async fn list_candidates(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let items = st
        .registry_store
        .list_raw(&tenant, "discovery_candidate")
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(serde_json::json!({
        "schema_version": "agent-discovery-candidate-list.v1",
        "candidates": items
    })))
}

async fn register_candidate(
    Path((tenant, candidate_id)): Path<(String, String)>,
    State(st): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let raw = st
        .registry_store
        .get_raw(&tenant, "discovery_candidate", &candidate_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(candidate_id.clone()))?;

    let candidate: dek_agent_discovery::model::DiscoveredAgentCandidateV2 =
        serde_json::from_value(raw).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;

    let agent = dek_agent_discovery::to_registry_agent_v2(&tenant, &candidate, &req)
        .map_err(ApiError::Internal)?;

    let registered = st
        .registry_store
        .upsert_agent(agent)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(serde_json::json!({
        "schema_version": "register-agent-candidate-response.v1",
        "agent_id": registered.agent_id,
        "status": "registered"
    })))
}

async fn get_scan_status(
    Path((tenant, scan_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let raw = st
        .registry_store
        .get_raw(&tenant, "discovery_scan", &scan_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(scan_id.clone()))?;

    Ok(Json(raw))
}

async fn list_scans(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let items = st
        .registry_store
        .list_raw(&tenant, "discovery_scan")
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(serde_json::json!({
        "schema_version": "agent-discovery-scan-list.v1",
        "scans": items
    })))
}

async fn cancel_scan(
    Path((tenant, scan_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let raw = st
        .registry_store
        .get_raw(&tenant, "discovery_scan", &scan_id)
        .await
        .map_err(ApiError::Internal)?;

    if let Some(mut scan_val) = raw {
        if scan_val.get("status").and_then(|v| v.as_str()) == Some("queued")
            || scan_val.get("status").and_then(|v| v.as_str()) == Some("running")
        {
            if let Some(obj) = scan_val.as_object_mut() {
                obj.insert("status".to_string(), serde_json::json!("cancelled"));
            }
            let _ = st
                .registry_store
                .upsert_raw(&tenant, "discovery_scan", &scan_id, &scan_val)
                .await;
        }
        Ok(Json(scan_val))
    } else {
        Err(ApiError::NotFound(scan_id))
    }
}

async fn generate_control_plan(
    Path((tenant, candidate_id)): Path<(String, String)>,
    State(_st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let plan_id = format!("plan_{}", uuid::Uuid::new_v4());

    // In a real scenario we'd lookup the candidate to find its original command
    let wrapper_cmd = format!(
        "dek-stdio-wrapper --tenant {} --agent {} --target-cmd <ORIGINAL_CMD> -- <ORIGINAL_ARGS>",
        tenant, candidate_id
    );

    Ok(Json(serde_json::json!({
        "candidate_id": candidate_id,
        "control_plan_id": plan_id,
        "status": "generated",
        "plan": {
            "strategy": "stdio_wrapper",
            "instructions": "Replace your original agent start command with the wrapper command provided.",
            "wrapper_command": wrapper_cmd
        }
    })))
}

async fn apply_control_binding(
    Path((_tenant, binding_id)): Path<(String, String)>,
    State(_st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    // Stub for Phase 6 implementation
    Ok(Json(serde_json::json!({
        "binding_id": binding_id,
        "status": "applied",
    })))
}

async fn rollback_control_binding(
    Path((_tenant, binding_id)): Path<(String, String)>,
    State(_st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    // Stub for Phase 6 implementation
    Ok(Json(serde_json::json!({
        "binding_id": binding_id,
        "status": "rolled_back",
    })))
}
