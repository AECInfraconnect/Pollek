use crate::model::{PolicySuggestion, SuggestedArtifact, SuggestedPolicyLanguage};

pub fn render_cedar(suggestion: &PolicySuggestion) -> SuggestedArtifact {
    let content = format!(
        "// {}\n// {}\npermit(\n    principal,\n    action,\n    resource\n);",
        suggestion.title, suggestion.summary
    );
    SuggestedArtifact {
        language: SuggestedPolicyLanguage::Cedar,
        filename: "policy.cedar".to_string(),
        content,
    }
}
