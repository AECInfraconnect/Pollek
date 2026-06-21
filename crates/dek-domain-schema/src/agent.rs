use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Runtime {
    pub vendor: String,
    pub runtime_name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AiAgent {
    pub schema_version: String,
    pub agent_id: String,
    pub tenant_id: String,
    pub name: String,
    pub agent_type: String,
    pub owner_principal_id: String,
    pub risk_level: String,
    pub capabilities: Vec<String>,
    pub runtime: Runtime,
    pub allowed_mcp_servers: Vec<String>,
    pub tags: Vec<String>,
    pub status: String,
}
