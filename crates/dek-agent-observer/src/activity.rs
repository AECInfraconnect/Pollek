// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use serde::{Deserialize, Serialize};

use crate::model::{AgentObservationEvent, EventKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityCounts {
    pub total_decisions: u32,
    pub denied_actions: u32,
    pub mcp_invocations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityItem {
    pub timestamp: String,
    pub event_type: String, // "network_egress", "mcp_tool_call", "file_read", etc.
    pub decision: Option<String>,
    pub resource: String,
    pub reason: String,

    #[serde(default)]
    pub pep_plane: Option<String>,
    #[serde(default)]
    pub enforced_for_real: Option<bool>,
    #[serde(default)]
    pub status_badge: Option<String>,
    #[serde(default)]
    pub message_th: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivitySet {
    pub start_time: String,
    pub end_time: String,
    pub duration_seconds: u32,
    pub counts: ActivityCounts,
    pub items: Vec<ActivityItem>,
}

impl ActivitySet {
    pub fn new(start_time: String) -> Self {
        Self {
            start_time,
            end_time: "".into(),
            duration_seconds: 0,
            counts: ActivityCounts {
                total_decisions: 0,
                denied_actions: 0,
                mcp_invocations: 0,
            },
            items: Vec::new(),
        }
    }

    pub fn add_item(&mut self, item: ActivityItem) {
        if item.decision.as_deref() == Some("deny") {
            self.counts.denied_actions += 1;
        }
        if item.event_type == "mcp_tool_call" {
            self.counts.mcp_invocations += 1;
        }
        if item.decision.is_some() {
            self.counts.total_decisions += 1;
        }
        self.items.push(item);
    }
}

pub fn activity_items_from_observations(events: &[AgentObservationEvent]) -> Vec<ActivityItem> {
    let mut items = events
        .iter()
        .map(activity_item_from_observation)
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.timestamp.cmp(&right.timestamp));
    items
}

pub fn activity_counts(items: &[ActivityItem]) -> ActivityCounts {
    let mut counts = ActivityCounts {
        total_decisions: 0,
        denied_actions: 0,
        mcp_invocations: 0,
    };
    for item in items {
        if item.decision.is_some() {
            counts.total_decisions += 1;
        }
        if item.decision.as_deref() == Some("deny") {
            counts.denied_actions += 1;
        }
        if item.event_type == "mcp_tool_call" {
            counts.mcp_invocations += 1;
        }
    }
    counts
}

pub fn activity_item_from_observation(event: &AgentObservationEvent) -> ActivityItem {
    let decision = event
        .decision
        .as_ref()
        .map(|decision| if decision.allow { "allow" } else { "deny" }.to_string());
    let resource = event
        .resource_access
        .as_ref()
        .map(|resource| resource.target_redacted.clone())
        .or_else(|| event.tool_call.as_ref().map(|tool| tool.tool_name.clone()))
        .or_else(|| event.resource_id.clone())
        .or_else(|| event.tool_id.clone())
        .unwrap_or_else(|| event.surface.clone());
    let reason = event
        .decision
        .as_ref()
        .map(|decision| decision.reason_code.clone())
        .unwrap_or_else(|| event.action.clone());
    let event_type = match &event.event_kind {
        EventKind::ToolCall => "mcp_tool_call",
        EventKind::ResourceAccess => "resource_access",
        EventKind::LlmCall => "llm_call",
        EventKind::Decision => "decision",
        EventKind::Generic => event.action.as_str(),
    }
    .to_string();

    ActivityItem {
        timestamp: event.timestamp.clone(),
        event_type,
        decision,
        resource,
        reason,
        pep_plane: event
            .decision
            .as_ref()
            .and_then(|decision| decision.pep_plane.clone())
            .or_else(|| event.pep_type.clone()),
        enforced_for_real: event
            .decision
            .as_ref()
            .and_then(|decision| decision.enforced_for_real),
        status_badge: event
            .decision
            .as_ref()
            .and_then(|decision| decision.status_badge.clone()),
        message_th: event
            .decision
            .as_ref()
            .and_then(|decision| decision.message_th.clone()),
    }
}

pub fn group_into_sets(raw_events: Vec<ActivityItem>, max_idle_seconds: u32) -> Vec<ActivitySet> {
    let mut sets = Vec::new();
    if raw_events.is_empty() {
        return sets;
    }

    let mut items = raw_events;
    items.sort_by(|left, right| left.timestamp.cmp(&right.timestamp));

    let mut current_set = ActivitySet::new(items[0].timestamp.clone());
    let mut previous_ts = parse_ts(&items[0].timestamp);
    for item in items {
        let current_ts = parse_ts(&item.timestamp);
        let should_split = previous_ts
            .zip(current_ts)
            .map(|(previous, current)| {
                current
                    .signed_duration_since(previous)
                    .num_seconds()
                    .gt(&(max_idle_seconds as i64))
            })
            .unwrap_or(false);

        if should_split && !current_set.items.is_empty() {
            finish_set(&mut current_set);
            sets.push(current_set);
            current_set = ActivitySet::new(item.timestamp.clone());
        }
        previous_ts = current_ts.or(previous_ts);
        current_set.add_item(item);
    }
    finish_set(&mut current_set);
    sets.push(current_set);
    sets
}

fn finish_set(set: &mut ActivitySet) {
    if let Some(last) = set.items.last() {
        set.end_time = last.timestamp.clone();
    }
    let duration = parse_ts(&set.start_time)
        .zip(parse_ts(&set.end_time))
        .map(|(start, end)| end.signed_duration_since(start).num_seconds().max(0))
        .unwrap_or(0);
    set.duration_seconds = duration.min(u32::MAX as i64) as u32;
}

fn parse_ts(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DecisionInfo, EventKind, ToolCall};

    fn event(id: &str, timestamp: &str, allow: bool) -> AgentObservationEvent {
        AgentObservationEvent {
            event_id: id.to_string(),
            tenant_id: "local".into(),
            trace_id: format!("trace_{id}"),
            agent_id: Some("agent_test".into()),
            shadow_candidate_id: None,
            tool_id: Some("tool_shell".into()),
            resource_id: None,
            surface: "mcp".into(),
            action: "tools/call".into(),
            pep_type: Some("mcp_proxy".into()),
            risk_level: None,
            timestamp: timestamp.into(),
            payload_json: "{}".into(),
            token_usage: None,
            browser_scope: None,
            event_kind: EventKind::ToolCall,
            decision: Some(DecisionInfo {
                allow,
                reason_code: if allow { "allowed" } else { "policy_denied" }.into(),
                obligations: vec![],
                matched_policy_ids: vec![],
                compliance_tags: vec![],
                pep_plane: Some("McpProxy".into()),
                enforced_for_real: Some(true),
                status_badge: Some(if allow { "Ok" } else { "Denied" }.into()),
                message_th: None,
            }),
            tool_call: Some(ToolCall {
                tool_name: "shell".into(),
                server: Some("local".into()),
                args_summary: None,
                result_status: if allow { "allowed" } else { "denied" }.into(),
            }),
            resource_access: None,
            latency_ms: Some(12),
            provider: None,
        }
    }

    #[test]
    fn observations_map_to_activity_items_without_mock_data() {
        let events = vec![event("one", "2026-06-27T00:00:00Z", false)];
        let items = activity_items_from_observations(&events);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].event_type, "mcp_tool_call");
        assert_eq!(items[0].decision.as_deref(), Some("deny"));
        assert_eq!(items[0].resource, "shell");
        assert_eq!(items[0].reason, "policy_denied");
    }

    #[test]
    fn grouping_splits_on_idle_gap_and_computes_counts() {
        let items = activity_items_from_observations(&[
            event("one", "2026-06-27T00:00:00Z", true),
            event("two", "2026-06-27T00:00:10Z", false),
            event("three", "2026-06-27T00:10:00Z", true),
        ]);
        let sets = group_into_sets(items, 300);

        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0].duration_seconds, 10);
        assert_eq!(sets[0].counts.total_decisions, 2);
        assert_eq!(sets[0].counts.denied_actions, 1);
        assert_eq!(sets[0].counts.mcp_invocations, 2);
        assert_eq!(sets[1].counts.total_decisions, 1);
    }
}
