use crate::{state::AppState, store::AiUsageQuery};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

mod friendly;
mod model;
use friendly::*;
use model::*;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tenants/:tenant/entity-graph", get(entity_graph))
        .route(
            "/v1/tenants/:tenant/entity-graph/node",
            get(entity_360_query),
        )
        .route(
            "/v1/tenants/:tenant/entity-graph/nodes/:entity_type/:entity_id",
            get(entity_360),
        )
        .route(
            "/v1/tenants/:tenant/entity-graph/activity",
            get(activity_timeline),
        )
        .route(
            "/v1/tenants/:tenant/entity-graph/policy-impact/:policy_id",
            get(policy_impact),
        )
        .route(
            "/v1/tenants/:tenant/activity-timeline",
            get(activity_timeline),
        )
        .route(
            "/v1/tenants/:tenant/user-friendly-activity",
            get(user_friendly_activity).delete(clear_user_friendly_activity),
        )
        .route("/v1/tenants/:tenant/graph/entities", get(entity_graph))
        .route("/v1/tenants/:tenant/graph/entity", get(entity_360_query))
        .route(
            "/v1/tenants/:tenant/graph/entities/:entity_type/:entity_id",
            get(entity_360),
        )
        .route("/v1/tenants/:tenant/graph/activity", get(activity_timeline))
        .route(
            "/v1/tenants/:tenant/graph/policy-impact/:policy_id",
            get(policy_impact),
        )
}

#[derive(Debug, Clone)]
struct ReadModel {
    graph: EntityGraphResponse,
    activity: Vec<ActivityTimelineItem>,
}

#[derive(Default)]
struct GraphBuilder {
    nodes: BTreeMap<String, GraphNode>,
    edges: BTreeMap<String, GraphEdge>,
    warnings: Vec<GraphWarning>,
}

impl GraphBuilder {
    fn add_node(&mut self, node: GraphNode) {
        self.nodes
            .entry(node.id.clone())
            .and_modify(|existing| {
                if existing.status == "observed" && node.status != "observed" {
                    *existing = node.clone();
                }
            })
            .or_insert(node);
    }

