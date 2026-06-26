use crate::model::{PolicyArtifact, PolicySuggestion};

pub fn render_rego(suggestion: &PolicySuggestion) -> PolicyArtifact {
    let content = format!(
        "package pollek.policy\n\ndefault allow = false\n\n# {}\n# {}",
        suggestion.title, suggestion.summary
    );
    PolicyArtifact {
        language: crate::model::SuggestedPolicyLanguage::Rego,
        name: "policy.rego".to_string(),
        content,
    }
}
