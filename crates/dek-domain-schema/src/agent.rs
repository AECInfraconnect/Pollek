use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Runtime {
    pub os: String,
    pub process_name: String,
    pub binary_hash: String,
    pub version: String,
    pub sandbox: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Agent {
    pub schema_version: String,
    pub tenant_id: String,
    pub agent_id: String,
    pub agent_type: String,
    pub name: String,
    pub owner_user_id: String,
    pub device_id: String,
    pub spiffe_id: String,
    pub runtime: Runtime,
    pub capabilities: Vec<String>,
    pub risk_level: String,
    pub status: String,
    pub labels: HashMap<String, String>,
}
