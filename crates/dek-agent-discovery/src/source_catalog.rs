use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSignature {
    pub id: String,
    pub display_name: String,
    pub agent_type: String,
    pub process_names: Vec<String>,
    pub config_paths: Option<std::collections::HashMap<String, Vec<String>>>,
    pub config_parsers: Option<Vec<String>>,
    pub ports: Option<Vec<u16>>,
    pub control_strategies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCatalog {
    pub schema_version: String,
    pub catalog_version: String,
    pub signatures: Vec<AgentSignature>,
}

pub fn load_default_catalog() -> SourceCatalog {
    SourceCatalog {
        schema_version: "pollen.agent_signature_catalog.v1".into(),
        catalog_version: "2026-06-21".into(),
        signatures: vec![
            AgentSignature {
                id: "claude_desktop".into(),
                display_name: "Claude Desktop".into(),
                agent_type: "desktop_agent".into(),
                process_names: vec!["Claude".into(), "Claude.exe".into()],
                config_paths: None,
                config_parsers: Some(vec!["mcpServers".into()]),
                ports: None,
                control_strategies: vec![
                    "mcp_stdio_wrapper".into(),
                    "mcp_http_proxy".into(),
                    "observe_only".into(),
                ],
            },
            AgentSignature {
                id: "ollama".into(),
                display_name: "Ollama".into(),
                agent_type: "local_model_server".into(),
                process_names: vec!["ollama".into(), "ollama.exe".into()],
                config_paths: None,
                config_parsers: None,
                ports: Some(vec![11434]),
                control_strategies: vec![
                    "ollama_proxy".into(),
                    "network_egress_pep".into(),
                    "observe_only".into(),
                ],
            },
        ],
    }
}
