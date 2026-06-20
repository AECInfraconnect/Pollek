use crate::model::PolicySuggestion;

pub trait SuggestionRule {
    fn evaluate(&self, context: &serde_json::Value) -> Option<PolicySuggestion>;
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

    pub fn evaluate_all(&self, context: &serde_json::Value) -> Vec<PolicySuggestion> {
        self.rules.iter().filter_map(|r| r.evaluate(context)).collect()
    }
}
