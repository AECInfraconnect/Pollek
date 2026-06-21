use crate::config::SuggesterConfig;
use crate::model::*;
use anyhow::Result;
use dek_agent_observer::model::AgentObservationEvent;

pub fn generate_suggestions(
    tenant: &str,
    _candidates: &[dek_agent_discovery::model::DiscoveredAgentCandidate],
    events: &[AgentObservationEvent],
) -> Result<Vec<PolicySuggestion>> {
    let mut engine = crate::rules::RuleEngine::new();

    // Add built-in rules
    engine.add_rule(Box::new(MockCostSpikeRule {
        tenant_id: tenant.to_string(),
    }));
    engine.add_rule(Box::new(MockUnregisteredEgressRule {
        tenant_id: tenant.to_string(),
    }));
    engine.add_rule(Box::new(MockPromptInjectionRule {
        tenant_id: tenant.to_string(),
    }));
    engine.add_rule(Box::new(MockPiiRedactionRule {
        tenant_id: tenant.to_string(),
    }));

    engine.evaluate_all(events)
}

struct MockCostSpikeRule {
    tenant_id: String,
}

impl crate::rules::SuggestionRule for MockCostSpikeRule {
    fn evaluate(&self, events: &[AgentObservationEvent]) -> Result<Vec<PolicySuggestion>> {
        let config = SuggesterConfig::default();
        let total_cost = if events.iter().any(|e| e.token_usage.is_some()) {
            30.0
        } else {
            0.0
        };

        if total_cost < config.cost_alert_threshold_usd {
            return Ok(vec![]);
        }

        Ok(vec![PolicySuggestion {
            suggestion_id: format!("sug_{}", uuid::Uuid::new_v4()),
            tenant_id: self.tenant_id.clone(),
            target_agent_id: None,
            target_resource_id: None,
            target_tool_id: None,
            suggestion_type: SuggestionType::EnforceCostBudget,
            title: "AI usage cost exceeded suggested daily threshold".into(),
            summary: format!(
                "Observed estimated cost ${:.2}. Suggest daily budget guardrail.",
                total_cost
            ),
            severity: SuggestionSeverity::Medium,
            confidence: 0.75,
            recommended_policy_type: SuggestedPolicyLanguage::Rego,
            recommended_pep_type: "forward_proxy".into(),
            artifacts: vec![PolicyArtifact {
                language: SuggestedPolicyLanguage::Rego,
                name: "daily_ai_cost_budget.rego".into(),
                content: include_str!("../templates/daily_ai_cost_budget.rego").to_string(),
            }],
            status: SuggestionStatus::Draft,
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
            suggestion_type: SuggestionType::RestrictExternalLlmProvider,
            title: "Block Unregistered AI Egress".into(),
            summary: "An unregistered process attempted to access api.openai.com. We suggest deploying a Rego policy to block this egress until the agent is registered.".into(),
            severity: SuggestionSeverity::High,
            confidence: 0.9,
            recommended_policy_type: SuggestedPolicyLanguage::Rego,
            recommended_pep_type: "envoy_proxy".into(),
            artifacts: vec![],
            status: SuggestionStatus::Draft,
            created_at: chrono::Utc::now().to_rfc3339(),
        }])
    }
}

struct MockPromptInjectionRule {
    tenant_id: String,
}

impl crate::rules::SuggestionRule for MockPromptInjectionRule {
    fn evaluate(&self, _events: &[AgentObservationEvent]) -> Result<Vec<PolicySuggestion>> {
        Ok(vec![PolicySuggestion {
            suggestion_id: format!("sug_{}", uuid::Uuid::new_v4()),
            tenant_id: self.tenant_id.clone(),
            target_agent_id: None,
            target_resource_id: None,
            target_tool_id: None,
            suggestion_type: SuggestionType::DeployPromptInjectionGuard,
            title: "Prompt Injection Detected".into(),
            summary: "Multiple attempts matching prompt injection signatures detected. Suggest deploying Content Guard.".into(),
            severity: SuggestionSeverity::High,
            confidence: 0.85,
            recommended_policy_type: SuggestedPolicyLanguage::Rego,
            recommended_pep_type: "mcp_proxy".into(),
            artifacts: vec![],
            status: SuggestionStatus::Draft,
            created_at: chrono::Utc::now().to_rfc3339(),
        }])
    }
}

struct MockPiiRedactionRule {
    tenant_id: String,
}

impl crate::rules::SuggestionRule for MockPiiRedactionRule {
    fn evaluate(&self, _events: &[AgentObservationEvent]) -> Result<Vec<PolicySuggestion>> {
        Ok(vec![PolicySuggestion {
            suggestion_id: format!("sug_{}", uuid::Uuid::new_v4()),
            tenant_id: self.tenant_id.clone(),
            target_agent_id: None,
            target_resource_id: None,
            target_tool_id: None,
            suggestion_type: SuggestionType::DeployPiiRedaction,
            title: "PII Egress Detected".into(),
            summary: "Possible PII (Email/Phone) sent to external LLM. Suggest enabling PII Redaction preset.".into(),
            severity: SuggestionSeverity::High,
            confidence: 0.90,
            recommended_policy_type: SuggestedPolicyLanguage::Rego,
            recommended_pep_type: "http_gateway".into(),
            artifacts: vec![],
            status: SuggestionStatus::Draft,
            created_at: chrono::Utc::now().to_rfc3339(),
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_suggestions() -> anyhow::Result<()> {
        let events = vec![];
        let suggestions = generate_suggestions("test_tenant", &[], &events)?;
        // Cost > 25 condition fails, Unregistered returns 1, Injection returns 1, PII returns 1
        assert_eq!(suggestions.len(), 3);
        Ok(())
    }
}
