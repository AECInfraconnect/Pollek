use crate::config::SuggesterConfig;
use crate::model::PolicySuggestion;
use anyhow::Result;
use dek_agent_observer::model::AgentObservationEvent;

pub trait SuggestionRule: Send + Sync {
    fn evaluate(&self, events: &[AgentObservationEvent]) -> Result<Vec<PolicySuggestion>>;
}

pub struct RuleEngine {
    rules: Vec<Box<dyn SuggestionRule + Send + Sync>>,
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: Box<dyn SuggestionRule + Send + Sync>) {
        self.rules.push(rule);
    }

    pub fn evaluate_all(&self, events: &[AgentObservationEvent]) -> Result<Vec<PolicySuggestion>> {
        let mut all_suggestions = Vec::new();
        for rule in &self.rules {
            let suggestions = rule.evaluate(events)?;
            all_suggestions.extend(suggestions);
        }
        Ok(all_suggestions)
    }
}

pub struct ShadowAgentDetectionRule;

impl SuggestionRule for ShadowAgentDetectionRule {
    fn evaluate(&self, events: &[AgentObservationEvent]) -> Result<Vec<PolicySuggestion>> {
        let mut suggestions = Vec::new();
        // Detect shadow agents that appeared more than once
        let mut shadow_counts = std::collections::HashMap::new();
        for event in events {
            if let Some(shadow_id) = &event.shadow_candidate_id {
                *shadow_counts.entry(shadow_id.clone()).or_insert(0) += 1;
            }
        }

        for (shadow_id, count) in shadow_counts {
            if count >= 3 {
                let suggestion = PolicySuggestion {
                    suggestion_id: format!("shadow-detect-{}", shadow_id),
                    tenant_id: "default".into(),
                    target_agent_id: Some(shadow_id.clone()),
                    target_resource_id: None,
                    target_tool_id: None,
                    suggestion_type: crate::model::SuggestionType::RegisterShadowAgent,
                    title: format!("Unregistered Agent Detected: {}", shadow_id),
                    summary: format!("An unregistered agent (ID: {}) was detected performing {} actions. Consider registering it or blocking it.", shadow_id, count),
                    severity: crate::model::SuggestionSeverity::High,
                    confidence: 0.9,
                    recommended_policy_type: crate::model::SuggestedPolicyLanguage::Cedar,
                    recommended_pep_type: "mcp_proxy".into(),
                    artifacts: vec![],
                    status: crate::model::SuggestionStatus::Draft,
                    created_at: chrono::Utc::now().to_rfc3339(),
                };
                suggestions.push(suggestion);
            }
        }
        Ok(suggestions)
    }
}

pub struct HighRiskResourceRule;

impl SuggestionRule for HighRiskResourceRule {
    fn evaluate(&self, events: &[AgentObservationEvent]) -> Result<Vec<PolicySuggestion>> {
        let mut suggestions = Vec::new();
        for event in events {
            if let Some(risk) = &event.risk_level {
                if risk == "high" || risk == "critical" {
                    if let (Some(agent_id), Some(resource_id)) =
                        (&event.agent_id, &event.resource_id)
                    {
                        let suggestion = PolicySuggestion {
                            suggestion_id: format!("high-risk-{}-{}", agent_id, resource_id),
                            tenant_id: event.tenant_id.clone(),
                            target_agent_id: Some(agent_id.clone()),
                            target_resource_id: Some(resource_id.clone()),
                            target_tool_id: event.tool_id.clone(),
                            suggestion_type: crate::model::SuggestionType::RequireApprovalForSensitiveResource,
                            title: "High Risk Resource Access Detected".into(),
                            summary: format!("Agent '{}' accessed high-risk resource '{}'. Consider requiring explicit approval.", agent_id, resource_id),
                            severity: crate::model::SuggestionSeverity::Critical,
                            confidence: 0.95,
                            recommended_policy_type: crate::model::SuggestedPolicyLanguage::Cedar,
                            recommended_pep_type: "stdio_wrapper".into(),
                            artifacts: vec![],
                            status: crate::model::SuggestionStatus::Draft,
                            created_at: chrono::Utc::now().to_rfc3339(),
                        };
                        suggestions.push(suggestion);
                        break; // Only suggest once per evaluate call to avoid spam
                    }
                }
            }
        }
        Ok(suggestions)
    }
}

