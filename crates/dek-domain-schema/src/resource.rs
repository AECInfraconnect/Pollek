use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Resource {
    pub schema_version: String,
    pub tenant_id: String,
    pub resource_id: String,
    pub resource_type: String,
    pub name: String,
    pub uri: String,
    pub classification: String,
    pub data_categories: Vec<String>,
    pub region: String,
    pub owner_entity_id: String,
    pub allowed_actions: Vec<String>,
    pub labels: HashMap<String, String>,
}
