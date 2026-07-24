//! Wire types for the entity-graph API: query parameters and the graph /
//! activity-timeline / entity-360 response DTOs. Pure serde data types.

use super::*;

#[derive(Debug, Deserialize, Default)]
pub(super) struct GraphQuery {
    pub(super) types: Option<String>,
    pub(super) status: Option<String>,
    pub(super) q: Option<String>,
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct ActivityQuery {
    pub(super) entity_type: Option<String>,
    pub(super) entity_id: Option<String>,
    pub(super) agent_id: Option<String>,
    pub(super) policy_id: Option<String>,
    pub(super) resource_id: Option<String>,
    pub(super) tool_id: Option<String>,
    pub(super) decision: Option<String>,
    pub(super) mode: Option<String>,
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct EntityNodeQuery {
    pub(super) entity_type: String,
    pub(super) entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct GraphMetric {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct GraphNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub entity_id: String,
    pub label: String,
    pub subtitle: Option<String>,
    pub status: String,
    pub risk: Option<String>,
    pub mode: Option<String>,
    pub badges: Vec<String>,
    pub metrics: Vec<GraphMetric>,
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation: String,
    pub label: String,
    pub evidence: String,
    pub observed: bool,
    pub enforced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RelationshipSummary {
    pub kind: String,
    pub label: String,
    pub count: usize,
    pub tone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct GraphWarning {
    pub code: String,
    pub message: String,
    pub entity_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct EntityGraphResponse {
    pub schema_version: String,
    pub tenant_id: String,
    pub generated_at: String,
    pub center: Option<GraphNode>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub summaries: Vec<RelationshipSummary>,
    pub warnings: Vec<GraphWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct GraphRef {
    pub id: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub entity_id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(super) struct ActivityCost {
    pub total_cost_usd: Option<f64>,
    pub total_tokens: Option<i64>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ActivityTimelineItem {
    pub event_id: String,
    pub timestamp: String,
    pub actor: Option<GraphRef>,
    pub action: String,
    pub tool: Option<GraphRef>,
    pub resource: Option<GraphRef>,
    pub policies: Vec<GraphRef>,
    pub decision: String,
    pub enforcement_mode: String,
    pub pep_plane: Option<String>,
    pub pdp_engine: Option<String>,
    pub trace_id: Option<String>,
    pub cost: Option<ActivityCost>,
    pub explanation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ActivityTimelineResponse {
    pub schema_version: String,
    pub tenant_id: String,
    pub generated_at: String,
    pub items: Vec<ActivityTimelineItem>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct UserFriendlyActivityAdvanced {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_item: Option<ActivityTimelineItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_agent_label: Option<String>,
    pub decision: Option<String>,
    pub mode: Option<String>,
    pub pep_plane: Option<String>,
    pub pdp_engine: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct UserFriendlyActivityEvent {
    pub schema_version: String,
    pub event_id: String,
    pub timestamp: String,
    pub agent_id: Option<String>,
    pub agent_name: String,
    pub category: String,
    pub action: String,
    pub target_label: String,
    pub target_kind: String,
    pub access_mode: String,
    pub result: String,
    pub result_label: String,
    pub plain_summary: String,
    pub rule_label: Option<String>,
    pub capability_note: String,
    pub next_step: String,
    pub privacy_note: String,
    pub cost_usd: Option<f64>,
    pub tokens: Option<i64>,
    pub trace_id: Option<String>,
    pub advanced: UserFriendlyActivityAdvanced,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct UserFriendlyActivityResponse {
    pub schema_version: String,
    pub tenant_id: String,
    pub generated_at: String,
    pub source: String,
    pub items: Vec<UserFriendlyActivityEvent>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Entity360Response {
    pub schema_version: String,
    pub tenant_id: String,
    pub generated_at: String,
    pub entity: GraphNode,
    pub graph: EntityGraphResponse,
    pub summaries: Vec<RelationshipSummary>,
    pub activity: Vec<ActivityTimelineItem>,
    pub warnings: Vec<GraphWarning>,
}
