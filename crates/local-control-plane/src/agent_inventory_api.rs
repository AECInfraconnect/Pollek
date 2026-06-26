// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use dek_domain_schema::AgentCapabilityInventory;

use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tenants/:tenant/agent-inventory", get(list_inventory))
        .route(
            "/v1/tenants/:tenant/agent-inventory/:agent_id",
            get(get_inventory),
        )
        .route(
            "/v1/tenants/:tenant/agent-inventory/rebuild",
            post(rebuild_inventory),
        )
        .route(
            "/v1/tenants/:tenant/agents/:agent_id/register",
            post(register_agent),
        )
}

async fn list_inventory(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<Json<Vec<AgentCapabilityInventory>>> {
    let items = st
        .registry_store
        .list_agent_inventories(&tenant)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(items))
}

async fn get_inventory(
    Path((tenant, agent_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<AgentCapabilityInventory>> {
    let item = st
        .registry_store
        .get_agent_inventory(&tenant, &agent_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(agent_id))?;
    Ok(Json(item))
}

async fn rebuild_inventory(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let candidates_raw = st
        .registry_store
        .list_raw(&tenant, "discovery_candidate")
        .await
        .map_err(ApiError::Internal)?;

    let mut rebuilt_count = 0;

    for raw in candidates_raw {
        if let Ok(candidate) =
            serde_json::from_value::<dek_agent_discovery::model::DiscoveredAgentCandidateV2>(raw)
        {
            let agent_kind_str = serde_json::to_string(&candidate.inferred_agent_type)
                .unwrap_or_else(|_| "\"UnknownAiProcess\"".to_string());
            let agent_kind: dek_domain_schema::AgentKind = serde_json::from_str(&agent_kind_str)
                .unwrap_or(dek_domain_schema::AgentKind::UnknownAiProcess);

            let mut mcp_surfaces = Vec::new();
            for mcp in &candidate.discovered_mcp_servers {
                mcp_surfaces.push(dek_domain_schema::McpSurface {
                    server_name: mcp.server_name.clone(),
                    client_hint: "discovered".to_string(),
                    transport: dek_domain_schema::McpTransportKind::Stdio,
                    command_template: mcp.command.clone().map(|c| vec![c]),
                    endpoint_domain: None,
                    has_auth_header: false,
                    env_key_names: vec![],
                    tools_known: vec![],
                    resources_known: vec![],
                });
            }

            let inv = dek_domain_schema::AgentCapabilityInventory {
                schema_version: "agent-capability-inventory.v1".to_string(),
                tenant_id: tenant.clone(),
                device_id: candidate.device_id.clone(),
                agent_id: candidate.candidate_id.clone(), // using candidate_id as agent_id for discovered but unregistered
                candidate_id: Some(candidate.candidate_id.clone()),
                display_name: candidate.display_name.clone(),
                agent_type: agent_kind,
                trust_level: candidate.suggested_registration.trust_level.clone(),
                confidence: candidate.confidence,
                risk_score: candidate.risk_score,
                process: None,
                config_surfaces: vec![],
                mcp_surfaces,
                model_endpoints: vec![],
                browser_surfaces: vec![],
                file_surfaces: vec![],
                network_surfaces: vec![],
                supported_pep_bindings: vec![],
                supported_pdp_routes: vec![],
                telemetry_capabilities: dek_domain_schema::TelemetryCapabilities {
                    emits_tool_logs: false,
                    emits_resource_logs: false,
                    emits_decision_logs: false,
                    emits_network_logs: false,
                    format: "pollek".to_string(),
                },
                last_scan_id: "rebuild".to_string(),
                last_seen_at: chrono::Utc::now().to_rfc3339(),
            };

            let _ = st.registry_store.upsert_agent_inventory(inv).await;
            rebuilt_count += 1;
        }
    }

    Ok(Json(
        serde_json::json!({"status": "rebuilt", "count": rebuilt_count}),
    ))
}

#[derive(serde::Deserialize)]
pub struct RegisterRequest {
    pub level: String,
}

async fn register_agent(
    Path((tenant, agent_id)): Path<(String, String)>,
    State(st): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // 1. Fetch capability inventory (shadow candidate)
    let inv_opt = match st.registry_store.list_agent_inventories(&tenant).await {
        Ok(list) => list.into_iter().find(|a| a.agent_id == agent_id),
        Err(_) => None,
    };

    let name = inv_opt
        .as_ref()
        .map(|i| i.display_name.clone())
        .unwrap_or_else(|| format!("Agent {}", agent_id));
    let agent_type = dek_control_plane_api::registry::AgentType::CustomMcpClient; // fallback, since the schema enum differs

    // 2. Promote to managed agent
    let agent = dek_control_plane_api::registry::AiAgent {
        meta: dek_control_plane_api::registry::ObjectMeta {
            schema_version: "agent.v1".into(),
            tenant_id: tenant.clone(),
            workspace_id: "default".into(),
            environment_id: "local".into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            created_by: "system".into(),
            updated_by: "system".into(),
            source: dek_control_plane_api::registry::RegistrationSource::Manual,
            status: dek_control_plane_api::registry::RegistryStatus::Registered,
            tags: vec![req.level.clone()],
        },
        agent_id: agent_id.clone(),
        name,
        agent_type,
        vendor: None,
        runtime: dek_control_plane_api::registry::AgentRuntime {
            runtime_name: "unknown".into(),
            version: None,
        },
        entrypoints: vec![],
        declared_tools: vec![],
        declared_resources: vec![],
        identity: dek_control_plane_api::registry::AgentIdentity {
            spiffe_id: None,
            process_path: None,
            user_subject: None,
            signing_key_fingerprint: None,
            token_bindings: vec![],
        },
        trust_level: dek_control_plane_api::registry::TrustLevel::Untrusted,
        capabilities: vec![],
        labels: std::collections::HashMap::new(),
    };

    let _ = st.registry_store.upsert_agent(agent).await;

    // 3. Setup preset policy binding
    let policy_id = format!("preset_{}", req.level.to_lowercase());
    let policy = dek_control_plane_api::policy::PolicyDraft {
        meta: dek_control_plane_api::registry::ObjectMeta {
            schema_version: "policy.v1".into(),
            tenant_id: tenant.clone(),
            workspace_id: "default".into(),
            environment_id: "local".into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            created_by: "system".into(),
            updated_by: "system".into(),
            source: dek_control_plane_api::registry::RegistrationSource::Manual,
            status: dek_control_plane_api::registry::RegistryStatus::Active,
            tags: vec![],
        },
        policy_id: policy_id.clone(),
        name: format!("Auto-generated {} policy", req.level),
        description: Some(format!("Auto-generated {} policy", req.level)),
        policy_type: dek_control_plane_api::policy::PolicyType::Cedar,
        targets: dek_control_plane_api::policy::PolicyTargets {
            agent_ids: vec![agent_id.clone()],
            tool_ids: vec![],
            resource_ids: vec![],
            entity_ids: vec![],
            route_ids: vec![],
        },
        source: dek_control_plane_api::policy::PolicySource::RawText {
            language: "cedar".into(),
            text: "// default allow".into(),
        },
        compile_options: dek_control_plane_api::policy::PolicyCompileOptions {
            optimization_level: None,
            fail_on_warnings: Some(false),
        },
    };
    let _ = st.policy_store.upsert_policy(policy).await;

    // Optional: Audit log would go here.

    Ok(Json(serde_json::json!({
        "status": "registered",
        "tenant": tenant,
        "agent_id": agent_id,
        "control_level": req.level,
        "bound_policy": policy_id
    })))
}
