use crate::model::{PolicyArtifact, PolicySuggestion};

pub fn render_rego(suggestion: &PolicySuggestion) -> PolicyArtifact {
    let content = format!(
        "package pollen.policy\n\ndefault allow = false\n\n# {}\n# {}",
        suggestion.title, suggestion.summary
    );
    PolicyArtifact {
        language: "rego".to_string(),
        name: "policy.rego".to_string(),
        content,
    }
}
