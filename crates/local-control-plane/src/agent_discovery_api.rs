use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<usize>,
    pub cursor: Option<usize>,
}

use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};

struct SpoolFlowSourceImpl {
    spooler: Option<dek_telemetry::spooler::Spooler>,
}

impl SpoolFlowSourceImpl {
    fn new() -> Self {
        let db_path = dek_config::paths::get_data_dir().join("telemetry_spool.db");
        Self {
            spooler: dek_telemetry::spooler::Spooler::new(&db_path.to_string_lossy()).ok(),
        }
    }
}

impl dek_agent_discovery::web_ai_scan::SniFlowSource for SpoolFlowSourceImpl {
    fn recent_flows(
        &self,
        _since: std::time::Duration,
    ) -> Vec<dek_agent_discovery::web_ai_scan::SniFlow> {
        let mut flows = Vec::new();
        if let Some(spool) = &self.spooler {
            if let Ok(batch) = spool.peek_recent(500) {
                for (_, val) in batch {
                    if val.get("event").and_then(|v| v.as_str()) == Some("network.flow.v1") {
                        if let Some(sni_host) = val.get("sni_host").and_then(|v| v.as_str()) {
                            let browser_pid =
                                val.get("pid").and_then(|v| v.as_u64()).map(|v| v as u32);
                            flows.push(dek_agent_discovery::web_ai_scan::SniFlow {
                                browser_pid,
                                sni_host: sni_host.to_string(),
                                ts: 0,
                            });
                        }
                    }
                }
            }
        }
        flows
    }
}

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
            get(list_candidates).delete(clear_candidates),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate_id",
            axum::routing::delete(delete_candidate),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate/register",
            post(register_candidate),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate_id/confirm",
            post(confirm_candidate),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate_id/control-plan",
            post(generate_control_plan),
        )
        .route(
            "/v1/tenants/:tenant/discovery/control-bindings/:binding_id/apply",
            post(crate::control_binding::apply_control_binding),
        )
        .route(
            "/v1/tenants/:tenant/discovery/control-bindings/:binding_id/rollback",
            post(crate::control_binding::rollback_control_binding),
        )
}