    fn ensure_node(&mut self, node_type: &str, entity_id: &str, label: &str, evidence: &str) {
        let node_type = normalize_type(node_type);
        let id = node_key(&node_type, entity_id);
        if self.nodes.contains_key(&id) {
            return;
        }
        self.add_node(GraphNode {
            id,
            node_type: node_type.clone(),
            entity_id: entity_id.to_string(),
            label: label.to_string(),
            subtitle: Some(evidence.to_string()),
            status: "observed".to_string(),
            risk: None,
            mode: Some("observe".to_string()),
            badges: vec!["Observed".to_string()],
            metrics: Vec::new(),
            href: route_for(&node_type, entity_id),
            raw: None,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn add_edge(
        &mut self,
        source_type: &str,
        source_id: &str,
        target_type: &str,
        target_id: &str,
        relation: &str,
        evidence: &str,
        observed: bool,
        enforced: bool,
    ) {
        if source_id.is_empty() || target_id.is_empty() {
            return;
        }
        let source_type = normalize_type(source_type);
        let target_type = normalize_type(target_type);
        self.ensure_node(
            &source_type,
            source_id,
            source_id,
            "Referenced by relationship data",
        );
        self.ensure_node(
            &target_type,
            target_id,
            target_id,
            "Referenced by relationship data",
        );
        let source = node_key(&source_type, source_id);
        let target = node_key(&target_type, target_id);
        let id = format!("{source}->{target}:{relation}");
        self.edges.entry(id.clone()).or_insert(GraphEdge {
            id,
            source,
            target,
            relation: relation.to_string(),
            label: edge_label(relation),
            evidence: evidence.to_string(),
            observed,
            enforced,
        });
    }

    fn finish(self, tenant_id: &str, center: Option<GraphNode>) -> EntityGraphResponse {
        let mut nodes: Vec<_> = self.nodes.into_values().collect();
        nodes.sort_by(|a, b| {
            a.node_type
                .cmp(&b.node_type)
                .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
        });
        let mut edges: Vec<_> = self.edges.into_values().collect();
        edges.sort_by(|a, b| a.id.cmp(&b.id));
        let summaries = summaries_from_nodes_edges(&nodes, &edges);
        let mut warnings = self.warnings;
        warnings.extend(coverage_warnings(&nodes, &edges));
        EntityGraphResponse {
            schema_version: "entity-graph.v1".to_string(),
            tenant_id: tenant_id.to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            center,
            nodes,
            edges,
            summaries,
            warnings,
        }
    }
}

async fn entity_graph(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Query(query): Query<GraphQuery>,
) -> impl IntoResponse {
    match build_read_model(&state, &tenant).await {
        Ok(model) => {
            let graph = filter_graph(model.graph, &query);
            (StatusCode::OK, Json(json!(graph)))
        }
        Err(err) => internal_error(err),
    }
}

async fn entity_360(
    State(state): State<AppState>,
    Path((tenant, entity_type, entity_id)): Path<(String, String, String)>,
) -> impl IntoResponse {
    entity_360_response(state, tenant, entity_type, entity_id).await
}

async fn entity_360_query(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Query(query): Query<EntityNodeQuery>,
) -> impl IntoResponse {
    entity_360_response(state, tenant, query.entity_type, query.entity_id).await
}

async fn entity_360_response(
    state: AppState,
    tenant: String,
    entity_type: String,
    entity_id: String,
) -> (StatusCode, Json<Value>) {
    match build_read_model(&state, &tenant).await {
        Ok(model) => match build_entity_360(model, &tenant, &entity_type, &entity_id) {
            Some(response) => (StatusCode::OK, Json(json!(response))),
            None => (
                StatusCode::NOT_FOUND,
                Json(
                    json!({"error": "entity not found", "entity_type": entity_type, "entity_id": entity_id}),
                ),
            ),
        },
        Err(err) => internal_error(err),
    }
}

async fn policy_impact(
    State(state): State<AppState>,
    Path((tenant, policy_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match build_read_model(&state, &tenant).await {
        Ok(model) => match build_entity_360(model, &tenant, "policy", &policy_id) {
            Some(response) => (StatusCode::OK, Json(json!(response))),
            None => (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "policy not found", "policy_id": policy_id})),
            ),
        },
        Err(err) => internal_error(err),
    }
}

async fn activity_timeline(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Query(query): Query<ActivityQuery>,
) -> impl IntoResponse {
    match build_read_model(&state, &tenant).await {
        Ok(model) => {
            let limit = query.limit.unwrap_or(100).min(500);
            let mut items: Vec<_> = model
                .activity
                .into_iter()
                .filter(|item| activity_matches(item, &query))
                .collect();
            items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            items.truncate(limit);
            (
                StatusCode::OK,
                Json(json!(ActivityTimelineResponse {
                    schema_version: "activity-timeline.v1".to_string(),
                    tenant_id: tenant,
                    generated_at: chrono::Utc::now().to_rfc3339(),
                    items,
                    next_cursor: None,
                })),
            )
        }
        Err(err) => internal_error(err),
    }
}

async fn user_friendly_activity(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Query(query): Query<ActivityQuery>,
) -> impl IntoResponse {
    match build_read_model(&state, &tenant).await {
        Ok(model) => {
            let limit = query.limit.unwrap_or(100).min(500);
            let mut items: Vec<_> = model
                .activity
                .into_iter()
                .filter(|item| activity_matches(item, &query))
                .collect();
            items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            items.truncate(limit);
            let items = items
                .iter()
                .map(user_friendly_activity_from_timeline)
                .collect();
            (
                StatusCode::OK,
                Json(json!(UserFriendlyActivityResponse {
                    schema_version: "user-friendly-activity-list.v1".to_string(),
                    tenant_id: tenant,
                    generated_at: chrono::Utc::now().to_rfc3339(),
                    source: "local-control-plane-read-model".to_string(),
                    items,
                    next_cursor: None,
                })),
            )
        }
        Err(err) => internal_error(err),
    }
}

async fn clear_user_friendly_activity(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
) -> impl IntoResponse {
    let observation_events = match state
        .observability_store
        .clear_observation_events(&tenant)
        .await
    {
        Ok(count) => count,
        Err(err) => return internal_error(err),
    };

    let decision_logs = match state
        .telemetry_store
        .clear_telemetry(&tenant, "decision_log")
        .await
    {
        Ok(count) => count,
        Err(err) => return internal_error(err),
    };

    let decisions = match state
        .telemetry_store
        .clear_telemetry(&tenant, "decision")
        .await
    {
        Ok(count) => count,
        Err(err) => return internal_error(err),
    };
    let guard_incidents = match state
        .telemetry_store
        .clear_telemetry(&tenant, "guard_incident")
        .await
    {
        Ok(count) => count,
        Err(err) => return internal_error(err),
    };
    let guard_events = match state
        .telemetry_store
        .clear_telemetry(&tenant, "guard_event")
        .await
    {
        Ok(count) => count,
        Err(err) => return internal_error(err),
    };
    let plugin_audit = match state
        .telemetry_store
        .clear_telemetry(&tenant, "plugin_audit")
        .await
    {
        Ok(count) => count,
        Err(err) => return internal_error(err),
    };

    (
        StatusCode::OK,
        Json(json!({
            "status": "cleared",
            "scope": "local_activity_history",
            "observation_events": observation_events,
            "decision_logs": decision_logs,
            "decisions": decisions,
            "guard_incidents": guard_incidents,
            "guard_events": guard_events,
            "plugin_audit": plugin_audit
        })),
    )
}

async fn build_read_model(state: &AppState, tenant: &str) -> anyhow::Result<ReadModel> {
    let mut builder = GraphBuilder::default();
    let mut activity = Vec::new();

    let agents = state
        .registry_store
        .list_agents(tenant)
        .await
        .unwrap_or_default();
    let tools = state
        .registry_store
        .list_tools(tenant)
        .await
        .unwrap_or_default();
    let resources = state
        .registry_store
        .list_resources(tenant)
        .await
        .unwrap_or_default();
    let entities = state
        .registry_store
        .list_entities(tenant)
        .await
        .unwrap_or_default();
    let providers = state
        .registry_store
        .list_blackbox_ai(tenant)
        .await
        .unwrap_or_default();
    let relationships = state
        .registry_store
        .list_relationships(tenant)
        .await
        .unwrap_or_default();
    let policies = state
        .policy_store
        .list_policies(tenant)
        .await
        .unwrap_or_default();
    let observations = state
        .observability_store
        .list_observation_events(tenant)
        .await
        .unwrap_or_default();
    let usage_events = state
        .observability_store
        .list_ai_usage_events(AiUsageQuery {
            tenant_id: tenant.to_string(),
            limit: Some(250),
            ..AiUsageQuery::default()
        })
        .await
        .unwrap_or_default();
    let mut guard_events = state
        .telemetry_store
        .list_telemetry(tenant, "guard_incident")
        .await
        .unwrap_or_default();
    if let Ok(mut events) = state
        .telemetry_store
        .list_telemetry(tenant, "guard_event")
        .await
    {
        guard_events.append(&mut events);
    }

    for agent in agents {
        let raw = serde_json::to_value(&agent).unwrap_or(Value::Null);
        let agent_id = string_path(&raw, &["agent_id"]).unwrap_or_default();
        let label = string_path(&raw, &["name"]).unwrap_or_else(|| agent_id.clone());
        let runtime = string_path(&raw, &["runtime", "runtime_name"]);
        let agent_type = string_path(&raw, &["agent_type"]);
        let status = string_path(&raw, &["enforcement_mode"])
            .or_else(|| string_path(&raw, &["meta", "status"]))
            .unwrap_or_else(|| "registered".to_string());
        let mut badges = Vec::new();
        if string_path(&raw, &["identity", "spiffe_id"]).is_some() {
            badges.push("SPIFFE bound".to_string());
        }
        if let Some(bindings) = raw
            .get("identity")
            .and_then(|v| v.get("token_bindings"))
            .and_then(Value::as_array)
        {
            if !bindings.is_empty() {
                badges.push(format!("{} token binding(s)", bindings.len()));
            }
        }
        if let Some(trust) = string_path(&raw, &["trust_level"]) {
            badges.push(format!("Trust: {trust}"));
        }
        builder.add_node(GraphNode {
            id: node_key("agent", &agent_id),
            node_type: "agent".to_string(),
            entity_id: agent_id.clone(),
            label,
            subtitle: runtime.or(agent_type),
            status: status.to_lowercase(),
            risk: string_path(&raw, &["trust_level"]),
            mode: string_path(&raw, &["enforcement_mode"]).map(|m| m.to_lowercase()),
            badges,
            metrics: vec![
                GraphMetric {
                    label: "Tools".to_string(),
                    value: array_strings(&raw, &["declared_tools"]).len().to_string(),
                },
                GraphMetric {
                    label: "Resources".to_string(),
                    value: array_strings(&raw, &["declared_resources"])
                        .len()
                        .to_string(),
                },
            ],
            href: route_for("agent", &agent_id),
            raw: Some(raw.clone()),
        });
        for tool_id in array_strings(&raw, &["declared_tools"]) {
            builder.add_edge(
                "agent",
                &agent_id,
                "tool",
                &tool_id,
                "uses",
                "Declared by registered agent",
                false,
                false,
            );
        }
        for resource_id in array_strings(&raw, &["declared_resources"]) {
            builder.add_edge(
                "agent",
                &agent_id,
                "resource",
                &resource_id,
                "accesses",
                "Declared by registered agent",
                false,
                false,
            );
        }
        if let Some(spiffe) = string_path(&raw, &["identity", "spiffe_id"]) {
            builder.add_edge(
                "agent",
                &agent_id,
                "identity",
                &spiffe,
                "bound_to",
                "SPIFFE identity binding",
                false,
                false,
            );
        }
    }

    for tool in tools {
        let raw = serde_json::to_value(&tool).unwrap_or(Value::Null);
        let tool_id = string_path(&raw, &["tool_id"]).unwrap_or_default();
        let label = string_path(&raw, &["name"]).unwrap_or_else(|| tool_id.clone());
        builder.add_node(GraphNode {
            id: node_key("tool", &tool_id),
            node_type: "tool".to_string(),
            entity_id: tool_id.clone(),
            label,
            subtitle: string_path(&raw, &["description"])
                .or_else(|| string_path(&raw, &["category"])),
            status: string_path(&raw, &["meta", "status"])
                .unwrap_or_else(|| "registered".to_string()),
            risk: string_path(&raw, &["risk_level"]),
            mode: None,
            badges: compact_badges(vec![
                string_path(&raw, &["category"]),
                string_path(&raw, &["data_access_level"]).map(|v| format!("Data: {v}")),
                string_path(&raw, &["side_effect_level"]).map(|v| format!("Effects: {v}")),
            ]),
            metrics: Vec::new(),
            href: route_for("tool", &tool_id),
            raw: Some(raw),
        });
    }

    for resource in resources {
        let raw = serde_json::to_value(&resource).unwrap_or(Value::Null);
        let resource_id = string_path(&raw, &["resource_id"])
            .or_else(|| string_path(&raw, &["id"]))
            .or_else(|| string_path(&raw, &["uri"]))
            .unwrap_or_default();
        let label = string_path(&raw, &["name"]).unwrap_or_else(|| resource_id.clone());
        builder.add_node(GraphNode {
            id: node_key("resource", &resource_id),
            node_type: "resource".to_string(),
            entity_id: resource_id.clone(),
            label,
            subtitle: string_path(&raw, &["uri"]).or_else(|| string_path(&raw, &["resource_type"])),
            status: string_path(&raw, &["meta", "status"])
                .unwrap_or_else(|| "registered".to_string()),
            risk: string_path(&raw, &["classification"]),
            mode: None,
            badges: compact_badges(vec![
                string_path(&raw, &["resource_type"]),
                string_path(&raw, &["classification"]).map(|v| format!("Class: {v}")),
            ]),
            metrics: Vec::new(),
            href: route_for("resource", &resource_id),
            raw: Some(raw),
        });
    }

    for entity in entities {
        let raw = serde_json::to_value(&entity).unwrap_or(Value::Null);
        let entity_id = string_path(&raw, &["entity_id"]).unwrap_or_default();
        let label = string_path(&raw, &["display_name"]).unwrap_or_else(|| entity_id.clone());
        let kind = string_path(&raw, &["entity_type"]).unwrap_or_else(|| "identity".to_string());
        builder.add_node(GraphNode {
            id: node_key("identity", &entity_id),
            node_type: "identity".to_string(),
            entity_id: entity_id.clone(),
            label,
            subtitle: Some(kind.clone()),
            status: string_path(&raw, &["meta", "status"])
                .unwrap_or_else(|| "registered".to_string()),
            risk: None,
            mode: None,
            badges: compact_badges(vec![
                Some(kind),
                raw.get("roles")
                    .and_then(Value::as_array)
                    .map(|roles| format!("{} role(s)", roles.len())),
            ]),
            metrics: Vec::new(),
            href: route_for("identity", &entity_id),
            raw: Some(raw),
        });
    }

    for provider in providers {
        let raw = serde_json::to_value(&provider).unwrap_or(Value::Null);
        let provider_id = string_path(&raw, &["provider_id"]).unwrap_or_default();
        let label = string_path(&raw, &["name"]).unwrap_or_else(|| provider_id.clone());
        builder.add_node(GraphNode {
            id: node_key("provider", &provider_id),
            node_type: "provider".to_string(),
            entity_id: provider_id.clone(),
            label,
            subtitle: string_path(&raw, &["provider_type"]),
            status: string_path(&raw, &["meta", "status"])
                .unwrap_or_else(|| "registered".to_string()),
            risk: string_path(&raw, &["trust_level"]),
            mode: None,
            badges: compact_badges(vec![
                string_path(&raw, &["provider_type"]),
                string_path(&raw, &["trust_level"]).map(|v| format!("Trust: {v}")),
            ]),
            metrics: Vec::new(),
            href: route_for("provider", &provider_id),
            raw: Some(raw),
        });
    }

    for policy in policies {
        let raw = serde_json::to_value(&policy).unwrap_or(Value::Null);
        let policy_id = string_path(&raw, &["policy_id"]).unwrap_or_default();
        let label = string_path(&raw, &["name"]).unwrap_or_else(|| policy_id.clone());
        builder.add_node(GraphNode {
            id: node_key("policy", &policy_id),
            node_type: "policy".to_string(),
            entity_id: policy_id.clone(),
            label,
            subtitle: string_path(&raw, &["description"])
                .or_else(|| string_path(&raw, &["policy_type"])),
            status: string_path(&raw, &["meta", "status"]).unwrap_or_else(|| "draft".to_string()),
            risk: None,
            mode: Some("govern".to_string()),
            badges: compact_badges(vec![
                string_path(&raw, &["policy_type"]),
                string_path(&raw, &["meta", "source"]).map(|v| format!("Source: {v}")),
            ]),
            metrics: vec![GraphMetric {
                label: "Targets".to_string(),
                value: target_count(&raw).to_string(),
            }],
            href: route_for("policy", &policy_id),
            raw: Some(raw.clone()),
        });
        for agent_id in array_strings(&raw, &["targets", "agent_ids"]) {
            builder.add_edge(
                "policy",
                &policy_id,
                "agent",
                &agent_id,
                "governs",
                "Policy target: agent",
                false,
                true,
            );
        }
        for tool_id in array_strings(&raw, &["targets", "tool_ids"]) {
            builder.add_edge(
                "policy",
                &policy_id,
                "tool",
                &tool_id,
                "governs",
                "Policy target: tool",
                false,
                true,
            );
        }
        for resource_id in array_strings(&raw, &["targets", "resource_ids"]) {
            builder.add_edge(
                "policy",
                &policy_id,
                "resource",
                &resource_id,
                "protects",
                "Policy target: resource",
                false,
                true,
            );
        }
        for entity_id in array_strings(&raw, &["targets", "entity_ids"]) {
            builder.add_edge(
                "policy",
                &policy_id,
                "identity",
                &entity_id,
                "governs",
                "Policy target: identity",
                false,
                true,
            );
        }
    }

    for relationship in relationships {
        let raw = serde_json::to_value(&relationship).unwrap_or(Value::Null);
        let subject_type = string_path(&raw, &["subject", "object_type"]).unwrap_or_default();
        let subject_id = string_path(&raw, &["subject", "object_id"]).unwrap_or_default();
        let object_type = string_path(&raw, &["object", "object_type"]).unwrap_or_default();
        let object_id = string_path(&raw, &["object", "object_id"]).unwrap_or_default();
        let relation = string_path(&raw, &["relation"]).unwrap_or_else(|| "related_to".to_string());
        builder.add_edge(
            &subject_type,
            &subject_id,
            &object_type,
            &object_id,
            &relation,
            "Registered relationship",
            false,
            false,
        );
    }

    for event in observations {
        let actor_id = event
            .agent_id
            .clone()
            .or_else(|| event.shadow_candidate_id.clone())
            .unwrap_or_else(|| "unknown-agent".to_string());
        if let Some(label) = system_actor_label(&actor_id) {
            builder.ensure_node("agent", &actor_id, label, "Pollek system activity");
        } else if event.agent_id.is_none() && event.shadow_candidate_id.is_none() {
            builder.ensure_node("agent", &actor_id, "Unknown AI app", "Observed activity");
        }

        let tool_id = event
            .tool_id
            .clone()
            .or_else(|| event.tool_call.as_ref().map(|tool| tool.tool_name.clone()));
        let resource_id = event.resource_id.clone().or_else(|| {
            event
                .resource_access
                .as_ref()
                .map(|resource| resource.target_redacted.clone())
        });
        if let (Some(resource_id), Some(resource)) = (&resource_id, &event.resource_access) {
            if resource.resource_type == "plugin" {
                builder.ensure_node(
                    "resource",
                    resource_id,
                    &resource.target_redacted,
                    "Plugin marketplace audit event",
                );
            }
        }

        if let Some(tool_id) = &tool_id {
            builder.add_edge(
                "agent",
                &actor_id,
                "tool",
                tool_id,
                "uses",
                "Observed tool invocation",
                true,
                decision_enforced(&event),
            );
        }
        if let Some(resource_id) = &resource_id {
            builder.add_edge(
                "agent",
                &actor_id,
                "resource",
                resource_id,
                "accesses",
                "Observed resource access",
                true,
                decision_enforced(&event),
            );
        }
        if let (Some(tool_id), Some(resource_id)) = (&tool_id, &resource_id) {
            builder.add_edge(
                "tool",
                tool_id,
                "resource",
                resource_id,
                "touches",
                "Observed through tool activity",
                true,
                decision_enforced(&event),
            );
        }
        let policy_ids = event
            .decision
            .as_ref()
            .map(|decision| decision.matched_policy_ids.clone())
            .unwrap_or_default();
        for policy_id in &policy_ids {
            builder.add_edge(
                "policy",
                policy_id,
                "agent",
                &actor_id,
                "matched_policy",
                "Matched in observed decision",
                true,
                decision_enforced(&event),
            );
        }
        activity.push(activity_from_observation(
            &builder.nodes,
            &event,
            &actor_id,
            tool_id,
            resource_id,
        ));
    }

    for event in guard_events {
        let (actor_id, resource_id, label) = guard_event_refs(&event);
        builder.ensure_node(
            "agent",
            &actor_id,
            &actor_id,
            "Prompt Guard incident source",
        );
        builder.ensure_node(
            "resource",
            &resource_id,
            &label,
            "Prompt Guard safety category",
        );
        builder.add_edge(
            "agent",
            &actor_id,
            "resource",
            &resource_id,
            "guarded",
            "Prompt Guard incident",
            true,
            guard_event_action(&event) == "deny",
        );
        activity.push(activity_from_guard_event(&builder.nodes, &event));
    }

    for event in usage_events {
        let agent_id = event
            .agent_id
            .clone()
            .or_else(|| event.shadow_candidate_id.clone())
            .unwrap_or_else(|| "unknown-agent".to_string());
        if let Some(provider) = &event.provider {
            builder.add_edge(
                "agent",
                &agent_id,
                "provider",
                provider,
                "uses_provider",
                "AI usage event",
                true,
                event.control_mode.as_deref() == Some("enforce"),
            );
        }
        if let Some(model) = &event.model {
            builder.ensure_node("model", model, model, "AI model seen in usage telemetry");
            builder.add_edge(
                "agent",
                &agent_id,
                "model",
                model,
                "uses_model",
                "AI usage event",
                true,
                event.control_mode.as_deref() == Some("enforce"),
            );
        }
        if let Some(tool_id) = &event.tool_id {
            builder.add_edge(
                "agent",
                &agent_id,
                "tool",
                tool_id,
                "uses",
                "AI usage event",
                true,
                event.control_mode.as_deref() == Some("enforce"),
            );
        }
        if let Some(resource_id) = &event.resource_id {
            builder.add_edge(
                "agent",
                &agent_id,
                "resource",
                resource_id,
                "accesses",
                "AI usage event",
                true,
                event.control_mode.as_deref() == Some("enforce"),
            );
        }
        for policy_id in &event.policy_ids {
            builder.add_edge(
                "policy",
                policy_id,
                "agent",
                &agent_id,
                "matched_policy",
                "AI usage policy reference",
                true,
                event.control_mode.as_deref() == Some("enforce"),
            );
        }
        activity.push(activity_from_usage(&builder.nodes, &event, &agent_id));
    }

    let graph = builder.finish(tenant, None);
    Ok(ReadModel { graph, activity })
}

fn build_entity_360(
    model: ReadModel,
    tenant: &str,
    entity_type: &str,
    entity_id: &str,
) -> Option<Entity360Response> {
    let normalized_type = normalize_type(entity_type);
    let center_key = node_key(&normalized_type, entity_id);
    let center = model
        .graph
        .nodes
        .iter()
        .find(|node| node.id == center_key)
        .cloned()?;

    let mut keep = BTreeSet::new();
    keep.insert(center.id.clone());
    for edge in &model.graph.edges {
        if edge.source == center.id {
            keep.insert(edge.target.clone());
        }
        if edge.target == center.id {
            keep.insert(edge.source.clone());
        }
    }

    let nodes: Vec<_> = model
        .graph
        .nodes
        .iter()
        .filter(|node| keep.contains(&node.id))
        .cloned()
        .collect();
    let edges: Vec<_> = model
        .graph
        .edges
        .iter()
        .filter(|edge| keep.contains(&edge.source) && keep.contains(&edge.target))
        .cloned()
        .collect();
    let summaries = summaries_from_nodes_edges(&nodes, &edges);
    let warnings = coverage_warnings(&nodes, &edges);
    let activity: Vec<_> = model
        .activity
        .into_iter()
        .filter(|item| activity_matches_entity(item, &normalized_type, entity_id))
        .take(50)
        .collect();
    let generated_at = chrono::Utc::now().to_rfc3339();
    let graph = EntityGraphResponse {
        schema_version: "entity-graph.v1".to_string(),
        tenant_id: tenant.to_string(),
        generated_at: generated_at.clone(),
        center: Some(center.clone()),
        nodes,
        edges,
        summaries: summaries.clone(),
        warnings: warnings.clone(),
    };

    Some(Entity360Response {
        schema_version: "entity-360.v1".to_string(),
        tenant_id: tenant.to_string(),
        generated_at,
        entity: center,
        graph,
        summaries,
        activity,
        warnings,
    })
}

fn filter_graph(mut graph: EntityGraphResponse, query: &GraphQuery) -> EntityGraphResponse {
    let allowed_types: Option<BTreeSet<String>> = query.types.as_ref().map(|types| {
        types
            .split(',')
            .map(|item| normalize_type(item.trim()))
            .collect::<BTreeSet<_>>()
    });
    let statuses: Option<BTreeSet<String>> = query.status.as_ref().map(|statuses| {
        statuses
            .split(',')
            .map(|item| item.trim().to_lowercase())
            .collect::<BTreeSet<_>>()
    });
    let search = query.q.as_ref().map(|q| q.to_lowercase());
    let limit = query.limit.unwrap_or(500).min(1000);

    graph.nodes = graph
        .nodes
        .into_iter()
        .filter(|node| {
            allowed_types
                .as_ref()
                .map(|types| types.contains(&node.node_type))
                .unwrap_or(true)
                && statuses
                    .as_ref()
                    .map(|values| values.contains(&node.status.to_lowercase()))
                    .unwrap_or(true)
                && search
                    .as_ref()
                    .map(|q| {
                        node.label.to_lowercase().contains(q)
                            || node.entity_id.to_lowercase().contains(q)
                            || node
                                .subtitle
                                .as_ref()
                                .map(|subtitle| subtitle.to_lowercase().contains(q))
                                .unwrap_or(false)
                    })
                    .unwrap_or(true)
        })
        .take(limit)
        .collect();
    let keep: BTreeSet<_> = graph.nodes.iter().map(|node| node.id.clone()).collect();
    graph
        .edges
        .retain(|edge| keep.contains(&edge.source) && keep.contains(&edge.target));
    graph.summaries = summaries_from_nodes_edges(&graph.nodes, &graph.edges);
    graph.warnings = coverage_warnings(&graph.nodes, &graph.edges);
    graph
}

fn activity_from_observation(
    nodes: &BTreeMap<String, GraphNode>,
    event: &dek_agent_observer::model::AgentObservationEvent,
    actor_id: &str,
    tool_id: Option<String>,
    resource_id: Option<String>,
) -> ActivityTimelineItem {
    let policies = event
        .decision
        .as_ref()
        .map(|decision| decision.matched_policy_ids.clone())
        .unwrap_or_default()
        .iter()
        .map(|policy_id| graph_ref(nodes, "policy", policy_id))
        .collect();
    let decision = event
        .decision
        .as_ref()
        .map(|decision| {
            if decision.allow {
                "allow".to_string()
            } else {
                "deny".to_string()
            }
        })
        .unwrap_or_else(|| "observe".to_string());
    let enforcement_mode = event
        .decision
        .as_ref()
        .and_then(|decision| decision.enforced_for_real)
        .map(|enforced| if enforced { "enforce" } else { "observe" })
        .unwrap_or("observe")
        .to_string();

    ActivityTimelineItem {
        event_id: event.event_id.clone(),
        timestamp: event.timestamp.clone(),
        actor: Some(graph_ref(nodes, "agent", actor_id)),
        action: event.action.clone(),
        tool: tool_id
            .as_ref()
            .map(|tool_id| graph_ref(nodes, "tool", tool_id)),
        resource: resource_id.as_ref().map(|resource_id| {
            let mut reference = graph_ref(nodes, "resource", resource_id);
            if let Some(resource) = &event.resource_access {
                if resource.resource_type == "plugin" {
                    reference.entity_type = "plugin".to_string();
                    reference.label = resource.target_redacted.clone();
                }
            }
            reference
        }),
        policies,
        decision,
        enforcement_mode,
        pep_plane: event
            .decision
            .as_ref()
            .and_then(|decision| decision.pep_plane.clone())
            .or_else(|| event.pep_type.clone()),
        pdp_engine: None,
        trace_id: Some(event.trace_id.clone()),
        cost: event.token_usage.as_ref().map(|usage| ActivityCost {
            total_cost_usd: None,
            total_tokens: usage.total_tokens,
            provider: event.provider.clone(),
            model: usage.model.clone(),
        }),
        explanation: event
            .decision
            .as_ref()
            .map(|decision| decision.reason_code.clone()),
        raw: serde_json::to_value(event).ok(),
    }
}

fn activity_from_usage(
    nodes: &BTreeMap<String, GraphNode>,
    event: &dek_agent_observer::usage_model::AiUsageEventV1,
    actor_id: &str,
) -> ActivityTimelineItem {
    let event_kind = serde_json::to_value(&event.event_kind)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_else(|| "model_call_completed".to_string());
    ActivityTimelineItem {
        event_id: event.event_id.clone(),
        timestamp: event.occurred_at.to_rfc3339(),
        actor: Some(graph_ref(nodes, "agent", actor_id)),
        action: event_kind,
        tool: event
            .tool_id
            .as_ref()
            .map(|tool_id| graph_ref(nodes, "tool", tool_id)),
        resource: event
            .resource_id
            .as_ref()
            .map(|resource_id| graph_ref(nodes, "resource", resource_id)),
        policies: event
            .policy_ids
            .iter()
            .map(|policy_id| graph_ref(nodes, "policy", policy_id))
            .collect(),
        decision: event.status.clone(),
        enforcement_mode: event
            .control_mode
            .clone()
            .unwrap_or_else(|| "observe".to_string()),
        pep_plane: event.pep_type.clone(),
        pdp_engine: None,
        trace_id: Some(event.trace_id.clone()),
        cost: Some(ActivityCost {
            total_cost_usd: Some(event.cost.total_cost),
            total_tokens: Some(event.tokens.total_tokens),
            provider: event.provider.clone(),
            model: event.model.clone(),
        }),
        explanation: event.error_code.clone(),
        raw: serde_json::to_value(event).ok(),
    }
}

fn activity_from_guard_event(
    nodes: &BTreeMap<String, GraphNode>,
    event: &Value,
) -> ActivityTimelineItem {
    let action = guard_event_action(event);
    let actor_id = guard_event_actor_id(event).unwrap_or_else(|| "unknown-agent".to_string());
    let category = guard_event_category(event);
    let label = guard_category_label(&category).to_string();
    let resource_id = guard_resource_id(&category);
    let timestamp = guard_event_string(
        event,
        &[
            "/payload/guard_event/ts",
            "/payload/guard_event/timestamp",
            "/payload/ts",
            "/timestamp",
            "/ts",
        ],
    )
    .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    let event_id = guard_event_string(
        event,
        &["/payload/guard_event/event_id", "/event_id", "/id"],
    )
    .unwrap_or_else(|| format!("guard-{category}-{timestamp}"));

    ActivityTimelineItem {
        event_id,
        timestamp,
        actor: Some(graph_ref(nodes, "agent", &actor_id)),
        action: format!("prompt_guard_{action}"),
        tool: None,
        resource: Some(graph_ref(nodes, "resource", &resource_id)),
        policies: Vec::new(),
        decision: guard_decision(&action).to_string(),
        enforcement_mode: if action == "allow" {
            "observe".to_string()
        } else {
            "guarded_path".to_string()
        },
        pep_plane: Some(
            guard_event_string(
                event,
                &[
                    "/payload/source_integration",
                    "/payload/integration",
                    "/payload/source",
                    "/source",
                ],
            )
            .unwrap_or_else(|| "prompt_guard".to_string()),
        ),
        pdp_engine: None,
        trace_id: guard_event_string(event, &["/trace_id", "/payload/trace_id"]),
        cost: None,
        explanation: Some(format!("{} - {}", label, guard_action_outcome(&action))),
        raw: Some(event.clone()),
    }
}

fn guard_event_refs(event: &Value) -> (String, String, String) {
    let actor_id = guard_event_actor_id(event).unwrap_or_else(|| "unknown-agent".to_string());
    let category = guard_event_category(event);
    let resource_id = guard_resource_id(&category);
    let label = guard_category_label(&category).to_string();
    (actor_id, resource_id, label)
}

fn guard_event_string(event: &Value, pointers: &[&str]) -> Option<String> {
    pointers.iter().find_map(|pointer| {
        event
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(ToString::to_string)
    })
}

fn guard_event_actor_id(event: &Value) -> Option<String> {
    guard_event_string(
        event,
        &[
            "/payload/guard_event/agent_id",
            "/payload/agent_id",
            "/agent_id",
        ],
    )
}

fn guard_event_action(event: &Value) -> String {
    guard_event_string(
        event,
        &["/payload/guard_event/action", "/payload/action", "/action"],
    )
    .unwrap_or_else(|| "allow".to_string())
    .to_ascii_lowercase()
}

fn guard_event_category(event: &Value) -> String {
    for pointer in [
        "/payload/guard_event/categories",
        "/payload/categories",
        "/categories",
    ] {
        if let Some(category) = event
            .pointer(pointer)
            .and_then(Value::as_array)
            .and_then(|items| items.iter().find_map(Value::as_str))
        {
            return category.to_string();
        }
    }
    "prompt_data_safety".to_string()
}

fn guard_resource_id(category: &str) -> String {
    format!("prompt-guard:{category}")
}

fn guard_decision(action: &str) -> &'static str {
    match action {
        "deny" => "deny",
        "redact" => "redact",
        "warn" => "warn",
        _ => "observe",
    }
}

fn guard_action_outcome(action: &str) -> &'static str {
    match action {
        "deny" => "blocked",
        "redact" => "redacted",
        "warn" => "warned",
        _ => "watched only",
    }
}

