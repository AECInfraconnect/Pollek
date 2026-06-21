use crate::model::*;
use anyhow::Result;
use dek_agent_observer::model::AgentObservationEvent;

pub fn generate_suggestions(
    _tenant: &str,
    _candidates: &[dek_agent_discovery::model::DiscoveredAgentCandidate],
    events: &[AgentObservationEvent],
) -> Result<Vec<PolicySuggestion>> {
    let mut engine = crate::rules::RuleEngine::new();
    
    // Add built-in rules
    engine.add_rule(Box::new(MockCostSpikeRule { tenant_id: _tenant.to_string() }));
    engine.add_rule(Box::new(MockUnregisteredEgressRule { tenant_id: _tenant.to_string() }));

    engine.evaluate_all(events)
}

struct MockCostSpikeRule {
    tenant_id: String,
}

impl crate::rules::SuggestionRule for MockCostSpikeRule {
    fn evaluate(&self, events: &[AgentObservationEvent]) -> Result<Vec<PolicySuggestion>> {
        let total_cost = if events.iter().any(|e| e.token_usage.is_some()) {
            30.0
        } else {
            0.0
        };

        if total_cost < 25.0 {
            return Ok(vec![]);
        }

        Ok(vec![PolicySuggestion {
            suggestion_id: format!("sug_{}", uuid::Uuid::new_v4()),
            tenant_id: self.tenant_id.clone(),
            target_agent_id: None,
            target_resource_id: None,
            target_tool_id: None,
            suggestion_type: "EnforceCostBudget".into(),
            title: "AI usage cost exceeded suggested daily threshold".into(),
            summary: format!("Observed estimated cost ${:.2}. Suggest daily budget guardrail.", total_cost),
            severity: "medium".into(),
            confidence: 0.75,
            recommended_policy_type: "rego".into(),
            recommended_pep_type: "forward_proxy".into(),
            artifacts: vec![PolicyArtifact {
                language: "rego".into(),
                name: "daily_ai_cost_budget.rego".into(),
                content: "package pollen.policies.daily_cost\nimport future.keywords.if\n\ndefault allow := true\nmax_daily_cost_usd := 25.00\nallow := false if {\n  input.cost.currency == \"USD\"\n  input.cost.total_cost > max_daily_cost_usd\n}".to_string(),
            }],
            status: "suggested".into(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }])
    }
}

struct MockUnregisteredEgressRule {
    tenant_id: String,
}

impl crate::rules::SuggestionRule for MockUnregisteredEgressRule {
    fn evaluate(&self, _events: &[AgentObservationEvent]) -> Result<Vec<PolicySuggestion>> {
        // Return 1 mock suggestion just for demo purposes
        Ok(vec![PolicySuggestion {
            suggestion_id: format!("sug_{}", uuid::Uuid::new_v4()),
            tenant_id: self.tenant_id.clone(),
            target_agent_id: None,
            target_resource_id: Some("online.api_endpoint".into()),
            target_tool_id: None,
            suggestion_type: "RestrictExternalLlmProvider".into(),
            title: "Block Unregistered AI Egress".into(),
            summary: "An unregistered process attempted to access api.openai.com. We suggest deploying a Rego policy to block this egress until the agent is registered.".into(),
            severity: "high".into(),
            confidence: 0.9,
            recommended_policy_type: "rego".into(),
            recommended_pep_type: "envoy_proxy".into(),
            artifacts: vec![],
            status: "suggested".into(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }])
    }
}
