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
        .route("/v1/tenants/:tenant/discovery/scans", post(start_scan))
        .route(
            "/v1/tenants/:tenant/discovery/candidates",
            get(list_candidates),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate/register",
            post(register_candidate),
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
        match dek_agent_discovery::run_scan(&tenant2, &req).await {
            Ok(candidates) => {
                for c in candidates {
                    let val = serde_json::to_value(&c).unwrap_or_default();
                    let _ = st2
                        .registry_store
                        .upsert_raw(&tenant2, "discovery_candidate", &c.candidate_id, &val)
                        .await;
                }
            }
            Err(e) => tracing::warn!(error=%e, scan_id=%scan_id2, "agent discovery scan failed"),
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

    let candidate: dek_agent_discovery::model::DiscoveredAgentCandidate = serde_json::from_value(raw)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;

    let agent = dek_agent_discovery::to_registry_agent(&tenant, &candidate, &req)
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
