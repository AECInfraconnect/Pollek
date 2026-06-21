use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Principal {
    pub schema_version: String,
    pub principal_id: String,
    pub tenant_id: String,
    pub r#type: String,
    pub display_name: String,
    pub roles: Vec<String>,
    pub groups: Vec<String>,
    pub attributes: HashMap<String, String>,
    pub identity_provider: String,
    pub external_subject: String,
}
