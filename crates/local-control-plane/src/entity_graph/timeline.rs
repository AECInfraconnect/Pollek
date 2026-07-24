//! Activity-timeline construction: turn observation, AI-usage, and guard-
//! incident events into ActivityTimelineItems, including the guard-event
//! field extractors and decision/category classifiers.

use super::*;

pub(super) fn activity_from_observation(
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

pub(super) fn activity_from_usage(
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

pub(super) fn activity_from_guard_event(
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

pub(super) fn guard_event_refs(event: &Value) -> (String, String, String) {
    let actor_id = guard_event_actor_id(event).unwrap_or_else(|| "unknown-agent".to_string());
    let category = guard_event_category(event);
    let resource_id = guard_resource_id(&category);
    let label = guard_category_label(&category).to_string();
    (actor_id, resource_id, label)
}

pub(super) fn guard_event_string(event: &Value, pointers: &[&str]) -> Option<String> {
    pointers.iter().find_map(|pointer| {
        event
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(ToString::to_string)
    })
}

pub(super) fn guard_event_actor_id(event: &Value) -> Option<String> {
    guard_event_string(
        event,
        &[
            "/payload/guard_event/agent_id",
            "/payload/agent_id",
            "/agent_id",
        ],
    )
}

pub(super) fn guard_event_action(event: &Value) -> String {
    guard_event_string(
        event,
        &["/payload/guard_event/action", "/payload/action", "/action"],
    )
    .unwrap_or_else(|| "allow".to_string())
    .to_ascii_lowercase()
}

pub(super) fn guard_event_category(event: &Value) -> String {
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

pub(super) fn guard_resource_id(category: &str) -> String {
    format!("prompt-guard:{category}")
}

pub(super) fn guard_decision(action: &str) -> &'static str {
    match action {
        "deny" => "deny",
        "redact" => "redact",
        "warn" => "warn",
        _ => "observe",
    }
}

pub(super) fn guard_action_outcome(action: &str) -> &'static str {
    match action {
        "deny" => "blocked",
        "redact" => "redacted",
        "warn" => "warned",
        _ => "watched only",
    }
}

pub(super) fn guard_category_label(category: &str) -> &'static str {
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
