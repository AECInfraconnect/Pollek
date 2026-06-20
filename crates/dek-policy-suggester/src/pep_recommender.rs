pub fn recommend_pep(suggestion_type: &crate::model::SuggestionType) -> Vec<String> {
    use crate::model::SuggestionType::*;
    match suggestion_type {
        RestrictExternalLlmProvider | EnforceTokenBudget | EnforceCostBudget => vec!["HttpGateway".to_string()],
        RestrictMcpTool => vec!["McpProxy".to_string(), "StdioWrapper".to_string()],
        CreateNetworkGuardrail => vec!["Envoy".to_string(), "LinuxEbpf".to_string(), "WindowsWfp".to_string(), "MacosNeFilter".to_string()],
        _ => vec![],
    }
}
