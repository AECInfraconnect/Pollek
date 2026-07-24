//! Candidate lifecycle: list, confirm, register, delete/clear, and control-plan generation.
use super::*;

pub(super) async fn confirm_candidate(
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

    reconcile_candidate_registered_status(&st, &tenant, &mut candidate)
        .await
        .map_err(ApiError::Internal)?;
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

pub(super) async fn list_candidates(
    Path(tenant): Path<String>,
    Query(query): Query<PaginationQuery>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let raw_items = st
        .registry_store
        .list_raw(&tenant, "discovery_candidate")
        .await
        .map_err(ApiError::Internal)?;
    let mut items = Vec::new();

    for raw in raw_items {
        match serde_json::from_value::<dek_agent_discovery::model::DiscoveredAgentCandidateV2>(
            raw.clone(),
        ) {
            Ok(mut candidate) => {
                reconcile_candidate_registered_status(&st, &tenant, &mut candidate)
                    .await
                    .map_err(ApiError::Internal)?;
                items.push(
                    serde_json::to_value(candidate)
                        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?,
                );
            }
            Err(error) => {
                tracing::warn!(
                    %error,
                    "skipping incompatible discovery candidate; clear discovery history to remove stale development records"
                );
            }
        }
    }

    let limit = query.limit.unwrap_or(100);
    let cursor = query.cursor.unwrap_or(0);

    let total = items.len();
    items = items.into_iter().skip(cursor).take(limit).collect();

    Ok(Json(serde_json::json!({
        "schema_version": "agent-discovery-candidate-list.v1",
        "candidates": items.clone(),
        "items": items,
        "next_cursor": if cursor + limit < total { Some(cursor + limit) } else { None },
        "total": total
    })))
}

pub(super) async fn list_discovery_entities(
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

pub(super) async fn clear_candidates(
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

pub(super) async fn delete_candidate(
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

pub(super) async fn register_candidate(
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

    reconcile_candidate_registered_status(&st, &tenant, &mut candidate)
        .await
        .map_err(ApiError::Internal)?;
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
    if let Ok(compiled) = jsonschema::validator_for(&schema_val) {
        let agent_val = serde_json::to_value(&agent).map_err(|e| ApiError::Internal(e.into()))?;
        let res = compiled.validate(&agent_val).map_err(|err| err.to_string());
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
    candidate.labels.insert(
        "registered_agent_id".to_string(),
        registered.agent_id.clone(),
    );
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

pub(super) async fn generate_control_plan(
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
