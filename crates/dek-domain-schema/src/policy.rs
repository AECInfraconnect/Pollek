use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyMatch {
    pub agent_id: Option<String>,
    pub tool_name: Option<String>,
    pub resource_classification: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Policy {
    pub schema_version: String,
    pub policy_id: String,
    pub tenant_id: String,
    pub name: String,
    pub policy_type: String,
    pub target_pdp: Vec<String>,
    pub target_pep_types: Vec<String>,
    pub status: String,
    pub priority: i32,
    pub effect: String,
    pub r#match: PolicyMatch,
    pub conditions: HashMap<String, serde_json::Value>,
    pub obligations: Vec<String>,
    pub version: String,
}
