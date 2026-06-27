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
            "/v1/tenants/:tenant/discovery/entities",
            get(list_discovery_entities),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate_id/capabilities",
            get(get_candidate_capabilities),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate_id/retrieve-capabilities",
            post(retrieve_candidate_capabilities),
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

    let learned_signature =
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

    if let Some(sig) = learned_signature {
        let sig_value =
            serde_json::to_value(&sig).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
        st.registry_store
            .upsert_raw(&tenant, "learned_signature", &sig.id, &sig_value)
            .await
            .map_err(ApiError::Internal)?;
    }

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
        let running_job = serde_json::json!({
            "scan_id": scan_id2,
            "tenant_id": tenant2,
            "status": "running",
            "started_at": chrono::Utc::now().to_rfc3339(),
            "sources": req.get("sources").unwrap_or(&serde_json::json!([])),
            "candidates_found": 0
        });
        let _ = st2
            .registry_store
            .upsert_raw(&tenant2, "discovery_scan", &scan_id2, &running_job)
            .await;

        let sni_source = std::sync::Arc::new(SpoolFlowSourceImpl::new());
        let (tx, mut rx) = tokio::sync::mpsc::channel::<
            dek_agent_discovery::model::DiscoveredAgentCandidateV2,
        >(100);
        let st3 = st2.clone();
        let tenant3 = tenant2.clone();

        // Spawn a receiver to handle incremental candidates
        let receiver_task = tokio::spawn(async move {
            while let Some(mut candidate) = rx.recv().await {
                if let Err(error) =
                    merge_and_persist_candidate(&st3, &tenant3, &mut candidate).await
                {
                    tracing::warn!(
                        %error,
                        candidate_id = %candidate.candidate_id,
                        "failed to persist incremental discovery candidate"
                    );
                }
            }
        });

        let scan_result = dek_agent_discovery::run_scan_v2(
            &tenant2,
            &scan_id2,
            &req,
            Some(sni_source),
            Some(tx),
            st2.def_store.get(),
        )
        .await;
        let _ = receiver_task.await;

        match scan_result {
            Ok((job, candidates)) => {
                for mut candidate in candidates {
                    if let Err(error) =
                        merge_and_persist_candidate(&st2, &tenant2, &mut candidate).await
                    {
                        tracing::warn!(
                            %error,
                            candidate_id = %candidate.candidate_id,
                            scan_id = %job.scan_id,
                            "failed to persist final discovery candidate snapshot"
                        );
                    }
                }
                let job_val = serde_json::to_value(&job).unwrap_or_default();
                let _ = st2
                    .registry_store
                    .upsert_raw(&tenant2, "discovery_scan", &job.scan_id, &job_val)
                    .await;
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

async fn merge_and_persist_candidate(
    st: &AppState,
    tenant: &str,
    candidate: &mut dek_agent_discovery::model::DiscoveredAgentCandidateV2,
) -> anyhow::Result<()> {
    if let Some(existing_raw) = st
        .registry_store
        .get_raw(tenant, "discovery_candidate", &candidate.candidate_id)
        .await?
    {
        if let Ok(existing) = serde_json::from_value::<
            dek_agent_discovery::model::DiscoveredAgentCandidateV2,
        >(existing_raw)
        {
            candidate.first_seen = existing.first_seen;
            for scan_id in existing.scan_ids {
                if !candidate.scan_ids.iter().any(|id| id == &scan_id) {
                    candidate.scan_ids.push(scan_id);
                }
            }
            if matches!(
                existing.status,
                dek_agent_discovery::model::DiscoveryStatus::Registered
                    | dek_agent_discovery::model::DiscoveryStatus::Ignored
            ) {
                candidate.status = existing.status;
                candidate.display_name = existing.display_name;
                candidate.suggested_registration.name = existing.suggested_registration.name;
            }
        }
    }

    let val = serde_json::to_value(&*candidate)?;
    st.registry_store
        .upsert_raw(tenant, "discovery_candidate", &candidate.candidate_id, &val)
        .await?;
    Ok(())
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

async fn list_discovery_entities(
    Path(tenant): Path<String>,
    Query(query): Query<PaginationQuery>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut candidates = Vec::new();
    for raw in st
        .registry_store
        .list_raw(&tenant, "discovery_candidate")
        .await
        .map_err(ApiError::Internal)?
    {
        if let Ok(candidate) =
            serde_json::from_value::<dek_agent_discovery::model::DiscoveredAgentCandidateV2>(raw)
        {
            candidates.push(candidate);
        }
    }

    let mut entities =
        dek_agent_discovery::capability_inventory::entities_for_candidates(&candidates);
    entities.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));

    let limit = query.limit.unwrap_or(100);
    let cursor = query.cursor.unwrap_or(0);
    let total = entities.len();
    let items: Vec<_> = entities.into_iter().skip(cursor).take(limit).collect();

    Ok(Json(serde_json::json!({
        "schema_version": "discovery-entity-list.v1",
        "entities": items,
        "next_cursor": if cursor + limit < total { Some(cursor + limit) } else { None },
        "total": total
    })))
}

async fn get_candidate_capabilities(
    Path((tenant, candidate_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let candidate = load_candidate(&st, &tenant, &candidate_id).await?;
    let entity = dek_agent_discovery::capability_inventory::entity_for_candidate(&candidate);
    Ok(Json(capability_inventory_response(&entity, "derived")))
}

async fn retrieve_candidate_capabilities(
    Path((tenant, candidate_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let candidate = load_candidate(&st, &tenant, &candidate_id).await?;
    let entity = dek_agent_discovery::capability_inventory::entity_for_candidate(&candidate);
    persist_discovery_entity(&st, &tenant, &entity).await?;

    Ok(Json(capability_inventory_response(&entity, "persisted")))
}

async fn load_candidate(
    st: &AppState,
    tenant: &str,
    candidate_id: &str,
) -> ApiResult<dek_agent_discovery::model::DiscoveredAgentCandidateV2> {
    let raw = st
        .registry_store
        .get_raw(tenant, "discovery_candidate", candidate_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(candidate_id.to_string()))?;

    serde_json::from_value(raw).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))
}

async fn persist_discovery_entity(
    st: &AppState,
    tenant: &str,
    entity: &dek_agent_discovery::model::DiscoveryEntityCandidate,
) -> ApiResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let entity_value =
        serde_json::to_value(entity).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    st.registry_store
        .upsert_raw(
            tenant,
            "discovery_entity",
            &entity.candidate_id,
            &entity_value,
        )
        .await
        .map_err(ApiError::Internal)?;

    for capability in &entity.capabilities {
        let mut value =
            serde_json::to_value(capability).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert("updated_at".to_string(), serde_json::json!(now.clone()));
        }
        st.registry_store
            .upsert_raw(
                tenant,
                "discovered_capability",
                &capability.capability_id,
                &value,
            )
            .await
            .map_err(ApiError::Internal)?;
    }

    for relationship in &entity.relationships {
        let mut value = serde_json::to_value(relationship)
            .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert("updated_at".to_string(), serde_json::json!(now.clone()));
        }
        st.registry_store
            .upsert_raw(
                tenant,
                "discovered_relationship",
                &relationship.relationship_id,
                &value,
            )
            .await
            .map_err(ApiError::Internal)?;
    }

    Ok(())
}

