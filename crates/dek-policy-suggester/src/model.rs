use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionType {
    RegisterShadowAgent,
    RestrictExternalLlmProvider,
    RequireApprovalForSensitiveResource,
    EnforceTokenBudget,
    EnforceCostBudget,
    RestrictMcpTool,
    CreateOpenFgaRelationshipGuard,
    CreateNetworkGuardrail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestedPolicyLanguage {
    Rego,
    Cedar,
    OpenFga,
    PollenPolicyIntent,
    NetworkGuardrailJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedArtifact {
    pub language: SuggestedPolicyLanguage,
    pub filename: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySuggestion {
    pub schema_version: String,
    pub suggestion_id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub suggestion_type: SuggestionType,
    pub title: String,
    pub summary: String,
    pub severity: String,
    pub confidence: f32,
    pub evidence_event_ids: Vec<String>,
    pub affected_agents: Vec<String>,
    pub affected_shadow_candidates: Vec<String>,
    pub affected_resources: Vec<String>,
    pub recommended_pep_types: Vec<String>,
    pub recommended_languages: Vec<SuggestedPolicyLanguage>,
    pub artifacts: Vec<SuggestedArtifact>,
    pub dry_run_required: bool,
    pub status: String,
    pub created_at: String,
}
