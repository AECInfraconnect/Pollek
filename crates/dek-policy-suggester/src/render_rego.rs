use crate::model::{PolicySuggestion, SuggestedArtifact, SuggestedPolicyLanguage};

pub fn render_rego(suggestion: &PolicySuggestion) -> SuggestedArtifact {
    let content = format!(
        "package pollen.policy\n\ndefault allow = false\n\n# {}\n# {}",
        suggestion.title, suggestion.summary
    );
    SuggestedArtifact {
        language: SuggestedPolicyLanguage::Rego,
        filename: "policy.rego".to_string(),
        content,
    }
}