fn guard_category_label(category: &str) -> &'static str {
    match category {
        "llm01_prompt_injection" | "prompt_injection" => "Prompt injection attempt",
        "llm02_sensitive_information_disclosure" => "Sensitive information disclosure",
        "llm07_system_prompt_leakage" | "system_prompt_leak" => "System prompt leak",
        "secret" | "credential" => "Secret or credential",
        "pii" => "Private personal data",
        "unsafe_output" => "Unsafe output",
        _ => "Prompt and data safety",
    }
}

fn next_step(result: &str, category: &str) -> &'static str {
    if result == "blocked" {
        return "Review the rule if this should be allowed next time.";
    }
    if result == "redacted" {
        return "Review the safety rule and confirm the AI app is using the guard path for prompts and outputs.";
    }
    match category {
        "files" => {
            "Set a rule for this folder, or restrict file access inside the AI app settings."
        }
        "web" => "Set an approved website rule, or restrict network access in the AI app settings.",
        "commands" | "apps" => {
            "Ask before commands, or disable command execution inside the AI app."
        }
        "email" => "Keep email access opt-in and review the connector permissions.",
        "plugins" => "Review installed plugins, granted capabilities, and whether any connector can send data off this device.",
        "safety" => {
            "Keep watching, enable Prompt Guard for this AI app, or tighten the AI app's own safety settings."
        }
        _ => "Keep watching or create a rule from similar activity.",
    }
}

