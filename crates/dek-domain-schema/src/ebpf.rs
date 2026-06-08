use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EbpfMapUpdate {
    pub schema_version: String,
    pub map_name: String,
    pub operation: String, // e.g. "insert", "delete"
    pub key: Value,
    pub value: Value,
    pub signature: Option<String>,
}
