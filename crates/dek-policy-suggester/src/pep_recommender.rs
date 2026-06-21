pub fn recommend_pep(suggestion_type: &str) -> String {
    match suggestion_type {
        "RestrictExternalLlmProvider" | "EnforceTokenBudget" | "EnforceCostBudget" => {
            "forward_proxy".to_string()
        }
        "RestrictMcpTool" => "mcp_proxy".to_string(),
        "CreateNetworkGuardrail" => "envoy_proxy".to_string(),
        _ => "unknown".to_string(),
    }
}
