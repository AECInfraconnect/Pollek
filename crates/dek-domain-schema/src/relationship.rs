use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Relationship {
    pub schema_version: String,
    pub tenant_id: String,
    pub subject: String,
    pub relation: String,
    pub object: String,
    pub conditions: HashMap<String, String>,
}