async fn confirm_candidate(
    Path((tenant, candidate_id)): Path<(String, String)>,
    State(st): State<AppState>,
    Json(req): Json<dek_agent_discovery::human_loop::IdentityConfirmation>,
) -> ApiResult<Json<serde_json::Value>> {
    let raw = st
        .registry_store
        .get_raw(&tenant, "discovery_candidate", &candidate_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(candidate_id.clone()))?;

    let mut candidate: dek_agent_discovery::model::DiscoveredAgentCandidateV2 =
        serde_json::from_value(raw).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;

    if candidate.status == dek_agent_discovery::model::DiscoveryStatus::Registered {
        return Err(ApiError::BadRequest(
            "Agent is already registered".to_string(),
        ));
    }

    let _learned_signature =
        dek_agent_discovery::human_loop::apply_confirmation(&mut candidate, &req);

    // Map capabilities to preset
    let risk = candidate.confidence;
    let preset_id =
        dek_policy_presets::catalog::preset_for_capabilities(&req.confirmed_capability_tags, risk);

    let updated_candidate_value =
        serde_json::to_value(&candidate).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;

    st.registry_store
        .upsert_raw(
            &tenant,
            "discovery_candidate",
            &candidate_id,
            &updated_candidate_value,
        )
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(serde_json::json!({
        "schema_version": "confirm-agent-candidate-response.v1",
        "candidate_id": candidate_id,
        "status": "confirmed",
        "applied_preset": preset_id,
    })))
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

    let initial_job = serde_json::json!({
        "scan_id": scan_id,
        "tenant_id": tenant,
        "status": "queued",
        "started_at": chrono::Utc::now().to_rfc3339(),
        "sources": req.get("sources").unwrap_or(&serde_json::json!([])),
        "candidates_found": 0
    });
    let _ = st
        .registry_store
        .upsert_raw(&tenant, "discovery_scan", &scan_id, &initial_job)
        .await;

    tokio::spawn(async move {
        let sni_source = std::sync::Arc::new(SpoolFlowSourceImpl::new());
        let (tx, mut rx) = tokio::sync::mpsc::channel::<
            dek_agent_discovery::model::DiscoveredAgentCandidateV2,
        >(100);
        let st3 = st2.clone();
        let tenant3 = tenant2.clone();

        // Spawn a receiver to handle incremental candidates
        tokio::spawn(async move {
            while let Some(candidate) = rx.recv().await {
                let val = serde_json::to_value(&candidate).unwrap_or_default();
                let _ = st3
                    .registry_store
                    .upsert_raw(
                        &tenant3,
                        "discovery_candidate",
                        &candidate.candidate_id,
                        &val,
                    )
                    .await;
            }
        });

        match dek_agent_discovery::run_scan_v2(
            &tenant2,
            &scan_id2,
            &req,
            Some(sni_source),
            Some(tx),
            st2.def_store.get(),
        )
        .await
        {
            Ok((job, _candidates)) => {
                let job_val = serde_json::to_value(&job).unwrap_or_default();
                let _ = st2
                    .registry_store
                    .upsert_raw(&tenant2, "discovery_scan", &job.scan_id, &job_val)
                    .await;

                // Candidates are already saved incrementally by the receiver loop
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
    Query(query): Query<PaginationQuery>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut items = st
        .registry_store
        .list_raw(&tenant, "discovery_candidate")
        .await
        .map_err(ApiError::Internal)?;

    let limit = query.limit.unwrap_or(100);
    let cursor = query.cursor.unwrap_or(0);

    let total = items.len();
    items = items.into_iter().skip(cursor).take(limit).collect();

    Ok(Json(serde_json::json!({
        "schema_version": "agent-discovery-candidate-list.v1",
        "candidates": items,
        "next_cursor": if cursor + limit < total { Some(cursor + limit) } else { None },
        "total": total
    })))
}

async fn clear_candidates(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let count = st
        .registry_store
        .clear_raw(&tenant, "discovery_candidate")
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(serde_json::json!({
        "status": "cleared",
        "deleted_count": count
    })))
}

async fn delete_candidate(
    Path((tenant, candidate_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let deleted = st
        .registry_store
        .delete_raw(&tenant, "discovery_candidate", &candidate_id)
        .await
        .map_err(ApiError::Internal)?;

    if deleted {
        Ok(Json(serde_json::json!({ "status": "deleted" })))
    } else {
        Err(ApiError::NotFound(candidate_id))
    }
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

    let mut candidate: dek_agent_discovery::model::DiscoveredAgentCandidateV2 =
        serde_json::from_value(raw).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;

    if candidate.status == dek_agent_discovery::model::DiscoveryStatus::Registered {
        return Err(ApiError::BadRequest(
            "Agent is already registered".to_string(),
        ));
    }

    let mut agent = dek_agent_discovery::to_registry_agent_v2(&tenant, &candidate, &req)
        .map_err(ApiError::Internal)?;

    // Bind stable identity
    agent.agent_id = candidate.candidate_id.replace("cand_", "agent_");

    // Deduplicate: check if this stable identity is already registered
    if let Ok(Some(_)) = st
        .registry_store
        .get_raw(&tenant, "agent", &agent.agent_id)
        .await
    {
        return Err(ApiError::BadRequest(
            "Agent with this identity is already registered".to_string(),
        ));
    }

    // Schema Validation
    let schema = schemars::schema_for!(dek_control_plane_api::registry::AiAgent);
    let schema_val = serde_json::to_value(&schema).map_err(|e| ApiError::Internal(e.into()))?;
    if let Ok(compiled) = jsonschema::JSONSchema::compile(&schema_val) {
        let agent_val = serde_json::to_value(&agent).map_err(|e| ApiError::Internal(e.into()))?;
        let res = compiled
            .validate(&agent_val)
            .map_err(|errs| errs.map(|e| e.to_string()).collect::<Vec<_>>().join(", "));
        if let Err(msg) = res {
            return Err(ApiError::BadRequest(format!(
                "Schema validation failed: {}",
                msg
            )));
        }
    }

    let registered = st
        .registry_store
        .upsert_agent(agent)
        .await
        .map_err(ApiError::Internal)?;

    // Create AgentBinding preserving discovered capabilities
    if let Some(sig_id) = &candidate.matched_signature_id {
        if let Some(sig) = st
            .def_store
            .get()
            .signatures
            .iter()
            .find(|s| s.id == *sig_id)
            .cloned()
        {
            let mut surfaces = Vec::new();
            for mcp in &candidate.discovered_mcp_servers {
                let s = match mcp.transport.as_str() {
                    "stdio" => dek_agent_binding::capability::Surface::McpStdio {
                        command: mcp.command.clone().unwrap_or_default(),
                        args: vec![],
                    },
                    "http" => dek_agent_binding::capability::Surface::McpHttp {
                        url: mcp.command.clone().unwrap_or_default(),
                    },
                    "sse" => dek_agent_binding::capability::Surface::McpSse {
                        url: mcp.command.clone().unwrap_or_default(),
                    },
                    _ => continue,
                };
                surfaces.push(s);
            }
            let binding = dek_agent_binding::binding::AgentBinding::from_discovery(
                &sig,
                &candidate.candidate_id,
                &tenant,
                &candidate.device_id,
                surfaces,
            );

            let binding_val =
                serde_json::to_value(&binding).map_err(|e| ApiError::Internal(e.into()))?;
            let _ = st
                .registry_store
                .upsert_raw(&tenant, "agent_binding", &binding.binding_id, &binding_val)
                .await;
        }
    }

    candidate.status = dek_agent_discovery::model::DiscoveryStatus::Registered;
    let updated_candidate_value =
        serde_json::to_value(&candidate).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;

    st.registry_store
        .upsert_raw(
            &tenant,
            "discovery_candidate",
            &candidate_id,
            &updated_candidate_value,
        )
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
    Query(query): Query<PaginationQuery>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut items = st
        .registry_store
        .list_raw(&tenant, "discovery_scan")
        .await
        .map_err(ApiError::Internal)?;

    let limit = query.limit.unwrap_or(100);
    let cursor = query.cursor.unwrap_or(0);

    let total = items.len();
    items = items.into_iter().skip(cursor).take(limit).collect();

    Ok(Json(serde_json::json!({
        "schema_version": "agent-discovery-scan-list.v1",
        "scans": items,
        "next_cursor": if cursor + limit < total { Some(cursor + limit) } else { None },
        "total": total
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
