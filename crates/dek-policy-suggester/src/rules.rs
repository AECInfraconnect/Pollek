use crate::model::PolicySuggestion;
use anyhow::Result;
use dek_agent_observer::model::AgentObservationEvent;

pub trait SuggestionRule {
    fn evaluate(&self, events: &[AgentObservationEvent]) -> Result<Vec<PolicySuggestion>>;
}

pub struct RuleEngine {
    rules: Vec<Box<dyn SuggestionRule>>,
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

    pub fn add_rule(&mut self, rule: Box<dyn SuggestionRule>) {
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

pub struct LowTrustRule {
    pub threshold: i32,
}

impl SuggestionRule for LowTrustRule {
    fn evaluate(&self, _events: &[AgentObservationEvent]) -> Result<Vec<PolicySuggestion>> {
        // ในสถานการณ์จริงจะวิเคราะห์จาก TrustScore ของ Agent
        // สำหรับ Phase A จะจำลองการทำงานคืนค่า PolicySuggestion
        let suggestion = PolicySuggestion {
            suggestion_id: "mock_uuid".into(),
            tenant_id: "default".into(),
            target_agent_id: Some("suspicious_agent".into()),
            target_resource_id: None,
            target_tool_id: Some("*".into()),
            suggestion_type: "RestrictMcpTool".into(),
            title: "Restrict suspicious agent".into(),
            summary: "Agent trust score dropped below threshold".into(),
            severity: "High".into(),
            confidence: 0.85,
            recommended_policy_type: "Cedar".into(),
            recommended_pep_type: "mcp_proxy".into(),
            artifacts: vec![],
            status: "draft".into(),
            created_at: "2026-06-21T00:00:00Z".into(),
        };
        Ok(vec![suggestion])
    }
}
