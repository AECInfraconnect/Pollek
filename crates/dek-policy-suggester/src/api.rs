use crate::model::*;
use anyhow::Result;
use dek_agent_observer::model::AgentObservationEvent;

pub fn generate_suggestions(
    _tenant: &str,
    _candidates: &[dek_agent_discovery::model::DiscoveredAgentCandidate],
    events: &[AgentObservationEvent],
) -> Result<Vec<PolicySuggestion>> {
    let mut out = Vec::new();

    // Mocking the detection logic based on the user's plan.
    out.push(PolicySuggestion {
        schema_version: "policy-suggestion.v1".into(),
        suggestion_id: format!("sug_{}", uuid::Uuid::new_v4()),
        tenant_id: _tenant.to_string(),
        device_id: "device-local".into(),
        suggestion_type: SuggestionType::RestrictExternalLlmProvider,
        title: "Block Unregistered AI Egress".into(),
        summary: "An unregistered process attempted to access api.openai.com. We suggest deploying a Rego policy to block this egress until the agent is registered.".into(),
        severity: "high".into(),
        confidence: 0.9,
        evidence_event_ids: vec![],
        affected_agents: vec![],
        affected_shadow_candidates: vec!["cand_example".into()],
        affected_resources: vec!["online.api_endpoint".into()],
        recommended_pep_types: vec!["envoy_proxy".into(), "stdio_wrapper".into()],
        recommended_languages: vec![SuggestedPolicyLanguage::Rego],
        artifacts: vec![],
        dry_run_required: true,
        status: "suggested".into(),
        created_at: chrono::Utc::now().to_rfc3339(),
    });

    out.extend(rule_cost_spike(_tenant, "device-local", events));

    Ok(out)
}

fn rule_cost_spike(
    tenant_id: &str,
    device_id: &str,
    events: &[AgentObservationEvent],
) -> Vec<PolicySuggestion> {
    let total_cost: f64 = events
        .iter()
        .filter_map(|e| e.cost.as_ref()?.total_cost)
        .sum();

    if total_cost < 25.0 {
        return vec![];
    }

    vec![PolicySuggestion {
        schema_version: "pollen.policy_suggestion.v1".into(),
        suggestion_id: format!("sug_{}", uuid::Uuid::new_v4()),
        tenant_id: tenant_id.into(),
        device_id: device_id.into(),
        suggestion_type: SuggestionType::EnforceCostBudget,
        title: "AI usage cost exceeded suggested daily threshold".into(),
        summary: format!("Observed estimated cost ${:.2}. Suggest daily budget guardrail.", total_cost),
        severity: "medium".into(),
        confidence: 0.75,
        evidence_event_ids: events.iter().filter(|e| e.cost.is_some()).map(|e| e.event_id.clone()).take(20).collect(),
        affected_agents: events.iter().filter_map(|e| e.agent_id.clone()).collect(),
        affected_shadow_candidates: events.iter().filter_map(|e| e.shadow_candidate_id.clone()).collect(),
        affected_resources: vec![],
        recommended_pep_types: vec!["forward_proxy".into(), "mcp_proxy".into()],
        recommended_languages: vec![SuggestedPolicyLanguage::Rego],
        artifacts: vec![SuggestedArtifact {
            language: SuggestedPolicyLanguage::Rego,
            filename: "daily_ai_cost_budget.rego".into(),
            content: format!("package pollen.policies.daily_cost\nimport future.keywords.if\n\ndefault allow := true\n\nmax_daily_cost_usd := 25.00\n\nallow := false if {{\n  input.cost.currency == \"USD\"\n  input.cost.total_cost > max_daily_cost_usd\n}}"),
        }],
        dry_run_required: true,
        status: "suggested".into(),
        created_at: chrono::Utc::now().to_rfc3339(),
    }]
}
