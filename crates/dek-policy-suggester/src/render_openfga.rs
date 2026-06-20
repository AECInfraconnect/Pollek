use crate::model::{PolicySuggestion, SuggestedArtifact, SuggestedPolicyLanguage};

pub fn render_openfga(_suggestion: &PolicySuggestion) -> SuggestedArtifact {
    let content = format!(
        "model\n  schema 1.1\n\ntype user\ntype resource\n  relations\n    define viewer: [user]"
    );
    SuggestedArtifact {
        language: SuggestedPolicyLanguage::OpenFga,
        filename: "model.fga".to_string(),
        content,
    }
}