// Conservative list-price estimates used to convert observed token usage into
// an approximate USD cost. AgentObservationEvent carries token counts, not
// billed cost, so the suggester falls back to these reference rates
// (USD per 1M tokens), aligned with dek-agent-observer's pricing examples.
const ESTIMATED_INPUT_USD_PER_1M_TOKENS: f64 = 5.0;
const ESTIMATED_OUTPUT_USD_PER_1M_TOKENS: f64 = 15.0;

pub struct CostSpikeRule {
    pub tenant_id: String,
}

impl SuggestionRule for CostSpikeRule {
    fn evaluate(&self, events: &[AgentObservationEvent]) -> Result<Vec<PolicySuggestion>> {
        let config = SuggesterConfig::default();
        let mut total_cost = 0.0;
        for event in events {
            if let Some(tokens) = &event.token_usage {
                let input = tokens.input_tokens.unwrap_or(0);
                let output = tokens.output_tokens.unwrap_or(0);
                // Unknown input/output split: bill the total at the higher output rate.
                let (input, output) = if input == 0 && output == 0 {
                    (0, tokens.total_tokens.unwrap_or(0))
                } else {
                    (input, output)
                };
                total_cost += (input as f64 / 1_000_000.0) * ESTIMATED_INPUT_USD_PER_1M_TOKENS
                    + (output as f64 / 1_000_000.0) * ESTIMATED_OUTPUT_USD_PER_1M_TOKENS;
            }
        }

        if total_cost < config.cost_alert_threshold_usd {
            return Ok(vec![]);
        }

        Ok(vec![PolicySuggestion {
            suggestion_id: format!("sug_{}", uuid::Uuid::new_v4()),
            tenant_id: self.tenant_id.clone(),
            target_agent_id: None,
            target_resource_id: None,
            target_tool_id: None,
            suggestion_type: crate::model::SuggestionType::EnforceCostBudget,
            title: "AI usage cost exceeded suggested daily threshold".into(),
            summary: format!(
                "Observed estimated cost ${:.2}. Suggest daily budget guardrail.",
                total_cost
            ),
            severity: crate::model::SuggestionSeverity::Medium,
            confidence: 0.75,
            recommended_policy_type: crate::model::SuggestedPolicyLanguage::Rego,
            recommended_pep_type: "forward_proxy".into(),
            artifacts: vec![crate::model::PolicyArtifact {
                language: crate::model::SuggestedPolicyLanguage::Rego,
                name: "daily_ai_cost_budget.rego".into(),
                content: include_str!("../templates/daily_ai_cost_budget.rego").to_string(),
            }],
            status: crate::model::SuggestionStatus::Draft,
            created_at: chrono::Utc::now().to_rfc3339(),
        }])
    }
}

pub struct PromptInjectionGuardRule {
    pub tenant_id: String,
}

impl SuggestionRule for PromptInjectionGuardRule {
    fn evaluate(&self, events: &[AgentObservationEvent]) -> Result<Vec<PolicySuggestion>> {
        // Honest signal: the guard pipeline flagged a request (injection
        // signature or sensitive content) and obligated content redaction.
        let flagged = events
            .iter()
            .filter(|event| {
                event.decision.as_ref().map_or(false, |decision| {
                    decision.obligations.iter().any(|o| o == "redact_content")
                        || decision
                            .compliance_tags
                            .iter()
                            .any(|t| t == "OWASP-LLM01")
                })
            })
            .count();

        if flagged == 0 {
            return Ok(vec![]);
        }

        Ok(vec![PolicySuggestion {
            suggestion_id: format!("sug_{}", uuid::Uuid::new_v4()),
            tenant_id: self.tenant_id.clone(),
            target_agent_id: None,
            target_resource_id: None,
            target_tool_id: None,
            suggestion_type: crate::model::SuggestionType::DeployPromptInjectionGuard,
            title: "Prompt Injection Guard Findings Observed".into(),
            summary: format!(
                "The guard pipeline flagged {} request(s) matching prompt-injection signatures (redact_content obligation). Suggest deploying a dedicated prompt-injection guard policy.",
                flagged
            ),
            severity: crate::model::SuggestionSeverity::High,
            confidence: 0.85,
            recommended_policy_type: crate::model::SuggestedPolicyLanguage::Rego,
            recommended_pep_type: "mcp_proxy".into(),
            artifacts: vec![],
            status: crate::model::SuggestionStatus::Draft,
            created_at: chrono::Utc::now().to_rfc3339(),
        }])
    }
}
