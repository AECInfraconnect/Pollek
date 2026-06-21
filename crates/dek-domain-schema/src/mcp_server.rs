use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpTool {
    pub tool_id: String,
    pub name: String,
    pub category: String,
    pub risk_level: String,
    pub input_schema_hash: String,
    pub output_schema_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpServer {
    pub schema_version: String,
    pub server_id: String,
    pub tenant_id: String,
    pub name: String,
    pub endpoint: String,
    pub transport: String,
    pub tools: Vec<McpTool>,
    pub status: String,
}
