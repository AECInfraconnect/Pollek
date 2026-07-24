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
mod graph_build;
mod model;
mod timeline;
use friendly::*;
use graph_build::*;
use model::*;
use timeline::*;

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