fn capability_inventory_response(
    entity: &dek_agent_discovery::model::DiscoveryEntityCandidate,
    retrieval_status: &str,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "discovery-capability-inventory.v1",
        "candidate_id": entity.candidate_id,
        "entity": entity,
        "capabilities": entity.capabilities,
        "relationships": entity.relationships,
        "retrieval_status": retrieval_status,
        "source": "local_discovery_metadata",
        "privacy_note": "Discovery capability inventory is derived from metadata already collected by Auto Discovery. It does not invoke MCP tools, read MCP resources, or capture raw prompts/responses."
    })
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
    let entity_count = st
        .registry_store
        .clear_raw(&tenant, "discovery_entity")
        .await
        .map_err(ApiError::Internal)?;
    let capability_count = st
        .registry_store
        .clear_raw(&tenant, "discovered_capability")
        .await
        .map_err(ApiError::Internal)?;
    let relationship_count = st
        .registry_store
        .clear_raw(&tenant, "discovered_relationship")
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(serde_json::json!({
        "status": "cleared",
        "deleted_count": count,
        "deleted_entities": entity_count,
        "deleted_capabilities": capability_count,
        "deleted_relationships": relationship_count
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
        clear_candidate_inventory(&st, &tenant, &candidate_id).await?;
        Ok(Json(serde_json::json!({ "status": "deleted" })))
    } else {
        Err(ApiError::NotFound(candidate_id))
    }
}

async fn clear_candidate_inventory(
    st: &AppState,
    tenant: &str,
    candidate_id: &str,
) -> ApiResult<()> {
    let _ = st
        .registry_store
        .delete_raw(tenant, "discovery_entity", candidate_id)
        .await
        .map_err(ApiError::Internal)?;

    for capability in st
        .registry_store
        .list_raw(tenant, "discovered_capability")
        .await
        .map_err(ApiError::Internal)?
    {
        if capability
            .get("candidate_id")
            .and_then(|value| value.as_str())
            == Some(candidate_id)
        {
            if let Some(capability_id) = capability
                .get("capability_id")
                .and_then(|value| value.as_str())
            {
                let _ = st
                    .registry_store
                    .delete_raw(tenant, "discovered_capability", capability_id)
                    .await
                    .map_err(ApiError::Internal)?;
            }
        }
    }

    for relationship in st
        .registry_store
        .list_raw(tenant, "discovered_relationship")
        .await
        .map_err(ApiError::Internal)?
    {
        if relationship
            .get("subject_candidate_id")
            .and_then(|value| value.as_str())
            == Some(candidate_id)
        {
            if let Some(relationship_id) = relationship
                .get("relationship_id")
                .and_then(|value| value.as_str())
            {
                let _ = st
                    .registry_store
                    .delete_raw(tenant, "discovered_relationship", relationship_id)
                    .await
                    .map_err(ApiError::Internal)?;
            }
        }
    }

    Ok(())
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

    let agent = dek_agent_discovery::to_registry_agent_v2(&tenant, &candidate, &req)
        .map_err(ApiError::Internal)?;

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
        "agent_name": registered.name,
        "capabilities": registered.capabilities,
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
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let candidate = load_candidate(&st, &tenant, &candidate_id).await?;
    let plan_id = format!("plan_{}", uuid::Uuid::new_v4());
    let stdio_server = candidate
        .discovered_mcp_servers
        .iter()
        .find(|server| server.transport == "stdio" && server.command.is_some());
    let config_paths = candidate
        .suggested_registration
        .mcp_stdio_config_paths
        .clone();
    let wrapper_command = stdio_server.and_then(|server| {
        server.command.as_ref().map(|command| {
            format!(
                "dek-stdio-wrapper --tenant {} --agent {} --target-cmd {}",
                tenant, candidate_id, command
            )
        })
    });

    Ok(Json(serde_json::json!({
        "candidate_id": candidate_id,
        "control_plan_id": plan_id,
        "status": if wrapper_command.is_some() || !config_paths.is_empty() { "generated" } else { "manual_input_required" },
        "plan": {
            "strategy": "stdio_wrapper",
            "instructions": if config_paths.is_empty() {
                "No editable MCP config path was captured. Start the agent through dek-stdio-wrapper manually or rescan with MCP config access enabled."
            } else {
                "Apply the wrapper to one of the discovered MCP config files, then restart the agent host."
            },
            "wrapper_command": wrapper_command,
            "config_paths": config_paths
        }
    })))
}
