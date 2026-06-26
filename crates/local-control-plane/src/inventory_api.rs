use crate::state::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use dek_agent_observer::aggregate::{
    aggregate_identities, aggregate_resources, aggregate_tools, ObservedIdentity, ObservedResource,
    ObservedTool,
};
use pollek_contract::{IdentityAccessPayload, ResourceAccessPayload, ToolUsagePayload};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInventoryPage {
    pub schema_version: String,
    pub items: Vec<ObservedResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInventoryPage {
    pub schema_version: String,
    pub items: Vec<ObservedTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityInventoryPage {
    pub schema_version: String,
    pub items: Vec<ObservedIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Deserialize)]
pub struct InventoryQuery {
    pub agent_id: Option<String>,
    pub scope: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tenants/local/telemetry/resources", get(list_resources))
        .route("/v1/tenants/local/telemetry/tools", get(list_tools))
        .route(
            "/v1/tenants/local/telemetry/identities",
            get(list_identities),
        )
        .route(
            "/v1/tenants/local/telemetry/observations",
            get(list_observations),
        )
}

fn telemetry_payload(ev: Value) -> Value {
    ev.get("payload")
        .cloned()
        .or_else(|| ev.get("details").cloned())
        .unwrap_or(ev)
}

async fn list_resources(
    State(state): State<AppState>,
    Query(query): Query<InventoryQuery>,
) -> impl IntoResponse {
    let tenant = "local".to_string();
    let events_json = state
        .telemetry_store
        .list_telemetry(&tenant, "resource_access")
        .await
        .unwrap_or_default();

    let mut payloads = Vec::new();
    for ev in events_json {
        if let Ok(p) = serde_json::from_value::<ResourceAccessPayload>(telemetry_payload(ev)) {
            if let Some(agent_id) = &query.agent_id {
                if p.agent_id != *agent_id {
                    continue;
                }
            }
            if let Some(scope) = &query.scope {
                if p.scope.to_string() != *scope {
                    continue;
                }
            }
            payloads.push(p);
        }
    }

    let items = aggregate_resources(&payloads);

    let page = ResourceInventoryPage {
        schema_version: "resource-inventory.v1".to_string(),
        items,
        next_cursor: None,
    };

    (StatusCode::OK, Json(page))
}

async fn list_tools(
    State(state): State<AppState>,
    Query(query): Query<InventoryQuery>,
) -> impl IntoResponse {
    let tenant = "local".to_string();
    let events_json = state
        .telemetry_store
        .list_telemetry(&tenant, "tool_usage")
        .await
        .unwrap_or_default();

    let mut payloads = Vec::new();
    for ev in events_json {
        if let Ok(p) = serde_json::from_value::<ToolUsagePayload>(telemetry_payload(ev)) {
            if let Some(agent_id) = &query.agent_id {
                if p.agent_id != *agent_id {
                    continue;
                }
            }
            payloads.push(p);
        }
    }

    let items = aggregate_tools(&payloads);

    let page = ToolInventoryPage {
        schema_version: "tool-inventory.v1".to_string(),
        items,
        next_cursor: None,
    };

    (StatusCode::OK, Json(page))
}

async fn list_identities(
    State(state): State<AppState>,
    Query(query): Query<InventoryQuery>,
) -> impl IntoResponse {
    let tenant = "local".to_string();
    let events_json = state
        .telemetry_store
        .list_telemetry(&tenant, "identity_access")
        .await
        .unwrap_or_default();

    let mut payloads = Vec::new();
    for ev in events_json {
        if let Ok(p) = serde_json::from_value::<IdentityAccessPayload>(telemetry_payload(ev)) {
            if let Some(agent_id) = &query.agent_id {
                if p.agent_id != *agent_id {
                    continue;
                }
            }
            if let Some(scope) = &query.scope {
                if p.scope.to_string() != *scope {
                    continue;
                }
            }
            payloads.push(p);
        }
    }

    let page = IdentityInventoryPage {
        schema_version: "identity-inventory.v1".to_string(),
        items: aggregate_identities(&payloads),
        next_cursor: None,
    };

    (StatusCode::OK, Json(page))
}

#[derive(Deserialize)]
pub struct ObservationsQuery {
    pub target_redacted: Option<String>,
    pub tool_id: Option<String>,
}

async fn list_observations(
    State(state): State<AppState>,
    Query(query): Query<ObservationsQuery>,
) -> impl IntoResponse {
    let tenant = "local".to_string();
    let mut filtered = Vec::new();

    if let Ok(evs) = state
        .telemetry_store
        .list_telemetry(&tenant, "resource_access")
        .await
    {
        for ev in evs {
            if let Some(target) = &query.target_redacted {
                let mut matched = false;
                if let Some(redacted) = ev
                    .pointer("/payload/target_redacted")
                    .and_then(|t| t.as_str())
                {
                    if *target == redacted {
                        matched = true;
                    }
                } else if let Some(redacted) = ev
                    .pointer("/details/target_redacted")
                    .and_then(|t| t.as_str())
                {
                    if *target == redacted {
                        matched = true;
                    }
                } else if let Some(redacted) = ev.get("target_redacted").and_then(|t| t.as_str()) {
                    if *target == redacted {
                        matched = true;
                    }
                }
                if matched {
                    filtered.push(ev);
                }
            } else if query.tool_id.is_none() {
                filtered.push(ev);
            }
        }
    }

    if let Ok(evs) = state
        .telemetry_store
        .list_telemetry(&tenant, "tool_usage")
        .await
    {
        for ev in evs {
            if let Some(tid) = &query.tool_id {
                let mut matched = false;
                if let Some(t) = ev.pointer("/payload/tool_id").and_then(|v| v.as_str()) {
                    if *tid == t {
                        matched = true;
                    }
                } else if let Some(t) = ev.pointer("/payload/tool_name").and_then(|v| v.as_str()) {
                    if *tid == t {
                        matched = true;
                    }
                } else if let Some(t) = ev.pointer("/payload/tool_kind").and_then(|v| v.as_str()) {
                    if *tid == t {
                        matched = true;
                    }
                } else if let Some(t) = ev.pointer("/details/tool_id").and_then(|v| v.as_str()) {
                    if *tid == t {
                        matched = true;
                    }
                } else if let Some(t) = ev.pointer("/details/tool_name").and_then(|v| v.as_str()) {
                    if *tid == t {
                        matched = true;
                    }
                } else if let Some(t) = ev.pointer("/details/tool_kind").and_then(|v| v.as_str()) {
                    if *tid == t {
                        matched = true;
                    }
                } else if let Some(t) = ev.get("tool_id").and_then(|v| v.as_str()) {
                    if *tid == t {
                        matched = true;
                    }
                }
                if matched {
                    filtered.push(ev);
                }
            } else if query.target_redacted.is_none() {
                filtered.push(ev);
            }
        }
    }

    if let Ok(evs) = state
        .telemetry_store
        .list_telemetry(&tenant, "identity_access")
        .await
    {
        if query.target_redacted.is_none() && query.tool_id.is_none() {
            filtered.extend(evs);
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "schema_version": "observations.v1",
            "items": filtered
        })),
    )
}
