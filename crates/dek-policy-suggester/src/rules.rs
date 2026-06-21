use crate::model::PolicySuggestion;
use dek_agent_observer::model::AgentObservationEvent;
use anyhow::Result;

pub trait SuggestionRule {
    fn evaluate(&self, events: &[AgentObservationEvent]) -> Result<Vec<PolicySuggestion>>;
}

pub struct RuleEngine {
    rules: Vec<Box<dyn SuggestionRule>>,
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
