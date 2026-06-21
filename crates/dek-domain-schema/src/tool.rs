use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Tool {
    pub schema_version: String,
    pub tool_id: String,
    pub tenant_id: String,
    pub mcp_server_id: String,
    pub name: String,
    pub category: String,
    pub risk_level: String,
    pub input_schema_hash: String,
    pub output_schema_hash: String,
}
