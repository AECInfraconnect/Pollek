use crate::model::AgentObservationEvent;
use pollek_contract::{IdentityAccessPayload, ResourceAccessPayload, ToolUsagePayload};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedResource {
    pub resource_id: String,
    pub scope: String,
    pub kind: String,
    pub target_redacted: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    pub agents: Vec<String>,
    pub modes: Vec<String>,
    pub last_access: chrono::DateTime<chrono::Utc>,
    pub access_count: u64,
    pub governed: bool,
    pub registered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedTool {
    pub tool_id: String,
    pub tool_kind: String,
    pub tool_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    pub agents: Vec<String>,
    pub last_used: chrono::DateTime<chrono::Utc>,
    pub use_count: u64,
    pub governed: bool,
    pub registered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedIdentity {
    pub identity_id: String,
    pub identity_label: String,
    pub identity_kind: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spiffe_id: Option<String>,
    pub agents: Vec<String>,
    pub actions: Vec<String>,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub access_count: u64,
    pub governed: bool,
    pub registered: bool,
}

pub struct AgentStats {
    pub total_events: u64,
    pub resources_accessed: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

pub fn aggregate_events(events: &[AgentObservationEvent]) -> HashMap<String, AgentStats> {
    let mut stats = HashMap::new();

    for event in events {
        let agent_id = event
            .agent_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        let entry = stats.entry(agent_id).or_insert(AgentStats {
            total_events: 0,
            resources_accessed: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
        });

        entry.total_events += 1;

        if event.resource_id.is_some() {
            entry.resources_accessed += 1;
        }

        if let Some(tokens) = &event.token_usage {
            entry.total_input_tokens += tokens.input_tokens.unwrap_or(0) as u64;
            entry.total_output_tokens += tokens.output_tokens.unwrap_or(0) as u64;
        }
    }

    stats
}

fn push_unique<T: PartialEq>(vec: &mut Vec<T>, item: T) {
    if !vec.contains(&item) {
        vec.push(item);
    }
}

pub fn aggregate_resources(events: &[ResourceAccessPayload]) -> Vec<ObservedResource> {
    let mut map: HashMap<String, ObservedResource> = HashMap::new();
    for e in events {
        let details = resource_details(e);
        let r = map
            .entry(e.target_hash.clone())
            .or_insert_with(|| ObservedResource {
                resource_id: uuid::Uuid::new_v4().to_string(),
                scope: e.scope.to_string(),
                kind: e.kind.to_string(),
                target_redacted: e.target_redacted.clone(),
                classification: e.classification.clone(),
                details: details.clone(),
                agents: vec![],
                modes: vec![],
                last_access: e.observed_at,
                access_count: 0,
                governed: false,
                registered: false,
            });
        r.access_count += 1;
        if r.last_access < e.observed_at {
            r.last_access = e.observed_at;
            r.details = details.clone().or_else(|| r.details.clone());
        }
        if r.details.is_none() {
            r.details = details;
        }
        push_unique(&mut r.agents, e.agent_id.clone());
        push_unique(&mut r.modes, e.mode.to_string());
    }
    map.into_values().collect()
}

fn resource_details(event: &ResourceAccessPayload) -> Option<Value> {
    serde_json::to_value(event)
        .ok()
        .and_then(|value| value.get("details").cloned())
        .filter(|value| !value.is_null())
}

pub fn aggregate_tools(events: &[ToolUsagePayload]) -> Vec<ObservedTool> {
    let mut map: HashMap<String, ObservedTool> = HashMap::new();
    for e in events {
        let tool_id = format!("{}:{}", e.server.as_deref().unwrap_or(""), e.tool_name);
        let t = map.entry(tool_id.clone()).or_insert_with(|| ObservedTool {
            tool_id: tool_id.clone(),
            tool_kind: e.tool_kind.to_string(),
            tool_name: e.tool_name.clone(),
            server: e.server.clone(),
            agents: vec![],
            last_used: e.observed_at,
            use_count: 0,
            governed: false,
            registered: false,
        });
        t.use_count += 1;
        if t.last_used < e.observed_at {
            t.last_used = e.observed_at;
        }
        push_unique(&mut t.agents, e.agent_id.clone());
    }
    map.into_values().collect()
}

pub fn aggregate_identities(events: &[IdentityAccessPayload]) -> Vec<ObservedIdentity> {
    let mut map: HashMap<String, ObservedIdentity> = HashMap::new();
    for e in events {
        let i = map
            .entry(e.identity_id.clone())
            .or_insert_with(|| ObservedIdentity {
                identity_id: e.identity_id.clone(),
                identity_label: e.identity_label.clone(),
                identity_kind: e.identity_kind.to_string(),
                scope: e.scope.to_string(),
                provider: e.provider.clone(),
                spiffe_id: e.spiffe_id.clone(),
                agents: vec![],
                actions: vec![],
                last_seen: e.observed_at,
                access_count: 0,
                governed: false,
                registered: false,
            });
        i.access_count += 1;
        if i.last_seen < e.observed_at {
            i.last_seen = e.observed_at;
        }
        push_unique(&mut i.agents, e.agent_id.clone());
        push_unique(&mut i.actions, e.action.to_string());
    }
    map.into_values().collect()
}