fn activity_matches(item: &ActivityTimelineItem, query: &ActivityQuery) -> bool {
    if let (Some(entity_type), Some(entity_id)) = (&query.entity_type, &query.entity_id) {
        if !activity_matches_entity(item, &normalize_type(entity_type), entity_id) {
            return false;
        }
    }
    if let Some(agent_id) = &query.agent_id {
        if !item
            .actor
            .as_ref()
            .map(|actor| actor.entity_type == "agent" && actor.entity_id == *agent_id)
            .unwrap_or(false)
        {
            return false;
        }
    }
    if let Some(policy_id) = &query.policy_id {
        if !item
            .policies
            .iter()
            .any(|policy| policy.entity_id == *policy_id)
        {
            return false;
        }
    }
    if let Some(resource_id) = &query.resource_id {
        if !item
            .resource
            .as_ref()
            .map(|resource| resource.entity_id == *resource_id)
            .unwrap_or(false)
        {
            return false;
        }
    }
    if let Some(tool_id) = &query.tool_id {
        if !item
            .tool
            .as_ref()
            .map(|tool| tool.entity_id == *tool_id)
            .unwrap_or(false)
        {
            return false;
        }
    }
    if let Some(decision) = &query.decision {
        if item.decision != *decision {
            return false;
        }
    }
    if let Some(mode) = &query.mode {
        if item.enforcement_mode != *mode {
            return false;
        }
    }
    true
}

