use crate::model::*;
use anyhow::Result;
use dek_agent_observer::model::AgentObservationEvent;

pub fn generate_suggestions(
    tenant: &str,
    _candidates: &[dek_agent_discovery::model::DiscoveredAgentCandidate],
    events: &[AgentObservationEvent],
) -> Result<Vec<PolicySuggestion>> {
    let mut engine = crate::rules::RuleEngine::new();

    // Real signal-driven rules
    engine.add_rule(Box::new(crate::rules::ShadowAgentDetectionRule));
    engine.add_rule(Box::new(crate::rules::HighRiskResourceRule));
    engine.add_rule(Box::new(crate::rules::CostSpikeRule {
        tenant_id: tenant.to_string(),
    }));
    engine.add_rule(Box::new(crate::rules::PromptInjectionGuardRule {
        tenant_id: tenant.to_string(),
    }));

    engine.evaluate_all(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dek_agent_observer::model::{DecisionInfo, EventKind, TokenUsage};

    fn base_event() -> AgentObservationEvent {
        AgentObservationEvent {
            event_id: "evt".into(),
            tenant_id: "test_tenant".into(),
            trace_id: "tr".into(),
            agent_id: None,
            shadow_candidate_id: None,
            tool_id: None,
            resource_id: None,
            surface: "test".into(),
            action: "test".into(),
            pep_type: None,
            risk_level: None,
            timestamp: "2026-07-19T00:00:00Z".into(),
            payload_json: "{}".into(),
            token_usage: None,
            browser_scope: None,
            event_kind: EventKind::Generic,
            decision: None,
            tool_call: None,
            resource_access: None,
            latency_ms: None,
            provider: None,
        }
    }

    #[test]
    fn test_generate_suggestions() -> anyhow::Result<()> {
        let events = vec![];
        let suggestions = generate_suggestions("test_tenant", &[], &events)?;
        assert_eq!(suggestions.len(), 0);
        Ok(())
    }

    #[test]
    fn shadow_events_trigger_single_register_suggestion() -> anyhow::Result<()> {
        let events: Vec<AgentObservationEvent> = (0..3)
            .map(|_| {
                let mut ev = base_event();
                ev.shadow_candidate_id = Some("shadow-1".into());
                ev
            })
            .collect();
        let suggestions = generate_suggestions("test_tenant", &[], &events)?;
        assert_eq!(suggestions.len(), 1);
        assert!(matches!(
            suggestions[0].suggestion_type,
            SuggestionType::RegisterShadowAgent
        ));
        Ok(())
    }

    #[test]
    fn high_risk_event_triggers_approval_suggestion() -> anyhow::Result<()> {
        let mut ev = base_event();
        ev.risk_level = Some("high".into());
        ev.agent_id = Some("agent-1".into());
        ev.resource_id = Some("res-1".into());
        let suggestions = generate_suggestions("test_tenant", &[], &[ev])?;
        assert_eq!(suggestions.len(), 1);
        assert!(matches!(
            suggestions[0].suggestion_type,
            SuggestionType::RequireApprovalForSensitiveResource
        ));
        Ok(())
    }

    #[test]
    fn cost_above_threshold_triggers_budget_suggestion_with_real_cost() -> anyhow::Result<()> {
        let token_usage = || {
            Some(TokenUsage {
                input_tokens: Some(1_000_000),
                output_tokens: Some(1_000_000),
                total_tokens: Some(2_000_000),
                model: Some("gpt-4o".into()),
            })
        };
        let mut ev1 = base_event();
        ev1.token_usage = token_usage();
        let mut ev2 = base_event();
        ev2.token_usage = token_usage();
        let suggestions = generate_suggestions("test_tenant", &[], &[ev1, ev2])?;
        assert_eq!(suggestions.len(), 1);
        let sug = &suggestions[0];
        assert!(matches!(
            sug.suggestion_type,
            SuggestionType::EnforceCostBudget
        ));
        assert!(
            sug.summary.contains("$40.00"),
            "summary should carry the real summed cost, got: {}",
            sug.summary
        );
        assert!(sug
            .artifacts
            .iter()
            .any(|a| a.name == "daily_ai_cost_budget.rego"));
        Ok(())
    }

    #[test]
    fn cost_below_threshold_yields_no_suggestion() -> anyhow::Result<()> {
        let mut ev = base_event();
        ev.token_usage = Some(TokenUsage {
            input_tokens: Some(100_000),
            output_tokens: Some(100_000),
            total_tokens: Some(200_000),
            model: Some("gpt-4o".into()),
        });
        let suggestions = generate_suggestions("test_tenant", &[], &[ev])?;
        assert_eq!(suggestions.len(), 0);
        Ok(())
    }

    #[test]
    fn guard_redaction_obligation_triggers_injection_guard_suggestion() -> anyhow::Result<()> {
        let mut ev = base_event();
        ev.decision = Some(DecisionInfo {
            allow: true,
            reason_code: "OK".into(),
            obligations: vec!["redact_content".into()],
            matched_policy_ids: vec![],
            compliance_tags: vec!["OWASP-LLM01".into()],
            pep_plane: None,
            enforced_for_real: None,
            status_badge: None,
            message_th: None,
        });
        let suggestions = generate_suggestions("test_tenant", &[], &[ev])?;
        assert_eq!(suggestions.len(), 1);
        assert!(matches!(
            suggestions[0].suggestion_type,
            SuggestionType::DeployPromptInjectionGuard
        ));
        Ok(())
    }
}
