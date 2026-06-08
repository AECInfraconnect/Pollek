use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Ref {
    pub r#type: String, // 'type' is reserved in Rust
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Relationship {
    pub schema_version: String,
    pub tenant_id: String,
    pub subject: Ref,
    pub relation: String,
    pub object: Ref,
    pub source: String,
    pub valid_from: String,
    pub valid_until: Option<String>,
}