fn activity_matches_entity(
    item: &ActivityTimelineItem,
    entity_type: &str,
    entity_id: &str,
) -> bool {
    item.actor
        .as_ref()
        .map(|actor| actor.entity_type == entity_type && actor.entity_id == entity_id)
        .unwrap_or(false)
        || item
            .tool
            .as_ref()
            .map(|tool| tool.entity_type == entity_type && tool.entity_id == entity_id)
            .unwrap_or(false)
        || item
            .resource
            .as_ref()
            .map(|resource| resource.entity_type == entity_type && resource.entity_id == entity_id)
            .unwrap_or(false)
        || item
            .policies
            .iter()
            .any(|policy| policy.entity_type == entity_type && policy.entity_id == entity_id)
}

fn graph_ref(nodes: &BTreeMap<String, GraphNode>, node_type: &str, entity_id: &str) -> GraphRef {
    let node_type = normalize_type(node_type);
    let id = node_key(&node_type, entity_id);
    let label = nodes
        .get(&id)
        .map(|node| node.label.clone())
        .unwrap_or_else(|| entity_id.to_string());
    GraphRef {
        id,
        entity_type: node_type,
        entity_id: entity_id.to_string(),
        label,
    }
}

fn coverage_warnings(nodes: &[GraphNode], edges: &[GraphEdge]) -> Vec<GraphWarning> {
    let mut protected = BTreeSet::new();
    for edge in edges {
        if matches!(
            edge.relation.as_str(),
            "governs" | "protects" | "matched_policy"
        ) {
            protected.insert(edge.target.clone());
        }
    }
    nodes
        .iter()
        .filter(|node| matches!(node.node_type.as_str(), "agent" | "tool" | "resource"))
        .filter(|node| !protected.contains(&node.id))
        .map(|node| GraphWarning {
            code: "policy_gap".to_string(),
            message: format!(
                "{} has observed or registered activity but no policy edge.",
                node.label
            ),
            entity_id: Some(node.id.clone()),
        })
        .collect()
}

