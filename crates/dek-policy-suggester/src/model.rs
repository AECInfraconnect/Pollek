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
    DeployPromptInjectionGuard,
    DeployPiiRedaction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestedPolicyLanguage {
    Rego,
    Cedar,
    OpenFga,
    PollekPolicyIntent,
    NetworkGuardrailJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionStatus {
    Draft,
    Active,
    Dismissed,
    Approved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyArtifact {
    pub name: String,
    pub content: String,
    pub language: SuggestedPolicyLanguage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySuggestion {
    pub suggestion_id: String,
    pub tenant_id: String,
    pub target_agent_id: Option<String>,
    pub target_resource_id: Option<String>,
    pub target_tool_id: Option<String>,
    pub suggestion_type: SuggestionType,
    pub title: String,
    pub summary: String,
    pub severity: SuggestionSeverity,
    pub confidence: f32,
    pub recommended_policy_type: SuggestedPolicyLanguage,
    pub recommended_pep_type: String,
    pub artifacts: Vec<PolicyArtifact>,
    pub status: SuggestionStatus,
    pub created_at: String,
}
