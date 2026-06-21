pub fn recommend_pep(suggestion_type: &str) -> String {
    match suggestion_type {
        "RestrictExternalLlmProvider" | "EnforceTokenBudget" | "EnforceCostBudget" => {
            "http_gateway".to_string() // mapped to V2 PEP Type instead of forward_proxy
        }
        "RestrictMcpTool" | "DeployPromptInjectionGuard" => "mcp_proxy".to_string(),
        "DeployPiiRedaction" => "http_gateway".to_string(),
        "CreateNetworkGuardrail" => "linux_ebpf".to_string(), // mapped to V2 PEP Type
        "CreateOpenFgaRelationshipGuard" => "mcp_proxy".to_string(),
        _ => "unknown".to_string(),
    }
}