fn summaries_from_nodes_edges(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
) -> Vec<RelationshipSummary> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for node in nodes {
        *counts.entry(node.node_type.clone()).or_default() += 1;
    }
    let observed_edges = edges.iter().filter(|edge| edge.observed).count();
    let enforced_edges = edges.iter().filter(|edge| edge.enforced).count();
    let mut out: Vec<_> = counts
        .into_iter()
        .map(|(kind, count)| RelationshipSummary {
            label: format!("{}{}", kind[..1].to_uppercase(), &kind[1..]),
            kind,
            count,
            tone: "neutral".to_string(),
        })
        .collect();
    out.push(RelationshipSummary {
        kind: "observed_edges".to_string(),
        label: "Observed links".to_string(),
        count: observed_edges,
        tone: "info".to_string(),
    });
    out.push(RelationshipSummary {
        kind: "enforced_edges".to_string(),
        label: "Enforced links".to_string(),
        count: enforced_edges,
        tone: "success".to_string(),
    });
    out
}

fn target_count(raw: &Value) -> usize {
    array_strings(raw, &["targets", "agent_ids"]).len()
        + array_strings(raw, &["targets", "tool_ids"]).len()
        + array_strings(raw, &["targets", "resource_ids"]).len()
        + array_strings(raw, &["targets", "entity_ids"]).len()
}

