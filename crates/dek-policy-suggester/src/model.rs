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
pub struct PolicyArtifact {
    pub name: String,
    pub content: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySuggestion {
    pub suggestion_id: String,
    pub tenant_id: String,
    pub target_agent_id: Option<String>,
    pub target_resource_id: Option<String>,
    pub target_tool_id: Option<String>,
    pub suggestion_type: String,
    pub title: String,
    pub summary: String,
    pub severity: String,
    pub confidence: f32,
    pub recommended_policy_type: String,
    pub recommended_pep_type: String,
    pub artifacts: Vec<PolicyArtifact>,
    pub status: String,
    pub created_at: String,
}