fn decision_enforced(event: &dek_agent_observer::model::AgentObservationEvent) -> bool {
    event
        .decision
        .as_ref()
        .and_then(|decision| decision.enforced_for_real)
        .unwrap_or(false)
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor.as_str().map(ToString::to_string)
}

fn array_strings(value: &Value, path: &[&str]) -> Vec<String> {
    let mut cursor = value;
    for key in path {
        match cursor.get(*key) {
            Some(next) => cursor = next,
            None => return Vec::new(),
        }
    }
    cursor
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn compact_badges(values: Vec<Option<String>>) -> Vec<String> {
    values
        .into_iter()
        .flatten()
        .filter(|value| !value.is_empty())
        .collect()
}

fn normalize_type(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "ai_agent" | "agent" | "agents" => "agent".to_string(),
        "policy_draft" | "policy" | "policies" => "policy".to_string(),
        "mcp_tool" | "tool" | "tools" => "tool".to_string(),
        "data_resource" | "resource" | "resources" => "resource".to_string(),
        "identity" | "identities" | "entity" | "human_user" | "service_account" | "workload"
        | "device" => "identity".to_string(),
        "blackbox_ai" | "provider" | "model_provider" => "provider".to_string(),
        "llm_model" | "model" => "model".to_string(),
        "plugin" | "plugins" | "connector" | "connectors" => "plugin".to_string(),
        other => other.to_string(),
    }
}

fn node_key(node_type: &str, entity_id: &str) -> String {
    format!("{}:{}", normalize_type(node_type), entity_id)
}

fn edge_label(relation: &str) -> String {
    match relation {
        "uses" => "uses".to_string(),
        "accesses" => "accesses".to_string(),
        "governs" => "governs".to_string(),
        "protects" => "protects".to_string(),
        "matched_policy" => "matched policy".to_string(),
        "bound_to" => "bound to".to_string(),
        "touches" => "touches".to_string(),
        "uses_provider" => "uses provider".to_string(),
        "uses_model" => "uses model".to_string(),
        "uses_plugin" => "uses plugin".to_string(),
        other => other.replace('_', " "),
    }
}

fn route_for(node_type: &str, entity_id: &str) -> Option<String> {
    let encoded = percent_encode(entity_id);
    match normalize_type(node_type).as_str() {
        "agent" => Some(format!("/agents?selected={encoded}")),
        "tool" => Some(format!("/tools?selected={encoded}")),
        "resource" => Some(format!("/resources?selected={encoded}")),
        "policy" => Some(format!("/policies?selected={encoded}")),
        "identity" => Some(format!("/identities?selected={encoded}")),
        "provider" => Some(format!("/agents?tab=models&selected={encoded}")),
        "plugin" => Some(format!("/plugin-marketplace?selected={encoded}")),
        _ => None,
    }
}

fn system_actor_label(actor_id: &str) -> Option<&'static str> {
    match actor_id {
        "pollek-plugin-marketplace" => Some("Pollek Plugin Marketplace"),
        _ => None,
    }
}

fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn internal_error(err: anyhow::Error) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": err.to_string() })),
    )
}
